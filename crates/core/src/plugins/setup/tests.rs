use super::super::tests::{fixture, update};
use super::*;

async fn configured(script: &str) -> (tempfile::TempDir, Store, PluginRecord) {
    let (dir, store, _, old) = fixture().await;
    let path = dir.path().join("setup.py");
    std::fs::write(&path, script).unwrap();
    let mut manifest = old.manifest.clone();
    manifest.api_min = 2;
    manifest.api_max = 2;
    manifest.requests.push("setup.check".into());
    manifest.requests.push("commands.invoke".into());
    manifest.configuration = vec![PluginConfigField {
        key: "path".into(),
        label: "Path".into(),
        kind: PluginFieldKind::String,
        required: false,
        choices: vec![],
        default: None,
        description: String::new(),
    }];
    manifest.setup = vec![PluginSetupStep {
        id: "file".into(),
        title: "Check file".into(),
        description: "Read-only check".into(),
        applies_to: vec!["review".into()],
        prerequisites: vec![PluginPrerequisite::Configuration { key: "path".into() }],
        check: true,
        optional: false,
        repair: Some(PluginSetupRepair::Configuration { key: "path".into() }),
    }];
    manifest.contributions = vec![
        PluginContribution {
            id: "review".into(),
            kind: PluginContributionKind::Command,
            title: "Review".into(),
            description: String::new(),
        },
        PluginContribution {
            id: "unrelated".into(),
            kind: PluginContributionKind::Command,
            title: "Unrelated".into(),
            description: String::new(),
        },
    ];
    let mut req = update(&old, false, "upgrade-setup");
    req.manifest = Some(manifest);
    req.executable = Some(PluginExecutable {
        program: "/usr/bin/python3".into(),
        args: vec![path.to_string_lossy().into()],
    });
    let upgraded = store.update_plugin(&old.id, req).await.unwrap();
    let mut req = update(&upgraded, true, "enable-setup");
    req.configuration = json!({"path":"synthetic"});
    req.grant
        .operations
        .extend(["setup.check".into(), "commands.invoke".into()]);
    let p = store.update_plugin(&old.id, req).await.unwrap();
    (dir, store, p)
}
const CHECK: &str = r#"import json,sys,os
v=json.loads(sys.stdin.readline())
assert v['method']=='readiness'
assert set(v['input'])=={'step'} and v['event'] is None
assert not any('TOKEN' in k or 'SECRET' in k for k in os.environ)
print(json.dumps({'api_version':2,'invocation_id':v['invocation_id'],'readiness':{'step_id':'file','state':'ready','detail':'Synthetic configuration checked; no data changed'}}))
"#;
fn request(p: &PluginRecord, key: &str) -> PluginSetupCheckRequest {
    PluginSetupCheckRequest {
        expected_version: p.version,
        idempotency_key: key.into(),
    }
}
#[tokio::test]
async fn setup_checks_are_explicit_durable_idempotent_and_bound_to_configuration() {
    let (dir, store, p) = configured(CHECK).await;
    let status = store.plugin_setup(&p.id).await.unwrap();
    assert_eq!(status[0].state, PluginSetupState::Unverified);
    assert!(store.require_plugin_setup(&p.id, "review").await.is_err());
    store
        .require_plugin_setup(&p.id, "unrelated")
        .await
        .unwrap();
    let run = store
        .request_plugin_setup_check(&p.id, "file", request(&p, "check"))
        .await
        .unwrap();
    assert_eq!(
        store
            .request_plugin_setup_check(&p.id, "file", request(&p, "check"))
            .await
            .unwrap()
            .id,
        run.id
    );
    assert_eq!(
        store.plugin_setup(&p.id).await.unwrap()[0].state,
        PluginSetupState::Checking
    );
    store.run_plugin_contribution(dir.path()).await.unwrap();
    assert_eq!(
        store
            .plugin_contribution_run(&p.id, &run.id)
            .await
            .unwrap()
            .status,
        "completed"
    );
    assert_eq!(
        store.plugin_setup(&p.id).await.unwrap()[0].state,
        PluginSetupState::Ready
    );
    store.require_plugin_setup(&p.id, "review").await.unwrap();
    let p = store
        .update_plugin(&p.id, update(&p, true, "config-change"))
        .await
        .unwrap();
    assert_eq!(
        store.plugin_setup(&p.id).await.unwrap()[0].state,
        PluginSetupState::Unverified
    );
    let mut req = update(&p, true, "missing-config");
    req.configuration = json!({});
    let p = store.update_plugin(&p.id, req).await.unwrap();
    assert_eq!(
        store.plugin_setup(&p.id).await.unwrap()[0].state,
        PluginSetupState::NeedsSetup
    );
    assert!(
        store
            .request_plugin_setup_check(&p.id, "file", request(&p, "missing-check"))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn setup_rejects_effects_mismatched_results_and_model_requests() {
    for result in [
        json!({"step_id":"other","state":"ready","detail":"Wrong step"}),
        json!({"step_id":"file","state":"checking","detail":"Unfinished"}),
        json!({"step_id":"file","state":"ready","detail":"x".repeat(2001)}),
    ] {
        let script = format!(
            "import json,sys\nv=json.loads(sys.stdin.readline())\nprint(json.dumps({{'api_version':2,'invocation_id':v['invocation_id'],'readiness':{}}}))\n",
            serde_json::to_string(&result).unwrap()
        );
        let (dir, store, p) = configured(&script).await;
        let run = store
            .request_plugin_setup_check(&p.id, "file", request(&p, "bad"))
            .await
            .unwrap();
        store.run_plugin_contribution(dir.path()).await.unwrap();
        assert_eq!(
            store
                .plugin_contribution_run(&p.id, &run.id)
                .await
                .unwrap()
                .status,
            "failed"
        );
        assert_ne!(
            store.plugin_setup(&p.id).await.unwrap()[0].state,
            PluginSetupState::Ready
        );
    }
    for (effect, payload) in [
        (
            "model_requests",
            json!([{"key":"request","prompt":"Should never run","max_output_tokens":32,"max_output_bytes":128}]),
        ),
        (
            "snapshots",
            json!([{"key":"snapshot","source_name":"Synthetic","kind":"markdown","title":"Synthetic","body":"Should never import"}]),
        ),
        (
            "actions",
            json!([{"provider":"task_status","target_id":"unused","expected_version":1,"status":"accepted","rationale":"Should never execute","evidence":[]}]),
        ),
        (
            "observations",
            json!([{"body":"Should never publish","source_ids":[],"evidence":[]}]),
        ),
        (
            "checks",
            json!([{"case_id":"unused","check_id":"unused","title":"Check","score":1.0,"detail":"Should never attach","evidence":[]}]),
        ),
    ] {
        let script = format!(
            "import json,sys\nv=json.loads(sys.stdin.readline())\nprint(json.dumps({{'api_version':2,'invocation_id':v['invocation_id'],'readiness':{{'step_id':'file','state':'ready','detail':'Checked'}},'{}':{}}}))\n",
            effect,
            serde_json::to_string(&payload).unwrap()
        );
        let (dir, store, p) = configured(&script).await;
        let run = store
            .request_plugin_setup_check(&p.id, "file", request(&p, "effects"))
            .await
            .unwrap();
        store.run_plugin_contribution(dir.path()).await.unwrap();
        assert_eq!(
            store
                .plugin_contribution_run(&p.id, &run.id)
                .await
                .unwrap()
                .status,
            "failed"
        );
        let models: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_model_proposals")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        assert_eq!(models, 0);
    }
}
#[tokio::test]
async fn setup_checks_expire_and_do_not_survive_service_restart_or_cancellation() {
    let (dir, store, p) = configured(CHECK).await;
    let run = store
        .request_plugin_setup_check(&p.id, "file", request(&p, "first"))
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    sqlx::query("UPDATE plugin_runs SET record=json_set(record,'$.finished_at','2000-01-01T00:00:00Z') WHERE id=?").bind(&run.id).execute(&store.pool).await.unwrap();
    assert_eq!(
        store.plugin_setup(&p.id).await.unwrap()[0].state,
        PluginSetupState::Unverified
    );
    let run = store
        .request_plugin_setup_check(&p.id, "file", request(&p, "second"))
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    assert_eq!(
        store.plugin_setup(&p.id).await.unwrap()[0].state,
        PluginSetupState::Ready
    );
    store.migrate_plugins().await.unwrap();
    assert_eq!(
        store.plugin_setup(&p.id).await.unwrap()[0].state,
        PluginSetupState::Unverified
    );
    let queued = store
        .request_plugin_setup_check(&p.id, "file", request(&p, "cancel"))
        .await
        .unwrap();
    store
        .cancel_plugin_contribution(&p.id, &queued.id)
        .await
        .unwrap();
    assert_eq!(
        store.plugin_setup(&p.id).await.unwrap()[0].state,
        PluginSetupState::Unverified
    );
    assert_eq!(
        store
            .plugin_contribution_run(&p.id, &run.id)
            .await
            .unwrap()
            .status,
        "completed"
    );
}
#[tokio::test]
async fn setup_grant_revocation_stops_running_check_and_queue_keeps_serial_order() {
    let script = CHECK.replace(
        "v=json.loads(sys.stdin.readline())",
        "v=json.loads(sys.stdin.readline())\nimport time\ntime.sleep(3)",
    );
    let (dir, store, p) = configured(&script).await;
    let run = store
        .request_plugin_setup_check(&p.id, "file", request(&p, "slow"))
        .await
        .unwrap();
    let worker_store = store.clone();
    let work = dir.path().to_owned();
    let worker =
        tokio::spawn(async move { worker_store.run_plugin_contribution(&work).await.unwrap() });
    for _ in 0..100 {
        if store
            .plugin_contribution_run(&p.id, &run.id)
            .await
            .unwrap()
            .status
            == "running"
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(!store.run_plugin_contribution(dir.path()).await.unwrap());
    let mut req = update(&p, true, "revoke");
    req.grant.operations.retain(|r| r != "setup.check");
    let p = store.update_plugin(&p.id, req).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), worker)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        store
            .plugin_contribution_run(&p.id, &run.id)
            .await
            .unwrap()
            .status,
        "cancelled"
    );
    assert!(
        store
            .request_plugin_setup_check(&p.id, "file", request(&p, "denied"))
            .await
            .is_err()
    );
}
#[tokio::test]
async fn setup_manifest_validation_rejects_invalid_links_and_legacy_check_api() {
    let (_dir, _store, p) = configured(CHECK).await;
    let mut m = p.manifest.clone();
    m.setup[0].applies_to = vec!["missing".into()];
    assert!(validate_manifest(&m).is_err());
    m = p.manifest.clone();
    m.setup[0].prerequisites = vec![PluginPrerequisite::Configuration {
        key: "missing".into(),
    }];
    assert!(validate_manifest(&m).is_err());
    m = p.manifest.clone();
    m.setup[0].repair = Some(PluginSetupRepair::Configuration {
        key: "missing".into(),
    });
    assert!(validate_manifest(&m).is_err());
    m = p.manifest.clone();
    m.api_min = 1;
    m.api_max = 1;
    assert!(validate_manifest(&m).is_err());
    m = p.manifest.clone();
    m.requests.retain(|op| op != "setup.check");
    assert!(validate_manifest(&m).is_err());
    m = p.manifest.clone();
    m.setup.push(m.setup[0].clone());
    assert!(validate_manifest(&m).is_err());
    let encoded = serde_json::to_value(&p.manifest).unwrap();
    let mut legacy = encoded.clone();
    legacy.as_object_mut().unwrap().remove("setup");
    let parsed: PluginManifest = serde_json::from_value(legacy).unwrap();
    assert!(parsed.setup.is_empty());
}
