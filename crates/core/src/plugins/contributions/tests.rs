use super::*;
pub(super) async fn finished(
    store: &Store,
    p: &PluginRecord,
    run: &PluginContributionRun,
) -> PluginContributionRun {
    store.plugin_contribution_run(&p.id, &run.id).await.unwrap()
}
async fn action_fixture() -> (tempfile::TempDir, Store, Document, PluginRecord, Task) {
    let (dir, store, d, p) = fixture().await;
    store
        .process_next(&crate::Config::default(), dir.path())
        .await
        .unwrap();
    let row = sqlx::query("SELECT * FROM tasks WHERE document_id=? ORDER BY id LIMIT 1")
        .bind(&d.id)
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let task = crate::store::task_row(&row);
    (dir, store, d, p, task)
}
pub(super) async fn fixture() -> (tempfile::TempDir, Store, Document, PluginRecord) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("contributions.sqlite3"))
        .await
        .unwrap();
    let document = store
        .import(ImportRequest {
            source_name: "Synthetic contribution source".into(),
            kind: "transcript".into(),
            uri: "fixture://contribution".into(),
            title: "Synthetic document".into(),
            body: "TODO: pinned first revision".into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document;
    let script = dir.path().join("contribution.py");
    std::fs::write(&script,r#"import json,sys,time,pathlib
v=json.loads(sys.stdin.readline())
params=v['input']['parameters']
mode=params.get('mode','ok')
pathlib.Path(sys.argv[1]).write_text('started')
if mode=='slow': time.sleep(3)
kind=v['input']['contribution']['kind']
if kind=='connector':
    snapshot={'key':params.get('snapshot_key','snapshot-a'),'source_name':'Synthetic connector','kind':'markdown','title':'Synthetic connector snapshot','body':params.get('body','TODO imported snapshot')}
    snapshots=[snapshot]
    if mode=='invalid-second': snapshots.append(dict(snapshot,key='snapshot-b',body=''))
    print(json.dumps({'api_version':v['api_version'],'invocation_id':v['invocation_id'],'result':None,'contribution':{'text':'Synthetic connector completed','evidence':[]},'snapshots':snapshots}))
    sys.exit(0)
d=v['input']['documents'][0]
e={'document_id':d['id'],'revision':d['revision'],'line':1,'quote':d['body'].splitlines()[0]}
kind=v['input']['contribution']['kind']
o={'body':'Synthetic attributed observation','source_ids':[d['source_id']],'evidence':[e]}
observations=[o] if kind=='processor' else []
if mode=='atomic-invalid': observations.append(dict(o,body=''))
if mode=='wrong-revision': e['revision']+=1
out={'text':'<script>synthetic text, never markup</script>','evidence':[e]}
if kind=='panel': out['panel']={'title':'Synthetic panel','sections':[{'heading':'<b>heading</b>','text':'<img src=x onerror=alert(1)>','evidence':[e]}]}
if mode=='panel-mutation': observations=[o]
actions=[]
if kind=='action':
    t=v['input']['tasks'][0]
    action={'provider':'task_status','target_id':t['id'],'expected_version':t['version'],'status':'accepted','rationale':'Synthetic cited task proposal','evidence':[t['evidence']]}
    if mode=='unrelated-task': action['target_id']='ungranted-task'
    actions=[action]
    if mode=='invalid-second': actions.append(dict(action,status='unsupported'))
pathlib.Path(sys.argv[2]).write_text('finished')
print(json.dumps({'api_version':v['api_version'],'invocation_id':v['invocation_id'],'result':None,'observations':observations,'contribution':out,'actions':actions}))
"#).unwrap();
    let manifest:PluginManifest=serde_json::from_value(json!({"id":"example.contributions","name":"Synthetic contributions","version":"1.0.0","api_min":2,"api_max":2,"host":"service","requests":["commands.invoke","sources.read","memory.propose","connectors.sync","actions.propose"],"contributions":[{"id":"connector","kind":"connector","title":"Connector","description":"Synthetic"},{"id":"action","kind":"action","title":"Action","description":"Synthetic"},{"id":"process","kind":"processor","title":"Process","description":"Synthetic"},{"id":"command","kind":"command","title":"Command","description":"Synthetic"},{"id":"panel","kind":"panel","title":"Panel","description":"Synthetic"}]})).unwrap();
    let p = store
        .install_plugin(PluginInstallRequest {
            manifest,
            executable: PluginExecutable {
                program: "/usr/bin/python3".into(),
                args: vec![
                    script.to_string_lossy().into(),
                    dir.path().join("started").to_string_lossy().into(),
                    dir.path().join("finished").to_string_lossy().into(),
                ],
            },
            grant: PluginPermissions {
                source_ids: vec![document.source_id.clone()],
                operations: vec![
                    "commands.invoke".into(),
                    "sources.read".into(),
                    "memory.propose".into(),
                    "connectors.sync".into(),
                    "actions.propose".into(),
                ],
                ..Default::default()
            },
            configuration: json!({}),
            idempotency_key: "install".into(),
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
                idempotency_key: "enable".into(),
            },
        )
        .await
        .unwrap();
    (dir, store, document, p)
}
pub(super) fn request(
    p: &PluginRecord,
    d: &Document,
    contribution: &str,
    key: &str,
    mode: &str,
) -> PluginContributionRequest {
    PluginContributionRequest {
        expected_version: p.version,
        contribution_id: contribution.into(),
        source_ids: vec![d.source_id.clone()],
        project_ids: vec![],
        destination_project_id: None,
        evaluation: None,
        input: json!({"mode":mode}),
        idempotency_key: key.into(),
    }
}
async fn observation_count(store: &Store) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM observations")
        .fetch_one(&store.pool)
        .await
        .unwrap()
}
async fn wait_started(path: &Path) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !path.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn contribution_processor_pins_revisions_replays_request_and_attributes_atomic_effects() {
    let (dir, store, d, p) = fixture().await;
    let req = request(&p, &d, "process", "process-once", "ok");
    let run = store
        .request_plugin_contribution(&p.id, req.clone())
        .await
        .unwrap();
    assert_eq!(
        store
            .request_plugin_contribution(&p.id, req.clone())
            .await
            .unwrap()
            .id,
        run.id
    );
    let mut changed = req;
    changed.input = json!({"mode":"changed"});
    assert!(
        store
            .request_plugin_contribution(&p.id, changed)
            .await
            .is_err()
    );
    let latest = store
        .import(ImportRequest {
            source_name: "Synthetic contribution source".into(),
            kind: "transcript".into(),
            uri: "fixture://contribution".into(),
            title: "Synthetic document".into(),
            body: "TODO new revision, excluded from this run".into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document;
    assert_eq!(latest.revision, d.revision + 1);
    assert!(store.run_plugin_contribution(dir.path()).await.unwrap());
    let result = store.plugin_contribution_run(&p.id, &run.id).await.unwrap();
    assert_eq!(result.status, "completed");
    assert_eq!(result.output.unwrap().evidence[0].revision, d.revision);
    assert_eq!(observation_count(&store).await, 1);
    let row = sqlx::query("SELECT contributor,evidence FROM observations")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(
        row.get::<String, _>("contributor"),
        "plugin:example.contributions"
    );
    let evidence: Vec<Evidence> = serde_json::from_str(row.get("evidence")).unwrap();
    assert_eq!(evidence[0].quote, d.body);
    let event=sqlx::query("SELECT origin_plugin,causation_id,correlation_id,depth FROM plugin_events WHERE kind='memory.observation.created'").fetch_one(&store.pool).await.unwrap();
    assert_eq!(event.get::<String, _>("origin_plugin"), p.id);
    assert_eq!(event.get::<String, _>("causation_id"), run.event_id);
    assert_eq!(event.get::<String, _>("correlation_id"), run.id);
    assert_eq!(event.get::<i64, _>("depth"), 1);
    assert!(!store.run_plugin_contribution(dir.path()).await.unwrap());
    assert_eq!(observation_count(&store).await, 1);
}
#[tokio::test]
async fn contribution_invalid_effect_rolls_back_all_effects_and_result() {
    let (dir, store, d, p) = fixture().await;
    let run = store
        .request_plugin_contribution(
            &p.id,
            request(&p, &d, "process", "atomic", "atomic-invalid"),
        )
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let run = store.plugin_contribution_run(&p.id, &run.id).await.unwrap();
    assert_eq!(run.status, "failed");
    assert!(run.output.is_none());
    assert_eq!(observation_count(&store).await, 0);
    let run = store
        .request_plugin_contribution(
            &p.id,
            request(&p, &d, "command", "wrong-rev", "wrong-revision"),
        )
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
}
#[tokio::test]
async fn contribution_panels_are_declarative_read_only_and_commands_execute() {
    let (dir, store, d, p) = fixture().await;
    for kind in ["panel", "command"] {
        let run = store
            .request_plugin_contribution(&p.id, request(&p, &d, kind, kind, "ok"))
            .await
            .unwrap();
        store.run_plugin_contribution(dir.path()).await.unwrap();
        let run = store.plugin_contribution_run(&p.id, &run.id).await.unwrap();
        assert_eq!(run.status, "completed");
        let output = run.output.unwrap();
        assert!(output.text.starts_with("<script>"));
        if kind == "panel" {
            assert_eq!(
                output.panel.unwrap().sections[0].text,
                "<img src=x onerror=alert(1)>"
            );
        } else {
            assert!(output.panel.is_none());
        }
    }
    let run = store
        .request_plugin_contribution(
            &p.id,
            request(&p, &d, "panel", "bad-panel", "panel-mutation"),
        )
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
    assert_eq!(observation_count(&store).await, 0);
}
#[tokio::test]
async fn contribution_cancel_stops_execution_and_serializes_each_plugin() {
    let (dir, store, d, p) = fixture().await;
    let run = store
        .request_plugin_contribution(&p.id, request(&p, &d, "process", "slow", "slow"))
        .await
        .unwrap();
    let second = store
        .request_plugin_contribution(&p.id, request(&p, &d, "command", "second", "ok"))
        .await
        .unwrap();
    let worker_store = store.clone();
    let work = dir.path().to_path_buf();
    let worker = tokio::spawn(async move { worker_store.run_plugin_contribution(&work).await });
    wait_started(&dir.path().join("started")).await;
    assert!(!store.run_plugin_contribution(dir.path()).await.unwrap());
    assert!(!store.run_plugin_delivery(dir.path()).await.unwrap());
    store
        .cancel_plugin_contribution(&p.id, &run.id)
        .await
        .unwrap();
    assert!(worker.await.unwrap().unwrap());
    assert!(!dir.path().join("finished").exists());
    assert_eq!(observation_count(&store).await, 0);
    assert_eq!(
        store
            .plugin_contribution_run(&p.id, &run.id)
            .await
            .unwrap()
            .status,
        "cancelled"
    );
    assert!(store.run_plugin_contribution(dir.path()).await.unwrap());
    assert_eq!(
        store
            .plugin_contribution_run(&p.id, &second.id)
            .await
            .unwrap()
            .status,
        "completed"
    );
}
#[tokio::test]
async fn contribution_scope_and_configuration_are_rechecked_before_input() {
    let (dir, store, d, p) = fixture().await;
    let mut wrong = request(&p, &d, "command", "wrong-source", "ok");
    wrong.source_ids.push("ungranted".into());
    assert!(
        store
            .request_plugin_contribution(&p.id, wrong)
            .await
            .is_err()
    );
    let run = store
        .request_plugin_contribution(&p.id, request(&p, &d, "process", "revoked", "ok"))
        .await
        .unwrap();
    store
        .update_plugin(
            &p.id,
            PluginUpdateRequest {
                expected_version: p.version,
                enabled: true,
                grant: PluginPermissions {
                    source_ids: vec![],
                    ..p.grant.clone()
                },
                configuration: p.configuration.clone(),
                manifest: None,
                executable: None,
                idempotency_key: "revoke".into(),
            },
        )
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    assert_eq!(
        store
            .plugin_contribution_run(&p.id, &run.id)
            .await
            .unwrap()
            .status,
        "cancelled"
    );
    assert!(!dir.path().join("started").exists());
    assert_eq!(observation_count(&store).await, 0);
}
#[tokio::test]
async fn contribution_interrupted_runs_are_not_automatically_replayed() {
    let (dir, store, d, p) = fixture().await;
    let mut run = store
        .request_plugin_contribution(&p.id, request(&p, &d, "process", "interrupted", "ok"))
        .await
        .unwrap();
    run.status = "running".into();
    run.started_at = Some(now());
    let mut tx = store.pool.begin().await.unwrap();
    save_run(&mut tx, &run).await.unwrap();
    tx.commit().await.unwrap();
    store.migrate_plugins().await.unwrap();
    let run = store.plugin_contribution_run(&p.id, &run.id).await.unwrap();
    assert_eq!(run.status, "interrupted");
    assert!(run.finished_at.is_some());
    assert!(!store.run_plugin_contribution(dir.path()).await.unwrap());
    assert!(!dir.path().join("started").exists());
    assert_eq!(observation_count(&store).await, 0);
}
#[tokio::test]
async fn contribution_source_policy_revocation_stops_an_active_process() {
    let (dir, store, d, p) = fixture().await;
    sqlx::query("UPDATE sources SET allow_external=1 WHERE id=?")
        .bind(&d.source_id)
        .execute(&store.pool)
        .await
        .unwrap();
    let p = store
        .update_plugin(
            &p.id,
            PluginUpdateRequest {
                expected_version: p.version,
                enabled: true,
                grant: PluginPermissions {
                    external: true,
                    ..p.grant.clone()
                },
                configuration: p.configuration.clone(),
                manifest: None,
                executable: None,
                idempotency_key: "external".into(),
            },
        )
        .await
        .unwrap();
    let run = store
        .request_plugin_contribution(&p.id, request(&p, &d, "process", "policy-slow", "slow"))
        .await
        .unwrap();
    let worker_store = store.clone();
    let work = dir.path().to_path_buf();
    let worker = tokio::spawn(async move { worker_store.run_plugin_contribution(&work).await });
    wait_started(&dir.path().join("started")).await;
    sqlx::query("UPDATE sources SET allow_external=0 WHERE id=?")
        .bind(&d.source_id)
        .execute(&store.pool)
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(2), worker)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    );
    assert_eq!(
        store
            .plugin_contribution_run(&p.id, &run.id)
            .await
            .unwrap()
            .status,
        "cancelled"
    );
    assert!(!dir.path().join("finished").exists());
    assert_eq!(observation_count(&store).await, 0);
}

#[tokio::test]
async fn connector_import_owns_identity_preserves_policy_and_publishes_project_scope_atomically() {
    let (dir, store, d, p) = fixture().await;
    let project = store
        .create_brief(BriefCreateRequest {
            name: "Connector destination".into(),
            source_ids: vec![d.source_id.clone()],
            idempotency_key: "connector-project".into(),
        })
        .await
        .unwrap();
    let p = store
        .update_plugin(
            &p.id,
            PluginUpdateRequest {
                expected_version: p.version,
                enabled: true,
                grant: PluginPermissions {
                    project_ids: vec![project.id.clone()],
                    ..p.grant.clone()
                },
                configuration: p.configuration.clone(),
                manifest: None,
                executable: None,
                idempotency_key: "grant-destination".into(),
            },
        )
        .await
        .unwrap();
    let mut req = request(&p, &d, "connector", "sync-one", "ok");
    req.source_ids.clear();
    req.destination_project_id = Some(project.id.clone());
    let first = store
        .request_plugin_contribution(&p.id, req.clone())
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let first = finished(&store, &p, &first).await;
    assert_eq!(first.status, "completed");
    assert_eq!(first.imported_documents.len(), 1);
    let imported = &first.imported_documents[0];
    assert_eq!(imported.revision, 1);
    assert!(imported.changed);
    let source = store
        .sources(&Access::admin())
        .await
        .unwrap()
        .into_iter()
        .find(|s| s.id == imported.source_id)
        .unwrap();
    assert!(!source.allow_external);
    assert!(source.uri.starts_with("plugin://example.contributions/"));
    assert!(
        store
            .plugin_access(&p)
            .await
            .unwrap()
            .source_ids
            .unwrap()
            .contains(&imported.source_id)
    );
    let row=sqlx::query("SELECT origin_plugin,causation_id,correlation_id,project_ids,depth FROM plugin_events WHERE kind='document.revision.created' AND record_id=?").bind(&imported.document_id).fetch_one(&store.pool).await.unwrap();
    assert_eq!(row.get::<String, _>("origin_plugin"), p.id);
    assert_eq!(row.get::<String, _>("causation_id"), first.event_id);
    assert_eq!(row.get::<String, _>("correlation_id"), first.id);
    assert_eq!(row.get::<i64, _>("depth"), 1);
    assert!(row.get::<String, _>("project_ids").contains(&project.id));
    req.idempotency_key = "sync-same-content".into();
    let second = store
        .request_plugin_contribution(&p.id, req.clone())
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let second = finished(&store, &p, &second).await;
    assert_eq!(
        second.imported_documents[0].document_id,
        imported.document_id
    );
    assert_eq!(second.imported_documents[0].revision, 1);
    assert!(!second.imported_documents[0].changed);
    sqlx::query("UPDATE sources SET allow_external=1 WHERE id=?")
        .bind(&imported.source_id)
        .execute(&store.pool)
        .await
        .unwrap();
    req.idempotency_key = "sync-new-content".into();
    req.input = json!({"body":"TODO changed imported snapshot"});
    let third = store.request_plugin_contribution(&p.id, req).await.unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let third = finished(&store, &p, &third).await;
    assert_eq!(third.imported_documents[0].revision, 2);
    assert!(
        store
            .sources(&Access::admin())
            .await
            .unwrap()
            .into_iter()
            .find(|s| s.id == imported.source_id)
            .unwrap()
            .allow_external
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM plugin_effect_context")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn connector_invalid_batch_rolls_back_sources_jobs_membership_and_provenance() {
    let (dir, store, d, p) = fixture().await;
    let before_sources = store.sources(&Access::admin()).await.unwrap().len();
    let run = store
        .request_plugin_contribution(
            &p.id,
            request(&p, &d, "connector", "invalid-batch", "invalid-second"),
        )
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    assert_eq!(finished(&store, &p, &run).await.status, "failed");
    assert_eq!(
        store.sources(&Access::admin()).await.unwrap().len(),
        before_sources
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM plugin_sources")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM plugin_events WHERE kind='source.sync.failed' AND record_id=?"
        )
        .bind(&run.id)
        .fetch_one(&store.pool)
        .await
        .unwrap(),
        1
    );
}
#[tokio::test]
async fn connector_refuses_namespace_collision_and_ungranted_destination() {
    let (dir, store, d, p) = fixture().await;
    let mut req = request(&p, &d, "connector", "ungranted-destination", "ok");
    req.destination_project_id = Some("ungranted-project".into());
    assert!(store.request_plugin_contribution(&p.id, req).await.is_err());
    let uri = format!(
        "plugin://{}/{}/snapshot-a",
        p.id,
        &crate::store::hash(&p.installed_at)[..24]
    );
    let protected = store
        .import(ImportRequest {
            source_name: "Existing protected source".into(),
            kind: "markdown".into(),
            uri,
            title: "Protected".into(),
            body: "Must remain untouched".into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document;
    let run = store
        .request_plugin_contribution(
            &p.id,
            request(&p, &d, "connector", "namespace-collision", "ok"),
        )
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    assert_eq!(finished(&store, &p, &run).await.status, "cancelled");
    assert_eq!(
        store
            .document(&protected.id, None, &Access::admin())
            .await
            .unwrap()
            .body,
        protected.body
    );
}
#[tokio::test]
async fn plugin_actions_require_review_record_origin_and_undo_after_disable() {
    let (dir, store, d, p, task) = action_fixture().await;
    let run = store
        .request_plugin_contribution(&p.id, request(&p, &d, "action", "propose-action", "ok"))
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let run = finished(&store, &p, &run).await;
    assert_eq!(run.status, "completed");
    assert_eq!(run.action_preview_ids.len(), 1);
    let preview = store
        .action_preview(&run.action_preview_ids[0])
        .await
        .unwrap();
    assert_eq!(preview.origin.as_ref().unwrap().plugin_id, p.id);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM tasks WHERE id=?")
            .bind(&task.id)
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        "candidate"
    );
    let mut execute = ActionExecuteRequest {
        fingerprint: preview.fingerprint.clone(),
        approve_effects: false,
        idempotency_key: "review-action".into(),
    };
    assert!(
        store
            .execute_action(&preview.id, execute.clone())
            .await
            .is_err()
    );
    execute.approve_effects = true;
    let receipt = store
        .execute_action(&preview.id, execute.clone())
        .await
        .unwrap();
    assert_eq!(receipt.after["status"], "accepted");
    assert_eq!(receipt.contributor, "You");
    assert_eq!(receipt.origin.as_ref().unwrap().run_id, run.id);
    assert_eq!(
        store.execute_action(&preview.id, execute).await.unwrap().id,
        receipt.id
    );
    let event=sqlx::query("SELECT origin_plugin,causation_id FROM plugin_events WHERE kind='task.accepted' AND record_id=?").bind(&task.id).fetch_one(&store.pool).await.unwrap();
    assert_eq!(event.get::<String, _>("origin_plugin"), p.id);
    assert_eq!(event.get::<String, _>("causation_id"), run.event_id);
    store
        .update_plugin(
            &p.id,
            PluginUpdateRequest {
                expected_version: p.version,
                enabled: false,
                grant: p.grant.clone(),
                configuration: p.configuration.clone(),
                manifest: None,
                executable: None,
                idempotency_key: "disable-after-effect".into(),
            },
        )
        .await
        .unwrap();
    let undone = store
        .undo_action(
            &receipt.id,
            ActionUndoRequest {
                expected_receipt_version: receipt.version,
                approve_effects: true,
                idempotency_key: "undo-plugin-action".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(undone.status, "undone");
    assert_eq!(undone.undo_effect.unwrap()["status"], "candidate");
    assert!(undone.origin.is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM plugin_effect_context")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
}
#[tokio::test]
async fn plugin_actions_recheck_grants_and_never_commit_a_partial_preview_batch() {
    let (dir, store, d, p, _task) = action_fixture().await;
    let invalid = store
        .request_plugin_contribution(
            &p.id,
            request(&p, &d, "action", "invalid-actions", "invalid-second"),
        )
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    assert_eq!(finished(&store, &p, &invalid).await.status, "failed");
    assert!(store.action_previews().await.unwrap().is_empty());
    let unrelated = store
        .request_plugin_contribution(
            &p.id,
            request(&p, &d, "action", "unrelated-target", "unrelated-task"),
        )
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    assert_eq!(finished(&store, &p, &unrelated).await.status, "failed");
    assert!(store.action_previews().await.unwrap().is_empty());
    let valid = store
        .request_plugin_contribution(&p.id, request(&p, &d, "action", "valid-actions", "ok"))
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let valid = finished(&store, &p, &valid).await;
    let preview = store
        .action_preview(&valid.action_preview_ids[0])
        .await
        .unwrap();
    let mut grant = p.grant.clone();
    grant.operations.retain(|op| op != "actions.propose");
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
                idempotency_key: "revoke-actions".into(),
            },
        )
        .await
        .unwrap();
    assert!(
        store
            .execute_action(
                &preview.id,
                ActionExecuteRequest {
                    fingerprint: preview.fingerprint,
                    approve_effects: true,
                    idempotency_key: "revoked-action".into()
                }
            )
            .await
            .is_err()
    );
    assert!(store.action_receipts().await.unwrap().is_empty());
}
#[tokio::test]
async fn plugin_destination_policy_blocks_input_and_pending_review_without_changing_plugin_version()
{
    let (dir, store, d, p, _task) = action_fixture().await;
    let disclosure = store
        .source_disclosure_preview(&d.source_id, &crate::Config::default(), &Access::admin())
        .await
        .unwrap();
    let destination = format!("plugin:{}", p.id);
    assert!(
        disclosure
            .destinations
            .iter()
            .any(|x| x.destination_id == destination && x.kind == "plugin" && x.allowed)
    );
    let run = store
        .request_plugin_contribution(&p.id, request(&p, &d, "action", "policy-action", "ok"))
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let run = finished(&store, &p, &run).await;
    let preview = store
        .action_preview(&run.action_preview_ids[0])
        .await
        .unwrap();
    store
        .update_source_policy(
            &d.source_id,
            SourcePolicyUpdate {
                expected_version: 0,
                external_enabled: false,
                denied_connections: vec![destination.clone()],
                denied_grants: vec![],
                idempotency_key: "block-plugin-destination".into(),
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    assert_eq!(store.plugin(&p.id).await.unwrap().version, p.version);
    assert!(
        !store
            .plugin_access(&p)
            .await
            .unwrap()
            .source_ids
            .unwrap()
            .contains(&d.source_id)
    );
    assert!(
        store
            .request_plugin_contribution(&p.id, request(&p, &d, "command", "blocked-input", "ok"))
            .await
            .is_err()
    );
    assert!(
        store
            .execute_action(
                &preview.id,
                ActionExecuteRequest {
                    fingerprint: preview.fingerprint,
                    approve_effects: true,
                    idempotency_key: "blocked-review".into()
                }
            )
            .await
            .is_err()
    );
    assert!(store.action_receipts().await.unwrap().is_empty());
    let disclosure = store
        .source_disclosure_preview(&d.source_id, &crate::Config::default(), &Access::admin())
        .await
        .unwrap();
    assert!(
        disclosure
            .destinations
            .iter()
            .any(|x| x.destination_id == destination && !x.allowed)
    );
}
#[tokio::test]
async fn plugin_destination_policy_cancels_an_active_local_contribution() {
    let (dir, store, d, p) = fixture().await;
    let run = store
        .request_plugin_contribution(
            &p.id,
            request(&p, &d, "process", "destination-active", "slow"),
        )
        .await
        .unwrap();
    let worker_store = store.clone();
    let work = dir.path().to_path_buf();
    let worker = tokio::spawn(async move { worker_store.run_plugin_contribution(&work).await });
    wait_started(&dir.path().join("started")).await;
    store
        .update_source_policy(
            &d.source_id,
            SourcePolicyUpdate {
                expected_version: 0,
                external_enabled: false,
                denied_connections: vec![format!("plugin:{}", p.id)],
                denied_grants: vec![],
                idempotency_key: "revoke-active-destination".into(),
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
    assert!(!dir.path().join("finished").exists());
    assert_eq!(observation_count(&store).await, 0);
}
#[tokio::test]
async fn ordinary_host_action_preview_keeps_its_fingerprint_replay_and_undo_contract() {
    let (_dir, store, _d, _p, task) = action_fixture().await;
    let req = ActionPreviewRequest {
        provider: "task_status".into(),
        target_id: task.id.clone(),
        expected_version: Some(task.version),
        status: Some("accepted".into()),
        idempotency_key: "ordinary-preview".into(),
    };
    let preview = store.preview_action(req.clone()).await.unwrap();
    assert!(preview.origin.is_none());
    assert_eq!(store.preview_action(req).await.unwrap().id, preview.id);
    assert_eq!(
        preview.fingerprint,
        crate::store::hash(
            &serde_json::to_string(&(
                &preview.provider,
                &preview.target_id,
                &preview.before,
                &preview.proposed
            ))
            .unwrap()
        )
    );
    let receipt = store
        .execute_action(
            &preview.id,
            ActionExecuteRequest {
                fingerprint: preview.fingerprint,
                approve_effects: true,
                idempotency_key: "ordinary-execute".into(),
            },
        )
        .await
        .unwrap();
    assert!(receipt.origin.is_none());
    assert_eq!(receipt.after["status"], "accepted");
    let undone = store
        .undo_action(
            &receipt.id,
            ActionUndoRequest {
                expected_receipt_version: 1,
                approve_effects: true,
                idempotency_key: "ordinary-undo".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(undone.status, "undone");
    assert!(undone.origin.is_none());
}
#[tokio::test]
async fn connector_reinstallation_does_not_inherit_previous_source_ownership_or_run_history() {
    let (dir, store, d, p) = fixture().await;
    let old = store
        .request_plugin_contribution(
            &p.id,
            request(&p, &d, "connector", "old-install-sync", "ok"),
        )
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let old = finished(&store, &p, &old).await;
    let old_source = old.imported_documents[0].source_id.clone();
    let disabled = store
        .update_plugin(
            &p.id,
            PluginUpdateRequest {
                expected_version: p.version,
                enabled: false,
                grant: p.grant.clone(),
                configuration: p.configuration.clone(),
                manifest: None,
                executable: None,
                idempotency_key: "disable-old-install".into(),
            },
        )
        .await
        .unwrap();
    store
        .uninstall_plugin(
            &p.id,
            PluginUninstallRequest {
                expected_version: disabled.version,
                idempotency_key: "remove-old-install".into(),
            },
        )
        .await
        .unwrap();
    let new = store
        .install_plugin(PluginInstallRequest {
            manifest: p.manifest.clone(),
            executable: p.executable.clone().unwrap(),
            grant: p.grant.clone(),
            configuration: p.configuration.clone(),
            idempotency_key: "install-new-epoch".into(),
        })
        .await
        .unwrap();
    assert_ne!(new.installed_at, p.installed_at);
    let new = store
        .update_plugin(
            &new.id,
            PluginUpdateRequest {
                expected_version: new.version,
                enabled: true,
                grant: new.grant.clone(),
                configuration: new.configuration.clone(),
                manifest: None,
                executable: None,
                idempotency_key: "enable-new-epoch".into(),
            },
        )
        .await
        .unwrap();
    let run = store
        .request_plugin_contribution(
            &new.id,
            request(&new, &d, "connector", "new-install-sync", "ok"),
        )
        .await
        .unwrap();
    store.run_plugin_contribution(dir.path()).await.unwrap();
    let run = finished(&store, &new, &run).await;
    assert_eq!(run.status, "completed");
    assert_ne!(run.imported_documents[0].source_id, old_source);
    assert!(
        !store
            .plugin_access(&new)
            .await
            .unwrap()
            .source_ids
            .unwrap()
            .contains(&old_source)
    );
    assert!(
        store
            .plugin_contribution_run(&new.id, &old.id)
            .await
            .is_err()
    );
    assert_eq!(
        store.plugin_contribution_runs(&new.id).await.unwrap().len(),
        1
    );
}
