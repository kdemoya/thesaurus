use super::*;
use chrono::{DateTime, Utc};
async fn coverage(tx: &mut Transaction<'_, Sqlite>, source: &str) -> Result<SourceCoverage> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sources WHERE id=?)")
        .bind(source)
        .fetch_one(&mut **tx)
        .await?;
    if !exists {
        bail!("record not found");
    }
    let record: Option<String> =
        sqlx::query_scalar("SELECT record FROM source_control_coverage WHERE source_id=?")
            .bind(source)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(match record {
        Some(record) => serde_json::from_str(&record)?,
        None => SourceCoverage {
            source_id: source.into(),
            version: 0,
            expected_interval_hours: None,
            excluded_content: vec![],
            capture_gaps: vec![],
            updated_at: None,
        },
    })
}
impl Store {
    pub async fn source_health(&self, source: &str, access: &Access) -> Result<SourceHealth> {
        let sources = self.sources(access).await?;
        let source = sources
            .into_iter()
            .find(|s| s.id == source)
            .context("record not found")?;
        let mut tx = self.pool.begin().await?;
        let coverage = coverage(&mut tx, &source.id).await?;
        let upstream_record: Option<String> =
            sqlx::query_scalar("SELECT record FROM retrieval_upstreams WHERE source_id=?")
                .bind(&source.id)
                .fetch_optional(&mut *tx)
                .await?;
        let upstream: Option<UpstreamConnection> = upstream_record
            .map(|record| serde_json::from_str(&record))
            .transpose()?;
        let last_synced_at = upstream
            .as_ref()
            .and_then(|upstream| upstream.last_fetched_at.clone());
        let upstream_id = upstream.as_ref().map(|upstream| upstream.id.clone());
        let retrieval_policy = upstream.as_ref().map(|upstream| upstream.policy.clone());
        let upstream_error=upstream.as_ref().and_then(|upstream|upstream.last_error.as_ref().map(|_|"The latest configured upstream read failed. Retry this source explicitly or inspect its connection in Retrieval.".to_string()));
        let docs: Vec<String> =
            sqlx::query_scalar("SELECT id FROM documents WHERE source_id=? ORDER BY id")
                .bind(&source.id)
                .fetch_all(&mut *tx)
                .await?;
        let processed:i64=sqlx::query_scalar("SELECT COUNT(*) FROM documents d WHERE d.source_id=? AND EXISTS(SELECT 1 FROM jobs j WHERE j.document_id=d.id AND j.revision=d.revision AND j.status='completed')").bind(&source.id).fetch_one(&mut *tx).await?;
        let failed:i64=sqlx::query_scalar("SELECT COUNT(*) FROM jobs j JOIN documents d ON d.id=j.document_id AND d.revision=j.revision WHERE d.source_id=? AND j.status='failed'").bind(&source.id).fetch_one(&mut *tx).await?;
        let pending:i64=sqlx::query_scalar("SELECT COUNT(*) FROM jobs j JOIN documents d ON d.id=j.document_id AND d.revision=j.revision WHERE d.source_id=? AND j.status IN ('queued','processing')").bind(&source.id).fetch_one(&mut *tx).await?;
        let age = DateTime::parse_from_rfc3339(&source.updated_at)
            .map(|time| (Utc::now() - time.with_timezone(&Utc)).num_seconds())
            .unwrap_or(i64::MAX);
        let stale = coverage
            .expected_interval_hours
            .is_some_and(|hours| age > i64::from(hours) * 3600);
        let mut effects = Vec::new();
        if upstream.is_some() && last_synced_at.is_none() {
            effects.push("This configured upstream source has no successful sync receipt; no upstream coverage is established.".into());
        }
        if upstream_error.is_some() {
            effects.push("The latest upstream attempt failed; the available snapshot may exclude newer information.".into());
        }
        if retrieval_policy.as_deref() == Some("live") {
            effects.push("This source is configured for live retrieval. Ordinary indexed search and general context omit its cached content; an explicitly approved upstream query is required.".into());
        }
        if stale {
            effects.push("The source is older than your expected update interval; answers may miss newer changes.".into());
        }
        if processed < docs.len() as i64 {
            effects.push("Some current documents have not completed action extraction; action coverage may be incomplete.".into());
        }
        if failed > 0 {
            effects.push(
                "Current extraction failures may omit commitments; retry the affected document."
                    .into(),
            );
        }
        if !coverage.excluded_content.is_empty() {
            effects.push("Declared exclusions are outside captured evidence; answers cannot establish facts from them.".into());
        }
        if !coverage.capture_gaps.is_empty() {
            effects.push("Declared capture gaps may omit discussion, decisions or commitments in those periods.".into());
        }
        if effects.is_empty() {
            effects.push("Current captured documents are available; no completeness beyond captured content is asserted.".into());
        }
        Ok(SourceHealth{coverage,source_name:source.name,last_ingested_at:source.updated_at,last_synced_at,upstream_id,upstream_error,retrieval_policy,document_ids:docs,current_documents_processed:processed,failed_current_jobs:failed,pending_current_jobs:pending,stale,ingestion_mode:"Captured/imported source snapshots. Last ingestion is local capture time; a successful configured upstream read establishes only that read's returned scope, not upstream completeness.".into(),effect_on_answers:effects})
    }
    pub async fn update_source_coverage(
        &self,
        source: &str,
        req: SourceCoverageUpdate,
        access: &Access,
    ) -> Result<SourceCoverage> {
        admin(access)?;
        if req
            .expected_interval_hours
            .is_some_and(|h| h == 0 || h > 87600)
            || req.excluded_content.len() > 30
            || req.capture_gaps.len() > 50
        {
            bail!("invalid coverage limits");
        }
        for text in &req.excluded_content {
            validate_text(text, 1000, "excluded content")?;
        }
        for gap in &req.capture_gaps {
            validate_text(&gap.note, 1000, "capture gap note")?;
            let start = DateTime::parse_from_rfc3339(&gap.started_at)
                .context("invalid gap start timestamp")?;
            if let Some(end) = &gap.ended_at {
                if DateTime::parse_from_rfc3339(end).context("invalid gap end timestamp")? < start {
                    bail!("invalid: capture gap ends before it starts");
                }
            }
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "source-coverage",
            source,
            &req.idempotency_key,
            &req,
        )
        .await?;
        let mut record = coverage(&mut tx, source).await?;
        if old.is_some() {
            return Ok(record);
        }
        if record.version != req.expected_version {
            bail!("conflict: coverage changed; reload");
        }
        record.version += 1;
        record.expected_interval_hours = req.expected_interval_hours;
        record.excluded_content = req.excluded_content;
        record.capture_gaps = req.capture_gaps;
        record.updated_at = Some(now());
        save(
            &mut tx,
            "source_control_coverage",
            source,
            record.version,
            &record,
        )
        .await?;
        remember(&mut tx, &req.idempotency_key, &digest, source).await?;
        tx.commit().await?;
        Ok(record)
    }
    pub async fn retry_source_capture(
        &self,
        source: &str,
        document: &str,
        access: &Access,
    ) -> Result<Job> {
        admin(access)?;
        let doc = self.document(document, None, access).await?;
        if doc.source_id != source {
            bail!("invalid: document is outside source");
        }
        self.queue_analysis(document, "local", None).await
    }
}
