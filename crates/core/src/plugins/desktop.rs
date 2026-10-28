//! A leased desktop host executes native commands; the service owns discovery,
//! authorization, provider configuration and the command receipt.
use super::*;
use chrono::{Duration, Utc};

pub(super) async fn registered_tx(
    tx: &mut Transaction<'_, Sqlite>,
    provider: &str,
) -> Result<bool> {
    for record in sqlx::query_scalar::<_, String>(
        "SELECT record FROM plugin_hosts WHERE julianday(expires_at)>julianday('now')",
    )
    .fetch_all(&mut **tx)
    .await?
    {
        let host: PluginHostRegistration = serde_json::from_str(&record)?;
        if host.manifests.iter().any(|m| m.id == provider) {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Store {
    pub async fn register_desktop_host(
        &self,
        req: PluginHostRegistration,
    ) -> Result<PluginHostSession> {
        if !validation::identifier(&req.host_id)
            || req.manifests.is_empty()
            || req.manifests.len() > 20
        {
            bail!("invalid: desktop host registration");
        }
        let mut ids = HashSet::new();
        for m in &req.manifests {
            validate_manifest(m)?;
            if m.host != PluginHost::Desktop
                || !m.id.starts_with("thesaurus.native.")
                || !ids.insert(m.id.clone())
                || m.capabilities.iter().any(|c| !c.id.starts_with("native."))
            {
                bail!("invalid: desktop provider namespace or capability");
            }
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM plugin_hosts WHERE host_id!=? AND julianday(expires_at)>julianday('now'))").bind(&req.host_id).fetch_one(&mut *tx).await?;
        if busy {
            bail!("conflict: another desktop host is already connected to this workspace");
        }
        let token = crate::store::token();
        sqlx::query("UPDATE plugin_native_commands SET status='expired' WHERE host_id=? AND status IN ('pending','running')").bind(&req.host_id).execute(&mut *tx).await?;
        for m in &req.manifests {
            let existing: Option<String> =
                sqlx::query_scalar("SELECT record FROM plugins WHERE id=?")
                    .bind(&m.id)
                    .fetch_optional(&mut *tx)
                    .await?;
            let p = if let Some(text) = existing {
                let mut p: PluginRecord = serde_json::from_str(&text)?;
                if !p.builtin || p.manifest.host != PluginHost::Desktop {
                    bail!("conflict: desktop provider identity is occupied");
                }
                if p.manifest != *m {
                    p.version += 1;
                    p.updated_at = now();
                }
                p.manifest = m.clone();
                p
            } else {
                PluginRecord {
                    id: m.id.clone(),
                    version: 1,
                    manifest: m.clone(),
                    executable: None,
                    builtin: true,
                    enabled: true,
                    grant: Default::default(),
                    configuration: json!({}),
                    installed_at: now(),
                    updated_at: now(),
                }
            };
            save(&mut tx, &p).await?;
            for capability in &m.capabilities {
                let current: Option<String> = sqlx::query_scalar(
                    "SELECT plugin_id FROM plugin_selections WHERE capability=?",
                )
                .bind(&capability.id)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(current) = current {
                    let selected = load(&mut tx, &current).await?;
                    if selected.manifest.host != PluginHost::Desktop {
                        bail!("conflict: desktop host cannot replace a service capability");
                    }
                }
                sqlx::query(
                    "INSERT OR IGNORE INTO plugin_selections(capability,plugin_id) VALUES(?,?)",
                )
                .bind(&capability.id)
                .bind(&m.id)
                .execute(&mut *tx)
                .await?;
            }
        }
        for manifest in &req.manifests {
            validation::validate_dependency_graph(&mut tx, manifest).await?;
        }
        sqlx::query("INSERT INTO plugin_hosts(host_id,token_hash,expires_at,record) VALUES(?,?,?,?) ON CONFLICT(host_id) DO UPDATE SET token_hash=excluded.token_hash,expires_at=excluded.expires_at,record=excluded.record,health='{\"providers\":[]}'")
            .bind(&req.host_id).bind(crate::store::hash(&token)).bind((Utc::now()+Duration::seconds(10)).to_rfc3339()).bind(serde_json::to_string(&req)?).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(PluginHostSession {
            host_id: req.host_id,
            lease_token: token,
        })
    }
    pub async fn desktop_available(&self, provider: &str) -> Result<bool> {
        Ok(self
            .desktop_provider_health(&self.plugin(provider).await?)
            .await
            .is_ok())
    }
    pub async fn desktop_registered(&self, provider: &str) -> Result<bool> {
        for record in sqlx::query_scalar::<_, String>(
            "SELECT record FROM plugin_hosts WHERE julianday(expires_at)>julianday('now')",
        )
        .fetch_all(&self.pool)
        .await?
        {
            let host: PluginHostRegistration = serde_json::from_str(&record)?;
            if host.manifests.iter().any(|m| m.id == provider) {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub async fn desktop_provider_health(&self, p: &PluginRecord) -> Result<()> {
        for (record, health) in sqlx::query_as::<_, (String, String)>(
            "SELECT record,health FROM plugin_hosts WHERE julianday(expires_at)>julianday('now')",
        )
        .fetch_all(&self.pool)
        .await?
        {
            let host: PluginHostRegistration = serde_json::from_str(&record)?;
            if host.manifests.iter().any(|m| m.id == p.id) {
                let heartbeat: PluginHostHeartbeat = serde_json::from_str(&health)?;
                let state = heartbeat
                    .providers
                    .iter()
                    .find(|s| s.provider_id == p.id && s.version == p.version)
                    .context("Desktop provider is applying configuration")?;
                if state.available {
                    return Ok(());
                }
                bail!("Desktop provider unavailable: {}", state.detail);
            }
        }
        bail!("Desktop host disconnected")
    }
    async fn desktop_host_authorized(&self, id: &str, token: &str) -> Result<()> {
        let digest:Option<String>=sqlx::query_scalar("SELECT token_hash FROM plugin_hosts WHERE host_id=? AND julianday(expires_at)>julianday('now')").bind(id).fetch_optional(&self.pool).await?;
        if digest.as_deref() != Some(&crate::store::hash(token)) {
            bail!("unauthorized desktop host lease");
        }
        Ok(())
    }
    pub async fn poll_desktop_host(
        &self,
        id: &str,
        token: &str,
        heartbeat: PluginHostHeartbeat,
    ) -> Result<PluginHostPoll> {
        self.desktop_host_authorized(id, token).await?;
        if heartbeat.providers.len() > 20
            || heartbeat.providers.iter().any(|p| p.detail.len() > 2000)
        {
            bail!("invalid: desktop heartbeat exceeds limit");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let renewed=sqlx::query("UPDATE plugin_hosts SET expires_at=?,health=? WHERE host_id=? AND token_hash=? AND julianday(expires_at)>julianday('now')")
            .bind((Utc::now() + Duration::seconds(10)).to_rfc3339())
            .bind(serde_json::to_string(&heartbeat)?)
            .bind(id)
            .bind(crate::store::hash(token))
            .execute(&mut *tx)
            .await?;
        if renewed.rows_affected() != 1 {
            bail!("unauthorized desktop host lease");
        }
        sqlx::query("UPDATE plugin_native_commands SET status='expired' WHERE status IN ('pending','running') AND julianday(expires_at)<=julianday('now')").execute(&mut *tx).await?;
        let rows=sqlx::query("SELECT * FROM plugin_native_commands WHERE host_id=? AND status='pending' AND julianday(expires_at)>julianday('now') ORDER BY created_at LIMIT 1").bind(id).fetch_all(&mut *tx).await?;
        let mut commands = vec![];
        for row in rows {
            let provider: String = row.get("provider_id");
            let expected: i64 = row.get("provider_version");
            let command_id: String = row.get("id");
            let p = load(&mut tx, &provider).await?;
            if !p.enabled
                || p.version != expected
                || !Self::plugin_native_authorized_tx(&mut tx, &command_id).await?
            {
                sqlx::query("UPDATE plugin_native_commands SET status='rejected' WHERE id=?")
                    .bind(&command_id)
                    .execute(&mut *tx)
                    .await?;
                continue;
            }
            if !heartbeat
                .providers
                .iter()
                .any(|h| h.provider_id == provider && h.version == expected && h.available)
            {
                continue;
            }
            sqlx::query("UPDATE plugin_native_commands SET status='running' WHERE id=? AND status='pending'").bind(&command_id).execute(&mut *tx).await?;
            commands.push(PluginNativeCommand {
                id: command_id,
                host_id: id.into(),
                provider_id: provider,
                provider_version: expected,
                capability: row.get("capability"),
                command: row.get("command"),
                input: serde_json::from_str(row.get("input"))?,
            });
        }
        tx.commit().await?;
        Ok(PluginHostPoll {
            plugins: self.plugins().await?,
            selections: self.plugin_capabilities().await?,
            commands,
        })
    }
    pub async fn enqueue_desktop_command(
        &self,
        capability: &str,
        req: PluginNativeRequest,
    ) -> Result<String> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = Self::enqueue_desktop_command_tx(&mut tx, capability, req).await?;
        tx.commit().await?;
        Ok(id)
    }
    pub(super) async fn enqueue_desktop_command_tx(
        tx: &mut Transaction<'_, Sqlite>,
        capability: &str,
        req: PluginNativeRequest,
    ) -> Result<String> {
        if serde_json::to_vec(&req.input)?.len() > 32_000 {
            bail!("invalid: native command input exceeds limit");
        }
        let (digest, old) = operation(
            tx,
            "desktop-command",
            capability,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if let Some(old) = old {
            return Ok(old);
        }
        let queued:i64=sqlx::query_scalar("SELECT COUNT(*) FROM plugin_native_commands WHERE status IN ('pending','running') AND julianday(expires_at)>julianday('now')").fetch_one(&mut **tx).await?;
        if queued >= 50 {
            bail!("conflict: desktop command queue is full");
        }
        let provider: String =
            sqlx::query_scalar("SELECT plugin_id FROM plugin_selections WHERE capability=?")
                .bind(capability)
                .fetch_optional(&mut **tx)
                .await?
                .context("unsupported: native capability unavailable")?;
        let p = load(tx, &provider).await?;
        if !p.enabled
            || p.manifest.host != PluginHost::Desktop
            || !p
                .manifest
                .capabilities
                .iter()
                .any(|c| c.id == capability && c.commands.contains(&req.command))
        {
            bail!("unsupported: enabled desktop provider command required");
        }
        let hosts:Vec<(String,String,String)>=sqlx::query_as("SELECT host_id,record,health FROM plugin_hosts WHERE julianday(expires_at)>julianday('now')").fetch_all(&mut **tx).await?;
        let host = hosts
            .into_iter()
            .find_map(|(id, text, health)| {
                let h = serde_json::from_str::<PluginHostRegistration>(&text).ok()?;
                let state = serde_json::from_str::<PluginHostHeartbeat>(&health).ok()?;
                (h.manifests.iter().any(|m| m.id == provider)
                    && state.providers.iter().any(|h| {
                        h.provider_id == provider && h.version == p.version && h.available
                    }))
                .then_some(id)
            })
            .context("unsupported: desktop provider is disconnected or not ready")?;
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO plugin_native_commands(id,host_id,provider_id,provider_version,capability,command,input,status,expires_at,created_at) VALUES(?,?,?,?,?,?,?,'pending',?,?)")
            .bind(&id).bind(host).bind(provider).bind(p.version).bind(capability).bind(req.command).bind(serde_json::to_string(&req.input)?).bind((Utc::now()+Duration::seconds(10)).to_rfc3339()).bind(now()).execute(&mut **tx).await?;
        remember(tx, &req.idempotency_key, &digest, &id).await?;
        Ok(id)
    }
    pub async fn reply_desktop_command(
        &self,
        host: &str,
        token: &str,
        id: &str,
        reply: PluginNativeReply,
    ) -> Result<()> {
        self.desktop_host_authorized(host, token).await?;
        if serde_json::to_vec(&reply)?.len() > 128_000 {
            bail!("invalid: native response exceeds limit");
        }
        let result=sqlx::query("UPDATE plugin_native_commands SET status='completed',reply=? WHERE id=? AND host_id=? AND status='running' AND julianday(expires_at)>julianday('now') AND EXISTS(SELECT 1 FROM plugin_hosts WHERE host_id=? AND token_hash=? AND julianday(expires_at)>julianday('now'))")
            .bind(serde_json::to_string(&reply)?).bind(id).bind(host).bind(host).bind(crate::store::hash(token)).execute(&self.pool).await?;
        if result.rows_affected() != 1 {
            bail!("conflict: native command expired or already replied");
        }
        Ok(())
    }
    pub async fn desktop_command_result(&self, id: &str) -> Result<Option<PluginNativeReply>> {
        let row =
            sqlx::query("SELECT status,reply,expires_at FROM plugin_native_commands WHERE id=?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .context("record not found")?;
        let status: String = row.get("status");
        if status == "completed" {
            return Ok(Some(serde_json::from_str(row.get("reply"))?));
        }
        let expires: chrono::DateTime<Utc> =
            chrono::DateTime::parse_from_rfc3339(row.get("expires_at"))?.with_timezone(&Utc);
        if status == "rejected" {
            bail!("conflict: desktop provider changed before command execution");
        }
        if status == "expired" || expires < Utc::now() {
            bail!(
                "conflict: desktop command outcome is unknown; it will not be retried automatically"
            );
        }
        Ok(None)
    }
}
