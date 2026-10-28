//! Explicit administrative browser launch through a service-owned provider.
//! Stage an uncertain external effect once; retries inspect its receipt, and
//! never reopen a browser after a lost response or service restart.
use super::*;
impl Store {
    pub async fn open_local_browser(&self, req: PluginNativeRequest) -> Result<Value> {
        if req.command != "open" || req.input.as_object().is_none_or(|v| v.len() != 1) {
            bail!("invalid: browser-open command required");
        }
        let url = native::browser::local_url(
            req.input["url"]
                .as_str()
                .context("invalid: local workspace URL required")?,
        )?;
        let mut tx = self.pool.begin().await?;
        let (_, old) = operation(
            &mut tx,
            "service-browser",
            "native.browser",
            &req.idempotency_key,
            &req,
        )
        .await?;
        tx.commit().await?;
        if let Some(old) = old {
            return self.browser_reply(&old).await;
        }
        let provider = self.native_provider("native.browser").await?;
        let lease = self.providers.acquire("native.browser")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "service-browser",
            "native.browser",
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(old) = old {
            tx.commit().await?;
            return self.browser_reply(&old).await;
        }
        let current = load(&mut tx, &provider.id).await?;
        let selected: String = sqlx::query_scalar(
            "SELECT plugin_id FROM plugin_selections WHERE capability='native.browser'",
        )
        .fetch_one(&mut *tx)
        .await?;
        if !current.enabled
            || current.manifest.host != PluginHost::Service
            || current.version != provider.version
            || selected != current.id
            || lease.id() != current.id
            || !current
                .manifest
                .capabilities
                .iter()
                .any(|c| c.id == "native.browser" && c.commands.contains(&req.command))
        {
            bail!("conflict: browser provider changed before dispatch");
        }
        let id = uuid::Uuid::new_v4().to_string();
        // Pairing codes expire quickly but are still credentials: retain only
        // the origin and a request digest, never the URL fragment in history.
        sqlx::query("INSERT INTO plugin_native_commands(id,host_id,provider_id,provider_version,capability,command,input,status,expires_at,created_at) VALUES(?,'__service__',?,?,'native.browser','open',?,'running',?,?)")
            .bind(&id).bind(&provider.id).bind(provider.version).bind(json!({"origin":url.origin().ascii_serialization(),"pairing_redacted":true}).to_string())
            .bind((chrono::Utc::now()+chrono::Duration::seconds(10)).to_rfc3339()).bind(now()).execute(&mut *tx).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &id).await?;
        tx.commit().await?;
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(6),
            lease.provider().invoke("open", req.input),
        )
        .await;
        let reply = match result {
            Ok(Ok(value)) if value["requested"].as_bool() == Some(true) && serde_json::to_vec(&value)?.len() <= 32_000 => PluginNativeReply {
                result: json!({"requested":true,"detail":"The OS accepted the browser-open request; page rendering is not confirmed"}), error: None,
            },
            _ => PluginNativeReply { result: Value::Null, error: Some("Browser provider did not confirm success; outcome may be unknown. No automatic retry was made".into()) },
        };
        sqlx::query("UPDATE plugin_native_commands SET status='completed',reply=? WHERE id=? AND status='running'").bind(serde_json::to_string(&reply)?).bind(&id).execute(&self.pool).await?;
        self.browser_reply(&id).await
    }
    async fn browser_reply(&self, id: &str) -> Result<Value> {
        let reply = self.desktop_command_result(id).await?.context("conflict: browser request is still running or its outcome is unknown; it will not be replayed")?;
        if let Some(error) = reply.error {
            bail!("invalid: {error}");
        }
        Ok(reply.result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::registry::{CapabilityProvider, ProviderFuture};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };
    struct Browser {
        calls: Arc<AtomicUsize>,
        gate: Arc<AtomicBool>,
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }
    impl CapabilityProvider for Browser {
        fn manifest(&self) -> PluginManifest {
            native::manifest(
                "example.browser",
                "Synthetic browser",
                "native.browser",
                &["open"],
                PluginHost::Service,
            )
        }
        fn health(&self) -> Result<()> {
            Ok(())
        }
        fn invoke<'a>(&'a self, _command: &'a str, _input: Value) -> ProviderFuture<'a> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                if self.gate.load(Ordering::SeqCst) {
                    self.entered.notify_one();
                    self.release.notified().await;
                }
                Ok(json!({"requested":true}))
            })
        }
    }
    fn request(key: &str) -> PluginNativeRequest {
        PluginNativeRequest {
            command: "open".into(),
            input: json!({"url":format!("http://127.0.0.1:4620/#pair={}","a".repeat(64))}),
            idempotency_key: key.into(),
        }
    }
    #[tokio::test]
    async fn browser_requests_use_selection_leases_redacted_receipts_and_no_uncertain_replay() {
        let (_dir, store, _doc, _) = super::super::tests::fixture().await;
        let calls = Arc::new(AtomicUsize::new(0));
        let gate = Arc::new(AtomicBool::new(false));
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        store
            .providers
            .register(Arc::new(Browser {
                calls: calls.clone(),
                gate: gate.clone(),
                entered: entered.clone(),
                release: release.clone(),
            }))
            .unwrap();
        store.seed_native_providers().await.unwrap();
        let cap = store
            .plugin_capabilities()
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.capability == "native.browser")
            .unwrap();
        store
            .select_plugin_provider(
                "native.browser",
                PluginProviderSelectionRequest {
                    expected_version: cap.selection_version,
                    provider_id: "example.browser".into(),
                    idempotency_key: "select-browser".into(),
                },
            )
            .await
            .unwrap();
        assert_eq!(
            store.open_local_browser(request("once")).await.unwrap()["requested"],
            true
        );
        assert_eq!(
            store.open_local_browser(request("once")).await.unwrap()["requested"],
            true
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let p = store.plugin("example.browser").await.unwrap();
        let p = store
            .update_plugin(
                &p.id,
                super::super::tests::update(&p, false, "disable-browser"),
            )
            .await
            .unwrap();
        assert!(store.open_local_browser(request("disabled")).await.is_err());
        assert_eq!(
            store.open_local_browser(request("once")).await.unwrap()["requested"],
            true
        );
        let p = store
            .update_plugin(
                &p.id,
                super::super::tests::update(&p, true, "enable-browser"),
            )
            .await
            .unwrap();
        gate.store(true, Ordering::SeqCst);
        let other = store.clone();
        let running =
            tokio::spawn(async move { other.open_local_browser(request("pending")).await });
        tokio::time::timeout(std::time::Duration::from_secs(2), entered.notified())
            .await
            .unwrap();
        assert!(
            store
                .open_local_browser(request("pending"))
                .await
                .unwrap_err()
                .to_string()
                .contains("unknown")
        );
        assert!(
            store
                .update_plugin(
                    &p.id,
                    super::super::tests::update(&p, false, "disable-busy-browser")
                )
                .await
                .is_err()
        );
        release.notify_one();
        running.await.unwrap().unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let other = store.clone();
        let interrupted =
            tokio::spawn(async move { other.open_local_browser(request("interrupted")).await });
        tokio::time::timeout(std::time::Duration::from_secs(2), entered.notified())
            .await
            .unwrap();
        interrupted.abort();
        assert!(interrupted.await.unwrap_err().is_cancelled());
        store.migrate_plugins().await.unwrap();
        assert!(
            store
                .open_local_browser(request("interrupted"))
                .await
                .is_err()
        );
        assert_eq!(
            calls.load(Ordering::SeqCst),
            3,
            "no uncertain effect is repeated"
        );
        let inputs: Vec<String> = sqlx::query_scalar(
            "SELECT input FROM plugin_native_commands WHERE capability='native.browser'",
        )
        .fetch_all(&store.pool)
        .await
        .unwrap();
        assert_eq!(inputs.len(), 3);
        for input in inputs {
            assert!(!input.contains(&"a".repeat(64)));
            assert!(!input.contains("#pair="));
            assert!(input.contains("pairing_redacted"));
        }
    }
}
