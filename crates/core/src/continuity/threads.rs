use super::*;

pub(super) async fn load(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    access: &Access,
) -> Result<WorkThread> {
    let record: String = sqlx::query_scalar("SELECT record FROM work_threads WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    let mut thread: WorkThread = serde_json::from_str(&record)?;
    permit(tx, &thread.source_ids, access).await?;
    for row in
        sqlx::query("SELECT record FROM thread_entries WHERE thread_id=? ORDER BY rowid DESC")
            .bind(id)
            .fetch_all(&mut **tx)
            .await?
    {
        thread
            .entries
            .push(serde_json::from_str(row.get("record"))?);
    }
    for row in
        sqlx::query("SELECT record FROM thread_history WHERE thread_id=? ORDER BY sequence DESC")
            .bind(id)
            .fetch_all(&mut **tx)
            .await?
    {
        thread
            .history
            .push(serde_json::from_str(row.get("record"))?);
    }
    for row in sqlx::query("SELECT c.id,c.created_at,r.record FROM thread_contexts c JOIN runtime_runs r ON r.id=c.run_id WHERE c.thread_id=? ORDER BY c.created_at DESC LIMIT 30").bind(id).fetch_all(&mut **tx).await? {
        let run:RuntimeRun=serde_json::from_str(row.get("record"))?;
        thread.runs.push(ThreadRun{context_id:row.get("id"),run_id:run.id,connection:run.connection,status:run.status,session_id:run.session_id,created_at:row.get("created_at")});
    }
    if access.source_ids.is_some() {
        while serde_json::to_vec(&thread)?.len() > 60000 {
            if thread.entries.pop().is_some() {
                thread.omitted_entries += 1;
            } else if thread.history.pop().is_some() {
                thread.omitted_history += 1;
            } else if thread.runs.pop().is_some() {
            } else {
                bail!(
                    "invalid: thread metadata exceeds the shared read limit; use its bounded context endpoint"
                );
            }
        }
    }
    Ok(thread)
}
pub(super) async fn save(tx: &mut Transaction<'_, Sqlite>, thread: &WorkThread) -> Result<()> {
    let mut meta = thread.clone();
    meta.omitted_entries = 0;
    meta.omitted_history = 0;
    meta.entries.clear();
    meta.history.clear();
    meta.runs.clear();
    sqlx::query("INSERT INTO work_threads(id,project_id,record) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record")
        .bind(&meta.id).bind(&meta.project_id).bind(serde_json::to_string(&meta)?).execute(&mut **tx).await?;
    event(tx, "thread", &thread.id).await
}
pub(super) async fn audit(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    action: &str,
    related: Option<String>,
    entries: Vec<String>,
) -> Result<()> {
    let record = ThreadHistory {
        action: action.into(),
        related_id: related,
        entry_ids: entries,
        created_at: now(),
    };
    sqlx::query("INSERT INTO thread_history(thread_id,record) VALUES(?,?)")
        .bind(id)
        .bind(serde_json::to_string(&record)?)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
fn editable(thread: &WorkThread, version: i64) -> Result<()> {
    if thread.version != version {
        bail!("conflict: work thread changed; refresh before editing");
    }
    if thread.status != "active" {
        bail!("conflict: reopen the work thread before changing its contents");
    }
    Ok(())
}
impl Store {
    pub async fn work_threads(&self, access: &Access) -> Result<Vec<WorkThread>> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM work_threads ORDER BY json_extract(record,'$.updated_at') DESC,id",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut threads = vec![];
        for id in ids {
            match self.work_thread(&id, access).await {
                Ok(thread) => threads.push(thread),
                Err(e) if e.to_string() == "record not found" => {}
                Err(e) => return Err(e),
            }
        }
        Ok(threads)
    }
    pub async fn work_thread(&self, id: &str, access: &Access) -> Result<WorkThread> {
        let mut tx = self.pool.begin().await?;
        let thread = load(&mut tx, id, access).await?;
        tx.commit().await?;
        Ok(thread)
    }
    pub async fn create_thread(&self, mut req: ThreadCreateRequest) -> Result<WorkThread> {
        validate_text(&req.title, 200, "thread title")?;
        normalize_sources(&mut req.source_ids)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "thread-create", "", &req.idempotency_key, &req).await?;
        let id = if let Some(id) = old {
            id
        } else {
            let scope = project_sources(&mut tx, &req.project_id).await?;
            if !req.source_ids.iter().all(|id| scope.contains(id)) {
                bail!("invalid: thread sources must belong to the project");
            }
            permit(&mut tx, &req.source_ids, &Access::admin()).await?;
            let thread = WorkThread {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: req.project_id.clone(),
                title: req.title.trim().into(),
                source_ids: req.source_ids.clone(),
                status: "active".into(),
                merged_into: None,
                version: 1,
                entries: vec![],
                omitted_entries: 0,
                omitted_history: 0,
                history: vec![],
                runs: vec![],
                created_at: now(),
                updated_at: now(),
            };
            save(&mut tx, &thread).await?;
            audit(&mut tx, &thread.id, "created", None, vec![]).await?;
            remember(&mut tx, &req.idempotency_key, &digest, &thread.id).await?;
            thread.id
        };
        tx.commit().await?;
        self.work_thread(&id, &Access::admin()).await
    }
    pub async fn update_thread(
        &self,
        id: &str,
        mut req: ThreadUpdateRequest,
    ) -> Result<WorkThread> {
        validate_text(&req.title, 200, "thread title")?;
        normalize_sources(&mut req.source_ids)?;
        if !["active", "closed"].contains(&req.status.as_str()) {
            bail!("invalid thread status");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "thread-update", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut thread = load(&mut tx, id, &Access::admin()).await?;
            if thread.version != req.expected_version || thread.status == "merged" {
                bail!("conflict: thread changed or was merged");
            }
            let scope = project_sources(&mut tx, &thread.project_id).await?;
            if !thread
                .source_ids
                .iter()
                .all(|id| req.source_ids.contains(id))
                || !req.source_ids.iter().all(|id| scope.contains(id))
            {
                bail!("invalid: sources can only expand within the project");
            }
            thread.title = req.title.trim().into();
            thread.status = req.status.clone();
            thread.source_ids = req.source_ids.clone();
            thread.version += 1;
            thread.updated_at = now();
            save(&mut tx, &thread).await?;
            audit(&mut tx, id, "updated", None, vec![]).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.work_thread(id, &Access::admin()).await
    }
    pub async fn append_thread_entry(
        &self,
        id: &str,
        req: ThreadEntryRequest,
    ) -> Result<WorkThread> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "thread-entry", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut thread = load(&mut tx, id, &Access::admin()).await?;
            editable(&thread, req.expected_version)?;
            if thread.entries.len() >= 200 {
                bail!("invalid: thread has 200 checkpoints; split it before adding more");
            }
            let mut entry = ThreadEntry {
                id: uuid::Uuid::new_v4().to_string(),
                kind: req.kind.clone(),
                body: req.body.clone(),
                record_id: req.record_id.clone(),
                evidence: req.evidence.clone(),
                contributor: "You".into(),
                truncated: false,
                created_at: now(),
            };
            if req.kind == "checkpoint" {
                validate_text(&req.body, 4000, "checkpoint")?;
                if req.record_id.is_some() {
                    bail!("invalid checkpoint record ID");
                }
            } else {
                if !req.body.is_empty() || !req.evidence.is_empty() {
                    bail!("invalid: linked records supply their own contents");
                }
                let record = req
                    .record_id
                    .as_deref()
                    .context("invalid: choose a record to attach")?;
                match req.kind.as_str() {
                    "task" => {
                        let row = sqlx::query("SELECT * FROM tasks WHERE id=?")
                            .bind(record)
                            .fetch_optional(&mut *tx)
                            .await?
                            .context("record not found")?;
                        let task = crate::store::task_row(&row);
                        entry.body =
                            format!("{} · status when attached: {}", task.title, task.status);
                        entry.evidence = vec![task.evidence];
                    }
                    "observation" => {
                        let row = sqlx::query("SELECT * FROM observations WHERE id=?")
                            .bind(record)
                            .fetch_optional(&mut *tx)
                            .await?
                            .context("record not found")?;
                        let obs = crate::store::observation_row(&row)?;
                        let obs =
                            crate::memory_review::effective_observations_tx(&mut tx, vec![obs])
                                .await?
                                .into_iter()
                                .next()
                                .context("invalid: suppressed memory cannot be attached")?;
                        if obs.source_ids.is_empty()
                            || !obs.source_ids.iter().all(|s| thread.source_ids.contains(s))
                        {
                            bail!("forbidden: observation is outside this thread's source scope");
                        }
                        entry.body = obs.body;
                        entry.evidence = obs.evidence;
                        entry.contributor = obs.contributor;
                    }
                    "agent_run" => {
                        let json: String =
                            sqlx::query_scalar("SELECT record FROM runtime_runs WHERE id=?")
                                .bind(record)
                                .fetch_optional(&mut *tx)
                                .await?
                                .context("record not found")?;
                        let run: RuntimeRun = serde_json::from_str(&json)?;
                        if !run.source_ids.iter().all(|s| thread.source_ids.contains(s)) {
                            bail!("forbidden: run is outside this thread's source scope");
                        }
                        if matches!(
                            run.status.as_str(),
                            "starting" | "running" | "waiting_permission" | "cancelling"
                        ) {
                            bail!("conflict: wait for the run to stop before attaching it");
                        }
                        entry.body = format!(
                            "Agent output ({}; {}):\n{}",
                            run.connection, run.status, run.output
                        );
                        entry.contributor = format!("Agent: {}", run.connection);
                    }
                    _ => bail!("invalid thread entry kind"),
                }
                if entry.body.len() > 4000 {
                    entry.body = entry
                        .body
                        .chars()
                        .scan(0, |n, c| {
                            *n += c.len_utf8();
                            (*n <= 4000).then_some(c)
                        })
                        .collect();
                    entry.truncated = true;
                }
            }
            if entry.evidence.len() > 8 {
                bail!("invalid: at most eight checkpoint citations");
            }
            for e in &entry.evidence {
                if e.line < 1 || e.quote.len() > 4000 {
                    bail!("invalid checkpoint citation");
                }
                let row=sqlx::query("SELECT d.source_id,r.body FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?").bind(&e.document_id).bind(e.revision).fetch_optional(&mut *tx).await?.context("invalid evidence revision")?;
                if !thread
                    .source_ids
                    .contains(&row.get::<String, _>("source_id"))
                    || row
                        .get::<String, _>("body")
                        .lines()
                        .nth(e.line as usize - 1)
                        != Some(&e.quote)
                {
                    bail!("invalid checkpoint evidence scope or locator");
                }
            }
            sqlx::query("INSERT INTO thread_entries(id,thread_id,record) VALUES(?,?,?)")
                .bind(&entry.id)
                .bind(id)
                .bind(serde_json::to_string(&entry)?)
                .execute(&mut *tx)
                .await?;
            audit(&mut tx, id, "checkpoint", None, vec![entry.id]).await?;
            thread.version += 1;
            thread.updated_at = now();
            save(&mut tx, &thread).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.work_thread(id, &Access::admin()).await
    }
    pub async fn split_thread(&self, id: &str, mut req: ThreadSplitRequest) -> Result<WorkThread> {
        validate_text(&req.title, 200, "thread title")?;
        req.entry_ids.sort();
        req.entry_ids.dedup();
        if req.entry_ids.is_empty() {
            bail!("invalid: select checkpoints to move");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "thread-split", id, &req.idempotency_key, &req).await?;
        let new_id = if let Some(id) = old {
            id
        } else {
            let mut parent = load(&mut tx, id, &Access::admin()).await?;
            editable(&parent, req.expected_version)?;
            if !req
                .entry_ids
                .iter()
                .all(|id| parent.entries.iter().any(|e| &e.id == id))
            {
                bail!("invalid split checkpoint selection");
            }
            let mut child = parent.clone();
            child.id = uuid::Uuid::new_v4().to_string();
            child.title = req.title.trim().into();
            child.version = 1;
            child.created_at = now();
            child.updated_at = now();
            save(&mut tx, &child).await?;
            for entry in &req.entry_ids {
                sqlx::query("UPDATE thread_entries SET thread_id=? WHERE id=? AND thread_id=?")
                    .bind(&child.id)
                    .bind(entry)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            parent.version += 1;
            parent.updated_at = now();
            save(&mut tx, &parent).await?;
            audit(
                &mut tx,
                id,
                "split_to",
                Some(child.id.clone()),
                req.entry_ids.clone(),
            )
            .await?;
            audit(
                &mut tx,
                &child.id,
                "split_from",
                Some(id.into()),
                req.entry_ids.clone(),
            )
            .await?;
            remember(&mut tx, &req.idempotency_key, &digest, &child.id).await?;
            child.id
        };
        tx.commit().await?;
        self.work_thread(&new_id, &Access::admin()).await
    }
    pub async fn merge_threads(&self, id: &str, req: ThreadMergeRequest) -> Result<WorkThread> {
        if id == req.other_id {
            bail!("invalid: choose a different thread to merge");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "thread-merge", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut target = load(&mut tx, id, &Access::admin()).await?;
            editable(&target, req.expected_version)?;
            let mut other = load(&mut tx, &req.other_id, &Access::admin()).await?;
            editable(&other, req.other_expected_version)?;
            if target.project_id != other.project_id {
                bail!("invalid: only threads in the same project can merge");
            }
            if target.entries.len() + other.entries.len() > 200 {
                bail!("invalid: merged thread would exceed 200 checkpoints");
            }
            target.source_ids.extend(other.source_ids.clone());
            normalize_sources(&mut target.source_ids)?;
            sqlx::query("UPDATE thread_entries SET thread_id=? WHERE thread_id=?")
                .bind(id)
                .bind(&other.id)
                .execute(&mut *tx)
                .await?;
            target.version += 1;
            target.updated_at = now();
            other.status = "merged".into();
            other.merged_into = Some(id.into());
            other.version += 1;
            other.updated_at = now();
            save(&mut tx, &target).await?;
            save(&mut tx, &other).await?;
            let entries = other
                .entries
                .iter()
                .map(|e| e.id.clone())
                .collect::<Vec<_>>();
            audit(
                &mut tx,
                id,
                "merged_from",
                Some(other.id.clone()),
                entries.clone(),
            )
            .await?;
            audit(&mut tx, &other.id, "merged_into", Some(id.into()), entries).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.work_thread(id, &Access::admin()).await
    }
}
