//! Plugin installation, grants and durable subscriptions. Domain writes and
//! outbox events commit together; extension execution is always outside them.
use crate::{
    Store,
    continuity::{event, operation, remember},
    store::{now, validate_text},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sqlx::{Row, Sqlite, Transaction};
use std::{collections::HashSet, path::Path};
use thesaurus_contracts::*;

pub mod bragbook;
pub mod contributions;
pub mod desktop;
pub mod native;
mod native_requests;
pub mod registry;
mod service_commands;
pub mod setup;
#[cfg(test)]
mod tests;
mod validation;
pub mod worker;
pub use validation::{validate_configuration, validate_manifest};

pub const MAX_EVENT_DEPTH: u32 = 4;

async fn load(tx: &mut Transaction<'_, Sqlite>, id: &str) -> Result<PluginRecord> {
    let text: String = sqlx::query_scalar("SELECT record FROM plugins WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&text)?)
}
async fn save(tx: &mut Transaction<'_, Sqlite>, p: &PluginRecord) -> Result<()> {
    sqlx::query("INSERT INTO plugins(id,version,enabled,builtin,record) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET version=excluded.version,enabled=excluded.enabled,builtin=excluded.builtin,record=excluded.record")
        .bind(&p.id).bind(p.version).bind(p.enabled).bind(p.builtin).bind(serde_json::to_string(p)?).execute(&mut **tx).await?;
    event(tx, "plugin", &p.id).await
}

pub async fn emit_tx(
    tx: &mut Transaction<'_, Sqlite>,
    kind: &str,
    record_kind: &str,
    record_id: &str,
    revision: i64,
    sources: &[String],
    projects: &[String],
) -> Result<()> {
    sqlx::query("INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id) VALUES(?,?,?,?,?,?,?,?)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(kind).bind(record_kind).bind(record_id).bind(revision)
        .bind(serde_json::to_string(sources)?).bind(serde_json::to_string(projects)?).bind(record_id)
        .execute(&mut **tx).await?;
    Ok(())
}

async fn validate_grant(
    tx: &mut Transaction<'_, Sqlite>,
    manifest: &PluginManifest,
    grant: &PluginPermissions,
) -> Result<()> {
    if grant.source_ids.len() > 100
        || grant.project_ids.len() > 50
        || grant.connection_ids.len() > 20
        || grant
            .operations
            .iter()
            .any(|op| !manifest.requests.contains(op))
    {
        bail!("invalid: plugin grant exceeds requested permissions or scope limits");
    }
    for source in &grant.source_ids {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM sources WHERE id=?)")
            .bind(source)
            .fetch_one(&mut **tx)
            .await?;
        if !exists {
            bail!("invalid: plugin source does not exist");
        }
    }
    for project in &grant.project_ids {
        crate::continuity::project_sources(tx, project).await?;
    }
    Ok(())
}

async fn ensure_resources_idle(tx: &mut Transaction<'_, Sqlite>, p: &PluginRecord) -> Result<()> {
    let caps: HashSet<_> = p
        .manifest
        .capabilities
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    if caps.contains("native.audio") || caps.contains("native.transcription") {
        let busy: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM live_meetings WHERE state IN ('starting','recording','pausing','stopping') OR COALESCE(json_extract(record,'$.pending_chunks'),0)>0)").fetch_one(&mut **tx).await?;
        if busy {
            bail!(
                "conflict: stop recording and let its queue finish before changing the audio provider"
            );
        }
    }
    if caps.contains("native.screen_capture") || caps.contains("native.ocr") {
        let busy: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM cap_sessions WHERE state='active')")
                .fetch_one(&mut **tx)
                .await?;
        if busy {
            bail!("conflict: pause activity capture before changing its provider");
        }
    }
    if caps
        .iter()
        .any(|c| matches!(*c, "native.windows" | "native.tray" | "native.shortcuts"))
    {
        let busy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM live_meetings WHERE state IN ('starting','recording','pausing','stopping'))").fetch_one(&mut **tx).await?;
        if busy {
            bail!("conflict: pause or stop recording before changing desktop controls");
        }
    }
    let command_busy: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM plugin_native_commands WHERE provider_id=? AND status='running' AND julianday(expires_at)>julianday('now'))").bind(&p.id).fetch_one(&mut **tx).await?;
    if command_busy {
        bail!("conflict: desktop provider has an active command; retry after it finishes");
    }
    Ok(())
}
async fn ensure_idle(tx: &mut Transaction<'_, Sqlite>, p: &PluginRecord) -> Result<()> {
    ensure_resources_idle(tx, p).await?;
    let busy: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM plugin_deliveries WHERE plugin_id=? AND status='running') OR EXISTS(SELECT 1 FROM plugin_runs WHERE plugin_id=? AND status IN ('running','cancelling'))",
    )
    .bind(&p.id)
    .bind(&p.id)
    .fetch_one(&mut **tx)
    .await?;
    if busy {
        bail!("conflict: plugin has an active invocation; retry after it finishes");
    }
    Ok(())
}

