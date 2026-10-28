use super::*;
use std::sync::Arc;
use tempfile::TempDir;

fn manifest() -> PluginManifest {
    serde_json::from_value(json!({"id":"example.review","name":"Synthetic review","version":"1.0.0","api_min":1,"api_max":1,"host":"service","hooks":["document.revision.created","memory.observation.created"],"requests":["sources.read","memory.propose"]})).unwrap()
}
pub(super) async fn fixture() -> (TempDir, Store, Document, PluginRecord) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("fixture.sqlite3"))
        .await
        .unwrap();
    let document = store
        .import(ImportRequest {
            source_name: "Synthetic transcript".into(),
            kind: "transcript".into(),
            uri: "fixture://plugin-review".into(),
            title: "Synthetic meeting".into(),
            body: "Initial transcript".into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document;
    let script = dir.path().join("plugin.py");
    std::fs::write(&script,r#"import json,sys
v=json.loads(sys.stdin.readline())
d=v['input'].get('document')
effects=[] if not d else [{'body':'Synthetic follow-up for review','source_ids':[d['source_id']],'evidence':[{'document_id':d['id'],'revision':d['revision'],'line':1,'quote':d['body'].splitlines()[0]}]}]
print(json.dumps({'api_version':1,'invocation_id':v['invocation_id'],'result':{},'observations':effects}))
"#).unwrap();
    let p = store
        .install_plugin(PluginInstallRequest {
            manifest: manifest(),
            executable: PluginExecutable {
                program: "/usr/bin/python3".into(),
                args: vec![script.to_string_lossy().into_owned()],
            },
            grant: PluginPermissions {
                source_ids: vec![document.source_id.clone()],
                operations: vec!["sources.read".into(), "memory.propose".into()],
                ..Default::default()
            },
            configuration: json!({}),
            idempotency_key: "install".into(),
        })
        .await
        .unwrap();
    (dir, store, document, p)
}
pub(super) fn update(p: &PluginRecord, enabled: bool, key: &str) -> PluginUpdateRequest {
    PluginUpdateRequest {
        expected_version: p.version,
        enabled,
        grant: p.grant.clone(),
        configuration: p.configuration.clone(),
        manifest: None,
        executable: None,
        idempotency_key: key.into(),
    }
}
async fn revise(store: &Store, body: &str) -> Document {
    store
        .import(ImportRequest {
            source_name: "Synthetic transcript".into(),
            kind: "transcript".into(),
            uri: "fixture://plugin-review".into(),
            title: "Synthetic meeting".into(),
            body: body.into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document
}

#[tokio::test]
async fn plugin_outbox_rolls_back_with_domain_write() {
    let (_dir, store, document, _p) = fixture().await;
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    let mut tx = store.pool.begin().await.unwrap();
    sqlx::query("UPDATE documents SET revision=99 WHERE id=?")
        .bind(&document.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    let outside: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(outside, before);
    tx.rollback().await.unwrap();
    let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(after, before);
}

#[tokio::test]
async fn lifecycle_hooks_cover_committed_policy_memory_priority_context_and_eval() {
    let (dir, store, initial, _p) = fixture().await;
    let doc = revise(&store, "Maya: I'll review the rollout by 2026-10-20.").await;
    let project = store
        .create_brief(BriefCreateRequest {
            name: "Synthetic lifecycle project".into(),
            source_ids: vec![doc.source_id.clone()],
            idempotency_key: "lifecycle-project".into(),
        })
        .await
        .unwrap();
    store
        .import(ImportRequest {
            source_name: "Synthetic transcript".into(),
            kind: "transcript".into(),
            uri: "fixture://plugin-review".into(),
            title: "Synthetic meeting".into(),
            body: doc.body.clone(),
            allow_external: true,
        })
        .await
        .unwrap();
    let observation = store
        .append_observation(
            ObservationRequest {
                body: "Original synthetic memory".into(),
                source_ids: vec![doc.source_id.clone()],
                evidence: vec![Evidence {
                    document_id: initial.id,
                    revision: initial.revision,
                    line: 1,
                    quote: initial.body,
                }],
                idempotency_key: "lifecycle-observation".into(),
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    store
        .revise_memory(
            &observation.id,
            MemoryReviseRequest {
                expected_version: 0,
                action: "correct".into(),
                body: Some("Corrected synthetic memory".into()),
                restore_version: None,
                reason: "Synthetic correction".into(),
                idempotency_key: "lifecycle-correct".into(),
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    while store
        .process_next(&crate::Config::default(), dir.path())
        .await
        .unwrap()
    {}
    let task = store
        .snapshot(&Access::admin())
        .await
        .unwrap()
        .tasks
        .into_iter()
        .find(|t| t.status == "candidate")
        .unwrap();
    store
        .update_task(
            &task.id,
            TaskUpdate {
                expected_version: task.version,
                status: "accepted".into(),
            },
        )
        .await
        .unwrap();
    store
        .set_priority_preference(
            &project.id,
            &task.id,
            PriorityPreferenceRequest {
                expected_version: 0,
                importance: "high".into(),
                note: "Synthetic priority".into(),
                idempotency_key: "lifecycle-priority".into(),
            },
        )
        .await
        .unwrap();
    let thread = store
        .create_thread(ThreadCreateRequest {
            project_id: project.id.clone(),
            title: "Synthetic thread".into(),
            source_ids: vec![doc.source_id.clone()],
            idempotency_key: "lifecycle-thread".into(),
        })
        .await
        .unwrap();
    let context = store
        .thread_context(&thread.id, &Access::admin(), 12000)
        .await
        .unwrap();
    store
        .context_budgeted(
            ContextBuildRequest {
                query: "rollout".into(),
                source_ids: Some(vec![doc.source_id.clone()]),
                character_budget: 12000,
                token_budget: 4000,
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    let case:EvalCase=serde_json::from_value(json!({"id":"retrieval","name":"Synthetic retrieval check","target":"retrieval","query":"rollout","expected_contains":["rollout"],"threshold":0.5})).unwrap();
    let suite = store
        .save_eval_suite(
            None,
            EvalSuiteRequest {
                expected_version: 0,
                name: "Synthetic hook suite".into(),
                project_id: Some(project.id.clone()),
                cases: vec![case],
                idempotency_key: "lifecycle-suite".into(),
            },
        )
        .await
        .unwrap();
    let request:EvalRunRequest=serde_json::from_value(json!({"suite_version":suite.version,"project_id":project.id,"baseline_id":null,"outputs":[],"max_cases":1,"max_output_bytes":16000,"max_cost_microunits":0,"idempotency_key":"lifecycle-eval"})).unwrap();
    let eval = store.execute_eval(&suite.id, request).await.unwrap();
    // Exercise each trigger's committed record shape independently of a paid
    // agent/MCP. These are outbox checks, not a live upstream integration claim.
    let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
    sqlx::query("INSERT INTO handoff_receipts(context_id,version,record) VALUES(?,1,?)")
        .bind(&context.id)
        .bind(json!({"state":"acknowledged"}).to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO retrieval_upstreams(id,source_id,project_id,version,record) VALUES('synthetic-upstream',?,?,1,'{}')").bind(&doc.source_id).bind(&project.id).execute(&mut *tx).await.unwrap();
    for (id, state) in [
        ("synthetic-fetch-ok", "completed"),
        ("synthetic-fetch-error", "failed"),
    ] {
        sqlx::query("INSERT INTO retrieval_fetches(id,upstream_id,request_hash,status,created_at) VALUES(?,'synthetic-upstream','synthetic','fetching',?)").bind(id).bind(now()).execute(&mut *tx).await.unwrap();
        sqlx::query("UPDATE retrieval_fetches SET status=?,document_id=?,revision=? WHERE id=?")
            .bind(state)
            .bind(&doc.id)
            .bind(doc.revision)
            .bind(id)
            .execute(&mut *tx)
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    for (kind, record) in [
        ("source.policy.changed", doc.source_id.as_str()),
        ("memory.observation.superseded", observation.id.as_str()),
        ("priority.changed", task.id.as_str()),
        ("context.bundle.created", context.id.as_str()),
        ("handoff.acknowledged", context.id.as_str()),
        ("eval.run.completed", eval.id.as_str()),
        ("source.sync.completed", "synthetic-fetch-ok"),
        ("source.sync.failed", "synthetic-fetch-error"),
    ] {
        let row=sqlx::query("SELECT * FROM plugin_events WHERE kind=? AND record_id=? ORDER BY sequence DESC LIMIT 1").bind(kind).bind(record).fetch_one(&store.pool).await.unwrap();
        let event = store.plugin_event(row.get("id")).await.unwrap();
        assert_eq!(event.schema_version, PLUGIN_EVENT_VERSION);
        assert!(
            event.source_ids.contains(&doc.source_id),
            "{kind} must retain its source scope"
        );
    }
    let previews:i64=sqlx::query_scalar("SELECT COUNT(*) FROM plugin_events WHERE kind='context.bundle.created' AND record_kind='context_preview'").fetch_one(&store.pool).await.unwrap();
    assert!(previews >= 1);
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_events")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    store.migrate_plugins().await.unwrap();
    assert_eq!(
        before,
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM plugin_events")
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        "migrating hooks must not replay old records"
    );
}

#[tokio::test]
async fn plugin_delivery_replay_scope_and_causation() {
    let (dir, store, document, p) = fixture().await;
    assert!(!p.enabled);
    let p = store
        .update_plugin(&p.id, update(&p, true, "enable"))
        .await
        .unwrap();
    revise(&store, "TODO: synthetic follow-up").await;
    assert_eq!(store.queue_plugin_deliveries().await.unwrap(), 1);
    assert!(store.run_plugin_delivery(dir.path()).await.unwrap());
    let deliveries = store.plugin_deliveries(&p.id).await.unwrap();
    assert_eq!(deliveries[0].status, "completed");
    let attempts = store
        .plugin_delivery_attempts(&p.id, &deliveries[0].id)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 1);
    assert_eq!(
        attempts[0].api_version, 1,
        "legacy API v1 plugins must keep their negotiated wire version"
    );
    assert_eq!(attempts[0].configuration_version, p.version);
    assert_eq!(attempts[0].grant, Some(p.grant.clone()));
    assert_eq!(
        attempts[0].configuration_hash,
        Some(crate::store::hash(
            &serde_json::to_string(&p.configuration).unwrap()
        ))
    );
    assert!(attempts[0].finished_at.is_some());
    let observations = store.observations(&Access::admin()).await.unwrap();
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].contributor, "plugin:example.review");
    assert_eq!(observations[0].source_ids, vec![document.source_id.clone()]);
    let origin:(String,i64)=sqlx::query_as("SELECT origin_plugin,depth FROM plugin_events WHERE record_id=? AND kind='memory.observation.created'")
        .bind(&observations[0].id).fetch_one(&store.pool).await.unwrap();
    assert_eq!(origin, (p.id.clone(), 1));
    let child_id: String = sqlx::query_scalar(
        "SELECT id FROM plugin_events WHERE record_id=? AND kind='memory.observation.created'",
    )
    .bind(&observations[0].id)
    .fetch_one(&store.pool)
    .await
    .unwrap();
    let child = store.plugin_event(&child_id).await.unwrap();
    let parent = store.plugin_event(&deliveries[0].event_id).await.unwrap();
    assert_eq!(child.causation_id, Some(parent.id));
    assert_eq!(child.correlation_id, parent.correlation_id);
    assert_eq!(
        store.queue_plugin_deliveries().await.unwrap(),
        0,
        "own effects must not reenter the same plugin"
    );
    sqlx::query("UPDATE plugin_deliveries SET status='failed' WHERE id=?")
        .bind(&deliveries[0].id)
        .execute(&store.pool)
        .await
        .unwrap();
    store
        .retry_plugin_delivery(&p.id, &deliveries[0].id)
        .await
        .unwrap();
    store.run_plugin_delivery(dir.path()).await.unwrap();
    let replay_attempts = store
        .plugin_delivery_attempts(&p.id, &deliveries[0].id)
        .await
        .unwrap();
    assert_eq!(replay_attempts.len(), 2);
    assert_ne!(replay_attempts[0].id, replay_attempts[1].id);
    assert_eq!(
        store.observations(&Access::admin()).await.unwrap().len(),
        1,
        "replaying a committed effect must not duplicate it"
    );
    let mut revoked = update(&p, true, "revoke");
    revoked.grant.source_ids.clear();
    store.update_plugin(&p.id, revoked).await.unwrap();
    revise(&store, "Another synthetic revision").await;
    store.queue_plugin_deliveries().await.unwrap();
    store.run_plugin_delivery(dir.path()).await.unwrap();
    assert_eq!(
        store.plugin_deliveries(&p.id).await.unwrap()[0].status,
        "rejected"
    );
    assert_eq!(store.observations(&Access::admin()).await.unwrap().len(), 1);
}

#[test]
fn plugin_api_negotiation_preserves_legacy_and_rejects_incompatible_packages() {
    let mut m = manifest();
    assert_eq!(validation::negotiated_api(&m).unwrap(), 1);
    m.api_max = 2;
    assert_eq!(validation::negotiated_api(&m).unwrap(), 2);
    m.api_min = 2;
    assert_eq!(validation::negotiated_api(&m).unwrap(), 2);
    m.api_min = 3;
    m.api_max = 3;
    assert!(validation::negotiated_api(&m).is_err());
    m.api_min = 0;
    m.api_max = 2;
    assert!(validation::negotiated_api(&m).is_err());
}

#[tokio::test]
async fn interrupted_delivery_attempt_history_survives_recovery_and_retry() {
    let (dir, store, _doc, p) = fixture().await;
    let p = store
        .update_plugin(&p.id, update(&p, true, "enable-for-recovery"))
        .await
        .unwrap();
    revise(&store, "TODO: synthetic recovery").await;
    store.queue_plugin_deliveries().await.unwrap();
    store.run_plugin_delivery(dir.path()).await.unwrap();
    let d = store.plugin_deliveries(&p.id).await.unwrap().remove(0);
    sqlx::query("UPDATE plugin_deliveries SET status='running' WHERE id=?")
        .bind(&d.id)
        .execute(&store.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO plugin_delivery_attempts(id,delivery_id,plugin_id,configuration_version,package_version,api_version,status,started_at) VALUES('interrupted-attempt',?,?,?,'1.0.0',1,'running',?)").bind(&d.id).bind(&p.id).bind(p.version).bind(now()).execute(&store.pool).await.unwrap();
    store.migrate_plugins().await.unwrap();
    let attempts = store.plugin_delivery_attempts(&p.id, &d.id).await.unwrap();
    assert!(attempts.iter().any(|a| a.id == "interrupted-attempt"
        && a.status == "interrupted"
        && a.finished_at.is_some()
        && a.grant.is_none()));
    store.run_plugin_delivery(dir.path()).await.unwrap();
    assert_eq!(store.observations(&Access::admin()).await.unwrap().len(), 1);
    assert_eq!(
        store
            .plugin_delivery_attempts(&p.id, &d.id)
            .await
            .unwrap()
            .len(),
        3
    );
}

#[tokio::test]
async fn plugin_enable_skips_disabled_history_and_reinstall_works() {
    let (_dir, store, _doc, p) = fixture().await;
    revise(&store, "Changed while disabled").await;
    let p = store
        .update_plugin(&p.id, update(&p, true, "enable"))
        .await
        .unwrap();
    assert_eq!(store.queue_plugin_deliveries().await.unwrap(), 0);
    let p = store
        .update_plugin(&p.id, update(&p, false, "disable"))
        .await
        .unwrap();
    store
        .uninstall_plugin(
            &p.id,
            PluginUninstallRequest {
                expected_version: p.version,
                idempotency_key: "uninstall".into(),
            },
        )
        .await
        .unwrap();
    let p2 = store
        .install_plugin(PluginInstallRequest {
            manifest: p.manifest,
            executable: p.executable.unwrap(),
            grant: p.grant,
            configuration: p.configuration,
            idempotency_key: "reinstall".into(),
        })
        .await
        .unwrap();
    assert_eq!(p2.version, 1);
    assert!(!p2.enabled);
}

struct MockProvider {
    id: &'static str,
}

struct SyntheticCredentials;
impl registry::CapabilityProvider for SyntheticCredentials {
    fn manifest(&self) -> PluginManifest {
        native::manifest(
            "example.credentials",
            "Synthetic credentials",
            "native.credentials",
            &["read"],
            PluginHost::Service,
        )
    }
    fn health(&self) -> Result<()> {
        Ok(())
    }
    fn invoke<'a>(&'a self, command: &'a str, input: Value) -> registry::ProviderFuture<'a> {
        Box::pin(async move {
            if command != "read" || input["reference"] != "env:THESAURUS_SYNTHETIC_TOKEN" {
                bail!("invalid: synthetic reference required");
            }
            Ok(json!({"secret":"synthetic-host-only-token"}))
        })
    }
}
#[tokio::test]
async fn selected_credentials_provider_supplies_model_auth_without_persisting_secrets() {
    let (_dir, store, document, _p) = fixture().await;
    store
        .providers
        .register(Arc::new(SyntheticCredentials))
        .unwrap();
    store.seed_native_providers().await.unwrap();
    let c = store
        .plugin_capabilities()
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.capability == "native.credentials")
        .unwrap();
    store
        .select_plugin_provider(
            &c.capability,
            PluginProviderSelectionRequest {
                expected_version: c.selection_version,
                provider_id: "example.credentials".into(),
                idempotency_key: "select-synthetic-credentials".into(),
            },
        )
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let http = tokio::spawn(async move {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = vec![];
        loop {
            let mut buffer = [0; 4096];
            let n = socket.read(&mut buffer).await.unwrap();
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..n]);
            if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        let request = String::from_utf8_lossy(&bytes).to_lowercase();
        assert!(request.contains("authorization: bearer synthetic-host-only-token\r\n"));
        let body = json!({"choices":[{"message":{"content":"{\"actions\":[]}"}}]}).to_string();
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
    });
    let config:crate::Config=serde_json::from_value(json!({"connections":[{"id":"synthetic-model","kind":"openai_compatible","endpoint":format!("http://{addr}/v1"),"model":"synthetic","credential_ref":"env:THESAURUS_SYNTHETIC_TOKEN","daily_limit":1}]})).unwrap();
    let result = crate::models::extract(
        &store,
        &document,
        "synthetic-model",
        &config,
        Path::new("."),
    )
    .await
    .unwrap();
    assert!(result.is_empty());
    http.await.unwrap();
    let records = serde_json::to_string(&store.plugins().await.unwrap()).unwrap();
    assert!(!records.contains("synthetic-host-only-token"));
    let p = store.plugin("example.credentials").await.unwrap();
    store
        .update_plugin(&p.id, update(&p, false, "disable-credentials"))
        .await
        .unwrap();
    assert!(
        store
            .resolve_credential("env:THESAURUS_SYNTHETIC_TOKEN")
            .await
            .is_err(),
        "disabled provider must not fall back to process environment"
    );
}

#[tokio::test]
async fn credential_reference_schema_rejects_empty_or_unbounded_names() {
    let (_dir, store, _document, _p) = fixture().await;
    for reference in [
        "env:",
        "keychain:",
        "keychain:outside/service",
        "env:BAD\nNAME",
        "other:name",
    ] {
        assert!(native::credentials::reference(reference).is_err());
    }
    assert!(native::credentials::reference("keychain:my-api-token").is_ok());
    assert!(
        store
            .resolve_credential("env:THESAURUS_SYNTHETIC_MISSING_TOKEN_127390875")
            .await
            .is_err()
    );
}

#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "Explicit opt-in: writes and removes one unique synthetic item in the OS Keychain"]
async fn native_keychain_round_trip_synthetic_only() {
    let (_dir, store, _document, _p) = fixture().await;
    let reference = format!("keychain:f44-verification-{}", uuid::Uuid::new_v4());
    store
        .native_invoke(
            "native.credentials",
            "write",
            json!({"reference":reference,"secret":"synthetic-fixture-not-an-api-key"}),
        )
        .await
        .unwrap();
    let read = store.resolve_credential(&reference).await;
    let removed = store
        .native_invoke(
            "native.credentials",
            "delete",
            json!({"reference":reference}),
        )
        .await;
    assert_eq!(read.unwrap(), "synthetic-fixture-not-an-api-key");
    assert!(removed.unwrap()["removed"].as_bool().unwrap());
}
impl registry::CapabilityProvider for MockProvider {
    fn manifest(&self) -> PluginManifest {
        let mut m = manifest();
        m.id = self.id.into();
        m.capabilities = vec![PluginCapability {
            id: "native.audio".into(),
            commands: vec!["inputs".into(), "record".into()],
            description: String::new(),
        }];
        m
    }
    fn health(&self) -> Result<()> {
        Ok(())
    }
    fn invoke<'a>(&'a self, _command: &'a str, _input: Value) -> registry::ProviderFuture<'a> {
        Box::pin(async { Ok(json!({"synthetic":true})) })
    }
    fn open<'a>(&'a self, _command: &'a str, _input: Value) -> registry::ProviderOpenFuture<'a> {
        Box::pin(async {
            Ok(tokio::process::Command::new("/bin/cat")
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn()?)
        })
    }
}
#[tokio::test]
async fn provider_lease_prevents_disable_or_replacement() {
    let r = registry::ProviderRegistry::default();
    r.register(Arc::new(MockProvider {
        id: "example.first",
    }))
    .unwrap();
    r.register(Arc::new(MockProvider {
        id: "example.second",
    }))
    .unwrap();
    let lease = r.acquire("native.audio").unwrap();
    assert!(r.set_enabled("example.first", false).is_err());
    assert!(r.select("native.audio", "example.second").is_err());
    drop(lease);
    r.select("native.audio", "example.second").unwrap();
    r.set_enabled("example.first", false).unwrap();
    assert_eq!(
        r.invoke("native.audio", "inputs", json!({})).await.unwrap(),
        json!({"synthetic":true})
    );
    assert!(
        r.invoke("native.audio", "unexpected", json!({}))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn native_store_selection_and_streaming_resource_guard() {
    let (_dir, store, _doc, _p) = fixture().await;
    store
        .providers
        .register(Arc::new(MockProvider {
            id: "example.native",
        }))
        .unwrap();
    store.seed_native_providers().await.unwrap();
    let capabilities = store.plugin_capabilities().await.unwrap();
    let audio = capabilities
        .into_iter()
        .find(|c| c.capability == "native.audio")
        .unwrap();
    store
        .select_plugin_provider(
            "native.audio",
            PluginProviderSelectionRequest {
                expected_version: audio.selection_version,
                provider_id: "example.native".into(),
                idempotency_key: "select-mock".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .native_invoke("native.audio", "inputs", json!({}))
            .await
            .unwrap(),
        json!({"synthetic":true})
    );
    let mut resource = store
        .native_open("native.audio", "record", json!({}))
        .await
        .unwrap();
    let p = store.plugin("example.native").await.unwrap();
    assert!(
        store
            .update_plugin(&p.id, update(&p, false, "disable-busy"))
            .await
            .is_err()
    );
    resource.child.kill().await.unwrap();
    let _ = resource.child.wait().await;
    drop(resource);
    store
        .update_plugin(&p.id, update(&p, false, "disable-idle"))
        .await
        .unwrap();
    assert!(
        store
            .native_invoke("native.audio", "inputs", json!({}))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn disabling_plugin_cancels_inflight_hook_without_effects() {
    let (dir, store, _doc, p) = fixture().await;
    let script = dir.path().join("plugin.py");
    let marker = dir.path().join("invocation-started");
    let code = format!(
        "import json,sys,time\nv=json.loads(sys.stdin.readline())\nopen({},'w').write('started')\ntime.sleep(30)\n",
        serde_json::to_string(marker.to_str().unwrap()).unwrap()
    );
    std::fs::write(script, code).unwrap();
    let p = store
        .update_plugin(&p.id, update(&p, true, "enable"))
        .await
        .unwrap();
    revise(&store, "Synthetic cancellation revision").await;
    store.queue_plugin_deliveries().await.unwrap();
    let task_store = store.clone();
    let path = dir.path().to_path_buf();
    let task = tokio::spawn(async move { task_store.run_plugin_delivery(&path).await });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !marker.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    store
        .update_plugin(&p.id, update(&p, false, "disable-running"))
        .await
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        store.plugin_deliveries(&p.id).await.unwrap()[0].status,
        "rejected"
    );
    assert!(
        store
            .observations(&Access::admin())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn external_source_policy_is_rechecked_at_delivery() {
    let (dir, store, doc, p) = fixture().await;
    let mut enable = update(&p, true, "enable-external");
    enable.grant.external = true;
    store.update_plugin(&p.id, enable).await.unwrap();
    revise(&store, "Synthetic policy revision").await;
    sqlx::query("UPDATE sources SET allow_external=1 WHERE id=?")
        .bind(&doc.source_id)
        .execute(&store.pool)
        .await
        .unwrap();
    store.queue_plugin_deliveries().await.unwrap();
    sqlx::query("UPDATE sources SET allow_external=0 WHERE id=?")
        .bind(&doc.source_id)
        .execute(&store.pool)
        .await
        .unwrap();
    store.run_plugin_delivery(dir.path()).await.unwrap();
    assert_eq!(
        store.plugin_deliveries(&p.id).await.unwrap()[0].status,
        "rejected"
    );
    assert!(
        store
            .observations(&Access::admin())
            .await
            .unwrap()
            .is_empty()
    );
}

fn desktop_manifest() -> PluginManifest {
    native::manifest(
        "thesaurus.native.test-window",
        "Synthetic desktop window",
        "native.test_window",
        &["show"],
        PluginHost::Desktop,
    )
}

#[tokio::test]
async fn package_upgrade_rejects_missing_dependencies_and_cycles_atomically() {
    let (_dir, store, _doc, first) = fixture().await;
    let mut second_manifest = manifest();
    second_manifest.id = "example.dependent".into();
    second_manifest.dependencies = vec![first.id.clone()];
    let second = store
        .install_plugin(PluginInstallRequest {
            manifest: second_manifest,
            executable: PluginExecutable {
                program: "/usr/bin/true".into(),
                args: vec![],
            },
            grant: Default::default(),
            configuration: json!({}),
            idempotency_key: "install-disabled-dependent".into(),
        })
        .await
        .unwrap();
    assert!(!second.enabled);
    assert!(
        store
            .update_plugin(
                &second.id,
                update(&second, true, "enable-before-dependency")
            )
            .await
            .is_err()
    );
    let mut cyclic = first.manifest.clone();
    cyclic.dependencies = vec![second.id.clone()];
    let mut req = update(&first, false, "cyclic-upgrade");
    req.manifest = Some(cyclic);
    assert!(
        store
            .update_plugin(&first.id, req)
            .await
            .unwrap_err()
            .to_string()
            .contains("cycle")
    );
    assert_eq!(
        store.plugin(&first.id).await.unwrap().version,
        first.version
    );
    let mut missing = first.manifest.clone();
    missing.dependencies = vec!["example.missing".into()];
    let mut req = update(&first, false, "missing-upgrade");
    req.manifest = Some(missing);
    assert!(
        store
            .update_plugin(&first.id, req)
            .await
            .unwrap_err()
            .to_string()
            .contains("not installed")
    );
    assert_eq!(
        store.plugin(&first.id).await.unwrap().manifest,
        first.manifest
    );
}
fn desktop_heartbeat(p: &PluginRecord) -> PluginHostHeartbeat {
    PluginHostHeartbeat {
        providers: vec![PluginProviderHealth {
            provider_id: p.id.clone(),
            version: p.version,
            available: true,
            detail: "Synthetic provider ready".into(),
        }],
    }
}
fn native_request(key: &str) -> PluginNativeRequest {
    PluginNativeRequest {
        command: "show".into(),
        input: json!({"synthetic":true}),
        idempotency_key: key.into(),
    }
}

#[tokio::test]
async fn desktop_heartbeats_and_commands_serialize_without_write_upgrade_conflicts() {
    let (_dir, store, _doc, _plugin) = fixture().await;
    let session = store
        .register_desktop_host(PluginHostRegistration {
            host_id: "synthetic-host".into(),
            manifests: vec![desktop_manifest()],
        })
        .await
        .unwrap();
    let p = store.plugin("thesaurus.native.test-window").await.unwrap();
    store
        .poll_desktop_host(
            &session.host_id,
            &session.lease_token,
            desktop_heartbeat(&p),
        )
        .await
        .unwrap();
    let heartbeat_store = store.clone();
    let heartbeat = tokio::spawn(async move {
        for _ in 0..30 {
            heartbeat_store
                .poll_desktop_host(
                    &session.host_id,
                    &session.lease_token,
                    desktop_heartbeat(&p),
                )
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    });
    let mut writers = vec![];
    for i in 0..24 {
        let store = store.clone();
        writers.push(tokio::spawn(async move {
            store
                .enqueue_desktop_command(
                    "native.test_window",
                    native_request(&format!("concurrent-{i}")),
                )
                .await
        }));
    }
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        for writer in writers {
            writer.await.unwrap().unwrap();
        }
        heartbeat.await.unwrap();
    })
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugin_native_commands")
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(count, 24);
}

#[tokio::test]
async fn desktop_alternate_selection_waits_for_new_provider_readiness() {
    let (_dir, store, _doc, _plugin) = fixture().await;
    let first = desktop_manifest();
    let mut second = first.clone();
    second.id = "thesaurus.native.test-alternate".into();
    let session = store
        .register_desktop_host(PluginHostRegistration {
            host_id: "synthetic-host".into(),
            manifests: vec![first.clone(), second.clone()],
        })
        .await
        .unwrap();
    let p = store.plugin(&first.id).await.unwrap();
    store
        .poll_desktop_host(
            &session.host_id,
            &session.lease_token,
            desktop_heartbeat(&p),
        )
        .await
        .unwrap();
    let cap = store
        .plugin_capabilities()
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.capability == "native.test_window")
        .unwrap();
    let caps = store
        .select_plugin_provider(
            &cap.capability,
            PluginProviderSelectionRequest {
                expected_version: cap.selection_version,
                provider_id: second.id.clone(),
                idempotency_key: "choose-alternate".into(),
            },
        )
        .await
        .unwrap();
    assert!(
        caps.iter()
            .any(|c| c.provider_id == second.id && !c.available)
    );
    assert!(
        store
            .enqueue_desktop_command(
                "native.test_window",
                native_request("before-alternate-ready")
            )
            .await
            .is_err()
    );
    let p = store.plugin(&second.id).await.unwrap();
    store
        .poll_desktop_host(
            &session.host_id,
            &session.lease_token,
            desktop_heartbeat(&p),
        )
        .await
        .unwrap();
    let id = store
        .enqueue_desktop_command(
            "native.test_window",
            native_request("after-alternate-ready"),
        )
        .await
        .unwrap();
    let poll = store
        .poll_desktop_host(
            &session.host_id,
            &session.lease_token,
            desktop_heartbeat(&p),
        )
        .await
        .unwrap();
    assert_eq!(poll.commands[0].id, id);
    assert_eq!(poll.commands[0].provider_id, second.id);
}

#[tokio::test]
async fn project_plugin_sees_first_published_transcript_revision() {
    let (dir, store, initial, p) = fixture().await;
    let project = store
        .create_brief(BriefCreateRequest {
            name: "Synthetic recording project".into(),
            source_ids: vec![initial.source_id],
            idempotency_key: "recording-project".into(),
        })
        .await
        .unwrap();
    let mut req = update(&p, true, "enable-project-subscriber");
    req.grant.source_ids.clear();
    req.grant.project_ids = vec![project.id.clone()];
    store.update_plugin(&p.id, req).await.unwrap();
    let published = store
        .import_into_project(
            ImportRequest {
                source_name: "Synthetic recording".into(),
                kind: "transcript".into(),
                uri: "fixture://new-recording-source".into(),
                title: "Synthetic recording".into(),
                body: "TODO: first recording follow-up".into(),
                allow_external: false,
            },
            Some(&project.id),
        )
        .await
        .unwrap()
        .document;
    assert_eq!(published.revision, 1);
    let projects:String=sqlx::query_scalar("SELECT project_ids FROM plugin_events WHERE kind='document.revision.created' AND record_id=? AND record_revision=1").bind(&published.id).fetch_one(&store.pool).await.unwrap();
    assert!(
        serde_json::from_str::<Vec<String>>(&projects)
            .unwrap()
            .contains(&project.id)
    );
    assert!(
        store
            .brief(&project.id, &Access::admin())
            .await
            .unwrap()
            .source_ids
            .contains(&published.source_id)
    );
    assert_eq!(store.queue_plugin_deliveries().await.unwrap(), 1);
    store.run_plugin_delivery(dir.path()).await.unwrap();
    assert_eq!(
        store.plugin_deliveries(&p.id).await.unwrap()[0].status,
        "completed"
    );
    assert_eq!(
        store.observations(&Access::admin()).await.unwrap()[0].source_ids,
        vec![published.source_id]
    );
}

#[tokio::test]
async fn desktop_host_requires_lease_and_applied_configuration() {
    let (_dir, store, _doc, _plugin) = fixture().await;
    let m = desktop_manifest();
    let session = store
        .register_desktop_host(PluginHostRegistration {
            host_id: "synthetic-host".into(),
            manifests: vec![m.clone()],
        })
        .await
        .unwrap();
    let p = store.plugin(&m.id).await.unwrap();
    assert!(
        !store.desktop_available(&p.id).await.unwrap(),
        "registration alone is not readiness"
    );
    assert!(
        store
            .enqueue_desktop_command("native.test_window", native_request("too-early"))
            .await
            .is_err()
    );
    assert!(
        store
            .poll_desktop_host(&session.host_id, "wrong-token", desktop_heartbeat(&p))
            .await
            .is_err()
    );
    assert!(
        store
            .register_desktop_host(PluginHostRegistration {
                host_id: "other-host".into(),
                manifests: vec![m]
            })
            .await
            .is_err()
    );
    store
        .poll_desktop_host(
            &session.host_id,
            &session.lease_token,
            desktop_heartbeat(&p),
        )
        .await
        .unwrap();
    assert!(store.desktop_available(&p.id).await.unwrap());
    let capabilities = store.plugin_capabilities().await.unwrap();
    assert!(
        capabilities
            .iter()
            .any(|c| c.capability == "native.test_window"
                && c.available
                && c.host == PluginHost::Desktop)
    );
    let id = store
        .enqueue_desktop_command("native.test_window", native_request("first-command"))
        .await
        .unwrap();
    assert_eq!(
        id,
        store
            .enqueue_desktop_command("native.test_window", native_request("first-command"))
            .await
            .unwrap()
    );
    let poll = store
        .poll_desktop_host(
            &session.host_id,
            &session.lease_token,
            desktop_heartbeat(&p),
        )
        .await
        .unwrap();
    assert_eq!(poll.commands.len(), 1);
    assert_eq!(poll.commands[0].provider_version, p.version);
    assert!(
        store
            .update_plugin(&p.id, update(&p, false, "disable-running-command"))
            .await
            .is_err()
    );
    let reply = PluginNativeReply {
        result: json!({"shown":true}),
        error: None,
    };
    assert!(
        store
            .reply_desktop_command(&session.host_id, "wrong-token", &id, reply.clone())
            .await
            .is_err()
    );
    store
        .reply_desktop_command(&session.host_id, &session.lease_token, &id, reply.clone())
        .await
        .unwrap();
    assert_eq!(
        store
            .desktop_command_result(&id)
            .await
            .unwrap()
            .unwrap()
            .result,
        reply.result
    );
    assert!(
        store
            .reply_desktop_command(&session.host_id, &session.lease_token, &id, reply)
            .await
            .is_err()
    );
    assert!(
        store
            .poll_desktop_host(
                &session.host_id,
                &session.lease_token,
                desktop_heartbeat(&p)
            )
            .await
            .unwrap()
            .commands
            .is_empty()
    );
    let disabled = store
        .update_plugin(&p.id, update(&p, false, "disable-idle-desktop"))
        .await
        .unwrap();
    assert!(!disabled.enabled);
    assert!(
        store
            .enqueue_desktop_command("native.test_window", native_request("disabled-command"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn desktop_command_restart_never_replays_unknown_outcomes() {
    let (_dir, store, _doc, _plugin) = fixture().await;
    let registration = PluginHostRegistration {
        host_id: "synthetic-host".into(),
        manifests: vec![desktop_manifest()],
    };
    let session = store
        .register_desktop_host(registration.clone())
        .await
        .unwrap();
    let p = store.plugin("thesaurus.native.test-window").await.unwrap();
    store
        .poll_desktop_host(
            &session.host_id,
            &session.lease_token,
            desktop_heartbeat(&p),
        )
        .await
        .unwrap();
    let pending = store
        .enqueue_desktop_command(
            "native.test_window",
            native_request("pending-before-reconnect"),
        )
        .await
        .unwrap();
    let renewed = store.register_desktop_host(registration).await.unwrap();
    assert!(
        store
            .desktop_command_result(&pending)
            .await
            .unwrap_err()
            .to_string()
            .contains("unknown")
    );
    assert!(
        store
            .poll_desktop_host(
                &session.host_id,
                &session.lease_token,
                desktop_heartbeat(&p)
            )
            .await
            .is_err()
    );
    assert!(
        store
            .poll_desktop_host(
                &renewed.host_id,
                &renewed.lease_token,
                desktop_heartbeat(&p)
            )
            .await
            .unwrap()
            .commands
            .is_empty()
    );
    let running = store
        .enqueue_desktop_command(
            "native.test_window",
            native_request("running-before-restart"),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .poll_desktop_host(
                &renewed.host_id,
                &renewed.lease_token,
                desktop_heartbeat(&p)
            )
            .await
            .unwrap()
            .commands
            .len(),
        1
    );
    store.migrate_plugins().await.unwrap();
    assert!(!store.desktop_available(&p.id).await.unwrap());
    assert!(
        store
            .desktop_command_result(&running)
            .await
            .unwrap_err()
            .to_string()
            .contains("unknown")
    );
    assert!(
        store
            .poll_desktop_host(
                &renewed.host_id,
                &renewed.lease_token,
                desktop_heartbeat(&p)
            )
            .await
            .is_err()
    );
    assert_eq!(
        running,
        store
            .enqueue_desktop_command(
                "native.test_window",
                native_request("running-before-restart")
            )
            .await
            .unwrap(),
        "repeated receipt lookup must not enqueue a second command"
    );
}

#[tokio::test]
async fn desktop_command_rejects_changed_provider_and_bounds_queue() {
    let (_dir, store, _doc, _plugin) = fixture().await;
    let session = store
        .register_desktop_host(PluginHostRegistration {
            host_id: "synthetic-host".into(),
            manifests: vec![desktop_manifest()],
        })
        .await
        .unwrap();
    let p = store.plugin("thesaurus.native.test-window").await.unwrap();
    store
        .poll_desktop_host(
            &session.host_id,
            &session.lease_token,
            desktop_heartbeat(&p),
        )
        .await
        .unwrap();
    let stale = store
        .enqueue_desktop_command("native.test_window", native_request("stale-command"))
        .await
        .unwrap();
    let p = store
        .update_plugin(&p.id, update(&p, true, "change-configuration"))
        .await
        .unwrap();
    let poll = store
        .poll_desktop_host(
            &session.host_id,
            &session.lease_token,
            desktop_heartbeat(&p),
        )
        .await
        .unwrap();
    assert!(poll.commands.is_empty());
    assert!(
        store
            .desktop_command_result(&stale)
            .await
            .unwrap_err()
            .to_string()
            .contains("changed")
    );
    for i in 0..50 {
        store
            .enqueue_desktop_command("native.test_window", native_request(&format!("queued-{i}")))
            .await
            .unwrap();
    }
    assert!(
        store
            .enqueue_desktop_command("native.test_window", native_request("queue-overflow"))
            .await
            .unwrap_err()
            .to_string()
            .contains("full")
    );
}

#[tokio::test]
async fn desktop_heartbeats_do_not_break_domain_task_analysis_and_live_writes() {
    let (dir, store, _doc, _plugin) = fixture().await;
    let doc = revise(&store, "Maya: I'll review the rollout by 2026-10-20.").await;
    while store
        .process_next(&crate::Config::default(), dir.path())
        .await
        .unwrap()
    {}
    let task = store
        .snapshot(&Access::admin())
        .await
        .unwrap()
        .tasks
        .into_iter()
        .find(|t| t.status == "candidate")
        .unwrap();
    let model = dir.path().join("synthetic-model");
    std::fs::write(&model, "synthetic, never used for inference").unwrap();
    let (meeting, _) = store
        .start_live_meeting(LiveMeetingStart {
            transcription: None,
            project_id: String::new(),
            series_id: None,
            title: "Synthetic concurrency".into(),
            microphone_id: Some("synthetic-not-opened".into()),
            system_audio: false,
            whisper_command: "/usr/bin/true".into(),
            model_path: model.to_string_lossy().into_owned(),
            chunk_seconds: 10,
            approve_recording: true,
            idempotency_key: "concurrency-meeting".into(),
        })
        .await
        .unwrap();
    let session = store
        .register_desktop_host(PluginHostRegistration {
            host_id: "domain-heartbeat".into(),
            manifests: vec![desktop_manifest()],
        })
        .await
        .unwrap();
    let provider = store.plugin("thesaurus.native.test-window").await.unwrap();
    let heartbeat_store = store.clone();
    let heartbeat = tokio::spawn(async move {
        for _ in 0..80 {
            heartbeat_store
                .poll_desktop_host(
                    &session.host_id,
                    &session.lease_token,
                    desktop_heartbeat(&provider),
                )
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    });
    let mut writers = vec![];
    for i in 0..20 {
        let store = store.clone();
        let id = meeting.id.clone();
        let doc = doc.clone();
        writers.push(tokio::spawn(async move {
            store
                .add_live_note(
                    &id,
                    LiveMeetingNoteRequest {
                        kind: "note".into(),
                        text: format!("Synthetic note {i}"),
                        idempotency_key: format!("note-{i}"),
                    },
                )
                .await?;
            store.live_processing(&id, true, None).await?;
            store.live_processing(&id, false, None).await?;
            store.queue_analysis(&doc.id, "local", None).await?;
            store
                .append_observation(
                    ObservationRequest {
                        body: format!("Synthetic observation {i}"),
                        source_ids: vec![doc.source_id.clone()],
                        evidence: vec![],
                        idempotency_key: format!("domain-observation-{i}"),
                    },
                    &Access::admin(),
                )
                .await?;
            Ok::<_, anyhow::Error>(())
        }));
    }
    let task_store = store.clone();
    let task_writer = tokio::spawn(async move {
        let mut version = task.version;
        for i in 0..20 {
            let updated = task_store
                .update_task(
                    &task.id,
                    TaskUpdate {
                        expected_version: version,
                        status: if i % 2 == 0 { "accepted" } else { "completed" }.into(),
                    },
                )
                .await
                .unwrap();
            version = updated.version;
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        for writer in writers {
            writer.await.unwrap().unwrap();
        }
        task_writer.await.unwrap();
        heartbeat.await.unwrap();
    })
    .await
    .unwrap();
    let details = store.live_meeting_details(&meeting.id).await.unwrap();
    assert_eq!(details.notes.len(), 20);
    assert_eq!(details.session.pending_chunks, 0);
    assert!(!details.session.processing);
    assert_eq!(
        store.observations(&Access::admin()).await.unwrap().len(),
        20
    );
    // These Store methods never open a device; native resource startup belongs
    // to the service orchestrator, which this synthetic unit test does not run.
}

#[tokio::test]
async fn subprocess_protocol_bounds_fail_closed_and_release_the_worker() {
    let dir = tempfile::tempdir().unwrap();
    let invocation = PluginInvocation {
        api_version: 2,
        invocation_id: "synthetic-bounds".into(),
        plugin_id: "example.bounds".into(),
        plugin_version: "1.0.0".into(),
        method: "contribution".into(),
        configuration: json!({}),
        event: None,
        input: json!({}),
    };
    for (name, code, expected) in [
        ("malformed", "print('not json')", "expected"),
        (
            "identity",
            "import json; print(json.dumps({'api_version':2,'invocation_id':'wrong'}))",
            "identity mismatch",
        ),
        ("oversized", "print('x'*128001)", "response too large"),
        (
            "diagnostics",
            "import sys; sys.stderr.write('x'*8193)",
            "diagnostic limit",
        ),
        ("crash", "import sys; sys.exit(9)", "exited unsuccessfully"),
        ("slow", "import time; time.sleep(60)", "timed out"),
    ] {
        let script = dir.path().join(format!("{name}.py"));
        std::fs::write(
            &script,
            format!("import sys\nsys.stdin.readline()\n{code}\n"),
        )
        .unwrap();
        let started = tokio::time::Instant::now();
        let result = worker::invoke_subprocess(
            &PluginExecutable {
                program: "/usr/bin/python3".into(),
                args: vec![script.to_string_lossy().into_owned()],
            },
            &invocation,
            &dir.path().join("work"),
        )
        .await;
        let error = result.unwrap_err().to_string();
        assert!(error.contains(expected), "{name}: {error}");
        assert!(started.elapsed() < std::time::Duration::from_secs(13));
    }
    let script = dir.path().join("valid.py");
    std::fs::write(&script, "import sys,json\nv=json.loads(sys.stdin.readline())\nprint(json.dumps({'api_version':v['api_version'],'invocation_id':v['invocation_id']}))\n").unwrap();
    let executable = PluginExecutable {
        program: "/usr/bin/python3".into(),
        args: vec![script.to_string_lossy().into_owned()],
    };
    let mut oversized = invocation.clone();
    oversized.input = json!({"text":"x".repeat(512_001)});
    assert!(
        worker::invoke_subprocess(&executable, &oversized, &dir.path().join("work"))
            .await
            .unwrap_err()
            .to_string()
            .contains("input exceeds bounds")
    );
    assert_eq!(
        worker::invoke_subprocess(&executable, &invocation, &dir.path().join("work"))
            .await
            .unwrap()
            .invocation_id,
        invocation.invocation_id
    );
}

#[tokio::test]
async fn desktop_heartbeats_and_broad_domain_mutations_share_the_writer_safely() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let (_dir, store, doc, _plugin) = fixture().await;
    let session = store
        .register_desktop_host(PluginHostRegistration {
            host_id: "broad-domain-heartbeat".into(),
            manifests: vec![desktop_manifest()],
        })
        .await
        .unwrap();
    let provider = store.plugin("thesaurus.native.test-window").await.unwrap();
    let running = Arc::new(AtomicBool::new(true));
    let active = running.clone();
    let heartbeats = store.clone();
    let heartbeat = tokio::spawn(async move {
        let mut polls = 0;
        while active.load(Ordering::SeqCst) {
            heartbeats
                .poll_desktop_host(
                    &session.host_id,
                    &session.lease_token,
                    desktop_heartbeat(&provider),
                )
                .await
                .unwrap();
            polls += 1;
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
        polls
    });
    let mut writers = vec![];
    for i in 0..8 {
        let store = store.clone();
        let doc = doc.clone();
        writers.push(tokio::spawn(async move {
            let key = |label| format!("broad-{i}-{label}");
            let evidence = Evidence {
                document_id: doc.id.clone(),
                revision: doc.revision,
                line: 1,
                quote: doc.body.clone(),
            };
            let project = store
                .create_brief(BriefCreateRequest {
                    name: format!("Synthetic project {i}"),
                    source_ids: vec![doc.source_id.clone()],
                    idempotency_key: key("project"),
                })
                .await?;
            let thread = store
                .create_thread(ThreadCreateRequest {
                    project_id: project.id.clone(),
                    title: "Synthetic thread".into(),
                    source_ids: vec![doc.source_id.clone()],
                    idempotency_key: key("thread"),
                })
                .await?;
            store
                .append_thread_entry(
                    &thread.id,
                    ThreadEntryRequest {
                        expected_version: thread.version,
                        kind: "checkpoint".into(),
                        body: "Synthetic thread note".into(),
                        record_id: None,
                        evidence: vec![evidence.clone()],
                        idempotency_key: key("entry"),
                    },
                )
                .await?;
            store
                .create_meeting_series(MeetingCreateRequest {
                    project_id: project.id.clone(),
                    name: "Synthetic meeting".into(),
                    cadence: "Weekly".into(),
                    idempotency_key: key("meeting"),
                })
                .await?;
            let observation = store
                .append_observation(
                    ObservationRequest {
                        body: "Synthetic mutable memory".into(),
                        source_ids: vec![doc.source_id.clone()],
                        evidence: vec![evidence.clone()],
                        idempotency_key: key("observation"),
                    },
                    &Access::admin(),
                )
                .await?;
            store
                .revise_memory(
                    &observation.id,
                    MemoryReviseRequest {
                        expected_version: 0,
                        action: "correct".into(),
                        body: Some("Corrected synthetic memory".into()),
                        restore_version: None,
                        reason: "Synthetic review".into(),
                        idempotency_key: key("correction"),
                    },
                    &Access::admin(),
                )
                .await?;
            store
                .create_person(PersonCreateRequest {
                    project_id: project.id.clone(),
                    display_name: "Synthetic person".into(),
                    identity_note: "Only a fixture".into(),
                    evidence: vec![evidence.clone()],
                    idempotency_key: key("person"),
                })
                .await?;
            store
                .create_dated_claim(ClaimCreateRequest {
                    project_id: project.id.clone(),
                    subject: "Synthetic fixture".into(),
                    predicate: "status".into(),
                    value: "in review".into(),
                    certainty: "asserted".into(),
                    valid_from: "2026-01-01".into(),
                    valid_until: None,
                    evidence: vec![evidence.clone()],
                    note: String::new(),
                    idempotency_key: key("claim"),
                })
                .await?;
            store
                .create_anchored_note(NoteCreateRequest {
                    evidence: evidence.clone(),
                    body: "Synthetic anchored note".into(),
                    emphasis: true,
                    idempotency_key: key("anchor"),
                })
                .await?;
            store
                .create_question_cue(CueCreateRequest {
                    project_id: project.id.clone(),
                    kind: "question".into(),
                    text: "Synthetic question?".into(),
                    evidence: vec![evidence.clone()],
                    note: String::new(),
                    idempotency_key: key("cue"),
                })
                .await?;
            store
                .create_knowledge_gap(GapCreateRequest {
                    project_id: project.id.clone(),
                    question: "What synthetic evidence is missing?".into(),
                    missing_source: "Synthetic reference".into(),
                    required_source_ids: vec![doc.source_id.clone()],
                    idempotency_key: key("gap"),
                })
                .await?;
            store
                .create_waiting(WaitingCreateRequest {
                    project_id: project.id.clone(),
                    task_id: None,
                    title: "Synthetic follow-up".into(),
                    person: "Synthetic person".into(),
                    requested_on: "2026-01-01".into(),
                    follow_up_on: "2026-01-02".into(),
                    evidence: vec![evidence.clone()],
                    note: String::new(),
                    idempotency_key: key("waiting"),
                })
                .await?;
            store
                .create_commitment(CommitmentCreateRequest {
                    project_id: project.id.clone(),
                    task_id: None,
                    title: "Synthetic promise".into(),
                    owner: "Synthetic person".into(),
                    promised_on: "2026-01-01".into(),
                    due: None,
                    evidence: vec![evidence],
                    note: String::new(),
                    idempotency_key: key("commitment"),
                })
                .await?;
            store.refresh_discovery(&project.id).await?;
            store.refresh_procedures(&project.id).await?;
            store.reconcile_notifications().await?;
            store
                .set_usage_policy(
                    &format!("synthetic-{i}"),
                    UsagePolicyRequest {
                        expected_version: 0,
                        daily_limit: 10,
                        interactive_reserve: 1,
                        background_paused: false,
                        fallback_connection: None,
                        idempotency_key: key("usage"),
                    },
                    10,
                )
                .await?;
            Ok::<_, anyhow::Error>(project.id)
        }));
    }
    let projects = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut projects = vec![];
        for writer in writers {
            projects.push(writer.await.unwrap().unwrap());
        }
        projects
    })
    .await
    .unwrap();
    running.store(false, Ordering::SeqCst);
    assert!(heartbeat.await.unwrap() > 0);
    assert_eq!(projects.len(), 8);
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM memory_review_revisions WHERE version=1")
            .fetch_one(&store.pool)
            .await
            .unwrap();
    assert_eq!(count, 8);
    assert_eq!(
        store
            .observations(&Access::admin())
            .await
            .unwrap()
            .into_iter()
            .filter(|o| o.body == "Corrected synthetic memory")
            .count(),
        8
    );
    for project in projects {
        assert_eq!(
            store
                .brief(&project, &Access::admin())
                .await
                .unwrap()
                .name
                .starts_with("Synthetic project"),
            true
        );
    }
}
