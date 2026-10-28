use super::*;
use crate::{Config, config::Connection};
use serde_json::json;

fn connection(id: &str) -> Connection {
    Connection {
        id: id.into(),
        kind: if id == "local" {
            "local"
        } else {
            "openai_compatible"
        }
        .into(),
        model: "fixture".into(),
        endpoint: Some("https://fixture.invalid/v1".into()),
        command: None,
        args: vec![],
        token_env: None,
        credential_ref: None,
        daily_limit: 20,
    }
}
async fn fixture(store: &Store, body: &str) -> (LiveMeeting, Document) {
    let id = uuid::Uuid::new_v4().to_string();
    let doc = store
        .import(ImportRequest {
            source_name: "Fixture meeting".into(),
            kind: "transcript".into(),
            uri: format!("fixture://meeting/{id}"),
            title: "Fixture meeting".into(),
            body: body.into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document;
    let session = LiveMeeting {
        transcription_assets: None,
        transcription: None,
        detected_language: None,
        use_bundled_transcription: false,
        id: id.clone(),
        version: 1,
        project_id: String::new(),
        series_id: None,
        title: "Fixture meeting".into(),
        microphone_id: Some("system-default".into()),
        system_audio: false,
        whisper_command: String::new(),
        model_path: String::new(),
        chunk_seconds: 20,
        state: "recording".into(),
        recorded_ms: 1000,
        active_since: None,
        transcript: body.into(),
        source_id: Some(doc.source_id.clone()),
        document_id: Some(doc.id.clone()),
        chunks_completed: 1,
        processing: true,
        pending_chunks: 1,
        error: None,
        created_at: now(),
        updated_at: now(),
    };
    let mut tx = store.pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO live_meetings(id,version,state,record,created_at) VALUES(?,1,'recording',?,?)",
    )
    .bind(&id)
    .bind(serde_json::to_string(&session).unwrap())
    .bind(now())
    .execute(&mut *tx)
    .await
    .unwrap();
    queue_new_meeting(&mut tx, &id).await.unwrap();
    tx.commit().await.unwrap();
    (session, doc)
}
async fn stop(store: &Store, s: &LiveMeeting) {
    let (s, _) = store
        .control_live_meeting(
            &s.id,
            LiveMeetingControl {
                expected_version: s.version,
                action: "stop".into(),
                approve_recording: false,
                idempotency_key: format!("stop:{}", s.id),
            },
        )
        .await
        .unwrap();
    store.finish_live_control(&s.id, s.version).await.unwrap();
}
async fn reserve(
    store: &Store,
    s: &LiveMeeting,
    doc: &Document,
    c: &Connection,
) -> (RuntimeRun, Option<String>, String) {
    let j = store
        .meeting_processing_state(&s.id)
        .await
        .unwrap()
        .unwrap();
    store
        .create_managed_meeting_summary_runtime(
            &s.id,
            j.generation,
            doc.revision,
            c,
            c.id != "local",
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn automatic_summary_waits_for_stop_and_final_chunk_and_publishes_cited_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("test.sqlite3")).await.unwrap();
    let (s,doc)=fixture(&store,"[00:00 · microphone] Maya: I'll share the checklist by 2026-11-01.\n[00:20 · system] Decision: Use a staged rollout.\n[00:40 · microphone] Who will notify support?").await;
    assert!(store.pending_meeting_processing().await.unwrap().is_empty());
    stop(&store, &s).await;
    assert!(store.pending_meeting_processing().await.unwrap().is_empty());
    store.live_processing(&s.id, false, None).await.unwrap();
    assert_eq!(store.pending_meeting_processing().await.unwrap().len(), 1);
    let (run, secret, _) = reserve(&store, &s, &doc, &connection("local")).await;
    assert!(secret.is_some());
    let output = store.local_meeting_summary_output(&run.id).await.unwrap();
    store
        .runtime_patch(
            &run.id,
            json!({"status":"completed","output":output,"stop_reason":"end_turn"}),
        )
        .await
        .unwrap();
    let summaries = store.meeting_summaries(&s.id).await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].status, "draft");
    let item = &summaries[0]
        .sections
        .iter()
        .find(|s| s.kind == "action_items")
        .unwrap()
        .items[0];
    assert_eq!(item.owner.as_deref(), Some("Maya"));
    assert_eq!(item.due.as_deref(), Some("2026-11-01"));
    assert_eq!(item.evidence.line, 1);
    assert_eq!(item.evidence.revision, doc.revision);
    assert!(item.task_id.is_some());
    let statuses: Vec<String> = sqlx::query_scalar("SELECT status FROM tasks WHERE document_id=?")
        .bind(&doc.id)
        .fetch_all(&store.pool)
        .await
        .unwrap();
    assert!(statuses.iter().all(|s| s == "candidate"));
    assert_eq!(
        store
            .meeting_processing_state(&s.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "complete"
    );
    assert!(store.pending_meeting_processing().await.unwrap().is_empty());
    assert!(
        !store
            .sources(&Access::admin())
            .await
            .unwrap()
            .iter()
            .find(|s| s.id == doc.source_id)
            .unwrap()
            .allow_external
    );
}

