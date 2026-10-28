use super::*;
use tempfile::TempDir;

async fn fixture() -> (TempDir, Store, Document) {
    let dir = TempDir::new().unwrap();
    let store = Store::open(&dir.path().join("bragbook.sqlite"))
        .await
        .unwrap();
    let doc = revise(&store, "Initial notes").await;
    let p = store.plugin(ID).await.unwrap();
    assert!(!p.enabled);
    store
        .update_plugin(
            ID,
            PluginUpdateRequest {
                expected_version: p.version,
                enabled: true,
                grant: PluginPermissions {
                    operations: vec!["sources.read".into()],
                    source_ids: vec![doc.source_id.clone()],
                    ..Default::default()
                },
                configuration: json!({}),
                manifest: None,
                executable: None,
                idempotency_key: "enable".into(),
            },
        )
        .await
        .unwrap();
    (dir, store, doc)
}
async fn revise(store: &Store, body: &str) -> Document {
    store
        .import(ImportRequest {
            source_name: "Synthetic achievements".into(),
            kind: "markdown".into(),
            uri: "fixture://brags".into(),
            title: "Release notes".into(),
            body: body.into(),
            allow_external: false,
        })
        .await
        .unwrap()
        .document
}
async fn all(store: &Store) -> Vec<BragbookEntry> {
    store
        .bragbook_entries(
            BragbookQuery {
                status: Some("all".into()),
                ..Default::default()
            },
            &Access::admin(),
        )
        .await
        .unwrap()
        .items
}
fn review(item: &BragbookEntry, status: &str, key: &str) -> BragbookReview {
    BragbookReview {
        expected_version: item.version,
        status: status.into(),
        title: item.title.clone(),
        contribution: "I implemented the import workflow.".into(),
        impact: "Reduced a measured 10-minute process to 2 minutes.".into(),
        occurred_on: None,
        idempotency_key: key.into(),
    }
}
async fn completed_task(store: &Store, dir: &std::path::Path) -> Task {
    revise(store, "Maya: I'll review the rollout by 2026-10-20.").await;
    while store
        .process_next(&crate::Config::default(), dir)
        .await
        .unwrap()
    {}
    let t = store
        .snapshot(&Access::admin())
        .await
        .unwrap()
        .tasks
        .into_iter()
        .find(|t| t.status == "candidate")
        .unwrap();
    let t = store
        .update_task(
            &t.id,
            TaskUpdate {
                status: "accepted".into(),
                expected_version: t.version,
            },
        )
        .await
        .unwrap();
    store
        .update_task(
            &t.id,
            TaskUpdate {
                status: "completed".into(),
                expected_version: t.version,
            },
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn markers_are_exact_scoped_deduplicated_and_persisted_across_restart() {
    let (dir, store, _) = fixture().await;
    let doc=revise(&store,"Planned: ship soon\nAccomplishment: shipped the importer\nImpact: saved 8 minutes per import").await;
    assert_eq!(store.collect_bragbook_events().await.unwrap(), 2);
    let entries = all(&store).await;
    assert_eq!(entries.len(), 2);
    assert!(
        entries
            .iter()
            .all(|e| e.status == "candidate" && e.occurred_on.is_none() && e.reviewed_at.is_none())
    );
    for e in &entries {
        assert_eq!(
            doc.body.lines().nth(e.evidence[0].line as usize - 1),
            Some(e.evidence[0].quote.as_str())
        );
    }
    assert_eq!(
        store
            .collect_existing_bragbook(&Access::admin())
            .await
            .unwrap()
            .created,
        0
    );
    let req = review(&entries[0], "dismissed", "dismiss");
    store
        .review_bragbook_entry(&entries[0].id, req, &Access::admin())
        .await
        .unwrap();
    revise(&store,"Intro\nPlanned: ship soon\nAccomplishment: shipped the importer\nImpact: saved 8 minutes per import").await;
    assert_eq!(store.collect_bragbook_events().await.unwrap(), 0);
    store.pool.close().await;
    let reopened = Store::open(&dir.path().join("bragbook.sqlite"))
        .await
        .unwrap();
    assert_eq!(reopened.collect_bragbook_events().await.unwrap(), 0);
    let entries = all(&reopened).await;
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().any(|e| e.status == "dismissed"));
    assert!(entries.iter().all(|e| e.stale_evidence));
}

#[tokio::test]
async fn reviews_require_current_versions_and_exports_only_include_accepted_selection() {
    let (_dir, store, _) = fixture().await;
    revise(
        &store,
        "Achievement: shipped importer\nAccomplishment: mentored team",
    )
    .await;
    store.collect_bragbook_events().await.unwrap();
    let items = all(&store).await;
    let req = review(&items[0], "accepted", "accept");
    let accepted = store
        .review_bragbook_entry(&items[0].id, req.clone(), &Access::admin())
        .await
        .unwrap();
    assert_eq!(accepted.version, 2);
    assert!(accepted.reviewed_at.is_some());
    assert_eq!(
        store
            .review_bragbook_entry(&items[0].id, req.clone(), &Access::admin())
            .await
            .unwrap()
            .version,
        2
    );
    let mut stale = req;
    stale.idempotency_key = "different".into();
    assert!(
        store
            .review_bragbook_entry(&items[0].id, stale, &Access::admin())
            .await
            .is_err()
    );
    for format in ["markdown", "json"] {
        let export = store
            .export_bragbook(
                BragbookExportRequest {
                    entry_ids: vec![accepted.id.clone()],
                    format: format.into(),
                },
                &Access::admin(),
            )
            .await
            .unwrap();
        assert_eq!(export["entry_count"], 1);
        assert!(
            export["body"]
                .as_str()
                .unwrap()
                .contains("I implemented the import workflow.")
        );
        if format == "json" {
            let payload: Value = serde_json::from_str(export["body"].as_str().unwrap()).unwrap();
            assert_eq!(payload["entries"][0]["evidence"][0]["revision"], 2);
        }
    }
    assert!(
        store
            .export_bragbook(
                BragbookExportRequest {
                    entry_ids: items.iter().map(|e| e.id.clone()).collect(),
                    format: "json".into()
                },
                &Access::admin()
            )
            .await
            .is_err()
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bragbook_history WHERE entry_id=?")
        .bind(&accepted.id)
        .fetch_one(&store.pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn task_reopening_blocks_export_before_worker_and_returns_entry_to_review() {
    let (dir, store, _) = fixture().await;
    let t = completed_task(&store, dir.path()).await;
    assert_eq!(store.collect_bragbook_events().await.unwrap(), 1);
    let item = all(&store).await.remove(0);
    assert_eq!(item.origin_kind, "task");
    assert!(item.occurred_on.is_some());
    let accepted = store
        .review_bragbook_entry(
            &item.id,
            review(&item, "accepted", "accept-task"),
            &Access::admin(),
        )
        .await
        .unwrap();
    let task = store
        .update_task(
            &t.id,
            TaskUpdate {
                status: "accepted".into(),
                expected_version: t.version,
            },
        )
        .await
        .unwrap();
    assert!(
        store
            .export_bragbook(
                BragbookExportRequest {
                    entry_ids: vec![accepted.id],
                    format: "markdown".into()
                },
                &Access::admin()
            )
            .await
            .is_err()
    );
    store.collect_bragbook_events().await.unwrap();
    let item = all(&store).await.remove(0);
    assert_eq!(item.status, "candidate");
    assert!(item.support_changed);
    assert!(
        store
            .review_bragbook_entry(
                &item.id,
                review(&item, "accepted", "accept-reopened"),
                &Access::admin()
            )
            .await
            .is_err()
    );
    store
        .update_task(
            &task.id,
            TaskUpdate {
                status: "completed".into(),
                expected_version: task.version,
            },
        )
        .await
        .unwrap();
    store.collect_bragbook_events().await.unwrap();
    assert_eq!(all(&store).await.len(), 1);
}

#[tokio::test]
async fn ungranted_and_revoked_sources_cannot_be_collected_read_or_exported() {
    let (_dir, store, doc) = fixture().await;
    revise(&store, "Achievement: shipped importer").await;
    store
        .import(ImportRequest {
            source_name: "Private".into(),
            kind: "markdown".into(),
            uri: "fixture://private".into(),
            title: "Private".into(),
            body: "Achievement: private work".into(),
            allow_external: false,
        })
        .await
        .unwrap();
    assert_eq!(store.collect_bragbook_events().await.unwrap(), 1);
    let item = all(&store).await.remove(0);
    let accepted = store
        .review_bragbook_entry(
            &item.id,
            review(&item, "accepted", "accept-scoped"),
            &Access::admin(),
        )
        .await
        .unwrap();
    let preview = store
        .source_disclosure_preview(&doc.source_id, &crate::Config::default(), &Access::admin())
        .await
        .unwrap();
    assert!(
        preview
            .destinations
            .iter()
            .any(|d| d.destination_id == format!("plugin:{ID}") && d.allowed)
    );
    store
        .update_source_policy(
            &doc.source_id,
            SourcePolicyUpdate {
                expected_version: 0,
                external_enabled: false,
                denied_connections: vec![format!("plugin:{ID}")],
                denied_grants: vec![],
                idempotency_key: "revoke-bragbook".into(),
            },
            &Access::admin(),
        )
        .await
        .unwrap();
    assert!(all(&store).await.is_empty());
    assert!(
        store
            .export_bragbook(
                BragbookExportRequest {
                    entry_ids: vec![accepted.id],
                    format: "json".into()
                },
                &Access::admin()
            )
            .await
            .is_err()
    );
    assert!(
        store
            .review_bragbook_entry(
                &item.id,
                review(&item, "dismissed", "deny-review"),
                &Access::admin()
            )
            .await
            .is_err()
    );
}

#[tokio::test]
async fn disabled_periods_are_skipped_and_explicit_backfill_preserves_reviews() {
    let (_dir, store, _) = fixture().await;
    let p = store.plugin(ID).await.unwrap();
    let mut update = PluginUpdateRequest {
        expected_version: p.version,
        enabled: false,
        grant: p.grant,
        configuration: p.configuration,
        manifest: None,
        executable: None,
        idempotency_key: "disable".into(),
    };
    let p = store.update_plugin(ID, update.clone()).await.unwrap();
    revise(&store, "Achievement: work during disabled period").await;
    assert_eq!(store.collect_bragbook_events().await.unwrap(), 0);
    assert!(
        store
            .collect_existing_bragbook(&Access::admin())
            .await
            .is_err()
    );
    update.expected_version = p.version;
    update.enabled = true;
    update.idempotency_key = "re-enable".into();
    store.update_plugin(ID, update).await.unwrap();
    assert_eq!(store.collect_bragbook_events().await.unwrap(), 0);
    assert_eq!(
        store
            .collect_existing_bragbook(&Access::admin())
            .await
            .unwrap()
            .created,
        1
    );
    let item = all(&store).await.remove(0);
    store
        .review_bragbook_entry(
            &item.id,
            review(&item, "dismissed", "dismiss-backfill"),
            &Access::admin(),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .collect_existing_bragbook(&Access::admin())
            .await
            .unwrap()
            .created,
        0
    );
    assert_eq!(all(&store).await[0].status, "dismissed");
}

#[tokio::test]
async fn oversized_lines_are_reported_and_source_edits_do_not_rewrite_citations() {
    let (_dir, store, _) = fixture().await;
    let body = format!(
        "Achievement: {}\nAccomplishment: shipped safely",
        "x".repeat(2001)
    );
    revise(&store, &body).await;
    let result = store
        .collect_existing_bragbook(&Access::admin())
        .await
        .unwrap();
    assert_eq!(result.created, 1);
    assert_eq!(result.skipped_lines, 1);
    let item = all(&store).await.remove(0);
    let accepted = store
        .review_bragbook_entry(
            &item.id,
            review(&item, "accepted", "accept-pinned"),
            &Access::admin(),
        )
        .await
        .unwrap();
    revise(&store, "Achievement: a different claim").await;
    let items = all(&store).await;
    assert!(items[0].stale_evidence);
    assert_eq!(items[0].evidence, accepted.evidence);
    let mut req = review(&items[0], "accepted", "future-date");
    req.occurred_on = Some("2999-01-01".into());
    assert!(
        store
            .review_bragbook_entry(&item.id, req, &Access::admin())
            .await
            .is_err()
    );
}

#[tokio::test]
async fn scoped_agent_access_is_rejected_even_for_reading_personal_entries() {
    let (_dir, store, doc) = fixture().await;
    let mut access = Access::admin();
    access.source_ids = Some(vec![doc.source_id]);
    assert!(
        store
            .bragbook_entries(BragbookQuery::default(), &access)
            .await
            .is_err()
    );
    assert!(store.collect_existing_bragbook(&access).await.is_err());
}

#[tokio::test]
async fn only_confirmed_deliveries_are_collected_and_linked_tasks_are_deduplicated() {
    let (dir, store, doc) = fixture().await;
    let task = completed_task(&store, dir.path()).await;
    let project = store
        .create_brief(BriefCreateRequest {
            name: "Release".into(),
            source_ids: vec![doc.source_id],
            idempotency_key: "project".into(),
        })
        .await
        .unwrap();
    let current = store
        .document(&doc.id, None, &Access::admin())
        .await
        .unwrap();
    let evidence = Evidence {
        document_id: current.id,
        revision: current.revision,
        line: 1,
        quote: current.body,
    };
    let c = store
        .create_commitment(CommitmentCreateRequest {
            project_id: project.id,
            task_id: None,
            title: "Ship importer".into(),
            owner: "Maya".into(),
            promised_on: "2026-10-17".into(),
            due: None,
            evidence: vec![evidence.clone()],
            note: String::new(),
            idempotency_key: "create-delivery".into(),
        })
        .await
        .unwrap();
    let req = |c: &Commitment, action: &str| CommitmentUpdateRequest {
        expected_version: c.version,
        action: action.into(),
        title: None,
        owner: None,
        due: None,
        clear_due: false,
        kind: None,
        evidence: vec![],
        link_ids: vec![],
        note: "Implementation reviewed".into(),
        idempotency_key: format!("{action}-{}", c.version),
    };
    let mut link = req(&c, "link");
    link.kind = Some("implementation".into());
    link.evidence = vec![evidence];
    let c = store.update_commitment(&c.id, link).await.unwrap();
    let mut proposal = req(&c, "propose_completion");
    proposal.link_ids = vec![c.links[0].id.clone()];
    let c = store.update_commitment(&c.id, proposal).await.unwrap();
    store.collect_bragbook_events().await.unwrap();
    assert!(
        all(&store)
            .await
            .iter()
            .all(|e| e.origin_kind != "delivery")
    );
    let c = store
        .update_commitment(&c.id, req(&c, "confirm_completion"))
        .await
        .unwrap();
    store.collect_bragbook_events().await.unwrap();
    let items = all(&store).await;
    assert_eq!(items.len(), 2);
    let delivery = items.iter().find(|e| e.origin_kind == "delivery").unwrap();
    assert_eq!(delivery.origin_id, c.id);
    let accepted = store
        .review_bragbook_entry(
            &delivery.id,
            review(delivery, "accepted", "accept-delivery"),
            &Access::admin(),
        )
        .await
        .unwrap();
    store
        .update_commitment(&c.id, req(&c, "reopen"))
        .await
        .unwrap();
    assert!(
        store
            .export_bragbook(
                BragbookExportRequest {
                    entry_ids: vec![accepted.id],
                    format: "json".into()
                },
                &Access::admin()
            )
            .await
            .is_err()
    );
    store.collect_bragbook_events().await.unwrap();
    assert!(
        all(&store)
            .await
            .iter()
            .any(|e| e.id == delivery.id && e.status == "candidate" && e.support_changed)
    );
    // A linked delivery is deduplicated against its already collected task.
    let mut tx = store.pool.begin().await.unwrap();
    let mut linked = c.clone();
    linked.id = "synthetic-linked".into();
    linked.task_id = Some(task.id);
    linked.status = "completed".into();
    sqlx::query("INSERT INTO delivery_commitments(id,project_id,record) VALUES(?,?,?)")
        .bind(&linked.id)
        .bind(&linked.project_id)
        .bind(serde_json::to_string(&linked).unwrap())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        store
            .collect_existing_bragbook(&Access::admin())
            .await
            .unwrap()
            .created,
        0
    );
}

#[tokio::test]
async fn rolled_back_source_revision_creates_no_candidate() {
    let (_dir, store, doc) = fixture().await;
    let mut tx = store.pool.begin().await.unwrap();
    sqlx::query(
        "UPDATE documents SET revision=revision+1,body='Achievement: rolled back' WHERE id=?",
    )
    .bind(&doc.id)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(store.collect_bragbook_events().await.unwrap(), 0);
    assert!(all(&store).await.is_empty());
}
