use super::*;

async fn policy(tx: &mut Transaction<'_, Sqlite>, source: &str) -> Result<SourcePolicy> {
    let external: bool = sqlx::query_scalar("SELECT allow_external FROM sources WHERE id=?")
        .bind(source)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    let record: Option<String> =
        sqlx::query_scalar("SELECT record FROM source_control_policy WHERE source_id=?")
            .bind(source)
            .fetch_optional(&mut **tx)
            .await?;
    Ok(match record {
        Some(value) => serde_json::from_str(&value)?,
        None => SourcePolicy {
            source_id: source.into(),
            version: 0,
            external_enabled: external,
            denied_connections: vec![],
            denied_grants: vec![],
            updated_at: None,
        },
    })
}
pub(crate) async fn check_connection_tx(
    tx: &mut Transaction<'_, Sqlite>,
    source: &str,
    connection: &str,
    external: bool,
) -> Result<()> {
    let record = policy(tx, source).await?;
    let source_external: bool = sqlx::query_scalar("SELECT allow_external FROM sources WHERE id=?")
        .bind(source)
        .fetch_one(&mut **tx)
        .await?;
    if record.denied_connections.iter().any(|id| id == connection)
        || (external && (!record.external_enabled || !source_external))
    {
        bail!("forbidden: source destination permission was withdrawn");
    }
    Ok(())
}
impl Store {
    pub async fn check_source_destination(
        &self,
        source: &str,
        connection: &str,
        external: bool,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        check_connection_tx(&mut tx, source, connection, external).await
    }
    /// Re-evaluated on every request, including scoped MCP grants belonging to an ACP run.
    pub async fn filter_grant_destinations(
        &self,
        grant: &str,
        sources: Vec<String>,
        external: bool,
    ) -> Result<Vec<String>> {
        let mut tx = self.pool.begin().await?;
        let connection: Option<String> =
            sqlx::query_scalar("SELECT connection FROM runtime_runs WHERE grant_id=?")
                .bind(grant)
                .fetch_optional(&mut *tx)
                .await?;
        let mut allowed = Vec::new();
        for source in sources {
            let record = policy(&mut tx, &source).await?;
            if record.denied_grants.iter().any(|id| id == grant) {
                continue;
            }
            let source_external: bool =
                sqlx::query_scalar("SELECT allow_external FROM sources WHERE id=?")
                    .bind(&source)
                    .fetch_one(&mut *tx)
                    .await?;
            if external && (!source_external || !record.external_enabled) {
                continue;
            }
            if connection
                .as_ref()
                .is_some_and(|id| record.denied_connections.contains(id))
            {
                continue;
            }
            allowed.push(source);
        }
        Ok(allowed)
    }
    pub async fn source_disclosure_preview(
        &self,
        source: &str,
        config: &crate::Config,
        access: &Access,
    ) -> Result<SourceDisclosurePreview> {
        admin(access)?;
        let mut tx = self.pool.begin().await?;
        let policy = policy(&mut tx, source).await?;
        let row = sqlx::query("SELECT name,uri,allow_external FROM sources WHERE id=?")
            .bind(source)
            .fetch_one(&mut *tx)
            .await?;
        let mut document_ids = Vec::new();
        for row in sqlx::query("SELECT id FROM documents WHERE source_id=? ORDER BY id")
            .bind(source)
            .fetch_all(&mut *tx)
            .await?
        {
            document_ids.push(row.get("id"));
        }
        let mut destinations = Vec::new();
        for connection in crate::models::connections(config) {
            let external = crate::models::external(config, &connection.id)?;
            let allowed = !policy.denied_connections.contains(&connection.id)
                && (!external || (policy.external_enabled && row.get::<bool, _>("allow_external")));
            destinations.push(DestinationDisclosure {
                destination_id: connection.id.clone(),
                kind: "connection".into(),
                name: connection.id,
                external,
                allowed,
                reason: if allowed {
                    "Permitted for explicit approved requests; no automatic usage."
                } else {
                    "Blocked by this source's destination policy."
                }
                .into(),
            });
        }
        for grant_row in sqlx::query("SELECT * FROM grants ORDER BY id")
            .fetch_all(&mut *tx)
            .await?
        {
            let grant = GrantInfo {
                id: grant_row.get("id"),
                name: grant_row.get("name"),
                source_ids: serde_json::from_str(grant_row.get("source_ids"))?,
                allow_write: grant_row.get("allow_write"),
                allow_external: grant_row.get("allow_external"),
                revoked: grant_row.get("revoked"),
            };
            if !grant.source_ids.iter().any(|id| id == source) {
                continue;
            }
            let originating: Option<String> =
                sqlx::query_scalar("SELECT connection FROM runtime_runs WHERE grant_id=?")
                    .bind(&grant.id)
                    .fetch_optional(&mut *tx)
                    .await?;
            let allowed = !grant.revoked
                && !policy.denied_grants.contains(&grant.id)
                && !originating
                    .as_ref()
                    .is_some_and(|id| policy.denied_connections.contains(id))
                && (!grant.allow_external
                    || (policy.external_enabled && row.get::<bool, _>("allow_external")));
            destinations.push(DestinationDisclosure{destination_id:grant.id,kind:"grant".into(),name:grant.name,external:grant.allow_external,allowed,reason:if grant.revoked{"Grant was revoked."}else if allowed{"Scoped source documents, tasks and observations may be retrieved; grant permissions still apply."}else{"Blocked for this source by destination policy."}.into()});
        }
        for text in sqlx::query_scalar::<_, String>(
            "SELECT record FROM plugins WHERE builtin=0 OR id='thesaurus.bragbook' ORDER BY id",
        )
        .fetch_all(&mut *tx)
        .await?
        {
            let plugin: PluginRecord = serde_json::from_str(&text)?;
            let mut scoped = plugin.grant.source_ids.iter().any(|s| s == source);
            for project in &plugin.grant.project_ids {
                scoped |= crate::continuity::project_sources(&mut tx, project)
                    .await?
                    .iter()
                    .any(|s| s == source);
            }
            if plugin
                .grant
                .operations
                .iter()
                .any(|op| op == "connectors.sync")
            {
                scoped|=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM plugin_sources WHERE plugin_id=? AND installed_at=? AND source_id=?)").bind(&plugin.id).bind(&plugin.installed_at).bind(source).fetch_one(&mut *tx).await?;
            }
            if !scoped {
                continue;
            }
            let id = format!("plugin:{}", plugin.id);
            let bragbook = plugin.id == crate::plugins::bragbook::ID;
            let allowed = (plugin.enabled || bragbook)
                && !policy.denied_connections.contains(&id)
                && (!plugin.grant.external
                    || (policy.external_enabled && row.get::<bool, _>("allow_external")));
            let reason = if bragbook && allowed {
                if plugin.enabled {
                    "Local accomplishment collection is enabled. Selected evidence supports personal review and highlights export."
                } else {
                    "Collection is disabled. Retained entries remain available for personal review and export while this source is permitted."
                }
            } else if !plugin.enabled {
                "Plugin is disabled."
            } else if allowed {
                "Scoped hooks and contributions may receive metadata; document input also requires sources.read. Executables are trusted local code, not OS-sandboxed."
            } else {
                "Blocked for this source by destination policy."
            };
            destinations.push(DestinationDisclosure {
                destination_id: id,
                kind: "plugin".into(),
                name: plugin.manifest.name,
                external: plugin.grant.external,
                allowed,
                reason: reason.into(),
            });
        }
        Ok(SourceDisclosurePreview{policy,source_name:row.get("name"),source_uri:row.get("uri"),document_ids,destinations,scope_note:"An allowed destination can receive this source's documents, cited tasks and observations through an explicitly requested analysis or scoped memory access. Revocation blocks future requests and retrieval; content already sent cannot be retracted. Local capture and indexing continue independently.".into()})
    }
    pub async fn update_source_policy(
        &self,
        source: &str,
        req: SourcePolicyUpdate,
        access: &Access,
    ) -> Result<SourcePolicy> {
        admin(access)?;
        if req.denied_connections.len() > 100 || req.denied_grants.len() > 1000 {
            bail!("invalid: too many denied destinations");
        }
        for id in req
            .denied_connections
            .iter()
            .chain(req.denied_grants.iter())
        {
            validate_text(id, 200, "destination ID")?;
        }
        if req.denied_connections.iter().any(|id| id == "local") {
            bail!(
                "invalid: local capture/indexing remains available; choose a model or agent destination"
            );
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) =
            operation(&mut tx, "source-policy", source, &req.idempotency_key, &req).await?;
        let mut record = policy(&mut tx, source).await?;
        if old.is_some() {
            return Ok(record);
        }
        if record.version != req.expected_version {
            bail!("conflict: source policy changed; preview again");
        }
        record.version += 1;
        record.external_enabled = req.external_enabled;
        record.denied_connections = req.denied_connections;
        record.denied_connections.sort();
        record.denied_connections.dedup();
        record.denied_grants = req.denied_grants;
        record.denied_grants.sort();
        record.denied_grants.dedup();
        record.updated_at = Some(now());
        sqlx::query("UPDATE sources SET allow_external=? WHERE id=?")
            .bind(record.external_enabled)
            .bind(source)
            .execute(&mut *tx)
            .await?;
        save(
            &mut tx,
            "source_control_policy",
            source,
            record.version,
            &record,
        )
        .await?;
        remember(&mut tx, &req.idempotency_key, &digest, source).await?;
        tx.commit().await?;
        Ok(record)
    }
}