#[tokio::test]
async fn concurrent_delivery_is_once_and_restart_does_not_resend_a_reserved_run() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("test.sqlite3")).await.unwrap();
    let (s, doc) = fixture(&store, "Action: Review the checklist.").await;
    stop(&store, &s).await;
    store.live_processing(&s.id, false, None).await.unwrap();
    let c = connection("local");
    let (a, b) = tokio::join!(
        store.create_managed_meeting_summary_runtime(&s.id, 1, doc.revision, &c, false),
        store.create_managed_meeting_summary_runtime(&s.id, 1, doc.revision, &c, false)
    );
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_eq!(a.0.id, b.0.id);
    assert_ne!(a.1.is_some(), b.1.is_some());
    store.recover_runtime().await.unwrap();
    store.recover_meeting_summaries().await.unwrap();
    assert_eq!(
        store
            .meeting_processing_state(&s.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "failed"
    );
    assert!(store.pending_meeting_processing().await.unwrap().is_empty());
    let request = MeetingProcessingRequest {
        idempotency_key: "retry-once".into(),
    };
    let retry = store
        .request_meeting_processing(&s.id, request.clone())
        .await
        .unwrap();
    assert_eq!(retry.generation, 2);
    assert_eq!(
        store
            .request_meeting_processing(&s.id, request)
            .await
            .unwrap()
            .generation,
        2
    );
    assert!(
        store
            .create_managed_meeting_summary_runtime(&s.id, 1, doc.revision, &c, false)
            .await
            .unwrap()
            .1
            .is_none()
    );
    let fresh = reserve(&store, &s, &doc, &c).await;
    assert!(fresh.1.is_some());
    assert_ne!(a.0.id, fresh.0.id);
}

#[tokio::test]
async fn disabling_automation_clears_queued_work_and_destination_is_pinned_at_recording_start() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("test.sqlite3")).await.unwrap();
    let c = connection("meeting-model");
    let config = Config {
        connections: vec![c.clone()],
    };
    store
        .save_onboarding(
            OnboardingSave {
                expected_version: 0,
                goal: "context".into(),
                step: 4,
                primary_connection: c.id.clone(),
                status: "complete".into(),
                memory_grant_id: None,
                idempotency_key: "setup".into(),
            },
            &config,
        )
        .await
        .unwrap();
    let (s, _) = fixture(&store, "We discussed rollout timing.").await;
    assert_eq!(
        store
            .meeting_processing_state(&s.id)
            .await
            .unwrap()
            .unwrap()
            .connection,
        c.id
    );
    let prefs = store
        .save_meeting_processing_settings(
            MeetingProcessingSettingsRequest {
                settings: MeetingProcessingSettings {
                    version: 0,
                    automatic_summaries: false,
                    connection: Some("local".into()),
                },
                idempotency_key: "off".into(),
            },
            &config,
        )
        .await
        .unwrap();
    let state = store
        .meeting_processing_state(&s.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(state.status, "disabled");
    assert_eq!(state.connection, c.id);
    stop(&store, &s).await;
    store.live_processing(&s.id, false, None).await.unwrap();
    assert!(store.pending_meeting_processing().await.unwrap().is_empty());
    let (other, _) = fixture(&store, "Another meeting.").await;
    assert_eq!(
        store
            .meeting_processing_state(&other.id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "disabled"
    );
    assert!(
        store
            .save_meeting_processing_settings(
                MeetingProcessingSettingsRequest {
                    settings: MeetingProcessingSettings {
                        version: prefs.version - 1,
                        ..prefs
                    },
                    idempotency_key: "stale".into()
                },
                &config
            )
            .await
            .unwrap_err()
            .to_string()
            .starts_with("conflict:")
    );
}

#[tokio::test]
async fn saved_summary_destination_never_enables_general_source_sharing_and_can_be_revoked() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("test.sqlite3")).await.unwrap();
    let c = connection("meeting-model");
    let config = Config {
        connections: vec![c.clone()],
    };
    store
        .save_meeting_processing_settings(
            MeetingProcessingSettingsRequest {
                settings: MeetingProcessingSettings {
                    version: 0,
                    automatic_summaries: true,
                    connection: Some(c.id.clone()),
                },
                idempotency_key: "model".into(),
            },
            &config,
        )
        .await
        .unwrap();
    let (s, doc) = fixture(&store, "Decision: Ship in phases.").await;
    stop(&store, &s).await;
    store.live_processing(&s.id, false, None).await.unwrap();
    let run = reserve(&store, &s, &doc, &c).await.0;
    assert!(
        store
            .meeting_summary_authority(&run.id, &c.id, true)
            .await
            .unwrap()
            .is_some()
    );
    assert!(
        store
            .check_source_destination(&doc.source_id, &c.id, true)
            .await
            .is_err()
    );
    assert!(
        store
            .meeting_summary_authority(&run.id, "another-model", true)
            .await
            .is_err()
    );
    store
        .update_source_policy(
            &doc.source_id,
            SourcePolicyUpdate {
                expected_version: 0,
                external_enabled: false,
                denied_connections: vec![c.id.clone()],
                denied_grants: vec![],
                idempotency_key: "revoke".into(),
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    assert!(
        store
            .meeting_summary_authority(&run.id, &c.id, true)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn invalid_generated_citations_fail_without_publishing_actions_or_automatic_retry() {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("test.sqlite3")).await.unwrap();
    let (s, doc) = fixture(&store, "We discussed the release.").await;
    stop(&store, &s).await;
    store.live_processing(&s.id, false, None).await.unwrap();
    let run = reserve(&store, &s, &doc, &connection("local")).await.0;
    let output=json!({"key_points":[],"decisions":[],"open_questions":[],"risks":[],"action_items":[{"text":"Invented action","owner":null,"due":null,"line":1,"quote":"Invented source"}]}).to_string();
    store
        .runtime_patch(&run.id, json!({"status":"completed","output":output}))
        .await
        .unwrap();
    assert_eq!(
        store.meeting_summaries(&s.id).await.unwrap()[0].status,
        "failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tasks WHERE document_id=?")
            .bind(&doc.id)
            .fetch_one(&store.pool)
            .await
            .unwrap(),
        0
    );
    assert!(store.pending_meeting_processing().await.unwrap().is_empty());
}
