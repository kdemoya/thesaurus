//! Scoped API 2 requests for allowlisted desktop commands. The host queues them
//! atomically and rechecks plugin authority immediately before broker dispatch.
use super::*;
use serde::{Deserialize, Serialize};

const COMMANDS: &[(&str, &str)] = &[
    ("native.windows", "workspace"),
    ("native.windows", "companion"),
    ("native.tray", "show"),
    ("native.tray", "dismiss"),
    ("native.notifications", "show"),
    ("native.notifications", "dismiss"),
];
#[derive(Serialize, Deserialize)]
struct Origin {
    plugin_id: String,
    installed_at: String,
    configuration_version: i64,
    package_version: String,
    grant: PluginPermissions,
    source_ids: Vec<String>,
    project_ids: Vec<String>,
    origin_kind: String,
    origin_id: String,
    operation: String,
}
pub(super) fn validate_response(response: &PluginResponse) -> Result<()> {
    if response.native_commands.is_empty() {
        return Ok(());
    }
    if response.api_version < 2
        || response.native_commands.len() > 5
        || !response.observations.is_empty()
        || !response.snapshots.is_empty()
        || !response.actions.is_empty()
        || !response.checks.is_empty()
        || !response.model_requests.is_empty()
        || response.readiness.is_some()
    {
        bail!("invalid: return at most five API 2 native requests without other host effects");
    }
    let mut keys = HashSet::new();
    for proposal in &response.native_commands {
        if !validation::identifier(&proposal.key)
            || !keys.insert(&proposal.key)
            || !COMMANDS.contains(&(proposal.capability.as_str(), proposal.command.as_str()))
        {
            bail!("invalid: native command identity is duplicated or unsupported");
        }
        input("validation", proposal)?;
    }
    Ok(())
}
fn input(installation: &str, proposal: &PluginNativeProposal) -> Result<Value> {
    let object = proposal
        .input
        .as_object()
        .context("invalid: native command input must be an object")?;
    if proposal.capability == "native.notifications" && proposal.command == "show" {
        let title = object
            .get("title")
            .and_then(Value::as_str)
            .context("invalid: notification title required")?;
        let body = object
            .get("body")
            .and_then(Value::as_str)
            .context("invalid: notification body required")?;
        validate_text(title, 120, "notification title")?;
        if object.len() != 2 || body.len() > 2000 || body.contains('\0') {
            bail!("invalid: notification body or fields exceed limits");
        }
        let id = format!(
            "plugin-{}",
            &crate::store::hash(&format!("{installation}:{}", proposal.key))[..40]
        );
        return Ok(json!({"id":id,"title":title,"body":body}));
    }
    if proposal.capability == "native.notifications" && proposal.command == "dismiss" {
        if !object.is_empty() {
            bail!("invalid: notification dismissal takes only its request key");
        }
        return Ok(
            json!({"id":format!("plugin-{}",&crate::store::hash(&format!("{installation}:{}",proposal.key))[..40])}),
        );
    }
    if proposal.capability == "native.windows" && proposal.command == "companion" {
        if object.len() != 1
            || !object
                .get("mode")
                .and_then(Value::as_str)
                .is_some_and(|s| matches!(s, "panel" | "left" | "right"))
        {
            bail!("invalid: companion mode must be panel, left or right");
        }
    } else if !object.is_empty() {
        bail!("invalid: native command does not accept input fields");
    }
    Ok(proposal.input.clone())
}
impl Store {
    pub(super) async fn enqueue_plugin_native_tx(
        tx: &mut Transaction<'_, Sqlite>,
        p: &PluginRecord,
        origin_kind: &str,
        origin_id: &str,
        sources: &[String],
        projects: &[String],
        proposals: Vec<PluginNativeProposal>,
    ) -> Result<()> {
        if proposals.is_empty() {
            return Ok(());
        }
        if !p.enabled {
            bail!("forbidden: native request plugin is disabled");
        }
        contributions::scope_tx(tx, p, sources, projects).await?;
        for proposal in proposals {
            let operation = format!("{}.{}", proposal.capability, proposal.command);
            if !COMMANDS.contains(&(proposal.capability.as_str(), proposal.command.as_str()))
                || !p.manifest.requests.contains(&operation)
                || !p.grant.operations.contains(&operation)
            {
                bail!("forbidden: explicit native command grant required");
            }
            let payload = input(&format!("{}:{}", p.id, p.installed_at), &proposal)?;
            let key = format!(
                "plugin-native:{}",
                crate::store::hash(&format!(
                    "{}:{}:{origin_kind}:{origin_id}:{}",
                    p.id, p.installed_at, proposal.key
                ))
            );
            let id = Self::enqueue_desktop_command_tx(
                tx,
                &proposal.capability,
                PluginNativeRequest {
                    command: proposal.command,
                    input: payload,
                    idempotency_key: key,
                },
            )
            .await?;
            let origin = Origin {
                plugin_id: p.id.clone(),
                installed_at: p.installed_at.clone(),
                configuration_version: p.version,
                package_version: p.manifest.version.clone(),
                grant: p.grant.clone(),
                source_ids: sources.into(),
                project_ids: projects.into(),
                origin_kind: origin_kind.into(),
                origin_id: origin_id.into(),
                operation,
            };
            sqlx::query("INSERT OR IGNORE INTO plugin_native_origins(command_id,plugin_id,installed_at,record) VALUES(?,?,?,?)")
                .bind(id).bind(&p.id).bind(&p.installed_at).bind(serde_json::to_string(&origin)?).execute(&mut **tx).await?;
        }
        Ok(())
    }
    pub(super) async fn plugin_native_authorized_tx(
        tx: &mut Transaction<'_, Sqlite>,
        command: &str,
    ) -> Result<bool> {
        let origin: Option<String> =
            sqlx::query_scalar("SELECT record FROM plugin_native_origins WHERE command_id=?")
                .bind(command)
                .fetch_optional(&mut **tx)
                .await?;
        let Some(origin) = origin else {
            return Ok(true);
        }; // ordinary administrative broker command
        let origin: Origin = serde_json::from_str(&origin)?;
        let record: Option<String> = sqlx::query_scalar("SELECT record FROM plugins WHERE id=?")
            .bind(&origin.plugin_id)
            .fetch_optional(&mut **tx)
            .await?;
        let Some(record) = record else {
            return Ok(false);
        };
        let p: PluginRecord = serde_json::from_str(&record)?;
        if !p.enabled
            || p.installed_at != origin.installed_at
            || p.version != origin.configuration_version
            || !p.grant.operations.contains(&origin.operation)
        {
            return Ok(false);
        }
        Ok(
            contributions::scope_tx(tx, &p, &origin.source_ids, &origin.project_ids)
                .await
                .is_ok(),
        )
    }
    pub async fn plugin_native_receipts(&self, plugin: &str) -> Result<Vec<PluginNativeReceipt>> {
        let p = self.plugin(plugin).await?;
        let rows = sqlx::query("SELECT c.*,o.record AS origin_record FROM plugin_native_commands c JOIN plugin_native_origins o ON o.command_id=c.id WHERE o.plugin_id=? AND o.installed_at=? ORDER BY c.created_at DESC,c.id LIMIT 100")
            .bind(plugin).bind(&p.installed_at).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                let origin: Origin = serde_json::from_str(row.get("origin_record"))?;
                let status: String = row.get("status");
                let expired = chrono::DateTime::parse_from_rfc3339(row.get("expires_at"))?
                    < chrono::Utc::now();
                let reply: Option<PluginNativeReply> = row
                    .get::<Option<String>, _>("reply")
                    .map(|r| serde_json::from_str(&r))
                    .transpose()?;
                Ok(PluginNativeReceipt {
                    id: row.get("id"),
                    plugin_id: plugin.into(),
                    origin_kind: origin.origin_kind,
                    origin_id: origin.origin_id,
                    configuration_version: origin.configuration_version,
                    package_version: origin.package_version,
                    grant: origin.grant,
                    source_ids: origin.source_ids,
                    project_ids: origin.project_ids,
                    capability: row.get("capability"),
                    command: row.get("command"),
                    status: if expired && matches!(status.as_str(), "pending" | "running") {
                        "expired".into()
                    } else {
                        status
                    },
                    created_at: row.get("created_at"),
                    result: reply.as_ref().map(|r| r.result.clone()),
                    error: reply.and_then(|r| r.error),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn proposal(key: &str) -> PluginNativeProposal {
        PluginNativeProposal {
            key: key.into(),
            capability: "native.notifications".into(),
            command: "show".into(),
            input: json!({"title":"Synthetic notification","body":"No OS notification is sent by this test"}),
        }
    }
    async fn enqueue(
        store: &Store,
        p: &PluginRecord,
        doc: &Document,
        origin: &str,
        proposals: Vec<PluginNativeProposal>,
    ) -> Result<()> {
        let mut tx = store.pool.begin_with("BEGIN IMMEDIATE").await?;
        Store::enqueue_plugin_native_tx(
            &mut tx,
            p,
            "run",
            origin,
            &[doc.source_id.clone()],
            &[],
            proposals,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
    async fn fixture() -> (
        tempfile::TempDir,
        Store,
        Document,
        PluginRecord,
        PluginHostSession,
        PluginRecord,
    ) {
        let (dir, store, doc, p) = super::super::tests::fixture().await;
        let mut manifest = p.manifest.clone();
        manifest.api_min = 2;
        manifest.api_max = 2;
        manifest.requests.extend([
            "native.notifications.show".into(),
            "native.notifications.dismiss".into(),
            "commands.invoke".into(),
        ]);
        manifest.contributions.push(PluginContribution {
            id: "notify".into(),
            kind: PluginContributionKind::Command,
            title: "Synthetic native request".into(),
            description: String::new(),
        });
        let mut req = super::super::tests::update(&p, false, "native-manifest");
        req.manifest = Some(manifest);
        req.grant.operations.extend([
            "native.notifications.show".into(),
            "native.notifications.dismiss".into(),
            "commands.invoke".into(),
        ]);
        let p = store.update_plugin(&p.id, req).await.unwrap();
        let p = store
            .update_plugin(
                &p.id,
                super::super::tests::update(&p, true, "enable-native-plugin"),
            )
            .await
            .unwrap();
        let host = store
            .register_desktop_host(PluginHostRegistration {
                host_id: "synthetic-native-broker".into(),
                manifests: vec![native::manifest(
                    "thesaurus.native.test-notifications",
                    "Synthetic notifications",
                    "native.notifications",
                    &["show", "dismiss"],
                    PluginHost::Desktop,
                )],
            })
            .await
            .unwrap();
        let provider = store
            .plugin("thesaurus.native.test-notifications")
            .await
            .unwrap();
        store
            .poll_desktop_host(&host.host_id, &host.lease_token, heartbeat(&provider))
            .await
            .unwrap();
        (dir, store, doc, p, host, provider)
    }
    fn heartbeat(p: &PluginRecord) -> PluginHostHeartbeat {
        PluginHostHeartbeat {
            providers: vec![PluginProviderHealth {
                provider_id: p.id.clone(),
                version: p.version,
                available: true,
                detail: "Synthetic only".into(),
            }],
        }
    }
    #[test]
    fn native_requests_reject_unsupported_commands_mixed_effects_and_unbounded_payloads() {
        let mut response = PluginResponse {
            api_version: 2,
            invocation_id: "synthetic".into(),
            native_commands: vec![proposal("follow-up")],
            ..Default::default()
        };
        validate_response(&response).unwrap();
        response.native_commands[0].capability = "native.credentials".into();
        assert!(validate_response(&response).is_err());
        response.native_commands[0] = proposal("follow-up");
        response.native_commands.push(proposal("follow-up"));
        assert!(validate_response(&response).is_err());
        response.native_commands.pop();
        response.native_commands[0].input["id"] = json!("another-notification");
        assert!(validate_response(&response).is_err());
        response.native_commands[0] = proposal("follow-up");
        response.observations.push(PluginObservationProposal {
            body: "Forbidden mixed effect".into(),
            source_ids: vec![],
            evidence: vec![],
        });
        assert!(validate_response(&response).is_err());
        response.observations.clear();
        response.native_commands[0].input["body"] = json!("x".repeat(2001));
        assert!(validate_response(&response).is_err());
        response.native_commands[0] = proposal("follow-up");
        response.api_version = 1;
        assert!(validate_response(&response).is_err());
    }
    #[tokio::test]
    async fn native_requests_are_scoped_atomic_idempotent_and_rechecked_at_dispatch() {
        let (_dir, store, doc, p, host, provider) = fixture().await;
        enqueue(&store, &p, &doc, "first", vec![proposal("follow-up")])
            .await
            .unwrap();
        enqueue(&store, &p, &doc, "first", vec![proposal("follow-up")])
            .await
            .unwrap();
        let receipts = store.plugin_native_receipts(&p.id).await.unwrap();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].status, "pending");
        assert_eq!(receipts[0].grant, p.grant);
        let commands = store
            .poll_desktop_host(&host.host_id, &host.lease_token, heartbeat(&provider))
            .await
            .unwrap()
            .commands;
        assert_eq!(commands.len(), 1);
        assert!(
            commands[0].input["id"]
                .as_str()
                .unwrap()
                .starts_with("plugin-")
        );
        store
            .reply_desktop_command(
                &host.host_id,
                &host.lease_token,
                &commands[0].id,
                PluginNativeReply {
                    result: json!({"submitted":true,"synthetic":true}),
                    error: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            store.plugin_native_receipts(&p.id).await.unwrap()[0].status,
            "completed"
        );
        let mut ungranted = proposal("workspace");
        ungranted.capability = "native.windows".into();
        ungranted.command = "workspace".into();
        ungranted.input = json!({});
        assert!(
            enqueue(
                &store,
                &p,
                &doc,
                "atomic-refusal",
                vec![proposal("allowed"), ungranted]
            )
            .await
            .is_err()
        );
        assert_eq!(store.plugin_native_receipts(&p.id).await.unwrap().len(), 1);
        enqueue(
            &store,
            &p,
            &doc,
            "revoked-source",
            vec![proposal("source-policy")],
        )
        .await
        .unwrap();
        store
            .update_source_policy(
                &doc.source_id,
                SourcePolicyUpdate {
                    expected_version: 0,
                    external_enabled: false,
                    denied_connections: vec![format!("plugin:{}", p.id)],
                    denied_grants: vec![],
                    idempotency_key: "native-policy-revoke".into(),
                },
                &Access::admin(),
            )
            .await
            .unwrap();
        assert!(
            store
                .poll_desktop_host(&host.host_id, &host.lease_token, heartbeat(&provider))
                .await
                .unwrap()
                .commands
                .is_empty()
        );
        assert!(
            store
                .plugin_native_receipts(&p.id)
                .await
                .unwrap()
                .iter()
                .any(|r| r.status == "rejected")
        );
        store
            .update_source_policy(
                &doc.source_id,
                SourcePolicyUpdate {
                    expected_version: 1,
                    external_enabled: false,
                    denied_connections: vec![],
                    denied_grants: vec![],
                    idempotency_key: "native-policy-restore".into(),
                },
                &Access::admin(),
            )
            .await
            .unwrap();
        enqueue(
            &store,
            &p,
            &doc,
            "disabled-plugin",
            vec![proposal("disabled")],
        )
        .await
        .unwrap();
        store
            .update_plugin(
                &p.id,
                super::super::tests::update(&p, false, "disable-native-plugin"),
            )
            .await
            .unwrap();
        assert!(
            store
                .poll_desktop_host(&host.host_id, &host.lease_token, heartbeat(&provider))
                .await
                .unwrap()
                .commands
                .is_empty()
        );
        assert_eq!(
            store
                .plugin_native_receipts(&p.id)
                .await
                .unwrap()
                .iter()
                .filter(|r| r.status == "rejected")
                .count(),
            2
        );
    }
}

#[cfg(test)]
mod execution_tests {
    use super::*;
    #[tokio::test]
    async fn subprocess_contributions_and_hooks_queue_native_effects_without_host_tokens() {
        let (dir, store, doc, p) = super::super::tests::fixture().await;
        std::fs::write(dir.path().join("plugin.py"),r#"import sys,json
v=json.loads(sys.stdin.readline())
assert 'token' not in v and 'credentials' not in v
r={'api_version':v['api_version'],'invocation_id':v['invocation_id'],'native_commands':[{'key':'synthetic','capability':'native.notifications','command':'show','input':{'title':'Synthetic broker test','body':'No actual notification is sent'}}]}
if v['method']=='contribution':r['contribution']={'text':'Native request accepted for broker review','evidence':[]}
print(json.dumps(r))
"#).unwrap();
        let mut manifest = p.manifest.clone();
        manifest.api_min = 2;
        manifest.api_max = 2;
        manifest
            .requests
            .extend(["commands.invoke".into(), "native.notifications.show".into()]);
        manifest.contributions.push(PluginContribution {
            id: "notify".into(),
            kind: PluginContributionKind::Command,
            title: "Synthetic native request".into(),
            description: String::new(),
        });
        let mut req = super::super::tests::update(&p, false, "native-execution-manifest");
        req.manifest = Some(manifest);
        req.grant
            .operations
            .extend(["commands.invoke".into(), "native.notifications.show".into()]);
        let p = store.update_plugin(&p.id, req).await.unwrap();
        let p = store
            .update_plugin(
                &p.id,
                super::super::tests::update(&p, true, "enable-native-execution"),
            )
            .await
            .unwrap();
        let host = store
            .register_desktop_host(PluginHostRegistration {
                host_id: "synthetic-native-execution".into(),
                manifests: vec![native::manifest(
                    "thesaurus.native.execution-notifications",
                    "Synthetic notifications",
                    "native.notifications",
                    &["show"],
                    PluginHost::Desktop,
                )],
            })
            .await
            .unwrap();
        let provider = store
            .plugin("thesaurus.native.execution-notifications")
            .await
            .unwrap();
        let heartbeat = PluginHostHeartbeat {
            providers: vec![PluginProviderHealth {
                provider_id: provider.id,
                version: provider.version,
                available: true,
                detail: "Synthetic only".into(),
            }],
        };
        store
            .poll_desktop_host(&host.host_id, &host.lease_token, heartbeat.clone())
            .await
            .unwrap();
        let request:PluginContributionRequest=serde_json::from_value(json!({"expected_version":p.version,"contribution_id":"notify","source_ids":[doc.source_id],"project_ids":[],"input":{},"idempotency_key":"native-command-run"})).unwrap();
        let run = store
            .request_plugin_contribution(&p.id, request.clone())
            .await
            .unwrap();
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
            store
                .request_plugin_contribution(&p.id, request)
                .await
                .unwrap()
                .id,
            run.id
        );
        store
            .import(ImportRequest {
                source_name: "Synthetic transcript".into(),
                kind: "transcript".into(),
                uri: "fixture://plugin-review".into(),
                title: "Synthetic meeting".into(),
                body: "TODO synthetic native hook".into(),
                allow_external: false,
            })
            .await
            .unwrap();
        store.queue_plugin_deliveries().await.unwrap();
        store.run_plugin_delivery(dir.path()).await.unwrap();
        let receipts = store.plugin_native_receipts(&p.id).await.unwrap();
        assert_eq!(receipts.len(), 2);
        assert!(
            receipts
                .iter()
                .any(|r| r.origin_kind == "run" && r.origin_id == run.id)
        );
        assert!(receipts.iter().any(|r| r.origin_kind == "delivery"));
        for _ in 0..2 {
            let commands = store
                .poll_desktop_host(&host.host_id, &host.lease_token, heartbeat.clone())
                .await
                .unwrap()
                .commands;
            assert_eq!(commands.len(), 1);
            store
                .reply_desktop_command(
                    &host.host_id,
                    &host.lease_token,
                    &commands[0].id,
                    PluginNativeReply {
                        result: json!({"submitted":true,"synthetic":true}),
                        error: None,
                    },
                )
                .await
                .unwrap();
        }
        assert!(
            store
                .plugin_native_receipts(&p.id)
                .await
                .unwrap()
                .iter()
                .all(|r| r.status == "completed")
        );
        let delivery = store
            .plugin_deliveries(&p.id)
            .await
            .unwrap()
            .into_iter()
            .find(|r| r.status == "completed")
            .unwrap();
        sqlx::query("UPDATE plugin_deliveries SET status='pending',next_attempt_at=? WHERE id=?")
            .bind(now())
            .bind(delivery.id)
            .execute(&store.pool)
            .await
            .unwrap();
        store.run_plugin_delivery(dir.path()).await.unwrap();
        assert_eq!(
            store.plugin_native_receipts(&p.id).await.unwrap().len(),
            2,
            "recovered/retried delivery does not duplicate native work"
        );
    }
}
