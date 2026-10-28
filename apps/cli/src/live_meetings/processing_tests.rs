use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use thesaurus_core::{Config, Store, config::Connection};

const TRANSCRIPT: &str = "[00:00 · microphone] Maya: I'll share the checklist by 2026-11-01.\n[00:20 · system] Decision: Use a staged rollout.";

async fn fixture(
    dir: &std::path::Path,
    connection: Option<Connection>,
) -> (App, String, sqlx::SqlitePool) {
    let path = dir.join("test.sqlite3");
    let store = Store::open(&path).await.unwrap();
    let doc = store
        .import(ImportRequest {
            source_name: "Synthetic meeting".into(),
            kind: "transcript".into(),
            uri: "fixture://automatic-meeting".into(),
            title: "Synthetic meeting".into(),
            body: TRANSCRIPT.into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document;
    let c = connection
        .as_ref()
        .map(|c| c.id.clone())
        .unwrap_or("local".into());
    let record = json!({"id":"synthetic-meeting","version":1,"project_id":"","series_id":null,"title":"Synthetic meeting","microphone_id":"system-default","system_audio":false,"whisper_command":"","model_path":"","chunk_seconds":20,"state":"stopped","recorded_ms":1000,"active_since":null,"transcript":TRANSCRIPT,"source_id":doc.source_id,"document_id":doc.id,"chunks_completed":1,"processing":true,"pending_chunks":1,"error":null,"created_at":"2026-10-27T00:00:00Z","updated_at":"2026-10-27T00:00:00Z"});
    let pool = sqlx::SqlitePool::connect(&format!("sqlite://{}", path.display()))
        .await
        .unwrap();
    sqlx::query("INSERT INTO live_meetings(id,version,state,record,created_at) VALUES('synthetic-meeting',1,'stopped',?,'2026-10-27T00:00:00Z')").bind(record.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO meeting_processing_jobs(session_id,connection,status) VALUES('synthetic-meeting',?,'waiting')").bind(c).execute(&pool).await.unwrap();
    let mut app = App::new(
        store,
        Config {
            connections: connection.into_iter().collect(),
        },
        0,
    );
    app.data_dir = dir.to_path_buf();
    (app, "synthetic-meeting".into(), pool)
}
async fn finished(app: &App, id: &str) -> MeetingProcessingState {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let s = app
                .store
                .meeting_processing_state(id)
                .await
                .unwrap()
                .unwrap();
            if matches!(s.status.as_str(), "complete" | "failed") {
                return s;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn automatic_worker_summarizes_locally_without_a_screen_or_a_processing_request() {
    let dir = tempfile::tempdir().unwrap();
    let (app, id, _pool) = fixture(dir.path(), None).await;
    process_completed_meetings(&app).await.unwrap();
    assert!(app.store.meeting_summaries(&id).await.unwrap().is_empty());
    app.store.live_processing(&id, false, None).await.unwrap();
    let (stop, receiver) = watch::channel(false);
    let worker = start_processing_worker(&app, receiver);
    let state = finished(&app, &id).await;
    assert_eq!(state.status, "complete");
    let summaries = app.store.meeting_summaries(&id).await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].connection, "local");
    assert_eq!(
        summaries[0]
            .sections
            .iter()
            .find(|s| s.kind == "action_items")
            .unwrap()
            .items
            .len(),
        1
    );
    process_completed_meetings(&app).await.unwrap();
    assert_eq!(app.store.meeting_summaries(&id).await.unwrap().len(), 1);
    stop.send(true).unwrap();
    worker.await.unwrap();
}

async fn api_fixture(fail: bool) -> (Connection, Arc<AtomicUsize>, JoinHandle<()>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let router=axum::Router::new().route("/v1/chat/completions",axum::routing::post(move |Json(body):Json<serde_json::Value>|{
        let count=count.clone();async move {
            count.fetch_add(1,Ordering::SeqCst);
            assert!(body["messages"][0]["content"].as_str().unwrap().contains(TRANSCRIPT.lines().next().unwrap()));
            if fail {return (axum::http::StatusCode::SERVICE_UNAVAILABLE,Json(json!({"error":"Synthetic provider unavailable"})));}
            let summary=json!({"key_points":[],"decisions":[{"text":"Use a staged rollout.","owner":null,"due":null,"line":2,"quote":TRANSCRIPT.lines().nth(1).unwrap()}],"action_items":[],"open_questions":[],"risks":[]});
            (axum::http::StatusCode::OK,Json(json!({"choices":[{"message":{"content":summary.to_string()},"finish_reason":"stop"}]})))
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (
        Connection {
            id: "synthetic-model".into(),
            kind: "openai_compatible".into(),
            model: "fixture".into(),
            endpoint: Some(format!("http://127.0.0.1:{port}/v1")),
            command: None,
            args: vec![],
            token_env: None,
            credential_ref: None,
            daily_limit: 2,
        },
        calls,
        server,
    )
}
#[tokio::test]
async fn automatic_model_summary_is_sent_once_after_final_transcription() {
    let (c, calls, server) = api_fixture(false).await;
    let dir = tempfile::tempdir().unwrap();
    let (app, id, _pool) = fixture(dir.path(), Some(c)).await;
    process_completed_meetings(&app).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    app.store.live_processing(&id, false, None).await.unwrap();
    process_completed_meetings(&app).await.unwrap();
    process_completed_meetings(&app).await.unwrap();
    assert_eq!(finished(&app, &id).await.status, "complete");
    process_completed_meetings(&app).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        app.store.meeting_summaries(&id).await.unwrap()[0]
            .sections
            .iter()
            .find(|s| s.kind == "decisions")
            .unwrap()
            .items
            .len(),
        1
    );
    server.abort();
}
#[tokio::test]
async fn failed_automatic_delivery_is_visible_and_never_resubmitted_by_worker_ticks() {
    let (c, calls, server) = api_fixture(true).await;
    let dir = tempfile::tempdir().unwrap();
    let (app, id, _pool) = fixture(dir.path(), Some(c)).await;
    app.store.live_processing(&id, false, None).await.unwrap();
    process_completed_meetings(&app).await.unwrap();
    let state = finished(&app, &id).await;
    assert_eq!(state.status, "failed");
    assert!(state.error.is_some());
    for _ in 0..3 {
        process_completed_meetings(&app).await.unwrap();
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    server.abort();
}
