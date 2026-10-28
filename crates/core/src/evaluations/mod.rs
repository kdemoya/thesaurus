//! Versioned evaluation suites. Deterministic runs never call a model or change source facts.
use crate::{
    Store,
    continuity::{documents, event, operation, permit, project_sources, remember},
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
use sqlx::{Sqlite, Transaction};
use std::collections::HashSet;
use thesaurus_contracts::*;

const MAX_SOURCE_BYTES: usize = 2_000_000;
fn bounded(s: &str, n: usize, label: &str) -> Result<()> {
    if s.len() > n || s.contains('\0') {
        bail!("invalid {label}");
    }
    Ok(())
}
fn suite_valid(r: &EvalSuiteRequest) -> Result<()> {
    validate_text(&r.name, 200, "evaluation name")?;
    if r.cases.is_empty() || r.cases.len() > 50 {
        bail!("invalid: choose 1–50 evaluation cases");
    }
    if serde_json::to_vec(r)?.len() > 256000 {
        bail!("invalid: suite exceeds 256KB");
    }
    let mut ids = HashSet::new();
    for c in &r.cases {
        validate_text(&c.id, 100, "case id")?;
        if !c
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            bail!("invalid: case IDs use letters, numbers, underscores and dashes");
        }
        validate_text(&c.name, 200, "case name")?;
        if !ids.insert(&c.id) {
            bail!("invalid: duplicate case id");
        }
        if !["retrieval", "memory", "brief", "priorities", "supplied"].contains(&c.target.as_str())
        {
            bail!("invalid evaluation target");
        }
        bounded(&c.query, 1024, "query")?;
        bounded(&c.rubric, 4000, "rubric")?;
        bounded(&c.replay_prompt, 4000, "replay prompt")?;
        bounded(&c.prompt_version, 200, "prompt version")?;
        if !c.replay_prompt.trim().is_empty() {
            validate_text(&c.prompt_version, 200, "replay prompt version")?;
        }
        if !c.threshold.is_finite() || !(0.0..=1.0).contains(&c.threshold) {
            bail!("invalid threshold");
        }
        if c.expected_contains.len() + c.forbidden_contains.len() > 50
            || c.required_evidence.len() > 50
        {
            bail!("invalid: too many criteria");
        }
        for text in c.expected_contains.iter().chain(&c.forbidden_contains) {
            validate_text(text, 1000, "criterion")?;
        }
        for e in &c.required_evidence {
            validate_text(&e.document_id, 100, "document id")?;
            validate_text(&e.quote, 2000, "quote")?;
            if e.line < 1 || e.revision < 1 {
                bail!("invalid evidence locator");
            }
        }
        if c.expected_contains.is_empty()
            && c.forbidden_contains.is_empty()
            && c.required_evidence.is_empty()
        {
            bail!(
                "invalid: each case needs deterministic criteria; rubrics may add human or model review"
            );
        }
    }
    Ok(())
}
async fn suite_tx(
    tx: &mut Transaction<'_, Sqlite>,
    id: &str,
    version: Option<i64>,
) -> Result<EvalSuite> {
    let record:String=if let Some(v)=version {sqlx::query_scalar("SELECT record FROM eval_suite_versions WHERE suite_id=? AND version=?").bind(id).bind(v).fetch_optional(&mut **tx).await?}else{sqlx::query_scalar("SELECT v.record FROM eval_suite_versions v JOIN eval_suites s ON s.id=v.suite_id AND s.version=v.version WHERE s.id=?").bind(id).fetch_optional(&mut **tx).await?}.context("record not found")?;
    Ok(serde_json::from_str(&record)?)
}
async fn suite_permit(
    tx: &mut Transaction<'_, Sqlite>,
    suite: &EvalSuite,
    access: &Access,
) -> Result<()> {
    if let Some(p) = &suite.project_id {
        let ids = project_sources(tx, p).await?;
        permit(tx, &ids, access).await?;
        permit(tx, &suite.source_ids, access).await?;
        for evidence in suite.cases.iter().flat_map(|c| &c.required_evidence) {
            let source: Option<String> =
                sqlx::query_scalar("SELECT source_id FROM documents WHERE id=?")
                    .bind(&evidence.document_id)
                    .fetch_optional(&mut **tx)
                    .await?;
            let source = source.context("record not found")?;
            let pinned = if suite.source_ids.is_empty() {
                &ids
            } else {
                &suite.source_ids
            };
            if !pinned.contains(&source) {
                bail!("record not found");
            }
            permit(tx, &[source], access).await?;
        }
    } else if access.source_ids.is_some() {
        bail!("record not found");
    }

    Ok(())
}
async fn run_permit(
    tx: &mut Transaction<'_, Sqlite>,
    run: &EvalRun,
    access: &Access,
) -> Result<()> {
    let scope = project_sources(tx, &run.project_id).await?;
    permit(tx, &scope, access).await?;
    permit(tx, &run.source_ids, access).await
}
fn sources_signature(docs: &[Document]) -> String {
    hash(
        &serde_json::to_string(
            &docs
                .iter()
                .map(|d| (&d.id, d.revision, &d.content_hash))
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default(),
    )
}
fn validate_output(o: &EvalOutput) -> Result<()> {
    bounded(&o.text, 64000, "case output")?;
    validate_text(
        &o.agent,
        200,
        "agent (use explicit unknown when unreported)",
    )?;
    validate_text(
        &o.model,
        200,
        "model (use explicit unknown when unreported)",
    )?;
    validate_text(&o.prompt_version, 200, "prompt version")?;
    bounded(&o.agent_version, 200, "agent version")?;
    bounded(&o.prompt_hash, 200, "prompt hash")?;
    bounded(&o.provenance_kind, 200, "provenance kind")?;
    bounded(&o.usage_status, 200, "usage status")?;
    if o.evidence.len() > 100 {
        bail!("invalid: output evidence exceeds 100 citations");
    }
    for e in &o.evidence {
        bounded(&e.quote, 2000, "evidence quote")?;
        validate_text(&e.document_id, 100, "document id")?;
    }
    Ok(())
}
impl Store {
    pub async fn migrate_evaluations(&self) -> Result<()> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    pub async fn save_eval_suite(
        &self,
        id: Option<&str>,
        req: EvalSuiteRequest,
    ) -> Result<EvalSuite> {
        suite_valid(&req)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let target = id.unwrap_or("new");
        let (digest, old) =
            operation(&mut tx, "eval-suite", target, &req.idempotency_key, &req).await?;
        if let Some(old) = old {
            let (old_id, v) = old
                .rsplit_once(':')
                .context("invalid stored eval reference")?;
            let s = suite_tx(&mut tx, old_id, Some(v.parse()?)).await?;
            tx.commit().await?;
            return Ok(s);
        }
        let (id, version) = if let Some(id) = id {
            let old = suite_tx(&mut tx, id, None).await?;
            if old.version != req.expected_version {
                bail!("conflict: suite changed");
            }
            if old.project_id != req.project_id {
                bail!("invalid: suite scope is immutable; copy the suite instead");
            }
            (id.to_string(), old.version + 1)
        } else {
            if req.expected_version != 0 {
                bail!("conflict: new suite expects version 0");
            }
            (uuid::Uuid::new_v4().to_string(), 1)
        };
        let suite_sources = if let Some(p) = &req.project_id {
            let scope = project_sources(&mut tx, p).await?;
            permit(&mut tx, &scope, &Access::admin()).await?;
            let docs = documents(&mut tx, &scope).await?;
            for e in req.cases.iter().flat_map(|c| &c.required_evidence) {
                if !docs.iter().any(|d| d.id == e.document_id) {
                    bail!("invalid: suite evidence is outside project");
                }
            }
            scope
        } else {
            Vec::new()
        };
        let s = EvalSuite {
            source_ids: suite_sources,
            id: id.clone(),
            version,
            name: req.name,
            project_id: req.project_id,
            cases: req.cases,
            created_at: now(),
        };
        sqlx::query("INSERT INTO eval_suites(id,version,project_id) VALUES(?,?,?) ON CONFLICT(id) DO UPDATE SET version=excluded.version").bind(&id).bind(version).bind(&s.project_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO eval_suite_versions(suite_id,version,record) VALUES(?,?,?)")
            .bind(&id)
            .bind(version)
            .bind(serde_json::to_string(&s)?)
            .execute(&mut *tx)
            .await?;
        remember(
            &mut tx,
            &req.idempotency_key,
            &digest,
            &format!("{id}:{version}"),
        )
        .await?;
        event(&mut tx, "evaluation", &id).await?;
        tx.commit().await?;
        Ok(s)
    }
    pub async fn eval_suite(
        &self,
        id: &str,
        version: Option<i64>,
        access: &Access,
    ) -> Result<EvalSuite> {
        let mut tx = self.pool.begin().await?;
        let s = suite_tx(&mut tx, id, version).await?;
        suite_permit(&mut tx, &s, access).await?;
        tx.commit().await?;
        Ok(s)
    }
    pub async fn eval_suites(&self, access: &Access) -> Result<Vec<EvalSuite>> {
        let rows =
            sqlx::query_scalar::<_, String>("SELECT id FROM eval_suites ORDER BY id LIMIT 500")
                .fetch_all(&self.pool)
                .await?;
        let mut result = vec![];
        for id in rows {
            if let Ok(s) = self.eval_suite(&id, None, access).await {
                result.push(s);
            }
        }
        Ok(result)
    }
    pub async fn eval_preview(&self, id: &str, req: &EvalRunRequest) -> Result<EvalPreview> {
        if serde_json::to_vec(req)?.len() > 1_000_000 {
            bail!("invalid: evaluation run request exceeds 1MB");
        }
        let mut tx = self.pool.begin().await?;
        let s = suite_tx(&mut tx, id, Some(req.suite_version)).await?;
        if s.project_id.as_ref().is_some_and(|p| p != &req.project_id) {
            bail!("invalid: suite belongs to another project");
        }
        let scope = project_sources(&mut tx, &req.project_id).await?;
        permit(&mut tx, &scope, &Access::admin()).await?;
        let docs = documents(&mut tx, &scope).await?;
        if docs.iter().map(|d| d.body.len()).sum::<usize>() > MAX_SOURCE_BYTES {
            bail!(
                "invalid: evaluation project sources exceed 2MB snapshot limit; use a smaller project"
            );
        }
        if req.max_judge_requests > 100
            || req.max_judge_input_tokens > 1_000_000
            || (req.max_judge_requests == 0) != (req.max_judge_input_tokens == 0)
        {
            bail!(
                "invalid: choose both zero to disable judges, or1–100judge requests and1–1000000total estimated input tokens"
            );
        }
        suite_permit(&mut tx, &s, &Access::admin()).await?;
        if req.max_cases == 0
            || req.max_cases > 50
            || s.cases.len() > req.max_cases
            || req.max_output_bytes == 0
            || req.max_output_bytes > 512000
        {
            bail!("invalid: evaluation run exceeds case/output budget");
        }
        let mut seen = HashSet::new();
        let mut cost = 0u64;
        for o in &req.outputs {
            validate_output(o)?;
            if !seen.insert(&o.case_id)
                || !s
                    .cases
                    .iter()
                    .any(|c| c.id == o.case_id && c.target == "supplied")
            {
                bail!("invalid: outputs must uniquely match supplied cases");
            }
            cost = cost
                .checked_add(o.cost_microunits)
                .context("invalid: cost overflow")?;
        }
        if req.outputs.iter().map(|o| o.text.len()).sum::<usize>() > req.max_output_bytes
            || cost > req.max_cost_microunits
        {
            bail!("invalid: submitted output exceeds run budget");
        }
        for c in &s.cases {
            if c.target == "supplied" && !req.outputs.iter().any(|o| o.case_id == c.id) {
                bail!("invalid: supplied case {} has no output", c.id);
            }
            for e in &c.required_evidence {
                if !docs.iter().any(|d| d.id == e.document_id) {
                    bail!(
                        "invalid: personal suite evidence is outside selected project; suite grants no source access"
                    );
                }
            }
        }
        let result = EvalPreview {
            max_judge_requests: req.max_judge_requests,
            max_judge_input_tokens: req.max_judge_input_tokens,
            suite_id: id.into(),
            suite_version: s.version,
            project_id: req.project_id.clone(),
            cases: s.cases.len(),
            source_ids: scope,
            source_bytes: docs.iter().map(|d| d.body.len()).sum(),
            submitted_cost_microunits: cost,
            model_calls: 0,
            signature: sources_signature(&docs),
        };
        tx.commit().await?;
        Ok(result)
    }
    pub async fn eval_run(&self, id: &str, access: &Access) -> Result<EvalRun> {
        let mut tx = self.pool.begin().await?;
        let record: String = sqlx::query_scalar("SELECT record FROM eval_runs WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .context("record not found")?;
        let mut run: EvalRun = serde_json::from_str(&record)?;
        run_permit(&mut tx, &run, access).await?;
        run.plugin_checks = sqlx::query_scalar::<_, String>(
            "SELECT record FROM plugin_eval_checks WHERE eval_run_id=? ORDER BY created_at,id",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|s| serde_json::from_str(&s))
        .collect::<std::result::Result<Vec<_>, _>>()?;
        tx.commit().await?;
        Ok(run)
    }
    pub async fn eval_runs(&self, project: &str, access: &Access) -> Result<Vec<EvalRunSummary>> {
        let mut tx = self.pool.begin().await?;
        let ids = project_sources(&mut tx, project).await?;
        permit(&mut tx, &ids, access).await?;
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT record FROM eval_runs WHERE project_id=? ORDER BY created_at DESC LIMIT 100",
        )
        .bind(project)
        .fetch_all(&mut *tx)
        .await?;
        let mut result = vec![];
        for row in rows {
            let r: EvalRun = serde_json::from_str(&row)?;
            if run_permit(&mut tx, &r, access).await.is_ok() {
                result.push(EvalRunSummary {
                    id: r.id,
                    suite_id: r.suite.id,
                    suite_version: r.suite.version,
                    project_id: r.project_id,
                    created_at: r.created_at,
                    cases: r.results.len(),
                    passed: r.results.iter().filter(|r| r.passed).count(),
                    regressions: r.results.iter().filter(|r| r.regression).count(),
                });
            }
        }
        tx.commit().await?;
        Ok(result)
    }
}
mod review;
mod runner;

mod replay;
pub use replay::config_hash as eval_connection_config_hash;
