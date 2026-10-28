//! Check saved evaluations without rewriting their original scores or outputs.
use super::*;

pub(super) async fn input_tx(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    target: &PluginEvaluationTarget,
) -> Result<(EvalRun, PluginEvaluationInput)> {
    if !p.grant.operations.iter().any(|op| op == "eval.check")
        || !p.grant.operations.iter().any(|op| op == "sources.read")
    {
        bail!("forbidden: evaluation checks require eval.check and sources.read");
    }
    if target.case_ids.len() > 50 {
        bail!("invalid: select at most fifty evaluation cases");
    }
    let record: String = sqlx::query_scalar("SELECT record FROM eval_runs WHERE id=?")
        .bind(&target.run_id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    let base: EvalRun = serde_json::from_str(&record)?;
    scope_tx(tx, p, &base.source_ids, &[]).await?;
    let mut selected = HashSet::new();
    for id in &target.case_ids {
        if !selected.insert(id.as_str()) || !base.results.iter().any(|c| &c.case_id == id) {
            bail!("invalid: select unique cases from this saved evaluation");
        }
    }
    let cases = base
        .results
        .iter()
        .filter(|r| selected.is_empty() || selected.contains(r.case_id.as_str()))
        .map(|r| {
            let definition = base
                .suite
                .cases
                .iter()
                .find(|c| c.id == r.case_id)
                .context("invalid: saved evaluation case definition missing")?;
            Ok(PluginEvaluationCaseInput {
                definition: definition.clone(),
                result: r.clone(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if cases.is_empty() {
        bail!("invalid: select a nonempty evaluation input");
    }
    let mut evidence = vec![];
    for case in &cases {
        evidence.extend(case.definition.required_evidence.clone());
        evidence.extend(case.result.output.evidence.clone());
    }
    for e in evidence {
        let source: Option<String> =
            sqlx::query_scalar("SELECT source_id FROM documents WHERE id=?")
                .bind(&e.document_id)
                .fetch_optional(&mut **tx)
                .await?;
        if source.is_some_and(|id| !base.source_ids.contains(&id)) {
            bail!(
                "forbidden: selected evaluation evidence references a source outside its recorded scope"
            );
        }
    }
    let input = PluginEvaluationInput {
        run_id: base.id.clone(),
        signature: crate::store::hash(&record),
        suite_id: base.suite.id.clone(),
        suite_version: base.suite.version,
        project_id: base.project_id.clone(),
        cases,
    };
    // Reserve space for user parameters and the invocation envelope. Never
    // truncate an eval's source scope or silently discard selected case output.
    if serde_json::to_vec(&(&input, &base.source_snapshots, &p.configuration))?.len() > 440_000 {
        bail!(
            "invalid: evaluation input exceeds plugin wire budget; select fewer cases or a smaller saved source scope"
        );
    }
    Ok((base, input))
}

pub(super) async fn record_checks_tx(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    run: &PluginContributionRun,
    input: &PluginEvaluationInput,
    checks: Vec<PluginEvaluationCheck>,
    docs: &[Document],
) -> Result<PluginEvaluationReceipt> {
    if checks.is_empty() || checks.len() > 50 {
        bail!(
            "invalid: return one or more checks for every selected evaluation case (maximum fifty)"
        );
    }
    let mut ids = HashSet::new();
    let mut cases = HashSet::new();
    let mut results = vec![];
    for check in checks {
        let case = input
            .cases
            .iter()
            .find(|c| c.definition.id == check.case_id)
            .context("invalid: check targets an unselected evaluation case")?;
        validate_text(&check.title, 120, "evaluation check title")?;
        validate_text(&check.detail, 2000, "evaluation check detail")?;
        if !validation::identifier(&check.check_id)
            || !ids.insert((check.case_id.clone(), check.check_id.clone()))
            || !check.score.is_finite()
            || !(0.0..=1.0).contains(&check.score)
            || check.evidence.len() > 20
            || check.evidence.iter().any(|e| !evidence_valid(e, docs))
        {
            bail!("invalid: check identity, score or pinned evidence is invalid");
        }
        cases.insert(check.case_id.clone());
        results.push(PluginEvaluationCheckResult {
            passed: check.score >= case.definition.threshold,
            threshold: case.definition.threshold,
            check,
        });
    }
    if input
        .cases
        .iter()
        .any(|c| !cases.contains(&c.definition.id))
    {
        bail!("invalid: a selected evaluation case has no returned checks");
    }
    let receipt = PluginEvaluationReceipt {
        id: uuid::Uuid::new_v4().to_string(),
        plugin_run_id: run.id.clone(),
        plugin_id: p.id.clone(),
        installed_at: p.installed_at.clone(),
        configuration_version: p.version,
        package_version: p.manifest.version.clone(),
        contribution_id: run.contribution.id.clone(),
        evaluation_run_id: input.run_id.clone(),
        evaluation_signature: input.signature.clone(),
        configuration_hash: run.configuration_hash.clone(),
        executable_definition_hash: run.executable_definition_hash.clone(),
        created_at: now(),
        results,
    };
    sqlx::query("INSERT INTO plugin_eval_checks(id,plugin_run_id,eval_run_id,created_at,record) VALUES(?,?,?,?,?)").bind(&receipt.id).bind(&run.id).bind(&input.run_id).bind(&receipt.created_at).bind(serde_json::to_string(&receipt)?).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id,causation_id,origin_plugin,depth) VALUES(?,'eval.run.completed','plugin_eval_check',?,1,?,?,?,?,?,1)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(&receipt.id).bind(serde_json::to_string(&run.source_ids)?).bind(serde_json::to_string(&vec![input.project_id.clone()])?).bind(&run.id).bind(&run.event_id).bind(&p.id).execute(&mut **tx).await?;
    event(tx, "evaluation", &input.run_id).await?;
    Ok(receipt)
}

impl Store {
    pub async fn plugin_evaluation_inputs(
        &self,
        plugin: &str,
    ) -> Result<Vec<PluginEvaluationInputSummary>> {
        let p = self.plugin(plugin).await?;
        if !p.enabled
            || !p.grant.operations.iter().any(|op| op == "eval.check")
            || !p.grant.operations.iter().any(|op| op == "sources.read")
        {
            return Ok(vec![]);
        }
        let mut tx = self.pool.begin().await?;
        // Only selection metadata is loaded for this list, not source bodies or
        // saved outputs for unrelated projects.
        let rows=sqlx::query("SELECT id,project_id,created_at,json_extract(record,'$.suite.name') AS suite_name,json_extract(record,'$.suite.version') AS suite_version,json_extract(record,'$.source_ids') AS sources,(SELECT json_group_array(json_object('id',json_extract(c.value,'$.case_id'),'name',json_extract(c.value,'$.name'))) FROM json_each(eval_runs.record,'$.results') c) AS cases FROM eval_runs ORDER BY created_at DESC,id DESC LIMIT 200").fetch_all(&mut *tx).await?;
        let mut result = vec![];
        for row in rows {
            let sources: Vec<String> = serde_json::from_str(row.get("sources"))?;
            if scope_tx(&mut tx, &p, &sources, &[]).await.is_err() {
                continue;
            }
            result.push(PluginEvaluationInputSummary {
                run_id: row.get("id"),
                suite_name: row.get("suite_name"),
                suite_version: row.get("suite_version"),
                project_id: row.get("project_id"),
                created_at: row.get("created_at"),
                source_count: sources.len(),
                cases: serde_json::from_str(row.get("cases"))?,
            });
        }
        Ok(result)
    }
}
