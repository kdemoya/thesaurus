use super::*;

async fn raw(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<WaitingFor> {
    let json: String = sqlx::query_scalar("SELECT record FROM waiting_for WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&json)?)
}
async fn refresh(
    tx: &mut Transaction<'_, Sqlite>,
    mut item: WaitingFor,
    day: NaiveDate,
    history: bool,
) -> Result<WaitingFor> {
    item.follow_up_due = item.status == "waiting" && date(&item.follow_up_on)? <= day;
    item.stale_evidence = false;
    for e in &item.evidence {
        let current: Option<i64> = sqlx::query_scalar("SELECT revision FROM documents WHERE id=?")
            .bind(&e.document_id)
            .fetch_optional(&mut **tx)
            .await?;
        if current != Some(e.revision) {
            item.stale_evidence = true;
        }
    }
    item.linked_task_status = if let Some(id) = &item.task_id {
        sqlx::query_scalar("SELECT status FROM tasks WHERE id=?")
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
    } else {
        None
    };
    item.suggested_follow_up = item
        .follow_up_due
        .then(|| format!("Hi {}, is there an update on {}?", item.person, item.title));
    item.history.clear();
    if history {
        for row in sqlx::query(
            "SELECT record FROM waiting_history WHERE waiting_id=? ORDER BY sequence DESC LIMIT 30",
        )
        .bind(&item.id)
        .fetch_all(&mut **tx)
        .await?
        {
            item.history.push(serde_json::from_str(row.get("record"))?);
        }
        while serde_json::to_vec(&item)?.len() > 60000 {
            if item.history.pop().is_none() {
                bail!("invalid: waiting record exceeds the shared read limit");
            }
        }
    }
    item.history_omitted = item.history_count.saturating_sub(item.history.len());
    Ok(item)
}
async fn save(
    tx: &mut Transaction<'_, Sqlite>,
    item: &WaitingFor,
    action: &str,
    note: &str,
    evidence: &[Evidence],
) -> Result<()> {
    let mut meta = item.clone();
    meta.history.clear();
    meta.follow_up_due = false;
    meta.stale_evidence = false;
    meta.linked_task_status = None;
    meta.suggested_follow_up = None;
    meta.history_omitted = 0;
    sqlx::query("INSERT INTO waiting_for(id,project_id,record) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&item.id).bind(&item.project_id).bind(serde_json::to_string(&meta)?).execute(&mut **tx).await?;
    let event_record = WaitingEvent {
        action: action.into(),
        note: note.into(),
        evidence: evidence.to_vec(),
        person: item.person.clone(),
        title: item.title.clone(),
        status: item.status.clone(),
        follow_up_on: item.follow_up_on.clone(),
        contributor: "You".into(),
        created_at: now(),
    };
    sqlx::query("INSERT INTO waiting_history(waiting_id,record) VALUES(?,?)")
        .bind(&item.id)
        .bind(serde_json::to_string(&event_record)?)
        .execute(&mut **tx)
        .await?;
    event(tx, "waiting", &item.id).await
}
async fn citations(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    evidence: &[Evidence],
) -> Result<Vec<String>> {
    if evidence.is_empty() || evidence.len() > 6 {
        bail!("invalid: waiting-for records require 1–6 source citations");
    }
    let scope = project_sources(tx, project).await?;
    let mut ids = vec![];
    for e in evidence {
        if e.line < 1 || e.quote.len() > 2000 {
            bail!("invalid: waiting citation must quote an exact line of at most 2000 UTF-8 bytes");
        }
        let row=sqlx::query("SELECT d.source_id,r.body FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?").bind(&e.document_id).bind(e.revision).fetch_optional(&mut **tx).await?.context("invalid waiting evidence revision")?;
        let source: String = row.get("source_id");
        if !scope.contains(&source)
            || row
                .get::<String, _>("body")
                .lines()
                .nth(e.line as usize - 1)
                != Some(e.quote.as_str())
        {
            bail!("invalid waiting evidence scope or locator");
        }
        ids.push(source);
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}
pub(crate) async fn for_sources(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    scope: &[String],
    day: NaiveDate,
) -> Result<Vec<WaitingFor>> {
    let mut items = vec![];
    for row in sqlx::query("SELECT record FROM waiting_for WHERE project_id=? AND json_extract(record,'$.status')='waiting' ORDER BY json_extract(record,'$.follow_up_on'),id").bind(project).fetch_all(&mut **tx).await? {
        let item:WaitingFor=serde_json::from_str(row.get("record"))?;
        if item.source_ids.iter().all(|id|scope.contains(id)){items.push(refresh(tx,item,day,false).await?);}
    }
    Ok(items)
}
impl Store {
    pub async fn waiting_for(&self, id: &str, access: &Access) -> Result<WaitingFor> {
        let mut tx = self.pool.begin().await?;
        let item = raw(&mut tx, id).await?;
        permit(&mut tx, &item.source_ids, access).await?;
        let item = refresh(&mut tx, item, today(), true).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn waiting_list(
        &self,
        project: &str,
        query: WaitingQuery,
        access: &Access,
    ) -> Result<WaitingList> {
        let status = query.status.as_deref().unwrap_or("waiting");
        if !["waiting", "resolved", "cancelled", "all"].contains(&status) {
            bail!("invalid waiting status filter");
        }
        let limit = query.limit.unwrap_or(25) as usize;
        let offset = query.offset.unwrap_or(0) as usize;
        if limit == 0 || limit > 50 || offset > 10000 {
            bail!("invalid waiting page; limit 1–50, offset 0–10000");
        }
        let mut tx = self.pool.begin().await?;
        let scope = project_sources(&mut tx, project).await?;
        let allowed: Vec<String> = sqlx::query("SELECT id,allow_external FROM sources ORDER BY id")
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .filter(|r| access.permits(r.get("id"), r.get("allow_external")))
            .map(|r| r.get("id"))
            .collect();
        if !scope.iter().any(|id| allowed.contains(id)) {
            bail!("record not found");
        }
        let day = today();
        let mut items = vec![];
        let mut total = 0usize;
        for row in sqlx::query("SELECT record FROM waiting_for WHERE project_id=? ORDER BY CASE json_extract(record,'$.status') WHEN 'waiting' THEN 0 ELSE 1 END,json_extract(record,'$.follow_up_on'),id").bind(project).fetch_all(&mut *tx).await? {
            let item:WaitingFor=serde_json::from_str(row.get("record"))?;
            if !item.source_ids.iter().all(|id|allowed.contains(id)) || (status!="all" && item.status!=status) || (query.due_only && !(item.status=="waiting" && date(&item.follow_up_on)?<=day)){continue;}
            if total>=offset && items.len()<limit {items.push(refresh(&mut tx,item,day,false).await?);}total+=1;
        }
        let watermark = sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM events")
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(WaitingList {
            project_id: project.into(),
            as_of_date: day.to_string(),
            watermark,
            items,
            total,
            offset,
            limit,
        })
    }
    pub async fn create_waiting(&self, mut req: WaitingCreateRequest) -> Result<WaitingFor> {
        validate_text(&req.title, 300, "waiting request title")?;
        validate_text(&req.person, 200, "waiting person")?;
        optional_text(&req.note, 1000, "waiting note")?;
        let requested = date(&req.requested_on)?;
        let next = date(&req.follow_up_on)?;
        let interval = next.signed_duration_since(requested).num_days();
        if requested > today() || !(0..=365).contains(&interval) {
            bail!(
                "invalid: requested date cannot be in the future; follow-up must be 0–365 days after it"
            );
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "waiting-create", "", &req.idempotency_key, &req).await?;
        let id = if let Some(id) = old {
            id
        } else {
            let scope = project_access(&mut tx, &req.project_id, &Access::admin()).await?;
            if let Some(task_id) = &req.task_id {
                let row=sqlx::query("SELECT t.*,d.source_id FROM tasks t JOIN documents d ON d.id=t.document_id WHERE t.id=?").bind(task_id).fetch_optional(&mut *tx).await?.context("record not found")?;
                if row.get::<String, _>("status") != "accepted"
                    || !scope.contains(&row.get::<String, _>("source_id"))
                {
                    bail!("invalid: link an accepted task in this project");
                }
                let e = crate::store::task_row(&row).evidence;
                if !req.evidence.contains(&e) {
                    req.evidence.push(e);
                }
            }
            let source_ids = citations(&mut tx, &req.project_id, &req.evidence).await?;
            let active:i64=sqlx::query_scalar("SELECT COUNT(*) FROM waiting_for WHERE project_id=? AND json_extract(record,'$.status')='waiting'").bind(&req.project_id).fetch_one(&mut *tx).await?;
            if active >= 1000 {
                bail!("invalid: project already has 1000 active waiting records");
            }
            let item = WaitingFor {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: req.project_id.clone(),
                task_id: req.task_id.clone(),
                title: req.title.trim().into(),
                person: req.person.trim().into(),
                source_ids,
                evidence: req.evidence.clone(),
                requested_on: req.requested_on.clone(),
                follow_up_on: req.follow_up_on.clone(),
                follow_up_interval_days: interval,
                status: "waiting".into(),
                version: 1,
                last_followed_up_at: None,
                resolved_at: None,
                created_at: now(),
                updated_at: now(),
                follow_up_due: false,
                stale_evidence: false,
                linked_task_status: None,
                suggested_follow_up: None,
                history: vec![],
                history_count: 1,
                history_omitted: 0,
            };
            save(&mut tx, &item, "created", &req.note, &item.evidence).await?;
            remember(&mut tx, &req.idempotency_key, &digest, &item.id).await?;
            item.id
        };
        tx.commit().await?;
        self.waiting_for(&id, &Access::admin()).await
    }
    pub async fn update_waiting(&self, id: &str, req: WaitingUpdateRequest) -> Result<WaitingFor> {
        if !["edit", "snooze", "follow_up", "resolve", "cancel", "reopen"]
            .contains(&req.action.as_str())
        {
            bail!("invalid waiting action");
        }
        optional_text(&req.note, 1000, "waiting note")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "waiting-update", id, &req.idempotency_key, &req).await?;
        if old.is_none() {
            let mut item = raw(&mut tx, id).await?;
            if item.version != req.expected_version {
                bail!("conflict: waiting record changed; refresh before editing");
            }
            if req.action == "reopen" {
                if item.status == "waiting" {
                    bail!("conflict: record is already waiting");
                }
            } else if item.status != "waiting" {
                bail!("conflict: reopen the waiting record before changing it");
            }
            if req.action != "edit" && (req.title.is_some() || req.person.is_some()) {
                bail!("invalid: title/person changes require the edit action");
            }
            if let Some(title) = &req.title {
                validate_text(title, 300, "waiting title")?;
                item.title = title.trim().into();
            }
            if let Some(person) = &req.person {
                validate_text(person, 200, "waiting person")?;
                item.person = person.trim().into();
            }
            if ["resolve", "cancel"].contains(&req.action.as_str()) && req.follow_up_on.is_some() {
                bail!("invalid: closing a request does not reschedule it");
            }
            if ["snooze", "follow_up", "reopen"].contains(&req.action.as_str())
                && req.follow_up_on.is_none()
            {
                bail!("invalid: choose the next follow-up date");
            }
            if let Some(next) = &req.follow_up_on {
                let day = date(next)?;
                let min = if ["snooze", "follow_up"].contains(&req.action.as_str()) {
                    today() + chrono::Duration::days(1)
                } else if req.action == "reopen" {
                    today()
                } else {
                    date(&item.requested_on)?
                };
                if day < min || day > today() + chrono::Duration::days(365) {
                    bail!(
                        "invalid: choose a valid next follow-up date within the next year; snoozes and recorded follow-ups require a future date"
                    );
                }
                item.follow_up_on = next.clone();
                item.follow_up_interval_days = day.signed_duration_since(today()).num_days().max(0);
            }
            if req.action == "reopen" {
                let active:i64=sqlx::query_scalar("SELECT COUNT(*) FROM waiting_for WHERE project_id=? AND json_extract(record,'$.status')='waiting'").bind(&item.project_id).fetch_one(&mut *tx).await?;
                if active >= 1000 {
                    bail!("invalid: project already has 1000 active waiting records");
                }
                item.status = "waiting".into();
                item.resolved_at = None;
            }
            if !req.evidence.is_empty() {
                let new_sources = citations(&mut tx, &item.project_id, &req.evidence).await?;
                item.source_ids.extend(new_sources);
                item.source_ids.sort();
                item.source_ids.dedup();
                // Preserve original request evidence; response evidence belongs to its attributed history event.
            }
            match req.action.as_str() {
                "follow_up" => item.last_followed_up_at = Some(now()),
                "resolve" => {
                    item.status = "resolved".into();
                    item.resolved_at = Some(now());
                }
                "cancel" => {
                    item.status = "cancelled".into();
                    item.resolved_at = Some(now());
                }
                _ => {}
            }
            item.version += 1;
            item.updated_at = now();
            item.history_count += 1;
            save(&mut tx, &item, &req.action, &req.note, &req.evidence).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        self.waiting_for(id, &Access::admin()).await
    }
}
