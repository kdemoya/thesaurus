use super::tests::{finished, fixture, request};
use super::*;
use crate::config::Connection;

async fn model_fixture() -> (
    tempfile::TempDir,
    Store,
    Document,
    PluginRecord,
    PluginContributionRun,
) {
    let (dir, store, d, _) = fixture().await;
    let script = dir.path().join("model.py");
    std::fs::write(&script,r#"import json,sys,os
v=json.loads(sys.stdin.readline());models=v['input'].get('models',[])
assert 'synthetic-provider-secret' not in json.dumps(v)
assert not any('TOKEN' in k or 'SECRET' in k for k in os.environ)
r={'api_version':v['api_version'],'invocation_id':v['invocation_id'],'result':None}
if not models or v['input'].get('parameters',{}).get('mode')=='loop':
    r['model_requests']=[{'key':('round-'+str(len(models)) if v['input'].get('parameters',{}).get('mode')=='loop' else 'review'),'prompt':'Suggest one action from the pinned work context','max_output_tokens':100,'max_output_bytes':2000}]
    if v['method']=='contribution': r['contribution']={'text':'Awaiting explicit model approval','evidence':[]}
elif v['method']=='contribution': r['contribution']={'text':'Model returned: '+models[0]['output'],'evidence':[]}
else:
    d=v['input']['document'];r['observations']=[{'body':'Model suggestion: '+models[0]['output'],'source_ids':[d['source_id']],'evidence':[{'document_id':d['id'],'revision':d['revision'],'line':1,'quote':d['body'].splitlines()[0]}]}]
print(json.dumps(r))
"#).unwrap();
    let manifest:PluginManifest=serde_json::from_value(json!({"id":"example.model-check","name":"Synthetic model review","version":"1.0.0","api_min":2,"api_max":2,"host":"service","hooks":["document.revision.created"],"requests":["commands.invoke","sources.read","memory.propose","models.request"],"contributions":[{"id":"review","kind":"command","title":"Model review","description":"Synthetic"}]})).unwrap();
    let p = store
        .install_plugin(PluginInstallRequest {
            manifest,
            executable: PluginExecutable {
                program: "/usr/bin/python3".into(),
                args: vec![script.to_string_lossy().into()],
            },
            grant: PluginPermissions {
                source_ids: vec![d.source_id.clone()],
                operations: vec![
                    "commands.invoke".into(),
                    "sources.read".into(),
                    "memory.propose".into(),
                    "models.request".into(),
                ],
                connection_ids: vec!["api-model".into(), "agent".into()],
                ..Default::default()
            },
            configuration: json!({}),
            idempotency_key: "install-model".into(),
        })
        .await
        .unwrap();
    let p = store
        .update_plugin(
            &p.id,
            PluginUpdateRequest {
                expected_version: p.version,
                enabled: true,
                grant: p.grant.clone(),
                configuration: p.configuration.clone(),
                manifest: None,
                executable: None,
                idempotency_key: "enable-model".into(),
            },
        )
        .await
        .unwrap();
    let run = store
        .request_plugin_contribution(&p.id, request(&p, &d, "review", "model-parent", "ok"))
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    (dir, store, d, p, run)
}
fn connection() -> Connection {
    Connection {
        id: "api-model".into(),
        kind: "openai_compatible".into(),
        model: "synthetic".into(),
        endpoint: Some("http://127.0.0.1:12345/v1".into()),
        command: None,
        args: vec![],
        token_env: Some("SYNTHETIC_TOKEN_REFERENCE".into()),
        credential_ref: None,
        daily_limit: 10,
    }
}
fn approve(p: &PluginModelPreview, key: &str) -> PluginModelApproveRequest {
    PluginModelApproveRequest {
        connection: p.connection.clone(),
        signature: p.signature.clone(),
        approve_usage: true,
        approve_result_delivery: true,
        max_approximate_input_tokens: p.approximate_input_tokens,
        idempotency_key: key.into(),
    }
}
async fn preview(store: &Store, p: &PluginRecord, r: &PluginContributionRun) -> PluginModelPreview {
    let r = finished(store, p, r).await;
    assert_eq!(r.status, "awaiting_model");
    store
        .plugin_model_preview(&p.id, &r.model_proposal_ids[0], &connection())
        .await
        .unwrap()
}
async fn runtime_done(store: &Store, id: &str, text: &str) {
    store
        .runtime_patch(
            id,
            json!({"status":"completed","output":text,"stop_reason":"end_turn"}),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn plugin_models_wait_for_approval_reserve_once_and_return_only_a_bounded_result() {
    let (dir, store, d, p, r) = model_fixture().await;
    let preview = self::preview(&store, &p, &r).await;
    assert!(preview.prompt.contains(&d.body));
    assert!(!preview.external);
    assert_eq!(preview.source_ids, vec![d.source_id]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runtime_runs")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
    let mut req = approve(&preview, "approve-model");
    req.approve_usage = false;
    assert!(
        store
            .create_plugin_model_runtime(&preview.proposal.id, req, &connection())
            .await
            .is_err()
    );
    let mut req = approve(&preview, "approve-model");
    req.approve_result_delivery = false;
    assert!(
        store
            .create_plugin_model_runtime(&preview.proposal.id, req, &connection())
            .await
            .is_err()
    );
    let req = approve(&preview, "approve-model");
    let (runtime, secret, _) = store
        .create_plugin_model_runtime(&preview.proposal.id, req.clone(), &connection())
        .await
        .unwrap();
    assert!(secret.is_some());
    let (again, secret, _) = store
        .create_plugin_model_runtime(&preview.proposal.id, req, &connection())
        .await
        .unwrap();
    assert_eq!(again.id, runtime.id);
    assert!(secret.is_none());
    runtime_done(&store, &runtime.id, "Synthetic model suggestion").await;
    store.reconcile_plugin_models().await.unwrap();
    assert!(store.run_plugin_contribution(dir.path()).await.unwrap());
    let finished = finished(&store, &p, &r).await;
    assert_eq!(finished.status, "completed");
    assert!(
        finished
            .output
            .unwrap()
            .text
            .contains("Synthetic model suggestion")
    );
    assert_eq!(finished.model_results.len(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runtime_runs")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn plugin_models_check_connection_grants_source_destinations_and_preview_signatures() {
    let (_dir, store, _d, p, r) = model_fixture().await;
    let preview = self::preview(&store, &p, &r).await;
    let mut c = connection();
    c.id = "ungranted".into();
    assert!(
        store
            .plugin_model_preview(&p.id, &preview.proposal.id, &c)
            .await
            .is_err()
    );
    let mut c = connection();
    c.kind = "acp".into();
    c.id = "agent".into();
    assert!(
        store
            .plugin_model_preview(&p.id, &preview.proposal.id, &c)
            .await
            .is_err(),
        "ACP must not receive local-only sources"
    );
    let mut c = connection();
    c.endpoint = Some("http://127.0.0.1:23456/v1".into());
    assert!(
        store
            .create_plugin_model_runtime(
                &preview.proposal.id,
                approve(&preview, "stale-preview"),
                &c
            )
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runtime_runs")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn plugin_models_rejection_and_daily_quota_do_not_start_requests_or_fallback() {
    let (_dir, store, _d, p, r) = model_fixture().await;
    let preview = self::preview(&store, &p, &r).await;
    let mut c = connection();
    c.daily_limit = 0;
    let exact = store
        .plugin_model_preview(&p.id, &preview.proposal.id, &c)
        .await
        .unwrap();
    assert!(
        store
            .create_plugin_model_runtime(&exact.proposal.id, approve(&exact, "quota"), &c)
            .await
            .is_err()
    );
    store
        .reject_plugin_model(&p.id, &preview.proposal.id)
        .await
        .unwrap();
    assert_eq!(finished(&store, &p, &r).await.status, "cancelled");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runtime_runs")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn plugin_models_withhold_outputs_on_revocation_and_restart_without_retrying() {
    let (_dir, store, d, p, r) = model_fixture().await;
    let preview = self::preview(&store, &p, &r).await;
    let (runtime, _, _) = store
        .create_plugin_model_runtime(
            &preview.proposal.id,
            approve(&preview, "model-running"),
            &connection(),
        )
        .await
        .unwrap();
    store
        .update_source_policy(
            &d.source_id,
            SourcePolicyUpdate {
                expected_version: 0,
                external_enabled: false,
                denied_connections: vec!["api-model".into()],
                denied_grants: vec![],
                idempotency_key: "revoke-model-destination".into(),
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    assert!(
        store
            .plugin_model_runtime_preview(&runtime.id, &connection())
            .await
            .is_err()
    );
    runtime_done(&store, &runtime.id, "Must not be delivered").await;
    store.reconcile_plugin_models().await.unwrap();
    let r = finished(&store, &p, &r).await;
    assert_eq!(r.status, "failed");
    assert!(r.model_results.is_empty());
    store.recover_runtime().await.unwrap();
    store.reconcile_plugin_models().await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runtime_runs")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn lifecycle_plugins_can_request_approved_models_without_blocking_the_outbox() {
    let (dir, store, d, p, manual) = model_fixture().await;
    let pending = finished(&store, &p, &manual).await;
    store
        .reject_plugin_model(&p.id, &pending.model_proposal_ids[0])
        .await
        .unwrap();
    store
        .import(ImportRequest {
            source_name: "Synthetic contribution source".into(),
            kind: "transcript".into(),
            uri: "fixture://contribution".into(),
            title: "New synthetic transcript".into(),
            body: "TODO: new hook model context".into(),
            allow_external: false,
        })
        .await
        .unwrap();
    store.queue_plugin_deliveries().await.unwrap();
    assert!(store.run_plugin_delivery(dir.path()).await.unwrap());
    let delivery = store.plugin_deliveries(&p.id).await.unwrap().remove(0);
    assert_eq!(delivery.status, "completed");
    let hook = store
        .plugin_contribution_run(&p.id, &delivery.id)
        .await
        .unwrap();
    assert_eq!(hook.invocation_method, "event");
    let preview = store
        .plugin_model_preview(&p.id, &hook.model_proposal_ids[0], &connection())
        .await
        .unwrap();
    let (runtime, _, _) = store
        .create_plugin_model_runtime(
            &preview.proposal.id,
            approve(&preview, "approve-hook-model"),
            &connection(),
        )
        .await
        .unwrap();
    runtime_done(&store, &runtime.id, "Synthetic hook answer").await;
    store.reconcile_plugin_models().await.unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    assert_eq!(finished(&store, &p, &hook).await.status, "completed");
    let observations = store.observations(&Access::admin()).await.unwrap();
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].source_ids, vec![d.source_id]);
    assert!(observations[0].body.contains("Synthetic hook answer"));
    let event=sqlx::query("SELECT origin_plugin,causation_id FROM plugin_events WHERE kind='agent.run.completed' AND record_id=?").bind(&runtime.id).fetch_one(&store.pool).await.unwrap();
    assert_eq!(event.get::<String, _>("origin_plugin"), p.id);
    assert_eq!(event.get::<String, _>("causation_id"), hook.event_id);
}
#[tokio::test]
async fn interrupted_plugin_models_are_not_relaunched_and_cancel_releases_memory_authority() {
    let (_dir, store, _d, p, r) = model_fixture().await;
    let preview = self::preview(&store, &p, &r).await;
    let req = approve(&preview, "interrupted-request");
    let (runtime, _, grant) = store
        .create_plugin_model_runtime(&preview.proposal.id, req.clone(), &connection())
        .await
        .unwrap();
    store.recover_runtime().await.unwrap();
    store.reconcile_plugin_models().await.unwrap();
    assert_eq!(finished(&store, &p, &r).await.status, "failed");
    let (again, secret, _) = store
        .create_plugin_model_runtime(&preview.proposal.id, req, &connection())
        .await
        .unwrap();
    assert_eq!(again.id, runtime.id);
    assert_eq!(again.status, "interrupted");
    assert!(secret.is_none());
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT revoked FROM grants WHERE id=?")
            .bind(grant)
            .fetch_one(&store.pool)
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runtime_runs")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        1
    );
}
#[tokio::test]
async fn plugin_model_oversized_results_and_cancelled_parents_never_resume_execution() {
    let (_dir, store, _d, p, r) = model_fixture().await;
    let preview = self::preview(&store, &p, &r).await;
    let (runtime, _, _) = store
        .create_plugin_model_runtime(
            &preview.proposal.id,
            approve(&preview, "oversized-output"),
            &connection(),
        )
        .await
        .unwrap();
    runtime_done(&store, &runtime.id, &"x".repeat(2001)).await;
    store.reconcile_plugin_models().await.unwrap();
    let finished = finished(&store, &p, &r).await;
    assert_eq!(finished.status, "failed");
    assert!(finished.model_results.is_empty());
    let (_dir, store, _d, p, r) = model_fixture().await;
    let preview = self::preview(&store, &p, &r).await;
    let (runtime, _, grant) = store
        .create_plugin_model_runtime(
            &preview.proposal.id,
            approve(&preview, "cancel-output"),
            &connection(),
        )
        .await
        .unwrap();
    store
        .cancel_plugin_contribution(&p.id, &r.id)
        .await
        .unwrap();
    assert_eq!(
        store.runtime(&runtime.id).await.unwrap().status,
        "cancelling"
    );
    assert!(
        sqlx::query_scalar::<_, bool>("SELECT revoked FROM grants WHERE id=?")
            .bind(grant)
            .fetch_one(&store.pool)
            .await
            .unwrap()
    );
    assert!(
        store
            .plugin_model_runtime_preview(&runtime.id, &connection())
            .await
            .is_err()
    );
}
#[tokio::test]
async fn repeated_plugin_model_rounds_remain_separately_approved_and_stop_after_three() {
    let (dir, store, d, p, first) = model_fixture().await;
    let first = finished(&store, &p, &first).await;
    store
        .reject_plugin_model(&p.id, &first.model_proposal_ids[0])
        .await
        .unwrap();
    let r = store
        .request_plugin_contribution(&p.id, request(&p, &d, "review", "loop-parent", "loop"))
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    for round in 0..3 {
        let current = finished(&store, &p, &r).await;
        assert_eq!(current.status, "awaiting_model");
        assert_eq!(current.model_results.len(), round);
        let preview = store
            .plugin_model_preview(
                &p.id,
                current.model_proposal_ids.last().unwrap(),
                &connection(),
            )
            .await
            .unwrap();
        let (runtime, _, _) = store
            .create_plugin_model_runtime(
                &preview.proposal.id,
                approve(&preview, &format!("round-{round}")),
                &connection(),
            )
            .await
            .unwrap();
        runtime_done(&store, &runtime.id, "Synthetic bounded answer").await;
        store.reconcile_plugin_models().await.unwrap();
        store.run_plugin_contribution(dir.path()).await.unwrap();
    }
    assert_eq!(finished(&store, &p, &r).await.status, "failed");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM runtime_runs")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        3
    );
}
#[tokio::test]
async fn ordinary_acp_runtime_grants_preserve_the_existing_memory_identity() {
    let (_dir, store, d, _p, _r) = model_fixture().await;
    sqlx::query("UPDATE sources SET allow_external=1 WHERE id=?")
        .bind(&d.source_id)
        .execute(&store.pool)
        .await
        .unwrap();
    let (runtime, secret, grant) = store
        .create_runtime(
            RuntimeRequest {
                connection: "ordinary-agent".into(),
                prompt: "Synthetic ordinary runtime".into(),
                source_ids: vec![d.source_id],
                approve_usage: true,
                idempotency_key: "ordinary-acp-identity".into(),
            },
            10,
        )
        .await
        .unwrap();
    assert!(secret.is_some());
    let row = sqlx::query("SELECT name,allow_write,allow_external FROM grants WHERE id=?")
        .bind(grant)
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        row.get::<String, _>("name"),
        format!("acp-run:{}", runtime.id)
    );
    assert!(!row.get::<bool, _>("allow_write"));
    assert!(row.get::<bool, _>("allow_external"));
}
