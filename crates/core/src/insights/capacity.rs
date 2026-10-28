use super::*;
use chrono::{FixedOffset, TimeZone};
type Span = (i64, i64);
fn span(start: &str, end: &str) -> Result<Span> {
    let start = DateTime::parse_from_rfc3339(start)
        .context("invalid start timestamp; include timezone")?
        .timestamp();
    let end = DateTime::parse_from_rfc3339(end)
        .context("invalid end timestamp; include timezone")?
        .timestamp();
    if end <= start || end - start > 32 * 86400 {
        bail!("invalid interval duration");
    }
    Ok((start, end))
}
fn merged(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort();
    let mut result: Vec<Span> = Vec::new();
    for (start, end) in spans {
        if let Some(last) = result.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        result.push((start, end));
    }
    result
}
fn intersect(a: Span, b: Span) -> Option<Span> {
    let start = a.0.max(b.0);
    let end = a.1.min(b.1);
    (start < end).then_some((start, end))
}
fn seconds(spans: &[Span]) -> i64 {
    spans.iter().map(|(s, e)| e - s).sum()
}
fn free_before(work: &[Span], events: &[Span], cutoff: i64) -> i64 {
    let clipped = work
        .iter()
        .filter_map(|s| intersect(*s, (i64::MIN, cutoff)))
        .collect::<Vec<_>>();
    let busy = merged(
        clipped
            .iter()
            .flat_map(|w| events.iter().filter_map(|e| intersect(*w, *e)))
            .collect(),
    );
    (seconds(&clipped) - seconds(&busy)) / 60
}
async fn raw(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    id: &str,
    access: &Access,
) -> Result<CapacityPlan> {
    scope(tx, project, access).await?;
    let json: String =
        sqlx::query_scalar("SELECT record FROM insight_capacity WHERE id=? AND project_id=?")
            .bind(id)
            .bind(project)
            .fetch_optional(&mut **tx)
            .await?
            .context("record not found")?;
    let item: CapacityPlan = serde_json::from_str(&json)?;
    permit(tx, &item.source_ids, access).await?;
    Ok(item)
}
impl Store {
    pub async fn task_efforts(&self, project: &str, access: &Access) -> Result<Vec<TaskEffort>> {
        let mut tx = self.pool.begin().await?;
        let ids = scope(&mut tx, project, access).await?;
        let docs = documents(&mut tx, &ids).await?;
        let current = tasks(&mut tx, &docs).await?;
        let mut result = Vec::new();
        for row in
            sqlx::query("SELECT record FROM insight_efforts WHERE project_id=? ORDER BY task_id")
                .bind(project)
                .fetch_all(&mut *tx)
                .await?
        {
            let effort: TaskEffort = serde_json::from_str(row.get("record"))?;
            if current.iter().any(|task| task.id == effort.task_id) {
                result.push(effort);
            }
        }
        Ok(result)
    }
    pub async fn set_task_effort(
        &self,
        project: &str,
        task: &str,
        req: TaskEffortUpdate,
        access: &Access,
    ) -> Result<TaskEffort> {
        admin(access)?;
        if req.minutes == 0 || req.minutes > 100000 || req.note.len() > 2000 {
            bail!("invalid effort estimate");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let ids = scope(&mut tx, project, access).await?;
        let docs = documents(&mut tx, &ids).await?;
        if !tasks(&mut tx, &docs).await?.iter().any(|t| t.id == task) {
            bail!("record not found");
        }
        let (digest, old) = operation(
            &mut tx,
            "task-effort",
            &format!("{project}:{task}"),
            &req.idempotency_key,
            &req,
        )
        .await?;
        let json: Option<String> = sqlx::query_scalar(
            "SELECT record FROM insight_efforts WHERE project_id=? AND task_id=?",
        )
        .bind(project)
        .bind(task)
        .fetch_optional(&mut *tx)
        .await?;
        let prior: Option<TaskEffort> = json.map(|j| serde_json::from_str(&j)).transpose()?;
        if old.is_some() {
            return prior.context("record not found");
        }
        let version = prior.map(|p| p.version).unwrap_or(0);
        if version != req.expected_version {
            bail!("conflict: effort estimate changed");
        }
        let item = TaskEffort {
            project_id: project.into(),
            task_id: task.into(),
            version: version + 1,
            minutes: req.minutes,
            note: req.note,
            updated_at: now(),
        };
        sqlx::query("INSERT INTO insight_efforts(project_id,task_id,record) VALUES(?,?,?) ON CONFLICT(project_id,task_id) DO UPDATE SET record=excluded.record").bind(project).bind(task).bind(serde_json::to_string(&item)?).execute(&mut *tx).await?;
        history(
            &mut tx,
            "task_effort",
            &format!("{project}:{task}"),
            item.version,
            &item,
        )
        .await?;
        remember(&mut tx, &req.idempotency_key, &digest, task).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn save_capacity_plan(
        &self,
        project: &str,
        req: CapacitySave,
        access: &Access,
    ) -> Result<CapacityPlan> {
        admin(access)?;
        validate_text(&req.name, 200, "capacity plan name")?;
        let (start, end) = period(&req.start_date, &req.end_date)?;
        if !(-840..=840).contains(&req.utc_offset_minutes)
            || req.working_windows.len() > 100
            || req.calendar.len() > 500
            || req.tradeoffs.len() > 500
            || req.selected_task_ids.len() > 500
        {
            bail!("invalid capacity plan limits");
        }
        let zone =
            FixedOffset::east_opt(req.utc_offset_minutes * 60).context("invalid UTC offset")?;
        let bounds = (
            zone.from_local_datetime(&start.and_hms_opt(0, 0, 0).unwrap())
                .single()
                .unwrap()
                .timestamp(),
            zone.from_local_datetime(
                &end.succ_opt()
                    .context("invalid end date")?
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
            .single()
            .unwrap()
            .timestamp(),
        );
        let mut work = Vec::new();
        for window in &req.working_windows {
            if let Some(value) = intersect(span(&window.starts_at, &window.ends_at)?, bounds) {
                work.push(value);
            }
        }
        let work = merged(work);
        if work.is_empty() {
            bail!("invalid: add a working-hours interval within the review period");
        }
        let mut events = Vec::new();
        let mut event_ids = Vec::new();
        for event in &req.calendar {
            validate_text(&event.id, 200, "calendar event ID")?;
            validate_text(&event.title, 500, "calendar event title")?;
            if event_ids.contains(&event.id) {
                bail!("invalid: duplicate calendar event ID");
            }
            event_ids.push(event.id.clone());
            let interval = span(&event.starts_at, &event.ends_at)?;
            if event.selected {
                if let Some(value) = intersect(interval, bounds) {
                    events.push(value);
                }
            }
        }
        let overlaps = events
            .iter()
            .enumerate()
            .map(|(i, a)| {
                events
                    .iter()
                    .skip(i + 1)
                    .filter(|b| intersect(*a, **b).is_some())
                    .count()
            })
            .sum();
        let event_union = merged(events);
        let busy = merged(
            work.iter()
                .flat_map(|w| event_union.iter().filter_map(|e| intersect(*w, *e)))
                .collect(),
        );
        let working_minutes = seconds(&work) / 60;
        let calendar_minutes = seconds(&busy) / 60;
        let free_minutes = (seconds(&work) - seconds(&busy)) / 60;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let ids = scope(&mut tx, project, access).await?;
        let (digest, old) = operation(
            &mut tx,
            "capacity-save",
            project,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(id) = old {
            return raw(&mut tx, project, &id, access).await;
        }
        let prior = if let Some(id) = &req.id {
            Some(raw(&mut tx, project, id, access).await?)
        } else {
            None
        };
        let version = prior.as_ref().map(|p| p.version).unwrap_or(0);
        if version != req.expected_version {
            bail!("conflict: capacity plan changed");
        }
        let docs = documents(&mut tx, &ids).await?;
        let mut accepted = tasks(&mut tx, &docs)
            .await?
            .into_iter()
            .filter(|t| t.status == "accepted")
            .collect::<Vec<_>>();
        if req.selected_task_ids.is_empty()
            || req
                .selected_task_ids
                .iter()
                .any(|id| !accepted.iter().any(|t| &t.id == id))
        {
            bail!("invalid: explicitly select accepted work to include in personal capacity");
        }
        accepted.retain(|task| req.selected_task_ids.contains(&task.id));
        if accepted.len() > 500 {
            bail!("invalid: capacity review limited to 500 accepted tasks");
        }
        accepted.sort_by_key(|t| t.due.clone().unwrap_or_else(|| "9999-99-99".into()));
        let mut estimates = std::collections::BTreeMap::new();
        for row in sqlx::query("SELECT record FROM insight_efforts WHERE project_id=?")
            .bind(project)
            .fetch_all(&mut *tx)
            .await?
        {
            let item: TaskEffort = serde_json::from_str(row.get("record"))?;
            estimates.insert(item.task_id, item.minutes);
        }
        let mut seen = Vec::new();
        for tradeoff in &req.tradeoffs {
            if !accepted.iter().any(|t| t.id == tradeoff.task_id)
                || seen.contains(&tradeoff.task_id)
            {
                bail!("invalid: choose distinct accepted tasks for tradeoffs");
            }
            seen.push(tradeoff.task_id.clone());
            validate_text(&tradeoff.note, 2000, "tradeoff explanation")?;
            match tradeoff.action.as_str() {
                "reduce" => {
                    if !tradeoff
                        .adjusted_minutes
                        .is_some_and(|m| m > 0 && m <= 100000)
                    {
                        bail!("invalid reduced effort");
                    }
                }
                "defer" => {
                    if date(
                        tradeoff
                            .new_due
                            .as_deref()
                            .context("invalid: deferred date required")?,
                    )? <= end
                    {
                        bail!("invalid: deferred date must follow the review period");
                    }
                }
                "delegate" => {}
                _ => bail!("invalid tradeoff action"),
            }
        }
        let mut planned = 0u64;
        let mut rows = Vec::new();
        let mut missing = 0;
        let mut cumulative = 0u64;
        for task in accepted {
            let due = task.due.as_deref().and_then(|s| date(s).ok());
            if due.is_some_and(|d| d > end) {
                continue;
            }
            let estimate = estimates.get(&task.id).copied();
            let tradeoff = req.tradeoffs.iter().find(|t| t.task_id == task.id);
            let adjusted = match tradeoff.map(|t| t.action.as_str()) {
                Some("delegate" | "defer") => Some(0),
                Some("reduce") => tradeoff.and_then(|t| t.adjusted_minutes),
                _ => estimate,
            };
            if adjusted.is_none() {
                missing += 1;
            }
            planned += u64::from(adjusted.unwrap_or(0));
            let available = due.map(|day| {
                let cutoff = zone
                    .from_local_datetime(
                        &day.succ_opt().unwrap_or(day).and_hms_opt(0, 0, 0).unwrap(),
                    )
                    .single()
                    .unwrap()
                    .timestamp();
                free_before(&work, &event_union, cutoff)
            });
            if due.is_some() {
                cumulative += u64::from(adjusted.unwrap_or(0));
            }
            rows.push(CapacityTask {
                task,
                estimated_minutes: estimate,
                planned_minutes: adjusted,
                available_before_deadline: available,
                cumulative_due_minutes: due.map(|_| cumulative),
                deadline_overload: available.is_some_and(|free| cumulative > free.max(0) as u64),
            });
        }
        let deadline_totals = rows
            .iter()
            .filter_map(|row| {
                row.task
                    .due
                    .as_deref()
                    .and_then(|value| date(value).ok())
                    .map(|due| (due, u64::from(row.planned_minutes.unwrap_or(0))))
            })
            .collect::<Vec<_>>();
        for row in &mut rows {
            if let Some(due) = row.task.due.as_deref().and_then(|value| date(value).ok()) {
                let total = deadline_totals
                    .iter()
                    .filter(|(d, _)| *d <= due)
                    .map(|(_, minutes)| minutes)
                    .sum::<u64>();
                row.cumulative_due_minutes = Some(total);
                row.deadline_overload = row
                    .available_before_deadline
                    .is_some_and(|free| total > free.max(0) as u64);
            }
        }
        let mut needed = planned.saturating_sub(free_minutes.max(0) as u64);
        let mut suggested_tradeoffs = Vec::new();
        if needed > 0 {
            for row in rows.iter().rev() {
                if needed == 0 {
                    break;
                }
                let Some(minutes) = row.planned_minutes.filter(|minutes| *minutes > 0) else {
                    continue;
                };
                if req.tradeoffs.iter().any(|t| t.task_id == row.task.id) {
                    continue;
                }
                let reduction = needed.min(u64::from(minutes));
                let action = if reduction < u64::from(minutes) {
                    "reduce"
                } else {
                    "delegate"
                };
                suggested_tradeoffs.push(CapacityTradeoff{task_id:row.task.id.clone(),action:action.into(),adjusted_minutes:(action=="reduce").then_some(minutes-reduction as u32),new_due:None,note:format!("Proposed capacity tradeoff: free {reduction} personal minutes from '{}'. Check importance, ownership and feasibility before applying this scenario.",row.task.title)});
                needed -= reduction;
            }
        }
        let mut notes=vec!["Calendar overlap is counted once inside the union of your working intervals. Outside-hours events do not consume working capacity. Dates use the plan's explicitly selected fixed UTC offset; verify intervals over daylight-saving transitions.".into(),"Tradeoffs are editable planning scenarios only. They do not change accepted task deadlines, owners, calendar events or external systems.".into()];
        if missing > 0 {
            notes.push(format!("{missing} accepted tasks have no effort estimate. Their effort is unknown, so the displayed load is a lower bound."));
        }
        if overlaps > 0 {
            notes.push(format!("{overlaps} pairs of selected calendar intervals overlap; review scheduling conflicts."));
        }
        if planned > free_minutes.max(0) as u64 {
            notes.push(format!("Planned effort exceeds free working capacity by {} minutes; consider reducing, deferring or delegating selected work.",planned-free_minutes.max(0)as u64));
        }
        let item = CapacityPlan {
            id: prior
                .as_ref()
                .map(|p| p.id.clone())
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            project_id: project.into(),
            source_ids: ids,
            version: version + 1,
            name: req.name,
            start_date: req.start_date,
            end_date: req.end_date,
            utc_offset_minutes: req.utc_offset_minutes,
            selected_task_ids: req.selected_task_ids,
            working_windows: req.working_windows,
            calendar: req.calendar,
            tradeoffs: req.tradeoffs,
            suggested_tradeoffs,
            tasks: rows,
            working_minutes,
            calendar_minutes,
            free_minutes,
            planned_minutes: planned,
            overlapping_calendar_pairs: overlaps,
            notes,
            created_at: prior.map(|p| p.created_at).unwrap_or_else(now),
            updated_at: now(),
        };
        if serde_json::to_vec(&item)?.len() > 1000000 {
            bail!("invalid: capacity plan exceeds response budget; narrow project scope");
        }
        sqlx::query("INSERT INTO insight_capacity(id,project_id,record) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(&item.id).bind(project).bind(serde_json::to_string(&item)?).execute(&mut *tx).await?;
        history(&mut tx, "capacity_plan", &item.id, item.version, &item).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &item.id).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn capacity_plans(
        &self,
        project: &str,
        access: &Access,
    ) -> Result<Vec<CapacityPlan>> {
        let mut tx = self.pool.begin().await?;
        scope(&mut tx, project, access).await?;
        let mut result = Vec::new();
        for row in sqlx::query(
            "SELECT id FROM insight_capacity WHERE project_id=? ORDER BY rowid DESC LIMIT 30",
        )
        .bind(project)
        .fetch_all(&mut *tx)
        .await?
        {
            result.push(raw(&mut tx, project, row.get("id"), access).await?);
        }
        Ok(result)
    }
}
