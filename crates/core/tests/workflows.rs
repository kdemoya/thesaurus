use thesaurus_contracts::*;
use thesaurus_core::{Config, Store};

fn input(uri: &str, body: &str, external: bool) -> ImportRequest {
    ImportRequest {
        source_name: "Project sync".into(),
        kind: "transcript".into(),
        uri: uri.into(),
        title: "Launch planning".into(),
        body: body.into(),
        allow_external: external,
    }
}
async fn setup() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(&dir.path().join("test.sqlite")).await.unwrap();
    (dir, store)
}

#[tokio::test]
async fn revisions_and_jobs_are_idempotent_and_old_citations_remain_readable() {
    let (dir, store) = setup().await;
    let first = store
        .import(input(
            "test://meeting",
            "Maya: I'll review the rollout by 2026-10-20.",
            false,
        ))
        .await
        .unwrap();
    let duplicate = store
        .import(input(
            "test://meeting",
            "Maya: I'll review the rollout by 2026-10-20.",
            false,
        ))
        .await
        .unwrap();
    assert!(!duplicate.changed);
    assert_eq!(first.document, duplicate.document);
    store
        .process_next(&Config::default(), dir.path())
        .await
        .unwrap();
    let snap = store.snapshot(&Access::admin()).await.unwrap();
    assert_eq!(snap.jobs.len(), 1);
    assert_eq!(snap.tasks.len(), 1);
    assert_eq!(snap.tasks[0].owner.as_deref(), Some("Maya"));
    let accepted = store
        .update_task(
            &snap.tasks[0].id,
            TaskUpdate {
                status: "accepted".into(),
                expected_version: 1,
            },
        )
        .await
        .unwrap();
    let revised = store
        .import(input(
            "test://meeting",
            "Maya: I'll review the revised rollout by 2026-10-22.",
            false,
        ))
        .await
        .unwrap();
    assert_eq!(revised.document.revision, 2);
    let historical = store
        .document(&first.document.id, Some(1), &Access::admin())
        .await
        .unwrap();
    assert_eq!(historical.body, first.document.body);
    assert_eq!(
        store.snapshot(&Access::admin()).await.unwrap().tasks[0].status,
        "accepted"
    );
    assert!(
        store
            .update_task(
                &accepted.id,
                TaskUpdate {
                    status: "completed".into(),
                    expected_version: 1
                }
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn source_scopes_and_destination_policy_filter_before_search() {
    let (_dir, store) = setup().await;
    let private = store
        .import(input(
            "test://private",
            "RareKeyword private roadmap",
            false,
        ))
        .await
        .unwrap()
        .document;
    let shared = store
        .import(input("test://shared", "RareKeyword shared launch", true))
        .await
        .unwrap()
        .document;
    let (grant, token) = store
        .create_grant(
            "cloud-agent".into(),
            vec![private.source_id.clone(), shared.source_id.clone()],
            true,
            true,
        )
        .await
        .unwrap();
    let access = store.authenticate(&token).await.unwrap();
    let hits = store
        .search(
            &SearchRequest {
                query: "RareKeyword".into(),
                source_id: None,
                limit: Some(1),
            },
            &access,
        )
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].document_id, shared.id);
    assert!(store.document(&private.id, None, &access).await.is_err());
    store.revoke_grant(&grant.id).await.unwrap();
    assert!(store.authenticate(&token).await.is_err());
}

#[tokio::test]
async fn observation_contributions_are_attributed_scoped_and_idempotent() {
    let (_dir, store) = setup().await;
    let doc = store
        .import(input("test://shared", "Action: Review the API.", true))
        .await
        .unwrap()
        .document;
    let (_, token) = store
        .create_grant("researcher".into(), vec![doc.source_id.clone()], true, true)
        .await
        .unwrap();
    let access = store.authenticate(&token).await.unwrap();
    let req = ObservationRequest {
        body: "The API needs review".into(),
        source_ids: vec![doc.source_id.clone()],
        evidence: vec![Evidence {
            document_id: doc.id,
            revision: 1,
            line: 1,
            quote: doc.body,
        }],
        idempotency_key: "run-123".into(),
    };
    let first = store
        .append_observation(req.clone(), &access)
        .await
        .unwrap();
    let second = store
        .append_observation(req.clone(), &access)
        .await
        .unwrap();
    assert_eq!(first, second);
    assert!(first.contributor.starts_with("researcher ("));
    let mut conflict = req.clone();
    conflict.body = "A different claim".into();
    assert!(store.append_observation(conflict, &access).await.is_err());
    let mut invalid = req.clone();
    invalid.idempotency_key = "run-456".into();
    invalid.source_ids = vec!["not-permitted".into()];
    assert!(store.append_observation(invalid, &access).await.is_err());
    let mut invalid = req;
    invalid.evidence[0].quote = "Invented evidence".into();
    assert!(store.append_observation(invalid, &access).await.is_err());
    assert_eq!(store.observations(&access).await.unwrap().len(), 1);
}

#[tokio::test]
async fn revisions_supersede_unreviewed_suggestions_and_search_handles_unicode() {
    let (dir, store) = setup().await;
    let doc = store
        .import(input(
            "test://notes",
            "Action: Review café capacity.",
            false,
        ))
        .await
        .unwrap()
        .document;
    store
        .process_next(&Config::default(), dir.path())
        .await
        .unwrap();
    store
        .import(input(
            "test://notes",
            "Action: Review café capacity tomorrow.",
            false,
        ))
        .await
        .unwrap();
    let old = store
        .snapshot(&Access::admin())
        .await
        .unwrap()
        .tasks
        .remove(0);
    assert_eq!(old.status, "superseded");
    assert!(
        store
            .update_task(
                &old.id,
                TaskUpdate {
                    status: "accepted".into(),
                    expected_version: old.version
                }
            )
            .await
            .is_err()
    );
    let hits = store
        .search(
            &SearchRequest {
                query: "café \"".into(),
                source_id: Some(doc.source_id),
                limit: Some(3),
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
}

#[tokio::test]
async fn context_is_bounded_and_agent_observations_do_not_create_extraction_jobs() {
    let (_dir, store) = setup().await;
    let doc = store
        .import(input(
            "test://notes",
            &format!("Action: Review launch {}", "long context ".repeat(800)),
            false,
        ))
        .await
        .unwrap()
        .document;
    store
        .append_observation(
            ObservationRequest {
                body: "Agent hypothesis".into(),
                source_ids: vec![doc.source_id],
                evidence: vec![],
                idempotency_key: "no-loop".into(),
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    // Budgets now include scope/provenance metadata, not only excerpts. Refuse
    // a budget that cannot hold that metadata rather than silently exceeding it.
    assert!(
        store
            .context("launch", 500, &Access::admin())
            .await
            .unwrap_err()
            .to_string()
            .contains("scope metadata and query exceed context budget")
    );
    let ctx = store
        .context("launch", 2000, &Access::admin())
        .await
        .unwrap();
    let wire = serde_json::to_string(&ctx).unwrap();
    assert!(wire.chars().count() <= 2000);
    assert_eq!(ctx.estimated_tokens, wire.len().div_ceil(4));
    assert_eq!(
        store.snapshot(&Access::admin()).await.unwrap().jobs.len(),
        1
    );
}

#[tokio::test]
async fn daily_limit_reserves_queued_runs_and_repeated_requests_do_not_spend_again() {
    let (_dir, store) = setup().await;
    let one = store
        .import(input("test://one", "First document", true))
        .await
        .unwrap()
        .document;
    let two = store
        .import(input("test://two", "Second document", true))
        .await
        .unwrap()
        .document;
    let job = store
        .queue_analysis(&one.id, "my-model", Some(1))
        .await
        .unwrap();
    assert_eq!(
        job.id,
        store
            .queue_analysis(&one.id, "my-model", Some(1))
            .await
            .unwrap()
            .id
    );
    assert!(
        store
            .queue_analysis(&two.id, "my-model", Some(1))
            .await
            .is_err()
    );
}