impl Store {
    pub async fn migrate_plugins(&self) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT OR REPLACE INTO plugin_setup_epoch(id,epoch) VALUES(1,?)")
            .bind(uuid::Uuid::new_v4().to_string())
            .execute(&mut *tx)
            .await?;
        let columns = sqlx::query("PRAGMA table_info(plugin_delivery_attempts)")
            .fetch_all(&mut *tx)
            .await?;
        for (name, definition) in [
            ("configuration_hash", "configuration_hash TEXT"),
            (
                "executable_definition_hash",
                "executable_definition_hash TEXT",
            ),
            ("grant_record", "grant_record TEXT"),
        ] {
            if !columns.iter().any(|r| r.get::<String, _>("name") == name) {
                sqlx::query(&format!(
                    "ALTER TABLE plugin_delivery_attempts ADD COLUMN {definition}"
                ))
                .execute(&mut *tx)
                .await?;
            }
        }
        let columns = sqlx::query("PRAGMA table_info(plugin_selections)")
            .fetch_all(&mut *tx)
            .await?;
        if !columns
            .iter()
            .any(|r| r.get::<String, _>("name") == "version")
        {
            sqlx::query(
                "ALTER TABLE plugin_selections ADD COLUMN version INTEGER NOT NULL DEFAULT 1",
            )
            .execute(&mut *tx)
            .await?;
        }
        let columns = sqlx::query("PRAGMA table_info(plugin_hosts)")
            .fetch_all(&mut *tx)
            .await?;
        if !columns
            .iter()
            .any(|r| r.get::<String, _>("name") == "health")
        {
            sqlx::query("ALTER TABLE plugin_hosts ADD COLUMN health TEXT NOT NULL DEFAULT '{\"providers\":[]}'").execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE plugin_deliveries SET status='pending',error='Host restarted; delivery will be retried',updated_at=?,next_attempt_at=? WHERE status='running'").bind(now()).bind(now()).execute(&mut *tx).await?;
        sqlx::query("UPDATE plugin_delivery_attempts SET status='interrupted',finished_at=?,error='Host restarted during this attempt; delivery may be retried with idempotent host effects' WHERE status='running'").bind(now()).execute(&mut *tx).await?;
        sqlx::query("UPDATE plugin_runs SET status='interrupted',record=json_set(record,'$.status','interrupted','$.finished_at',?,'$.error','Host restarted during this invocation; it will not be replayed automatically') WHERE status IN ('running','cancelling')").bind(now()).execute(&mut *tx).await?;
        // Never replay a UI command after a service restart: its effect may
        // already have happened in the independent desktop process.
        sqlx::query("DELETE FROM plugin_hosts")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE plugin_native_commands SET status='expired' WHERE status IN ('pending','running')").execute(&mut *tx).await?;
        let columns = sqlx::query("PRAGMA table_info(plugin_events)")
            .fetch_all(&mut *tx)
            .await?;
        if !columns
            .iter()
            .any(|r| r.get::<String, _>("name") == "schema_version")
        {
            sqlx::query(
                "ALTER TABLE plugin_events ADD COLUMN schema_version INTEGER NOT NULL DEFAULT 1",
            )
            .execute(&mut *tx)
            .await?;
        }
        sqlx::raw_sql(include_str!("hooks.sql"))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn plugins(&self) -> Result<Vec<PluginRecord>> {
        let rows: Vec<String> = sqlx::query_scalar("SELECT record FROM plugins ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(|s| Ok(serde_json::from_str(s)?)).collect()
    }
    pub async fn select_plugin_provider(
        &self,
        capability: &str,
        req: PluginProviderSelectionRequest,
    ) -> Result<Vec<PluginCapabilityStatus>> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "plugin-provider",
            capability,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            let (selected, version): (String, i64) = sqlx::query_as(
                "SELECT plugin_id,version FROM plugin_selections WHERE capability=?",
            )
            .bind(capability)
            .fetch_optional(&mut *tx)
            .await?
            .context("unknown capability")?;
            if version != req.expected_version {
                bail!("conflict: capability provider changed; refresh first");
            }
            let p = load(&mut tx, &req.provider_id).await?;
            if !p.enabled || !p.manifest.capabilities.iter().any(|c| c.id == capability) {
                bail!("invalid: enabled provider for this capability required");
            }
            if selected != req.provider_id {
                let previous = load(&mut tx, &selected).await?;
                if previous.manifest.host != p.manifest.host {
                    bail!("invalid: provider host cannot change for this capability");
                }
                ensure_idle(&mut tx, &previous).await?;
                if p.manifest.host == PluginHost::Service {
                    self.providers.select(capability, &req.provider_id)?;
                } else if !desktop::registered_tx(&mut tx, &p.id).await? {
                    bail!("unsupported: desktop provider host is disconnected");
                }
                sqlx::query(
                    "UPDATE plugin_selections SET plugin_id=?,version=version+1 WHERE capability=?",
                )
                .bind(&req.provider_id)
                .bind(capability)
                .execute(&mut *tx)
                .await?;
            }
            remember(&mut tx, &req.idempotency_key, &digest, capability).await?;
            event(&mut tx, "plugin", &req.provider_id).await?;
        }
        tx.commit().await?;
        self.plugin_capabilities().await
    }
    pub async fn plugin_capabilities(&self) -> Result<Vec<PluginCapabilityStatus>> {
        let rows=sqlx::query("SELECT s.capability,s.version,p.record FROM plugin_selections s JOIN plugins p ON p.id=s.plugin_id ORDER BY s.capability")
            .fetch_all(&self.pool).await?;
        let mut statuses = vec![];
        for r in rows {
            let p: PluginRecord = serde_json::from_str(r.get("record"))?;
            let health = if !p.enabled {
                Err(anyhow::anyhow!("Provider disabled"))
            } else if p.manifest.host == PluginHost::Desktop {
                self.desktop_provider_health(&p).await
            } else {
                self.providers.provider_health(&p.id)
            };
            statuses.push(PluginCapabilityStatus {
                capability: r.get("capability"),
                provider_id: p.id,
                host: p.manifest.host,
                available: health.is_ok(),
                detail: health.err().map(|e| e.to_string()).unwrap_or_else(|| {
                    "Provider available; input permissions are checked on use".into()
                }),
                selection_version: r.get("version"),
            });
        }
        Ok(statuses)
    }
    pub async fn plugin(&self, id: &str) -> Result<PluginRecord> {
        load(&mut self.pool.begin().await?, id).await
    }
    pub async fn install_plugin(&self, req: PluginInstallRequest) -> Result<PluginRecord> {
        validate_manifest(&req.manifest)?;
        validate_configuration(&req.manifest, &req.configuration)?;
        if req.manifest.id.starts_with("thesaurus.") || req.manifest.host != PluginHost::Service {
            bail!("invalid: user plugins must target the service and use their own namespace");
        }
        let program = Path::new(&req.executable.program);
        if !program.is_absolute()
            || !program.is_file()
            || req.executable.args.len() > 32
            || req
                .executable
                .args
                .iter()
                .any(|a| a.len() > 4096 || a.contains('\0'))
        {
            bail!("invalid: choose an existing absolute plugin executable and bounded arguments");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "plugin-install",
            &req.manifest.id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_some() {
            return load(&mut tx, &req.manifest.id).await;
        }
        validate_grant(&mut tx, &req.manifest, &req.grant).await?;
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM plugins WHERE id=?)")
            .bind(&req.manifest.id)
            .fetch_one(&mut *tx)
            .await?;
        if exists {
            bail!("conflict: plugin already installed; update it explicitly");
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM plugins WHERE builtin=0")
            .fetch_one(&mut *tx)
            .await?;
        if count >= 50 {
            bail!("invalid: at most 50 user plugins may be installed");
        }
        validation::validate_dependency_graph(&mut tx, &req.manifest).await?;
        let configuration =
            validation::configured_values(&req.manifest, req.configuration.clone())?;
        let p = PluginRecord {
            id: req.manifest.id.clone(),
            version: 1,
            manifest: req.manifest,
            executable: Some(req.executable),
            builtin: false,
            enabled: false,
            grant: req.grant,
            configuration,
            installed_at: now(),
            updated_at: now(),
        };
        save(&mut tx, &p).await?;
        // New subscriptions start now. Replays require explicit delivery retry.
        sqlx::query("INSERT INTO plugin_cursors(plugin_id,sequence) SELECT ?,COALESCE(MAX(sequence),0) FROM plugin_events").bind(&p.id).execute(&mut *tx).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &p.id).await?;
        tx.commit().await?;
        Ok(p)
    }
    pub async fn update_plugin(&self, id: &str, req: PluginUpdateRequest) -> Result<PluginRecord> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "plugin-update", id, &req.idempotency_key, &req).await?;
        let mut p = load(&mut tx, id).await?;
        if old.is_some() {
            return Ok(p);
        }
        if req.expected_version != p.version {
            bail!("conflict: plugin changed; refresh first");
        }
        if p.builtin || req.manifest.is_some() || req.executable.is_some() {
            ensure_idle(&mut tx, &p).await?;
        } else {
            ensure_resources_idle(&mut tx, &p).await?;
        }
        if req.manifest.is_some() || req.executable.is_some() {
            if p.builtin || p.enabled || req.enabled {
                bail!("conflict: disable a user plugin before upgrading its package");
            }
            if let Some(m) = &req.manifest {
                validate_manifest(m)?;
                if m.id != p.id || m.host != PluginHost::Service {
                    bail!("invalid: upgrade cannot change plugin identity or host");
                }
                p.manifest = m.clone();
            }
            if let Some(exe) = &req.executable {
                if !Path::new(&exe.program).is_absolute()
                    || !Path::new(&exe.program).is_file()
                    || exe.args.len() > 32
                    || exe.args.iter().any(|a| a.len() > 4096 || a.contains('\0'))
                {
                    bail!("invalid: plugin executable or arguments");
                }
                p.executable = Some(exe.clone());
            }
        }
        validate_configuration(&p.manifest, &req.configuration)?;
        validate_grant(&mut tx, &p.manifest, &req.grant).await?;
        validation::validate_dependency_graph(&mut tx, &p.manifest).await?;
        if req.enabled {
            for dependency in &p.manifest.dependencies {
                if !load(&mut tx, dependency).await?.enabled {
                    bail!("invalid: enable dependencies first");
                }
            }
        } else {
            for text in sqlx::query_scalar::<_, String>(
                "SELECT record FROM plugins WHERE enabled=1 AND id!=?",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?
            {
                let other: PluginRecord = serde_json::from_str(&text)?;
                if other.manifest.dependencies.iter().any(|d| d == id) {
                    bail!("conflict: an enabled plugin depends on this provider");
                }
            }
        }
        let changed_enabled = p.enabled != req.enabled;
        if p.builtin && p.manifest.host == PluginHost::Service && changed_enabled {
            self.providers.set_enabled(id, req.enabled)?;
        }
        p.enabled = req.enabled;
        p.grant = req.grant;
        p.configuration = validation::configured_values(&p.manifest, req.configuration)?;
        p.version += 1;
        p.updated_at = now();
        save(&mut tx, &p).await?;
        sqlx::query("UPDATE plugin_runs SET status='cancelled',record=json_set(record,'$.status','cancelled','$.finished_at',?,'$.error','Plugin configuration or grants changed while awaiting model work') WHERE plugin_id=? AND status='awaiting_model'").bind(now()).bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE plugin_model_proposals SET status='cancelled',record=json_set(record,'$.status','cancelled','$.error','Plugin configuration or grants changed') WHERE plugin_id=? AND status='pending_approval'").bind(id).execute(&mut *tx).await?;
        if changed_enabled {
            if p.enabled {
                sqlx::query("INSERT INTO plugin_cursors(plugin_id,sequence) SELECT ?,COALESCE(MAX(sequence),0) FROM plugin_events WHERE 1 ON CONFLICT(plugin_id) DO UPDATE SET sequence=excluded.sequence")
                    .bind(id).execute(&mut *tx).await?;
            } else {
                sqlx::query("UPDATE plugin_deliveries SET status='cancelled',updated_at=? WHERE plugin_id=? AND status IN ('pending','failed')").bind(now()).bind(id).execute(&mut *tx).await?;
            }
            emit_tx(
                &mut tx,
                if p.enabled {
                    "plugin.enabled"
                } else {
                    "plugin.disabled"
                },
                "plugin",
                id,
                p.version,
                &[],
                &[],
            )
            .await?;
        }
        remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        tx.commit().await?;
        Ok(p)
    }
    pub async fn uninstall_plugin(&self, id: &str, req: PluginUninstallRequest) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "plugin-uninstall", id, &req.idempotency_key, &req).await?;
        if old.is_some() {
            return Ok(());
        }
        let p = load(&mut tx, id).await?;
        if p.builtin || p.enabled || req.expected_version != p.version {
            bail!("conflict: disable the current user plugin version before uninstalling");
        }
        ensure_idle(&mut tx, &p).await?;
        for text in sqlx::query_scalar::<_, String>("SELECT record FROM plugins WHERE id!=?")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?
        {
            if serde_json::from_str::<PluginRecord>(&text)?
                .manifest
                .dependencies
                .iter()
                .any(|d| d == id)
            {
                bail!("conflict: another installed plugin depends on this plugin");
            }
        }
        sqlx::query("DELETE FROM plugins WHERE id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM plugin_cursors WHERE plugin_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM plugin_selections WHERE plugin_id=?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE plugin_deliveries SET status='cancelled',updated_at=? WHERE plugin_id=? AND status IN ('pending','failed')").bind(now()).bind(id).execute(&mut *tx).await?;
        sqlx::query("UPDATE plugin_runs SET status='cancelled',record=json_set(record,'$.status','cancelled','$.finished_at',?,'$.error','Plugin uninstalled before invocation') WHERE plugin_id=? AND status='queued'").bind(now()).bind(id).execute(&mut *tx).await?;
        event(&mut tx, "plugin", id).await?;
        remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn plugin_access(&self, p: &PluginRecord) -> Result<Access> {
        let mut sources = p.grant.source_ids.clone();
        let mut tx = self.pool.begin().await?;
        sources.extend(contributions::owned_sources_tx(&mut tx, p).await?);
        for project in &p.grant.project_ids {
            sources.extend(crate::continuity::project_sources(&mut tx, project).await?);
        }
        sources.sort();
        sources.dedup();
        let mut filtered = vec![];
        for source in sources {
            if crate::source_controls::check_connection_tx(
                &mut tx,
                &source,
                &format!("plugin:{}", p.id),
                p.grant.external,
            )
            .await
            .is_ok()
            {
                filtered.push(source);
            }
        }
        Ok(Access {
            contributor: format!("plugin:{}", p.id),
            source_ids: Some(filtered),
            allow_write: p.grant.operations.iter().any(|o| o == "memory.propose"),
            external: p.grant.external,
        })
    }
}
