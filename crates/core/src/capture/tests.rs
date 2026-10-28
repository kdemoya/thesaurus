use super::*;
use crate::plugins::{
    native,
    registry::{CapabilityProvider, ProviderFuture},
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn window() -> CaptureWindow {
    CaptureWindow {
        window_id: 42,
        process_id: 99,
        app: "Synthetic fixture".into(),
        title: "Capture test".into(),
    }
}
struct Screen {
    pool: sqlx::SqlitePool,
    calls: Arc<AtomicUsize>,
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    gate: Arc<std::sync::atomic::AtomicBool>,
}
impl CapabilityProvider for Screen {
    fn manifest(&self) -> PluginManifest {
        native::manifest(
            "example.synthetic-screen",
            "Synthetic screen",
            "native.screen_capture",
            &["windows"],
            PluginHost::Service,
        )
    }
    fn health(&self) -> Result<()> {
        Ok(())
    }
    fn invoke<'a>(&'a self, command: &'a str, _input: Value) -> ProviderFuture<'a> {
        Box::pin(async move {
            assert_eq!(command, "windows");
            self.calls.fetch_add(1, Ordering::SeqCst);
            // This must be able to acquire the sole pool connection and writer.
            let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
            sqlx::query(
                "INSERT INTO events(kind,record_id) VALUES('synthetic-provider','enumeration')",
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            if self.gate.load(Ordering::SeqCst) {
                self.entered.notify_one();
                self.release.notified().await;
            }
            Ok(json!([window()]))
        })
    }
}

#[tokio::test]
async fn activity_provider_calls_release_pool_and_revalidate_after_enumeration() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("capture.sqlite3");
    let mut store = Store::open(&path).await.unwrap();
    let document = store
        .import(ImportRequest {
            source_name: "Synthetic capture source".into(),
            kind: "markdown".into(),
            uri: "fixture://capture-test".into(),
            title: "Synthetic capture source".into(),
            body: "Synthetic text only".into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document;
    let project = store
        .create_brief(BriefCreateRequest {
            name: "Synthetic capture project".into(),
            source_ids: vec![document.source_id],
            idempotency_key: "project".into(),
        })
        .await
        .unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .busy_timeout(std::time::Duration::from_secs(2));
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap();
    store.pool.close().await;
    store.pool = pool.clone();
    let calls = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
    store
        .providers
        .register(Arc::new(Screen {
            pool,
            calls: calls.clone(),
            entered: entered.clone(),
            release: release.clone(),
            gate: gate.clone(),
        }))
        .unwrap();
    store.seed_native_providers().await.unwrap();
    let capability = store
        .plugin_capabilities()
        .await
        .unwrap()
        .into_iter()
        .find(|c| c.capability == "native.screen_capture")
        .unwrap();
    store
        .select_plugin_provider(
            "native.screen_capture",
            PluginProviderSelectionRequest {
                expected_version: capability.selection_version,
                provider_id: "example.synthetic-screen".into(),
                idempotency_key: "select".into(),
            },
        )
        .await
        .unwrap();
    let request = ActivityStartRequest {
        project_id: project.id,
        window: window(),
        interval_seconds: 15,
        retention_hours: 1,
        max_frames: 2,
        use_local_ocr: false,
        approve_capture: true,
        idempotency_key: "start".into(),
    };
    let (session, launch) = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        store.start_activity(request.clone()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(launch);
    assert!(!store.start_activity(request).await.unwrap().1);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "replay does not enumerate again"
    );
    let (paused, _) = store
        .control_activity(
            &session.id,
            ActivityControlRequest {
                expected_version: session.version,
                action: "pause".into(),
                approve_capture: false,
                idempotency_key: "pause".into(),
            },
        )
        .await
        .unwrap();
    let resume = ActivityControlRequest {
        expected_version: paused.version,
        action: "resume".into(),
        approve_capture: true,
        idempotency_key: "resume".into(),
    };
    let (resumed, launch) = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        store.control_activity(&session.id, resume.clone()),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(launch);
    assert!(!store.control_activity(&session.id, resume).await.unwrap().1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    let (paused, _) = store
        .control_activity(
            &session.id,
            ActivityControlRequest {
                expected_version: resumed.version,
                action: "pause".into(),
                approve_capture: false,
                idempotency_key: "pause-again".into(),
            },
        )
        .await
        .unwrap();
    gate.store(true, Ordering::SeqCst);
    let other = store.clone();
    let id = session.id.clone();
    let expected = paused.version;
    let pending = tokio::spawn(async move {
        other
            .control_activity(
                &id,
                ActivityControlRequest {
                    expected_version: expected,
                    action: "resume".into(),
                    approve_capture: true,
                    idempotency_key: "racing-resume".into(),
                },
            )
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3), entered.notified())
        .await
        .unwrap();
    let (stopped, _) = store
        .control_activity(
            &session.id,
            ActivityControlRequest {
                expected_version: paused.version,
                action: "stop".into(),
                approve_capture: false,
                idempotency_key: "stop-during-enumeration".into(),
            },
        )
        .await
        .unwrap();
    release.notify_one();
    let error = pending.await.unwrap().unwrap_err().to_string();
    assert!(error.contains("conflict"), "{error}");
    assert_eq!(
        store.activity_session(&session.id).await.unwrap().version,
        stopped.version
    );
    assert_eq!(stopped.state, "stopped");
}
