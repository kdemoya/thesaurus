use super::tests::{finished, fixture, request};
use super::*;

async fn eval_fixture(
    personal: bool,
) -> (tempfile::TempDir, Store, Document, PluginRecord, EvalRun) {
    let (dir, store, d, _) = fixture().await;
    let script = dir.path().join("evaluation.py");
    std::fs::write(&script,r#"import sys,json,time,pathlib
v=json.loads(sys.stdin.readline())
mode=v['input']['parameters'].get('mode','ok')
pathlib.Path(sys.argv[1]).write_text('started')
if mode=='slow': time.sleep(3)
checks=[]
for c in v['input']['evaluation']['cases']:
    evidence=c['result']['output']['evidence']
    checks.append({'case_id':c['definition']['id'],'check_id':'citation-required','title':'Evidence coverage','score':1.0 if evidence else 0.0,'detail':'Synthetic plugin requires a citation','evidence':evidence})
if mode=='bad-score': checks[0]['score']=1.1
if mode=='unknown-case': checks[0]['case_id']='not-selected'
if mode=='duplicate': checks.append(checks[0])
if mode=='missing': checks.pop()
if mode=='bad-citation': checks[0]['evidence'][0]['quote']='Wrong quote'
if mode=='forged-pass': checks[0]['passed']=True
if mode=='empty': checks=[]
print(json.dumps({'api_version':v['api_version'],'invocation_id':v['invocation_id'],'result':None,'contribution':{'text':'Synthetic evaluation checks returned','evidence':[]},'checks':checks}))
"#).unwrap();
    let manifest:PluginManifest=serde_json::from_value(json!({"id":"example.eval-checks","name":"Synthetic eval checks","version":"1.0.0","api_min":2,"api_max":2,"host":"service","requests":["eval.check","sources.read"],"contributions":[{"id":"check","kind":"evaluation","title":"Check evidence","description":"Synthetic check"}]})).unwrap();
    let plugin = store
        .install_plugin(PluginInstallRequest {
            manifest,
            executable: PluginExecutable {
                program: "/usr/bin/python3".into(),
                args: vec![
                    script.to_string_lossy().into(),
                    dir.path().join("eval-started").to_string_lossy().into(),
                ],
            },
            grant: PluginPermissions {
                source_ids: vec![d.source_id.clone()],
                operations: vec!["eval.check".into(), "sources.read".into()],
                ..Default::default()
            },
            configuration: json!({}),
            idempotency_key: "install-eval-plugin".into(),
        })
        .await
        .unwrap();
    let plugin = store
        .update_plugin(
            &plugin.id,
            PluginUpdateRequest {
                expected_version: plugin.version,
                enabled: true,
                grant: plugin.grant.clone(),
                configuration: plugin.configuration.clone(),
                manifest: None,
                executable: None,
                idempotency_key: "enable-eval-plugin".into(),
            },
        )
        .await
        .unwrap();
    let project = store
        .create_brief(BriefCreateRequest {
            name: "Synthetic eval project".into(),
            source_ids: vec![d.source_id.clone()],
            idempotency_key: "eval-project".into(),
        })
        .await
        .unwrap();
    let suite = store
        .save_eval_suite(
            None,
            EvalSuiteRequest {
                expected_version: 0,
                name: if personal {
                    "Personal checks"
                } else {
                    "Project checks"
                }
                .into(),
                project_id: if personal {
                    None
                } else {
                    Some(project.id.clone())
                },
                cases: vec![case("first", 0.75), case("second", 0.5)],
                idempotency_key: "eval-suite".into(),
            },
        )
        .await
        .unwrap();
    let citation = Evidence {
        document_id: d.id.clone(),
        revision: d.revision,
        line: 1,
        quote: d.body.clone(),
    };
    let eval = store
        .execute_eval(
            &suite.id,
            EvalRunRequest {
                max_judge_requests: 0,
                max_judge_input_tokens: 0,
                suite_version: suite.version,
                project_id: project.id,
                baseline_id: None,
                outputs: vec![output("first", vec![citation]), output("second", vec![])],
                max_cases: 2,
                max_output_bytes: 10000,
                max_cost_microunits: 0,
                idempotency_key: "eval-run".into(),
            },
        )
        .await
        .unwrap();
    (dir, store, d, plugin, eval)
}
fn case(id: &str, threshold: f64) -> EvalCase {
    serde_json::from_value(json!({"id":id,"name":format!("Synthetic {id}"),"target":"supplied","expected_contains":["Synthetic"],"threshold":threshold})).unwrap()
}
fn output(id: &str, evidence: Vec<Evidence>) -> EvalOutput {
    EvalOutput {
        case_id: id.into(),
        text: "Synthetic answer".into(),
        evidence,
        agent: "synthetic".into(),
        model: "none".into(),
        prompt_version: "f44".into(),
        ..Default::default()
    }
}
fn eval_request(
    p: &PluginRecord,
    d: &Document,
    e: &EvalRun,
    key: &str,
    mode: &str,
) -> PluginContributionRequest {
    let mut req = request(p, d, "check", key, mode);
    req.source_ids.clear();
    req.evaluation = Some(PluginEvaluationTarget {
        run_id: e.id.clone(),
        case_ids: vec![],
    });
    req
}
async fn raw_eval(store: &Store, id: &str) -> String {
    sqlx::query_scalar("SELECT record FROM eval_runs WHERE id=?")
        .bind(id)
        .fetch_one(&store.pool)
        .await
        .unwrap()
}
#[tokio::test]
async fn plugin_eval_checks_attach_to_personal_and_project_runs_without_rewriting_scores() {
    for personal in [true, false] {
        let (dir, store, d, p, e) = eval_fixture(personal).await;
        let before = raw_eval(&store, &e.id).await;
        assert!(e.results.iter().all(|r| r.passed));
        let req = eval_request(&p, &d, &e, "check-once", "ok");
        let run = store
            .request_plugin_contribution(&p.id, req.clone())
            .await
            .unwrap();
        assert_eq!(run.documents[0].revision, d.revision);
        assert_eq!(run.source_ids, vec![d.source_id.clone()]);
        store
            .import(ImportRequest {
                source_name: "Synthetic contribution source".into(),
                kind: "transcript".into(),
                uri: "fixture://contribution".into(),
                title: "Changed after eval".into(),
                body: "New text not used by saved eval".into(),
                allow_external: false,
            })
            .await
            .unwrap();
        assert!(store.run_plugin_contribution(dir.path()).await.unwrap());
        let checked = finished(&store, &p, &run).await;
        assert_eq!(checked.status, "completed");
        let receipt = checked.evaluation_receipt.unwrap();
        assert_eq!(receipt.results.len(), 2);
        assert!(receipt.results[0].passed);
        assert!(!receipt.results[1].passed);
        assert_eq!(receipt.results[0].threshold, 0.75);
        assert_eq!(receipt.results[0].check.evidence[0].revision, d.revision);
        assert_eq!(receipt.plugin_id, p.id);
        assert_eq!(receipt.configuration_version, p.version);
        assert_eq!(receipt.evaluation_signature, crate::store::hash(&before));
        assert_eq!(receipt.plugin_run_id, run.id);
        assert_eq!(raw_eval(&store, &e.id).await, before);
        let loaded = store.eval_run(&e.id, &Access::admin()).await.unwrap();
        assert_eq!(loaded.plugin_checks.len(), 1);
        assert!(loaded.results.iter().all(|r| r.passed));
        assert_eq!(loaded.suite.project_id.is_none(), personal);
        assert_eq!(
            store
                .request_plugin_contribution(&p.id, req)
                .await
                .unwrap()
                .id,
            run.id
        );
        assert!(!store.run_plugin_contribution(dir.path()).await.unwrap());
        let event=sqlx::query("SELECT origin_plugin,causation_id,correlation_id,depth FROM plugin_events WHERE kind='eval.run.completed' AND record_kind='plugin_eval_check'").fetch_one(&store.pool).await.unwrap();
        assert_eq!(event.get::<String, _>("origin_plugin"), p.id);
        assert_eq!(event.get::<String, _>("causation_id"), run.event_id);
        assert_eq!(event.get::<String, _>("correlation_id"), run.id);
        assert_eq!(event.get::<i64, _>("depth"), 1);
    }
}
#[tokio::test]
async fn plugin_eval_malformed_or_incomplete_checks_leave_no_partial_receipt() {
    let (dir, store, d, p, e) = eval_fixture(false).await;
    let before = raw_eval(&store, &e.id).await;
    for mode in [
        "bad-score",
        "unknown-case",
        "duplicate",
        "missing",
        "bad-citation",
        "forged-pass",
        "empty",
    ] {
        let run = store
            .request_plugin_contribution(&p.id, eval_request(&p, &d, &e, mode, mode))
            .await
            .unwrap();
        store.run_plugin_contribution(dir.path()).await.unwrap();
        let checked = finished(&store, &p, &run).await;
        assert_eq!(checked.status, "failed", "{mode}");
        assert!(checked.evaluation_receipt.is_none());
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM plugin_eval_checks")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(raw_eval(&store, &e.id).await, before);
}
#[tokio::test]
async fn plugin_eval_selection_is_explicit_and_requires_the_complete_recorded_scope() {
    let (dir, store, d, p, e) = eval_fixture(false).await;
    let mut req = eval_request(&p, &d, &e, "mixed-input", "ok");
    req.source_ids.push(d.source_id.clone());
    assert!(store.request_plugin_contribution(&p.id, req).await.is_err());
    let mut req = eval_request(&p, &d, &e, "unknown-case", "ok");
    req.evaluation.as_mut().unwrap().case_ids = vec!["unknown".into()];
    assert!(store.request_plugin_contribution(&p.id, req).await.is_err());
    let mut req = eval_request(&p, &d, &e, "one-case", "ok");
    req.evaluation.as_mut().unwrap().case_ids = vec!["second".into()];
    let run = store.request_plugin_contribution(&p.id, req).await.unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let checked = finished(&store, &p, &run).await;
    assert_eq!(checked.evaluation_receipt.unwrap().results.len(), 1);
    let visible = store.plugin_evaluation_inputs(&p.id).await.unwrap();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].cases.len(), 2);
    let mut grant = p.grant.clone();
    grant.source_ids.clear();
    store
        .update_plugin(
            &p.id,
            PluginUpdateRequest {
                expected_version: p.version,
                enabled: true,
                grant,
                configuration: p.configuration.clone(),
                manifest: None,
                executable: None,
                idempotency_key: "revoke-eval-scope".into(),
            },
        )
        .await
        .unwrap();
    assert!(
        store
            .plugin_evaluation_inputs(&p.id)
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn plugin_eval_revocation_cancels_active_execution_and_commits_no_checks() {
    let (dir, store, d, p, e) = eval_fixture(false).await;
    let run = store
        .request_plugin_contribution(&p.id, eval_request(&p, &d, &e, "slow-check", "slow"))
        .await
        .unwrap();
    let work = dir.path().to_path_buf();
    let worker_store = store.clone();
    let worker = tokio::spawn(async move { worker_store.run_plugin_contribution(&work).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !dir.path().join("eval-started").exists() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    store
        .update_source_policy(
            &d.source_id,
            SourcePolicyUpdate {
                expected_version: 0,
                external_enabled: false,
                denied_connections: vec![format!("plugin:{}", p.id)],
                denied_grants: vec![],
                idempotency_key: "block-eval-destination".into(),
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    );
    assert_eq!(finished(&store, &p, &run).await.status, "cancelled");
    assert!(
        store
            .eval_run(&e.id, &Access::admin())
            .await
            .unwrap()
            .plugin_checks
            .is_empty()
    );
}
#[tokio::test]
async fn plugin_eval_rejects_saved_record_changes_before_invoking_the_executable() {
    let (dir, store, d, p, e) = eval_fixture(false).await;
    let run = store
        .request_plugin_contribution(&p.id, eval_request(&p, &d, &e, "record-pinned", "ok"))
        .await
        .unwrap();
    sqlx::query("UPDATE eval_runs SET record=json_set(record,'$.results[0].output.text','Changed outside the immutable run path') WHERE id=?").bind(&e.id).execute(&store.pool).await.unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    assert_eq!(finished(&store, &p, &run).await.status, "cancelled");
    assert!(!dir.path().join("eval-started").exists());
    assert!(
        store
            .eval_run(&e.id, &Access::admin())
            .await
            .unwrap()
            .plugin_checks
            .is_empty()
    );
}
#[tokio::test]
async fn plugin_eval_rejects_oversized_selected_inputs_without_queuing_work() {
    let (_dir, store, _d, p, old) = eval_fixture(false).await;
    let d = store
        .import(ImportRequest {
            source_name: "Synthetic contribution source".into(),
            kind: "transcript".into(),
            uri: "fixture://contribution".into(),
            title: "Large saved scope".into(),
            body: "x".repeat(200000),
            allow_external: false,
        })
        .await
        .unwrap()
        .document;
    let ids = ["large-a", "large-b", "large-c", "large-d"];
    let suite = store
        .save_eval_suite(
            None,
            EvalSuiteRequest {
                expected_version: 0,
                name: "Large saved outputs".into(),
                project_id: Some(old.project_id.clone()),
                cases: ids.iter().map(|id| case(id, 1.0)).collect(),
                idempotency_key: "large-suite".into(),
            },
        )
        .await
        .unwrap();
    let outputs = ids
        .iter()
        .map(|id| {
            let mut out = output(id, vec![]);
            out.text = format!("Synthetic{}", "x".repeat(63991));
            out
        })
        .collect();
    let e = store
        .execute_eval(
            &suite.id,
            EvalRunRequest {
                max_judge_requests: 0,
                max_judge_input_tokens: 0,
                suite_version: suite.version,
                project_id: old.project_id,
                baseline_id: None,
                outputs,
                max_cases: 4,
                max_output_bytes: 512000,
                max_cost_microunits: 0,
                idempotency_key: "large-eval-run".into(),
            },
        )
        .await
        .unwrap();
    let error = store
        .request_plugin_contribution(&p.id, eval_request(&p, &d, &e, "oversized-check", "ok"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("wire budget"));
    assert!(
        store
            .plugin_contribution_runs(&p.id)
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn plugin_eval_never_supplies_known_out_of_scope_citation_text_to_the_executable() {
    let (dir, store, d, p, e) = eval_fixture(false).await;
    let private = store
        .import(ImportRequest {
            source_name: "Unrelated private source".into(),
            kind: "markdown".into(),
            uri: "fixture://private-eval-source".into(),
            title: "Private".into(),
            body: "Private citation must not reach this plugin".into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document;
    let citation = Evidence {
        document_id: private.id.clone(),
        revision: private.revision,
        line: 1,
        quote: private.body.clone(),
    };
    let bad = store
        .execute_eval(
            &e.suite.id,
            EvalRunRequest {
                max_judge_requests: 0,
                max_judge_input_tokens: 0,
                suite_version: e.suite.version,
                project_id: e.project_id,
                baseline_id: None,
                outputs: vec![output("first", vec![citation]), output("second", vec![])],
                max_cases: 2,
                max_output_bytes: 10000,
                max_cost_microunits: 0,
                idempotency_key: "invalid-citation-base".into(),
            },
        )
        .await
        .unwrap();
    assert!(bad.results[0].invalid_evidence > 0);
    let error = store
        .request_plugin_contribution(&p.id, eval_request(&p, &d, &bad, "private-reference", "ok"))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("outside its recorded scope"));
    assert!(!dir.path().join("eval-started").exists());
    assert!(
        store
            .plugin_contribution_runs(&p.id)
            .await
            .unwrap()
            .is_empty()
    );
}
