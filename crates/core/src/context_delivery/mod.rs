use crate::{
    Store,
    continuity::{event, operation, permit, project_sources, remember},
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
use sqlx::Row;
use thesaurus_contracts::*;
impl Store {
    pub async fn migrate_context_delivery(&self) -> Result<()> {
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS handoff_receipts(context_id TEXT PRIMARY KEY,version INTEGER NOT NULL,record TEXT NOT NULL);").execute(&self.pool).await?;
        Ok(())
    }
    pub async fn context_budgeted(
        &self,
        req: ContextBuildRequest,
        access: &Access,
    ) -> Result<ContextBundle> {
        if req.query.len() > 1024 || req.query.contains('\0') {
            bail!("invalid context query");
        }
        if !(500..=16000).contains(&req.character_budget)
            || !(125..=16000).contains(&req.token_budget)
        {
            bail!("invalid: context budget is 500–16000 characters and 125–16000 estimated tokens");
        }
        let allowed = self.sources(access).await?;
        let mut scope = req
            .source_ids
            .clone()
            .unwrap_or_else(|| allowed.iter().map(|s| s.id.clone()).collect());
        scope.sort();
        scope.dedup();
        if scope.len() > 50 || scope.iter().any(|id| !allowed.iter().any(|s| &s.id == id)) {
            bail!("record not found");
        }
        let live_only = self.live_only_sources().await?;
        let omitted_live = scope.iter().filter(|id| live_only.contains(id)).count();
        scope.retain(|id| !live_only.contains(id));
        let scoped = Access {
            contributor: access.contributor.clone(),
            source_ids: Some(scope.clone()),
            allow_write: false,
            external: access.external,
        };
        let before = self.watermark().await?;
        let hits = self
            .search(
                &SearchRequest {
                    query: req.query.clone(),
                    source_id: None,
                    limit: Some(50),
                },
                &scoped,
            )
            .await?;
        let snap = self.snapshot(&scoped).await?;
        let all_tasks: Vec<_> = snap
            .tasks
            .into_iter()
            .filter(|t| ["accepted", "candidate"].contains(&t.status.as_str()))
            .collect();
        let doc_count = hits.len();
        let task_count = all_tasks.len();
        let obs_count = snap.observations.len();
        let mut citations = vec![];
        let terms: Vec<_> = req
            .query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        for hit in hits.iter().take(10) {
            if let Some(d) = snap
                .documents
                .iter()
                .find(|d| d.id == hit.document_id && d.revision == hit.revision)
            {
                for (n, line) in d.body.lines().enumerate() {
                    if line.len() <= 1000 && terms.iter().any(|t| line.to_lowercase().contains(t)) {
                        citations.push(Evidence {
                            document_id: d.id.clone(),
                            revision: d.revision,
                            line: n as i64 + 1,
                            quote: line.into(),
                        });
                        if citations.len() >= 30 {
                            break;
                        }
                    }
                }
                if citations.len() >= 30 {
                    break;
                }
            }
        }
        let sources = snap
            .documents
            .iter()
            .map(|d| BriefSourceRevision {
                source_id: d.source_id.clone(),
                document_id: d.id.clone(),
                title: d.title.clone(),
                revision: d.revision,
                content_hash: d.content_hash.clone(),
            })
            .collect();
        let mut b = ContextBundle {
            query: req.query,
            documents: hits.into_iter().take(10).collect(),
            tasks: all_tasks.into_iter().take(20).collect(),
            observations: snap.observations.into_iter().take(10).collect(),
            watermark: snap.watermark,
            character_budget: req.character_budget,
            truncated: false,
            source_ids: scope,
            sources,
            citations,
            token_budget: req.token_budget,
            estimated_tokens: 0,
            omissions: BundleOmissions::default(),
        };
        b.omissions.sources = omitted_live;
        if omitted_live > 0 {
            b.omissions.notes.push(format!("{omitted_live} live-only sources omitted from this indexed context; use explicit federated retrieval for fresh approved reads"));
        }
        b.omissions.documents = doc_count.saturating_sub(b.documents.len());
        b.omissions.tasks = task_count.saturating_sub(b.tasks.len());
        b.omissions.observations = obs_count.saturating_sub(b.observations.len());
        b.omissions.notes.push("Token estimate uses UTF-8 bytes / 4, rounded up; provider tokenization may differ. Search considers top 50 matches; citations consider top 10 matches and at most 30 short exact lines. Source metadata pins the complete chosen scope.".into());
        loop {
            b.truncated = b.omissions.sources
                + b.omissions.documents
                + b.omissions.tasks
                + b.omissions.observations
                + b.omissions.citations
                > 0;
            // Iterate until the self-described estimate is stable (digit counts count too).
            let wire = serde_json::to_string(&b)?;
            let estimate = wire.len().div_ceil(4);
            if b.estimated_tokens != estimate {
                b.estimated_tokens = estimate;
                continue;
            }
            if wire.chars().count() <= req.character_budget && estimate <= req.token_budget {
                break;
            }
            if b.citations.pop().is_some() {
                b.omissions.citations += 1;
            } else if b.observations.pop().is_some() {
                b.omissions.observations += 1;
            } else if b.tasks.pop().is_some() {
                b.omissions.tasks += 1;
            } else if b.documents.pop().is_some() {
                b.omissions.documents += 1;
            } else {
                bail!(
                    "invalid: scope metadata and query exceed context budget; narrow sources or increase budgets"
                );
            }
        }
        if self.watermark().await? != before || b.watermark != before {
            bail!("conflict: workspace changed while building context; retry");
        }
        // This bundle is a returned preview rather than a retained thread
        // snapshot. Its hook carries a stable request reference and source
        // scope; source bodies never enter the event envelope.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let watermark: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM events")
            .fetch_one(&mut *tx)
            .await?;
        if watermark != before {
            bail!("conflict: workspace changed while committing context; retry");
        }
        crate::plugins::emit_tx(
            &mut tx,
            "context.bundle.created",
            "context_preview",
            &uuid::Uuid::new_v4().to_string(),
            before,
            &b.source_ids,
            &[],
        )
        .await?;
        tx.commit().await?;
        Ok(b)
    }
    pub async fn handoff_receipt(
        &self,
        thread: &str,
        context: &str,
        access: &Access,
    ) -> Result<HandoffReceipt> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT record,run_id,created_at FROM thread_contexts WHERE id=? AND thread_id=?",
        )
        .bind(context)
        .bind(thread)
        .fetch_optional(&mut *tx)
        .await?
        .context("record not found")?;
        let snapshot: ThreadContext = serde_json::from_str(row.get("record"))?;
        let mut source_ids: Vec<_> = snapshot
            .sources
            .iter()
            .map(|s| s.source_id.clone())
            .collect();
        source_ids.sort();
        source_ids.dedup();
        permit(&mut tx, &source_ids, access).await?;
        let run_id: String = row
            .get::<Option<String>, _>("run_id")
            .context("invalid: this context has not been delivered")?;
        // A filtered runtime may inspect only its own delivered receipt without receiving
        // the omitted project sources. Every other grant still needs full project scope.
        if !own_runtime_receipt(&mut tx, &run_id, access).await? {
            let current = project_sources(&mut tx, &snapshot.project_id).await?;
            permit(&mut tx, &current, access).await?;
        }
        let runtime_record: String =
            sqlx::query_scalar("SELECT record FROM runtime_runs WHERE id=?")
                .bind(&run_id)
                .fetch_one(&mut *tx)
                .await?;
        let runtime: RuntimeRun = serde_json::from_str(&runtime_record)?;
        // Hash exactly what was delivered. Re-serializing a legacy snapshot would add
        // newly defaulted fields and could change its historical revision identifier.
        let delivered_json = runtime
            .prompt
            .split_once("Context snapshot (JSON):\n")
            .map(|(_, json)| json)
            .context("invalid: runtime has no delivered context snapshot")?;
        let _: serde_json::Value = serde_json::from_str(delivered_json)
            .context("invalid: runtime delivered context JSON")?;
        let context_revision = hash(delivered_json);
        let stored: Option<String> =
            sqlx::query_scalar("SELECT record FROM handoff_receipts WHERE context_id=?")
                .bind(context)
                .fetch_optional(&mut *tx)
                .await?;
        let mut receipt = if let Some(s) = stored {
            serde_json::from_str::<HandoffReceipt>(&s)?
        } else {
            HandoffReceipt{thread_id:thread.into(),context_id:context.into(),context_revision:context_revision.clone(),context_watermark:snapshot.watermark,run_id:run_id.clone(),runtime_status:runtime.status.clone(),session_id:runtime.session_id.clone(),state:"delivered".into(),version:0,delivered_at:runtime.created_at.clone(),history:vec![HandoffEvent{action:"delivered".into(),contributor:"Thesaurus runtime".into(),note:"Immutable context associated with this run. Delivery is not acknowledgement or completion.".into(),created_at:runtime.created_at.clone()}]}
        };
        receipt.context_revision = context_revision;
        receipt.runtime_status = runtime.status;
        receipt.session_id = runtime.session_id;
        tx.commit().await?;
        Ok(receipt)
    }
    pub async fn update_handoff(
        &self,
        thread: &str,
        context: &str,
        req: HandoffUpdateRequest,
        access: &Access,
    ) -> Result<HandoffReceipt> {
        validate_text(&req.note, 2000, "handoff note")?;
        let mut receipt = self.handoff_receipt(thread, context, access).await?;
        if !access.allow_write {
            let mut tx = self.pool.begin().await?;
            let own = own_runtime_receipt(&mut tx, &receipt.run_id, access).await?;
            tx.commit().await?;
            if !own || !["acknowledged", "completion_claimed"].contains(&req.action.as_str()) {
                bail!(
                    "forbidden: receipt updates require write permission, except acknowledging or claiming completion of this runtime's own delivery"
                );
            }
        }
        if ![
            "acknowledged",
            "completion_claimed",
            "completed",
            "reopened",
        ]
        .contains(&req.action.as_str())
        {
            bail!("invalid handoff action");
        }
        if access.source_ids.is_some() && ["completed", "reopened"].contains(&req.action.as_str()) {
            bail!("forbidden: only the user can verify completed work or reopen it");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if !access.allow_write && !own_runtime_receipt(&mut tx, &receipt.run_id, access).await? {
            bail!("forbidden: runtime delivery grant is no longer active");
        }
        let (digest, old) =
            operation(&mut tx, "handoff", context, &req.idempotency_key, &req).await?;
        if old.is_some() {
            tx.commit().await?;
            return self.handoff_receipt(thread, context, access).await;
        }
        let actual: Option<i64> =
            sqlx::query_scalar("SELECT version FROM handoff_receipts WHERE context_id=?")
                .bind(context)
                .fetch_optional(&mut *tx)
                .await?;
        if receipt.version != req.expected_version || actual.unwrap_or(0) != req.expected_version {
            bail!("conflict: receipt changed; refresh");
        }
        if receipt.history.len() >= 100 {
            bail!("invalid: receipt history limit reached");
        }
        match req.action.as_str() {
            "acknowledged" if receipt.state != "delivered" && receipt.state != "reopened" => {
                bail!("invalid: receipt already acknowledged")
            }
            "completion_claimed"
                if !["acknowledged", "reopened"].contains(&receipt.state.as_str()) =>
            {
                bail!("invalid: acknowledge this delivery before claiming completion")
            }
            "completed"
                if !["acknowledged", "completion_claimed"].contains(&receipt.state.as_str()) =>
            {
                bail!("invalid: acknowledge delivery before verifying completion")
            }
            "reopened" if receipt.state != "completed" => {
                bail!("invalid: only completed handoffs can be reopened")
            }
            _ => {}
        }
        receipt.state = req.action.clone();
        receipt.version += 1;
        receipt.history.push(HandoffEvent {
            action: req.action.clone(),
            contributor: access.contributor.clone(),
            note: req.note.clone(),
            created_at: now(),
        });
        if serde_json::to_vec(&receipt)?.len() > 250000 {
            bail!("invalid: receipt exceeds 250KB history limit");
        }
        sqlx::query("INSERT INTO handoff_receipts(context_id,version,record) VALUES(?,?,?) ON CONFLICT(context_id) DO UPDATE SET version=excluded.version,record=excluded.record").bind(context).bind(receipt.version).bind(serde_json::to_string(&receipt)?).execute(&mut *tx).await?;
        remember(&mut tx, &req.idempotency_key, &digest, context).await?;
        event(&mut tx, "handoff", context).await?;
        tx.commit().await?;
        Ok(receipt)
    }
    pub async fn observe_runtime(
        &self,
        id: &str,
        cursor: Option<&str>,
    ) -> Result<RuntimeObservation> {
        let run = self.runtime(id).await?;
        let current = hash(&serde_json::to_string(&run)?);
        Ok(RuntimeObservation {
            changed: cursor != Some(current.as_str()),
            cursor: current,
            run,
            watermark: self.watermark().await?,
            observed_at: now(),
        })
    }
}

/// This identity check uses the runtime's actual grant ID, not a caller-chosen grant name.
async fn own_runtime_receipt(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    runtime: &str,
    access: &Access,
) -> Result<bool> {
    if !access.external || access.source_ids.is_none() {
        return Ok(false);
    }
    let row=sqlx::query("SELECT g.id,g.name FROM runtime_runs r JOIN grants g ON g.id=r.grant_id WHERE r.id=? AND g.revoked=0 AND g.allow_external=1 AND r.status IN ('starting','running','waiting_permission')").bind(runtime).fetch_optional(&mut **tx).await?;
    Ok(row.is_some_and(|r| {
        let id: String = r.get("id");
        let name: String = r.get("name");
        name == format!("acp-run:{runtime}") && access.contributor == format!("{name} ({id})")
    }))
}
