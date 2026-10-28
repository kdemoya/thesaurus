use super::*;
fn hit_evidence(hits: &[SearchHit], docs: &[Document], query: &str) -> Vec<Evidence> {
    let terms: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut result = vec![];
    for h in hits {
        if let Some(d) = docs
            .iter()
            .find(|d| d.id == h.document_id && d.revision == h.revision)
        {
            for (n, line) in d.body.lines().enumerate() {
                if line.len() <= 2000 && terms.iter().any(|t| line.to_lowercase().contains(t)) {
                    result.push(Evidence {
                        document_id: d.id.clone(),
                        revision: d.revision,
                        line: n as i64 + 1,
                        quote: line.into(),
                    });
                    if result.len() >= 100 {
                        return result;
                    }
                }
            }
        }
    }
    result
}
pub(super) fn score(
    case: &EvalCase,
    output: EvalOutput,
    docs: &[Document],
    baseline: Option<&EvalCaseResult>,
) -> EvalCaseResult {
    let mut checks = vec![];
    let mut passed = 0usize;
    let mut count = 0usize;
    for text in &case.expected_contains {
        count += 1;
        let ok = output.text.contains(text);
        passed += usize::from(ok);
        checks.push(format!(
            "{} required text: {}",
            if ok { "PASS" } else { "FAIL" },
            text
        ));
    }
    for text in &case.forbidden_contains {
        count += 1;
        let ok = !output.text.contains(text);
        passed += usize::from(ok);
        checks.push(format!(
            "{} forbidden text: {}",
            if ok { "PASS" } else { "FAIL" },
            text
        ));
    }
    let mut stale = 0;
    let mut invalid = 0;
    let mut duplicate = 0;
    let mut seen = HashSet::new();
    for e in &output.evidence {
        if !seen.insert((e.document_id.clone(), e.revision, e.line, e.quote.clone())) {
            duplicate += 1;
        }
        match docs.iter().find(|d| d.id == e.document_id) {
            None => invalid += 1,
            Some(d) => {
                if d.revision != e.revision {
                    stale += 1;
                }
                if e.line < 1
                    || d.body.lines().nth(e.line.saturating_sub(1) as usize)
                        != Some(e.quote.as_str())
                {
                    invalid += 1;
                }
            }
        }
    }
    for required in &case.required_evidence {
        count += 1;
        let valid = docs.iter().any(|d| {
            d.id == required.document_id
                && d.revision == required.revision
                && d.body.lines().nth(required.line.saturating_sub(1) as usize)
                    == Some(required.quote.as_str())
        });
        let ok = valid && output.evidence.contains(required);
        passed += usize::from(ok);
        checks.push(format!(
            "{} exact evidence {} r{} line {}{}",
            if ok { "PASS" } else { "FAIL" },
            required.document_id,
            required.revision,
            required.line,
            if valid {
                ""
            } else {
                " (expected evidence no longer matches pinned snapshot)"
            }
        ));
    }
    // Citation integrity is an explicit criterion, including when no evidence is required.
    count += 1;
    let integrity = stale == 0 && invalid == 0;
    passed += usize::from(integrity);
    checks.push(format!(
        "{} citation integrity; {} stale, {} invalid, {} duplicates",
        if integrity { "PASS" } else { "FAIL" },
        stale,
        invalid,
        duplicate
    ));
    let value = passed as f64 / count as f64;
    let baseline_score = baseline.map(|r| r.score);
    if baseline.is_some_and(|b| b.target != case.target || b.threshold != case.threshold) {
        checks.push("Baseline target or threshold changed; inspect versioned suite before interpreting score changes".into());
    }
    let comparison = baseline.map(|b| {
        let usage = if b.output.usage_status.contains("unreported") || output.usage_status.contains("unreported") || b.output.usage_status.is_empty() || output.usage_status.is_empty() {
            "Token/cost usage unreported or legacy unknown; numeric totals are not comparable and zero is not a free-usage claim".to_string()
        } else {
            format!("Reported cost {} → {} micro-units; input tokens {} → {}; output tokens {} → {} (supplied usage is self-reported, unverified)",b.output.cost_microunits,output.cost_microunits,b.output.input_tokens,output.input_tokens,b.output.output_tokens,output.output_tokens)
        };
        vec![format!("Score {:.3} → {:.3}; stale evidence {} → {}; invalid evidence {} → {}; duplicate evidence {} → {}",b.score,value,b.stale_evidence,stale,b.invalid_evidence,invalid,b.duplicate_evidence,duplicate),usage]
    }).unwrap_or_else(||vec!["New case; no matching baseline result".into()]);
    let regression = baseline.is_some_and(|b| {
        b.target == case.target && (value < b.score || (b.passed && value < case.threshold))
    });
    EvalCaseResult {
        case_id: case.id.clone(),
        name: case.name.clone(),
        target: case.target.clone(),
        threshold: case.threshold,
        score: value,
        passed: value >= case.threshold,
        checks,
        output,
        stale_evidence: stale,
        duplicate_evidence: duplicate,
        invalid_evidence: invalid,
        baseline_score,
        regression,
        comparison,
    }
}
impl Store {
    pub async fn execute_eval(&self, id: &str, req: EvalRunRequest) -> Result<EvalRun> {
        let started = std::time::Instant::now();
        // Idempotent replay precedes current-state/budget validation.
        let mut tx = self.pool.begin().await?;
        let (digest, old) = operation(&mut tx, "eval-run", id, &req.idempotency_key, &req).await?;
        if let Some(old) = old {
            tx.commit().await?;
            return self.eval_run(&old, &Access::admin()).await;
        }
        tx.commit().await?;
        let preview = self.eval_preview(id, &req).await?;
        let suite = self
            .eval_suite(id, Some(req.suite_version), &Access::admin())
            .await?;
        let mut tx = self.pool.begin().await?;
        let scope = project_sources(&mut tx, &req.project_id).await?;
        let docs = documents(&mut tx, &scope).await?;
        if sources_signature(&docs) != preview.signature {
            bail!("conflict: sources changed; preview and run again");
        }
        tx.commit().await?;
        let watermark = self.watermark().await?;
        let access = Access {
            contributor: "evaluation".into(),
            source_ids: Some(scope.clone()),
            allow_write: false,
            external: false,
        };
        let baseline = if let Some(b) = &req.baseline_id {
            let b = self.eval_run(b, &Access::admin()).await?;
            if b.project_id != req.project_id || b.suite.id != suite.id {
                bail!("invalid: baseline must use the same suite and project");
            }
            Some(b)
        } else {
            None
        };
        let mut results = vec![];
        let mut output_bytes = 0usize;
        for case in &suite.cases {
            let mut output = EvalOutput {
                case_id: case.id.clone(),
                agent: "thesaurus deterministic runner v1".into(),
                prompt_version: "eval-v1".into(),
                model: "not-applicable".into(),
                provenance_kind: "deterministic".into(),
                usage_status: "no model request".into(),
                ..Default::default()
            };
            match case.target.as_str() {
                "retrieval" => {
                    let hits = self
                        .search(
                            &SearchRequest {
                                query: case.query.clone(),
                                source_id: None,
                                limit: Some(20),
                            },
                            &access,
                        )
                        .await?;
                    output.evidence = hit_evidence(&hits, &docs, &case.query);
                    output.text = serde_json::to_string(&hits)?;
                }
                "memory" => {
                    let bundle = self.context(&case.query, 16000, &access).await?;
                    output.evidence = hit_evidence(&bundle.documents, &docs, &case.query);
                    for t in &bundle.tasks {
                        output.evidence.push(t.evidence.clone());
                    }
                    output.evidence.truncate(100);
                    output.text = serde_json::to_string(&bundle)?;
                }
                "brief" => {
                    let p = self.brief(&req.project_id, &Access::admin()).await?;
                    if let Some(b) = p
                        .revisions
                        .into_iter()
                        .find(|r| Some(r.revision) == p.accepted_revision)
                    {
                        for s in &b.sections {
                            for i in &s.items {
                                output.evidence.extend(i.evidence.clone());
                            }
                        }
                        output.evidence.truncate(100);
                        output.text = serde_json::to_string(&b)?;
                        output.agent = if b.connection.trim().is_empty() {
                            "unknown (legacy brief producer)".into()
                        } else {
                            b.connection.clone()
                        };
                        output.model = "unknown (legacy ACP model unreported)".into();
                        output.agent_version =
                            "unknown (legacy ACP agent version unreported)".into();
                        output.prompt_version = "unknown (legacy brief prompt version)".into();
                        output.provenance_kind = "accepted brief revision".into();
                        output.usage_status =
                            "ACP usage unreported; zero is not a cost claim".into();
                        output.runtime_id = b.run_id.clone();
                        if let Some(id) = &b.run_id {
                            if let Ok(runtime) = self.runtime(id).await {
                                output.agent = runtime.connection;
                                output.prompt_hash = hash(&runtime.prompt);
                                output.prompt_version =
                                    format!("brief-prompt-sha256:{}", output.prompt_hash);
                                if let Some(p) = runtime.provenance {
                                    output.agent = p.agent_name;
                                    output.agent_version = p.agent_version;
                                    output.model = p.model;
                                    output.prompt_hash = p.actual_prompt_hash;
                                    output.prompt_version =
                                        format!("brief-wire-prompt-sha256:{}", output.prompt_hash);
                                }
                            }
                        }
                    } else {
                        output.text = "No accepted project brief".into();
                    }
                }
                "priorities" => {
                    let board = self.priorities(&req.project_id, &Access::admin()).await?;
                    for i in &board.items {
                        let e = &i.current.task.evidence;
                        if let Some(q) = &e.quote {
                            output.evidence.push(Evidence {
                                document_id: e.document_id.clone(),
                                revision: e.revision,
                                line: e.line,
                                quote: q.clone(),
                            });
                        }
                    }
                    output.evidence.truncate(100);
                    output.text = serde_json::to_string(&board)?;
                }
                "supplied" => {
                    output = req
                        .outputs
                        .iter()
                        .find(|o| o.case_id == case.id)
                        .cloned()
                        .context("invalid: missing supplied output")?;
                    output.runtime_id = None;
                    if output.agent_version.trim().is_empty() {
                        output.agent_version = "unknown (user did not report agent version)".into();
                    }
                    output.provenance_kind = "user-reported, unverified supplied output".into();
                    output.usage_status = "user-reported, unverified usage".into();
                }
                _ => bail!("invalid target"),
            }
            validate_output(&output)?;
            output_bytes += output.text.len();
            if output_bytes > req.max_output_bytes {
                bail!("invalid: generated output exceeds run byte budget; no run saved");
            }
            results.push(score(
                case,
                output,
                &docs,
                baseline
                    .as_ref()
                    .and_then(|b| b.results.iter().find(|r| r.case_id == case.id)),
            ));
        }
        let sum = |f: fn(&EvalOutput) -> u64| -> Result<u64> {
            results.iter().try_fold(0u64, |n, r| {
                n.checked_add(f(&r.output))
                    .context("invalid: usage overflow")
            })
        };
        let cost = sum(|o| o.cost_microunits)?;
        let input = sum(|o| o.input_tokens)?;
        let out = sum(|o| o.output_tokens)?;
        let baseline_removed_cases = baseline
            .as_ref()
            .map(|b| {
                b.results
                    .iter()
                    .filter(|old| !suite.cases.iter().any(|c| c.id == old.case_id))
                    .map(|old| old.case_id.clone())
                    .collect()
            })
            .unwrap_or_default();
        let run = EvalRun {
            plugin_checks: vec![],
            max_judge_requests: req.max_judge_requests,
            max_judge_input_tokens: req.max_judge_input_tokens,
            id: uuid::Uuid::new_v4().to_string(),
            suite,
            project_id: req.project_id.clone(),
            source_ids: scope.clone(),
            source_snapshots: docs,
            created_at: now(),
            runner_version: "thesaurus-eval-v1".into(),
            baseline_id: req.baseline_id.clone(),
            baseline_removed_cases,
            results,
            cost_microunits: cost,
            input_tokens: input,
            output_tokens: out,
            duration_ms: started.elapsed().as_millis() as u64,
        };
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (_, old) = operation(&mut tx, "eval-run", id, &req.idempotency_key, &req).await?;
        if let Some(old) = old {
            tx.commit().await?;
            return self.eval_run(&old, &Access::admin()).await;
        }
        let current = documents(&mut tx, &scope).await?;
        let actual_watermark: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(sequence),0) FROM events")
                .fetch_one(&mut *tx)
                .await?;
        if actual_watermark != watermark
            || sources_signature(&current) != preview.signature
            || project_sources(&mut tx, &req.project_id).await? != scope
        {
            bail!("conflict: workspace changed during evaluation; run again");
        }
        sqlx::query(
            "INSERT INTO eval_runs(id,suite_id,project_id,record,created_at) VALUES(?,?,?,?,?)",
        )
        .bind(&run.id)
        .bind(id)
        .bind(&req.project_id)
        .bind(serde_json::to_string(&run)?)
        .bind(&run.created_at)
        .execute(&mut *tx)
        .await?;
        remember(&mut tx, &req.idempotency_key, &digest, &run.id).await?;
        event(&mut tx, "evaluation", &run.id).await?;
        tx.commit().await?;
        Ok(run)
    }
}
