use super::*;
use std::collections::HashSet;

async fn load(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    access: &Access,
) -> Result<MeetingSeries> {
    let record: String = sqlx::query_scalar("SELECT record FROM meeting_series WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    let mut series: MeetingSeries = serde_json::from_str(&record)?;
    let scope = project_sources(tx, &series.project_id).await?;
    permit(tx, &scope, access).await?;
    for row in sqlx::query("SELECT record FROM meeting_occurrences WHERE series_id=? ORDER BY json_extract(record,'$.held_on') DESC,json_extract(record,'$.created_at') DESC,id").bind(id).fetch_all(&mut **tx).await? {series.occurrences.push(serde_json::from_str(row.get("record"))?);}
    Ok(series)
}
async fn save(tx: &mut Transaction<'_, Sqlite>, series: &MeetingSeries) -> Result<()> {
    let mut meta = series.clone();
    meta.occurrences.clear();
    sqlx::query("INSERT INTO meeting_series(id,project_id,record) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&series.id).bind(&series.project_id).bind(serde_json::to_string(&meta)?).execute(&mut **tx).await?;
    event(tx, "meeting", &series.id).await
}
impl Store {
    pub async fn meeting_series(&self, access: &Access) -> Result<Vec<MeetingSeries>> {
        let ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM meeting_series ORDER BY json_extract(record,'$.updated_at') DESC,id",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut result = vec![];
        for id in ids {
            match self.meeting(&id, access).await {
                Ok(s) => result.push(s),
                Err(e) if e.to_string() == "record not found" => {}
                Err(e) => return Err(e),
            }
        }
        Ok(result)
    }
    pub async fn meeting(&self, id: &str, access: &Access) -> Result<MeetingSeries> {
        let mut tx = self.pool.begin().await?;
        let series = load(&mut tx, id, access).await?;
        tx.commit().await?;
        Ok(series)
    }
    pub async fn create_meeting_series(&self, req: MeetingCreateRequest) -> Result<MeetingSeries> {
        validate_text(&req.name, 200, "meeting series name")?;
        if req.cadence.len() > 200 || req.cadence.contains('\0') {
            bail!("invalid meeting cadence");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "meeting-create", "", &req.idempotency_key, &req).await?;
        let id = if let Some(id) = old {
            id
        } else {
            let scope = project_sources(&mut tx, &req.project_id).await?;
            permit(&mut tx, &scope, &Access::admin()).await?;
            let series = MeetingSeries {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: req.project_id.clone(),
                name: req.name.trim().into(),
                cadence: req.cadence.trim().into(),
                version: 1,
                occurrences: vec![],
                created_at: now(),
                updated_at: now(),
            };
            save(&mut tx, &series).await?;
            remember(&mut tx, &req.idempotency_key, &digest, &series.id).await?;
            series.id
        };
        tx.commit().await?;
        self.meeting(&id, &Access::admin()).await
    }
    pub async fn link_meeting(&self, id: &str, req: MeetingLinkRequest) -> Result<MeetingSeries> {
        let held = chrono::NaiveDate::parse_from_str(&req.held_on, "%Y-%m-%d")
            .context("invalid meeting date; use YYYY-MM-DD")?;
        if held.format("%Y-%m-%d").to_string() != req.held_on
            || held > chrono::Utc::now().date_naive()
        {
            bail!("invalid: link a completed meeting with a date no later than today (UTC)");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "meeting-link", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut series = load(&mut tx, id, &Access::admin()).await?;
            if series.version != req.expected_version {
                bail!("conflict: meeting series changed; refresh before linking");
            }
            if series.occurrences.len() >= 200 {
                bail!("invalid: meeting series history reached 200 occurrences");
            }
            let scope = project_sources(&mut tx, &series.project_id).await?;
            if !scope.contains(&req.source_id) {
                bail!("invalid: add the transcript source to this project first");
            }
            let kind: String = sqlx::query_scalar("SELECT kind FROM sources WHERE id=?")
                .bind(&req.source_id)
                .fetch_one(&mut *tx)
                .await?;
            if !["transcript", "markdown"].contains(&kind.as_str()) {
                bail!("invalid: select a transcript or meeting note source");
            }
            let row=sqlx::query("SELECT d.id,r.title FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.source_id=? AND r.revision=?").bind(&req.source_id).bind(req.revision).fetch_optional(&mut *tx).await?.context("invalid transcript revision")?;
            let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM meeting_occurrences WHERE series_id=? AND source_id=? AND revision=?)").bind(id).bind(&req.source_id).bind(req.revision).fetch_one(&mut *tx).await?;
            if exists {
                bail!(
                    "conflict: this transcript revision is already linked; restore its occurrence if it was removed"
                );
            }
            let docs = documents(&mut tx, &scope).await?;
            let tasks = tasks(&mut tx, &docs)
                .await?
                .into_iter()
                .map(|t| TaskStamp {
                    id: t.id,
                    version: t.version,
                    status: t.status,
                })
                .collect();
            let occurrence = MeetingOccurrence {
                id: uuid::Uuid::new_v4().to_string(),
                source_id: req.source_id.clone(),
                document_id: row.get("id"),
                revision: req.revision,
                title: row.get("title"),
                held_on: req.held_on.clone(),
                status: "active".into(),
                sources: watermarks(&docs),
                tasks,
                created_at: now(),
            };
            sqlx::query("INSERT INTO meeting_occurrences(id,series_id,source_id,revision,record) VALUES(?,?,?,?,?)").bind(&occurrence.id).bind(id).bind(&occurrence.source_id).bind(occurrence.revision).bind(serde_json::to_string(&occurrence)?).execute(&mut *tx).await?;
            series.version += 1;
            series.updated_at = now();
            save(&mut tx, &series).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.meeting(id, &Access::admin()).await
    }
    pub async fn update_meeting_occurrence(
        &self,
        id: &str,
        occurrence_id: &str,
        req: MeetingOccurrenceUpdate,
    ) -> Result<MeetingSeries> {
        if !["active", "removed"].contains(&req.status.as_str()) {
            bail!("invalid meeting occurrence status");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "meeting-occurrence",
            &format!("{id}/{occurrence_id}"),
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            let mut series = load(&mut tx, id, &Access::admin()).await?;
            if series.version != req.expected_version {
                bail!("conflict: meeting series changed; refresh before editing");
            }
            let occurrence = series
                .occurrences
                .iter_mut()
                .find(|o| o.id == occurrence_id)
                .context("record not found")?;
            occurrence.status = req.status.clone();
            sqlx::query("UPDATE meeting_occurrences SET record=? WHERE id=? AND series_id=?")
                .bind(serde_json::to_string(occurrence)?)
                .bind(occurrence_id)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            series.version += 1;
            series.updated_at = now();
            save(&mut tx, &series).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.meeting(id, &Access::admin()).await
    }
    /// Deterministic preparation, requiring no model or calendar account.
    pub async fn meeting_preparation(
        &self,
        id: &str,
        access: &Access,
    ) -> Result<MeetingPreparation> {
        let mut tx = self.pool.begin().await?;
        let series = load(&mut tx, id, access).await?;
        let scope = project_sources(&mut tx, &series.project_id).await?;
        let docs = documents(&mut tx, &scope).await?;
        let all_tasks = tasks(&mut tx, &docs).await?;
        let baseline = series
            .occurrences
            .iter()
            .find(|o| o.status == "active")
            .cloned();
        let mut commitments = vec![];
        let mut task_changes = vec![];
        for task in all_tasks {
            let stamp = baseline
                .as_ref()
                .and_then(|b| b.tasks.iter().find(|s| s.id == task.id));
            let change = if baseline.is_none() {
                "current"
            } else if stamp.is_none() {
                "new"
            } else if stamp.is_some_and(|s| s.status != task.status) {
                "status changed"
            } else if stamp.is_some_and(|s| s.version != task.version) {
                "updated"
            } else {
                "unchanged"
            };
            let item = MeetingCommitment {
                stale_evidence: !docs.iter().any(|d| {
                    d.id == task.evidence.document_id && d.revision == task.evidence.revision
                }),
                meeting_id: series
                    .occurrences
                    .iter()
                    .find(|o| {
                        o.status == "active"
                            && o.document_id == task.evidence.document_id
                            && o.revision == task.evidence.revision
                    })
                    .map(|o| o.id.clone()),
                change: change.into(),
                task,
            };
            if ["accepted", "candidate"].contains(&item.task.status.as_str()) {
                commitments.push(item);
            } else if stamp.is_some() && change != "unchanged" {
                task_changes.push(item);
            }
        }
        let mut source_changes = vec![];
        if let Some(base) = &baseline {
            for doc in &docs {
                let old = base.sources.iter().find(|s| s.document_id == doc.id);
                if old.is_some_and(|s| {
                    s.revision == doc.revision && s.content_hash == doc.content_hash
                }) {
                    continue;
                }
                let previous: Option<String> = if let Some(old) = old {
                    sqlx::query_scalar(
                        "SELECT body FROM revisions WHERE document_id=? AND revision=?",
                    )
                    .bind(&doc.id)
                    .bind(old.revision)
                    .fetch_optional(&mut *tx)
                    .await?
                } else {
                    None
                };
                let old_lines: HashSet<&str> =
                    previous.as_deref().unwrap_or_default().lines().collect();
                let mut excerpts = vec![];
                let mut omitted = false;
                for (i, line) in doc
                    .body
                    .lines()
                    .enumerate()
                    .filter(|(_, l)| !l.trim().is_empty() && !old_lines.contains(l))
                {
                    if excerpts.len() >= 4 || line.len() > 1000 {
                        omitted = true;
                        continue;
                    }
                    excerpts.push(Evidence {
                        document_id: doc.id.clone(),
                        revision: doc.revision,
                        line: (i + 1) as i64,
                        quote: line.into(),
                    });
                }
                source_changes.push(SourceChange {
                    source_id: doc.source_id.clone(),
                    document_id: doc.id.clone(),
                    title: doc.title.clone(),
                    from_revision: old.map(|s| s.revision),
                    to_revision: Some(doc.revision),
                    excerpts,
                    excerpts_omitted: omitted,
                });
            }
            for old in &base.sources {
                if !docs.iter().any(|d| d.id == old.document_id) {
                    source_changes.push(SourceChange {
                        source_id: old.source_id.clone(),
                        document_id: old.document_id.clone(),
                        title: old.title.clone(),
                        from_revision: Some(old.revision),
                        to_revision: None,
                        excerpts: vec![],
                        excerpts_omitted: false,
                    });
                }
            }
        }
        let transcript_changed = baseline.as_ref().is_some_and(|b| {
            !docs
                .iter()
                .any(|d| d.id == b.document_id && d.revision == b.revision)
        });
        let mut baseline_meta = baseline;
        if let Some(b) = &mut baseline_meta {
            b.tasks.clear();
            b.sources.clear();
        }
        let mut prep = MeetingPreparation {
            series_id: id.into(),
            project_id: series.project_id.clone(),
            name: series.name,
            baseline: baseline_meta,
            baseline_transcript_changed: transcript_changed,
            commitments,
            waiting_for: crate::focus::waiting::for_sources(
                &mut tx,
                &series.project_id,
                &scope,
                chrono::Utc::now().date_naive(),
            )
            .await?,
            task_changes,
            source_changes,
            brief: reviewed_brief(&mut tx, &series.project_id, &scope).await?,
            sources: watermarks(&docs),
            omissions: vec![],
            watermark: sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM events")
                .fetch_one(&mut *tx)
                .await?,
            created_at: now(),
        };
        let (mut omitted_tasks, mut omitted_changes, mut omitted_brief, mut omitted_lines) =
            (0, 0, 0, 0);
        let mut omitted_waiting = 0;
        while serde_json::to_vec(&prep)?.len() > 62000 {
            if let Some(s) = prep
                .source_changes
                .iter_mut()
                .rev()
                .find(|s| !s.excerpts.is_empty())
            {
                s.excerpts.pop();
                s.excerpts_omitted = true;
                omitted_lines += 1;
            } else if let Some(s) = prep
                .brief
                .as_mut()
                .and_then(|b| b.sections.iter_mut().find(|s| !s.items.is_empty()))
            {
                s.items.pop();
                omitted_brief += 1;
            } else if prep.task_changes.pop().is_some() {
                omitted_changes += 1;
            } else if prep.waiting_for.pop().is_some() {
                omitted_waiting += 1;
            } else if prep.commitments.pop().is_some() {
                omitted_tasks += 1;
            } else {
                bail!("invalid: meeting preparation metadata exceeds 62 KB; use a smaller project");
            }
        }
        if omitted_tasks + omitted_changes + omitted_brief + omitted_lines + omitted_waiting > 0 {
            prep.omissions.push(format!("Display limit omitted {omitted_tasks} open commitments, {omitted_changes} task changes, {omitted_brief} brief items, {omitted_waiting} waiting requests and {omitted_lines} excerpt lines. Original records remain available."));
        }
        tx.commit().await?;
        Ok(prep)
    }
}
