use super::*;
async fn raw(
    tx: &mut Transaction<'_, Sqlite>,
    project: &str,
    id: &str,
    access: &Access,
) -> Result<WeeklyOutcomes> {
    scope(tx, project, access).await?;
    let json: String =
        sqlx::query_scalar("SELECT record FROM insight_weekly WHERE id=? AND project_id=?")
            .bind(id)
            .bind(project)
            .fetch_optional(&mut **tx)
            .await?
            .context("record not found")?;
    let item: WeeklyOutcomes = serde_json::from_str(&json)?;
    let ids = item
        .sources
        .iter()
        .map(|s| s.source_id.clone())
        .collect::<Vec<_>>();
    permit(tx, &ids, access).await?;
    Ok(item)
}
fn section(kind: &str, items: Vec<BriefItem>) -> BriefSection {
    BriefSection {
        kind: kind.into(),
        items,
        stale: false,
    }
}
impl Store {
    pub async fn create_weekly_outcomes(
        &self,
        project: &str,
        req: WeeklyCreate,
        access: &Access,
    ) -> Result<WeeklyOutcomes> {
        admin(access)?;
        let (start, end) = period(&req.start_date, &req.end_date)?;
        if end > Utc::now().date_naive() {
            bail!("invalid: outcomes cannot include future dates");
        }
        let start = format!("{start}T00:00:00+00:00");
        let end_boundary = end
            .succ_opt()
            .context("invalid end date")?
            .and_hms_opt(0, 0, 0)
            .context("invalid end date")?
            .and_utc();
        let start_time = DateTime::parse_from_rfc3339(&start)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let ids = scope(&mut tx, project, access).await?;
        let (digest, old) = operation(
            &mut tx,
            "weekly-create",
            project,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(id) = old {
            return raw(&mut tx, project, &id, access).await;
        }
        let docs = documents(&mut tx, &ids).await?;
        let doc_ids: Vec<_> = docs.iter().map(|d| d.id.clone()).collect();
        let mut transitions = Vec::new();
        let mut completed = Vec::new();
        let mut loose = Vec::new();
        let mut notes:Vec<String>=vec!["The period uses UTC dates, inclusive at both date boundaries. Captured snapshots do not establish completeness of uncaptured work. For an end date of today, only history recorded through draft creation is available.".into()];
        let mut states = std::collections::BTreeMap::<String, TaskTransition>::new();
        for row in sqlx::query(
            "SELECT record FROM insight_task_history ORDER BY recorded_at,task_id,version",
        )
        .fetch_all(&mut *tx)
        .await?
        {
            let transition: TaskTransition = serde_json::from_str(row.get("record"))?;
            if !doc_ids.contains(&transition.task.evidence.document_id) {
                continue;
            }
            let stamp = DateTime::parse_from_rfc3339(&transition.recorded_at)?;
            if stamp >= end_boundary {
                continue;
            }
            if stamp >= start_time
                && transition.origin != "migration_baseline_prior_history_unknown"
                && transition.previous_status.as_deref() != Some("completed")
                && transition.task.status == "completed"
            {
                completed.push(BriefItem {
                    text: format!(
                        "{} · completed {}",
                        transition.task.title, transition.recorded_at
                    ),
                    evidence: vec![transition.task.evidence.clone()],
                });
            }
            if stamp >= start_time && transitions.len() < 500 {
                transitions.push(transition.clone());
            }
            states.insert(transition.task.id.clone(), transition);
        }
        for state in states.values() {
            if state.task.status == "accepted" {
                loose.push(BriefItem {
                    text: format!(
                        "{} · accepted and unresolved at the period boundary",
                        state.task.title
                    ),
                    evidence: vec![state.task.evidence.clone()],
                });
            }
            if state.origin == "migration_baseline_prior_history_unknown"
                && !notes.iter().any(|n| n.contains("migration baseline"))
            {
                notes.push("Some tasks have only a migration baseline. Their earlier completion/acceptance dates are unknown; no historical outcome is inferred from that baseline.".into());
            }
        }
        let mut decisions = Vec::new();
        for row in
            sqlx::query("SELECT record FROM brief_revisions WHERE brief_id=? ORDER BY revision")
                .bind(project)
                .fetch_all(&mut *tx)
                .await?
        {
            let rev: BriefRevision = serde_json::from_str(row.get("record"))?;
            if rev.status != "accepted" || !rev.sources.iter().all(|s| ids.contains(&s.source_id)) {
                continue;
            }
            let Some(stamp) = rev
                .reviewed_at
                .as_deref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            else {
                continue;
            };
            if stamp < start_time || stamp >= end_boundary {
                continue;
            }
            for change in rev.changes.into_iter().filter(|s| s.kind == "decisions") {
                for mut entry in change.added {
                    entry.text = format!("Accepted decision added: {}", entry.text);
                    decisions.push(entry);
                }
                for mut entry in change.removed {
                    entry.text =
                        format!("Prior decision removed from accepted brief: {}", entry.text);
                    decisions.push(entry);
                }
            }
        }
        let mut blockers = Vec::new();
        let mut pinned = Vec::new();
        for doc in &docs {
            // Select the latest captured revision at the end boundary; never scan a later revision as if it existed earlier.
            let rows =
                sqlx::query("SELECT * FROM revisions WHERE document_id=? ORDER BY revision DESC")
                    .bind(&doc.id)
                    .fetch_all(&mut *tx)
                    .await?;
            for row in rows {
                let stamp = DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))?;
                if stamp >= end_boundary {
                    continue;
                }
                let body: String = row.get("body");
                let revision: i64 = row.get("revision");
                pinned.push(BriefSourceRevision {
                    source_id: doc.source_id.clone(),
                    document_id: doc.id.clone(),
                    title: row.get("title"),
                    revision,
                    content_hash: crate::store::hash(&format!(
                        "{}\0{}",
                        row.get::<String, _>("title"),
                        body
                    )),
                });
                for (index, line) in body.lines().enumerate() {
                    let text = line.trim().to_lowercase();
                    if line.len() <= 2000
                        && (text.starts_with("blocker:")
                            || text.starts_with("blocked:")
                            || text.starts_with("risk:"))
                    {
                        blockers.push(BriefItem {
                            text: line.trim().into(),
                            evidence: vec![Evidence {
                                document_id: doc.id.clone(),
                                revision,
                                line: index as i64 + 1,
                                quote: line.into(),
                            }],
                        });
                    }
                }
                break;
            }
        }
        // Ledger promises can exist without extracted tasks. Reconstruct their
        // state from immutable events, never from today's status/title/owner.
        let mut commitment_snapshots = Vec::new();
        let mut ledger_omitted = 0usize;
        for row in
            sqlx::query("SELECT id,record FROM delivery_commitments WHERE project_id=? ORDER BY id")
                .bind(project)
                .fetch_all(&mut *tx)
                .await?
        {
            let current: Commitment = serde_json::from_str(row.get("record"))?;
            permit(&mut tx, &current.source_ids, access).await?;
            let mut history = Vec::new();
            for event in sqlx::query("SELECT sequence,record FROM delivery_history WHERE commitment_id=? ORDER BY sequence")
                .bind(&current.id).fetch_all(&mut *tx).await?
            {
                let mut event_record: CommitmentEvent = serde_json::from_str(event.get("record"))?;
                if DateTime::parse_from_rfc3339(&event_record.created_at)? < end_boundary {
                    event_record.sequence = event.get("sequence");
                    history.push(event_record);
                }
            }
            let Some(as_of) = history.last().cloned() else {
                continue;
            };
            if !["open", "proposed"].contains(&as_of.status.as_str()) {
                continue;
            }
            if commitment_snapshots.len() >= 100 {
                ledger_omitted += 1;
                continue;
            }
            // Preserve original promise evidence and every amendment supporting
            // the state shown at the boundary. Later amendments are excluded.
            let mut promise_evidence = Vec::new();
            for event in history
                .iter()
                .filter(|event| ["created", "amend"].contains(&event.action.as_str()))
            {
                for evidence in &event.evidence {
                    if !promise_evidence.contains(evidence) {
                        promise_evidence.push(evidence.clone());
                    }
                }
            }
            if let Some(task_id) = &current.task_id {
                if let Some(state) = states.get(task_id) {
                    if state.task.status == "accepted" {
                        let task_evidence = &state.task.evidence;
                        loose.retain(|entry| {
                            !(entry.evidence == vec![task_evidence.clone()]
                                && entry.text
                                    == format!(
                                        "{} · accepted and unresolved at the period boundary",
                                        state.task.title
                                    ))
                        });
                    }
                }
            }
            loose.push(BriefItem {
                text: format!(
                    "{} · promised by {} on {} · {} at period boundary{} · ledger {} v{}",
                    as_of.title,
                    as_of.owner,
                    current.promised_on,
                    as_of.status,
                    as_of
                        .due
                        .as_ref()
                        .map(|due| format!(" · due {due}"))
                        .unwrap_or_default(),
                    current.id,
                    as_of.version
                ),
                evidence: promise_evidence.clone(),
            });
            // Pin all revisions used in the retained promise history, including
            // an earlier revision than the latest period-end source snapshot.
            for evidence in history.iter().flat_map(|event| &event.evidence) {
                if pinned.iter().any(|source| {
                    source.document_id == evidence.document_id
                        && source.revision == evidence.revision
                }) {
                    continue;
                }
                let source = sqlx::query("SELECT d.source_id,r.title,r.content_hash FROM documents d JOIN revisions r ON r.document_id=d.id WHERE d.id=? AND r.revision=?")
                    .bind(&evidence.document_id).bind(evidence.revision).fetch_one(&mut *tx).await?;
                let source_id: String = source.get("source_id");
                if !ids.contains(&source_id) {
                    bail!("record not found");
                }
                pinned.push(BriefSourceRevision {
                    source_id,
                    document_id: evidence.document_id.clone(),
                    title: source.get("title"),
                    revision: evidence.revision,
                    content_hash: source.get("content_hash"),
                });
            }
            commitment_snapshots.push(WeeklyCommitmentSnapshot {
                commitment_id: current.id,
                linked_task_id: current.task_id,
                promised_on: current.promised_on,
                as_of,
                promise_evidence,
                history,
            });
        }
        if ledger_omitted > 0 {
            notes.push(format!("{ledger_omitted} unresolved ledger promises omitted by the 100-promise snapshot limit."));
        }
        if !commitment_snapshots.is_empty() {
            notes.push("Unresolved ledger promises use immutable history recorded before the UTC period-end boundary. Linked accepted tasks are represented once by their ledger promise. Promise dates are user declarations; observation dates are the recorded history timestamps.".into());
        }
        let mut omitted = 0;
        for items in [&mut completed, &mut decisions, &mut blockers, &mut loose] {
            omitted += items.len().saturating_sub(100);
            items.truncate(100);
        }
        if omitted > 0 {
            notes.push(format!(
                "{omitted} section items omitted by the 100-item-per-section limit."
            ));
        }
        if docs
            .iter()
            .any(|doc| !pinned.iter().any(|source| source.document_id == doc.id))
        {
            notes.push("Some project documents had no captured revision at this period boundary and are omitted.".into());
        }
        let item = WeeklyOutcomes {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project.into(),
            version: 1,
            start_date: req.start_date,
            end_date: req.end_date,
            sections: vec![
                section("completed_work", completed),
                section("changed_decisions", decisions),
                section("blockers_at_period_end", blockers),
                section("unresolved_commitments", loose),
            ],
            transitions,
            commitment_snapshots,
            sources: pinned,
            notes,
            status: "draft".into(),
            review_note: None,
            created_at: now(),
        };
        if serde_json::to_vec(&item)?.len() > 1000000 {
            bail!(
                "invalid: outcome draft exceeds size budget; narrow the period or project sources"
            );
        }
        sqlx::query("INSERT INTO insight_weekly(id,project_id,record) VALUES(?,?,?)")
            .bind(&item.id)
            .bind(project)
            .bind(serde_json::to_string(&item)?)
            .execute(&mut *tx)
            .await?;
        history(&mut tx, "weekly_outcomes", &item.id, 1, &item).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &item.id).await?;
        tx.commit().await?;
        Ok(item)
    }
    pub async fn weekly_outcomes(
        &self,
        project: &str,
        access: &Access,
    ) -> Result<Vec<WeeklyOutcomes>> {
        let mut tx = self.pool.begin().await?;
        scope(&mut tx, project, access).await?;
        let mut result = Vec::new();
        for row in sqlx::query(
            "SELECT id FROM insight_weekly WHERE project_id=? ORDER BY rowid DESC LIMIT 30",
        )
        .bind(project)
        .fetch_all(&mut *tx)
        .await?
        {
            result.push(raw(&mut tx, project, row.get("id"), access).await?);
        }
        Ok(result)
    }
    pub async fn weekly_outcome(
        &self,
        project: &str,
        id: &str,
        access: &Access,
    ) -> Result<WeeklyOutcomes> {
        let mut tx = self.pool.begin().await?;
        raw(&mut tx, project, id, access).await
    }
    pub async fn review_weekly_outcomes(
        &self,
        project: &str,
        id: &str,
        req: InsightReview,
        access: &Access,
    ) -> Result<WeeklyOutcomes> {
        admin(access)?;
        validate_text(&req.note, 2000, "outcome review note")?;
        if !["reviewed", "rejected", "draft"].contains(&req.decision.as_str()) {
            bail!("invalid outcome review decision");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "weekly-review", id, &req.idempotency_key, &req).await?;
        let mut item = raw(&mut tx, project, id, access).await?;
        if old.is_some() {
            return Ok(item);
        }
        if item.version != req.expected_version {
            bail!("conflict: outcome draft changed");
        }
        item.version += 1;
        item.status = req.decision;
        item.review_note = Some(req.note);
        sqlx::query("UPDATE insight_weekly SET record=? WHERE id=?")
            .bind(serde_json::to_string(&item)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        history(&mut tx, "weekly_outcomes", id, item.version, &item).await?;
        remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        tx.commit().await?;
        Ok(item)
    }
}
