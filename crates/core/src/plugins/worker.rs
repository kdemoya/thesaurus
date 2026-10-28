use super::*;
use chrono::{Duration, Utc};
use std::process::Stdio;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};

fn event_row(r: &sqlx::sqlite::SqliteRow) -> Result<PluginEvent> {
    Ok(PluginEvent {
        id: r.get("id"),
        sequence: r.get("sequence"),
        schema_version: r.get::<i64, _>("schema_version") as u32,
        kind: r.get("kind"),
        record_kind: r.get("record_kind"),
        record_id: r.get("record_id"),
        record_revision: r.get("record_revision"),
        source_ids: serde_json::from_str(r.get("source_ids"))?,
        project_ids: serde_json::from_str(r.get("project_ids"))?,
        correlation_id: r.get("correlation_id"),
        causation_id: r.get("causation_id"),
        origin_plugin: r.get("origin_plugin"),
        depth: r.get::<i64, _>("depth") as u32,
        created_at: r.get("created_at"),
    })
}
fn delivery_row(r: &sqlx::sqlite::SqliteRow) -> PluginDelivery {
    PluginDelivery {
        id: r.get("id"),
        plugin_id: r.get("plugin_id"),
        event_id: r.get("event_id"),
        event_sequence: r.get("event_sequence"),
        status: r.get("status"),
        attempts: r.get::<i64, _>("attempts") as u32,
        next_attempt_at: r.get("next_attempt_at"),
        error: r.get("error"),
        created_at: r.get("created_at"),
        updated_at: r.get("updated_at"),
    }
}

impl Store {
    pub async fn plugin_delivery_attempts(
        &self,
        plugin: &str,
        delivery: &str,
    ) -> Result<Vec<PluginDeliveryAttempt>> {
        self.plugin(plugin).await?;
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM plugin_deliveries WHERE id=? AND plugin_id=?)",
        )
        .bind(delivery)
        .bind(plugin)
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            bail!("record not found");
        }
        sqlx::query("SELECT * FROM plugin_delivery_attempts WHERE delivery_id=? AND plugin_id=? ORDER BY started_at,id LIMIT 1000").bind(delivery).bind(plugin).fetch_all(&self.pool).await?.into_iter().map(|r|Ok(PluginDeliveryAttempt{id:r.get("id"),delivery_id:r.get("delivery_id"),plugin_id:r.get("plugin_id"),configuration_version:r.get("configuration_version"),package_version:r.get("package_version"),api_version:r.get::<i64,_>("api_version") as u32,status:r.get("status"),started_at:r.get("started_at"),finished_at:r.get("finished_at"),error:r.get("error"),configuration_hash:r.get("configuration_hash"),executable_definition_hash:r.get("executable_definition_hash"),grant:r.get::<Option<String>,_>("grant_record").map(|v|serde_json::from_str(&v)).transpose()?})).collect()
    }
    pub async fn plugin_deliveries(&self, id: &str) -> Result<Vec<PluginDelivery>> {
        self.plugin(id).await?;
        Ok(sqlx::query("SELECT * FROM plugin_deliveries WHERE plugin_id=? ORDER BY event_sequence DESC LIMIT 100").bind(id).fetch_all(&self.pool).await?.iter().map(delivery_row).collect())
    }
    pub async fn plugin_event(&self, id: &str) -> Result<PluginEvent> {
        let r = sqlx::query("SELECT * FROM plugin_events WHERE id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .context("record not found")?;
        event_row(&r)
    }
    async fn plugin_event_allowed(&self, p: &PluginRecord, e: &PluginEvent) -> Result<bool> {
        if !p.enabled
            || e.schema_version != PLUGIN_EVENT_VERSION
            || e.depth >= MAX_EVENT_DEPTH
            || e.origin_plugin.as_deref() == Some(&p.id)
            || !p.manifest.hooks.contains(&e.kind)
        {
            return Ok(false);
        }
        let access = self.plugin_access(p).await?;
        let allowed = self
            .sources(&access)
            .await?
            .into_iter()
            .map(|s| s.id)
            .collect::<HashSet<_>>();
        if !e.source_ids.is_empty() {
            return Ok(e.source_ids.iter().all(|id| allowed.contains(id)));
        }
        if !e.project_ids.is_empty() {
            return Ok(e
                .project_ids
                .iter()
                .all(|id| p.grant.project_ids.contains(id)));
        }
        let permission = if matches!(e.record_kind.as_str(), "plugin" | "service") {
            "events.lifecycle"
        } else {
            "events.workspace"
        };
        Ok(p.grant.operations.iter().any(|op| op == permission))
    }
    pub async fn queue_plugin_deliveries(&self) -> Result<usize> {
        let mut count = 0;
        for p in self
            .plugins()
            .await?
            .into_iter()
            .filter(|p| p.enabled && !p.builtin && p.manifest.host == PluginHost::Service)
        {
            let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
            let current = load(&mut tx, &p.id).await?;
            if !current.enabled || current.version != p.version {
                continue;
            }
            let cursor: i64 = sqlx::query_scalar(
                "SELECT COALESCE((SELECT sequence FROM plugin_cursors WHERE plugin_id=?),0)",
            )
            .bind(&p.id)
            .fetch_one(&mut *tx)
            .await?;
            let rows = sqlx::query(
                "SELECT * FROM plugin_events WHERE sequence>? ORDER BY sequence LIMIT 100",
            )
            .bind(cursor)
            .fetch_all(&mut *tx)
            .await?;
            let mut last = cursor;
            for row in rows {
                let e = event_row(&row)?;
                last = e.sequence;
                // Scoping is rechecked at invocation and again before effects.
                if !p.manifest.hooks.contains(&e.kind)
                    || e.origin_plugin.as_deref() == Some(&p.id)
                    || e.depth >= MAX_EVENT_DEPTH
                {
                    continue;
                }
                let t = now();
                count+=sqlx::query("INSERT OR IGNORE INTO plugin_deliveries(id,plugin_id,event_id,event_sequence,status,next_attempt_at,created_at,updated_at) VALUES(?,?,?,?,'pending',?,?,?)")
                    .bind(uuid::Uuid::new_v4().to_string()).bind(&p.id).bind(&e.id).bind(e.sequence).bind(&t).bind(&t).bind(&t).execute(&mut *tx).await?.rows_affected() as usize;
            }
            sqlx::query("INSERT INTO plugin_cursors(plugin_id,sequence) VALUES(?,?) ON CONFLICT(plugin_id) DO UPDATE SET sequence=MAX(sequence,excluded.sequence)").bind(&p.id).bind(last).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        Ok(count)
    }
    pub async fn retry_plugin_delivery(&self, plugin: &str, id: &str) -> Result<()> {
        let p = self.plugin(plugin).await?;
        if !p.enabled {
            bail!("invalid: enable plugin before retrying a delivery");
        }
        let result=sqlx::query("UPDATE plugin_deliveries SET status='pending',attempts=0,next_attempt_at=?,error=NULL,updated_at=? WHERE id=? AND plugin_id=? AND status IN ('failed','rejected','cancelled')")
            .bind(now()).bind(now()).bind(id).bind(plugin).execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            bail!("conflict: delivery is not retryable");
        }
        Ok(())
    }
    async fn claim_plugin_delivery(
        &self,
    ) -> Result<Option<(PluginDelivery, PluginDeliveryAttempt)>> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        // A failed older event blocks later delivery for that plugin until it
        // succeeds or exhausts retries. Other plugins retain independent progress.
        let row=sqlx::query("SELECT d.* FROM plugin_deliveries d JOIN plugins p ON p.id=d.plugin_id WHERE p.enabled=1 AND d.status='pending' AND d.next_attempt_at<=? AND NOT EXISTS(SELECT 1 FROM plugin_runs r WHERE r.plugin_id=d.plugin_id AND r.status IN ('running','cancelling')) AND NOT EXISTS(SELECT 1 FROM plugin_deliveries active WHERE active.plugin_id=d.plugin_id AND active.status='running') AND NOT EXISTS(SELECT 1 FROM plugin_deliveries older WHERE older.plugin_id=d.plugin_id AND older.event_sequence<d.event_sequence AND older.status IN ('pending','running')) ORDER BY d.event_sequence LIMIT 1")
            .bind(now()).fetch_optional(&mut *tx).await?;
        let Some(row) = row else { return Ok(None) };
        let mut d = delivery_row(&row);
        d.status = "running".into();
        d.attempts += 1;
        d.updated_at = now();
        sqlx::query("UPDATE plugin_deliveries SET status='running',attempts=?,updated_at=? WHERE id=? AND status='pending'").bind(d.attempts).bind(&d.updated_at).bind(&d.id).execute(&mut *tx).await?;
        let plugin = load(&mut tx, &d.plugin_id).await?;
        let attempt = PluginDeliveryAttempt {
            id: uuid::Uuid::new_v4().to_string(),
            delivery_id: d.id.clone(),
            plugin_id: d.plugin_id.clone(),
            configuration_version: plugin.version,
            package_version: plugin.manifest.version.clone(),
            api_version: validation::negotiated_api(&plugin.manifest)?,
            status: "running".into(),
            started_at: now(),
            finished_at: None,
            error: None,
            configuration_hash: Some(crate::store::hash(&serde_json::to_string(
                &plugin.configuration,
            )?)),
            executable_definition_hash: Some(crate::store::hash(&serde_json::to_string(
                &plugin.executable,
            )?)),
            grant: Some(plugin.grant.clone()),
        };
        sqlx::query("INSERT INTO plugin_delivery_attempts(id,delivery_id,plugin_id,configuration_version,package_version,api_version,status,started_at,configuration_hash,executable_definition_hash,grant_record) VALUES(?,?,?,?,?,?,'running',?,?,?,?)")
            .bind(&attempt.id).bind(&d.id).bind(&plugin.id).bind(plugin.version).bind(&attempt.package_version).bind(attempt.api_version).bind(&attempt.started_at).bind(&attempt.configuration_hash).bind(&attempt.executable_definition_hash).bind(serde_json::to_string(&plugin.grant)?).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some((d, attempt)))
    }
    async fn finish_plugin_delivery(
        &self,
        d: &PluginDelivery,
        attempt: &PluginDeliveryAttempt,
        status: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let delay = (1i64 << d.attempts.min(8)).min(300);
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("UPDATE plugin_deliveries SET status=?,error=?,next_attempt_at=?,updated_at=? WHERE id=? AND status='running'")
            .bind(status).bind(error).bind((Utc::now()+Duration::seconds(delay)).to_rfc3339()).bind(now()).bind(&d.id).execute(&mut *tx).await?;
        sqlx::query("UPDATE plugin_delivery_attempts SET status=?,error=?,finished_at=? WHERE id=? AND status='running'").bind(if status=="pending" {"retry_scheduled"}else{status}).bind(error).bind(now()).bind(&attempt.id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn run_plugin_delivery(&self, dir: &Path) -> Result<bool> {
        let Some((d, attempt)) = self.claim_plugin_delivery().await? else {
            return Ok(false);
        };
        let outcome = self.execute_plugin_delivery(&d, &attempt, dir).await;
        match outcome {
            Ok(true)=>self.finish_plugin_delivery(&d,&attempt,"completed",None).await?,
            Ok(false)=>self.finish_plugin_delivery(&d,&attempt,"rejected",Some("Current grant, subscription or source policy does not permit delivery")).await?,
            Err(_)=>self.finish_plugin_delivery(&d,&attempt,if d.attempts>=5{"failed"}else{"pending"},Some("Plugin invocation failed or returned an invalid effect; inspect the plugin locally and retry")).await?,
        }
        Ok(true)
    }
    async fn append_plugin_effect(
        &self,
        p: &PluginRecord,
        e: &PluginEvent,
        index: usize,
        proposal: PluginObservationProposal,
    ) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        Self::append_plugin_effect_tx(&mut tx, p, e, index, proposal).await?;
        tx.commit().await?;
        Ok(())
    }
    pub(super) async fn append_plugin_effect_tx(
        tx: &mut Transaction<'_, Sqlite>,
        p: &PluginRecord,
        e: &PluginEvent,
        index: usize,
        proposal: PluginObservationProposal,
    ) -> Result<()> {
        validate_text(&proposal.body, 16000, "plugin observation")?;
        if proposal.source_ids.len() > 100 || proposal.evidence.len() > 50 {
            bail!("invalid: plugin effect exceeds bounds");
        }
        let current = load(tx, &p.id).await?;
        if !current.enabled
            || current.version != p.version
            || !current
                .grant
                .operations
                .iter()
                .any(|op| op == "memory.propose")
        {
            bail!("forbidden: plugin grant changed before its effect committed");
        }
        super::contributions::scope_tx(tx, &current, &proposal.source_ids, &[]).await?;
        if proposal
            .source_ids
            .iter()
            .any(|id| !e.source_ids.contains(id))
        {
            bail!("forbidden: current event does not permit this effect");
        }
        for evidence in &proposal.evidence {
            let row=sqlx::query("SELECT d.source_id,r.body FROM revisions r JOIN documents d ON d.id=r.document_id WHERE r.document_id=? AND r.revision=?")
                .bind(&evidence.document_id).bind(evidence.revision).fetch_optional(&mut **tx).await?.context("invalid: plugin evidence not found")?;
            let source: String = row.get("source_id");
            let body: String = row.get("body");
            if !proposal.source_ids.contains(&source)
                || evidence.line < 1
                || body.lines().nth(evidence.line as usize - 1) != Some(&evidence.quote)
            {
                bail!("invalid: plugin evidence locator does not match the cited revision");
            }
        }
        let contributor = format!("plugin:{}", p.id);
        let key = format!("plugin:{}:{}:{index}", p.id, e.id);
        if let Some(row)=sqlx::query("SELECT body,source_ids,evidence FROM observations WHERE contributor=? AND idempotency_key=?")
            .bind(&contributor).bind(&key).fetch_optional(&mut **tx).await? {
            let sources:Vec<String>=serde_json::from_str(row.get("source_ids"))?;
            let evidence:Vec<Evidence>=serde_json::from_str(row.get("evidence"))?;
            if row.get::<String,_>("body")!=proposal.body || sources!=proposal.source_ids || evidence!=proposal.evidence {
                bail!("conflict: plugin changed an already committed effect");
            }
            return Ok(());
        }
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO plugin_effect_context(contributor,record) VALUES(?,?) ON CONFLICT(contributor) DO UPDATE SET record=excluded.record")
            .bind(&contributor).bind(json!({"plugin_id":p.id,"event_id":e.id,"correlation_id":e.correlation_id,"depth":e.depth+1}).to_string()).execute(&mut **tx).await?;
        sqlx::query("INSERT INTO observations(id,body,contributor,source_ids,evidence,idempotency_key,created_at) VALUES(?,?,?,?,?,?,?)")
            .bind(&id).bind(&proposal.body).bind(&contributor).bind(serde_json::to_string(&proposal.source_ids)?)
            .bind(serde_json::to_string(&proposal.evidence)?).bind(&key).bind(now()).execute(&mut **tx).await?;
        sqlx::query("DELETE FROM plugin_effect_context WHERE contributor=?")
            .bind(&contributor)
            .execute(&mut **tx)
            .await?;
        event(tx, "observation", &id).await?;
        Ok(())
    }
    async fn execute_plugin_delivery(
        &self,
        d: &PluginDelivery,
        attempt: &PluginDeliveryAttempt,
        dir: &Path,
    ) -> Result<bool> {
        let p = self.plugin(&d.plugin_id).await?;
        let e = self.plugin_event(&d.event_id).await?;
        if p.version != attempt.configuration_version || !self.plugin_event_allowed(&p, &e).await? {
            return Ok(false);
        }
        if sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM plugin_runs WHERE id=? AND plugin_id=?)",
        )
        .bind(&d.id)
        .bind(&p.id)
        .fetch_one(&self.pool)
        .await?
        {
            return Ok(true);
        }
        let access = self.plugin_access(&p).await?;
        let mut input = json!({});
        if p.grant.operations.iter().any(|op| op == "sources.read") && e.record_kind == "document" {
            input["document"] = serde_json::to_value(
                self.document(&e.record_id, Some(e.record_revision), &access)
                    .await?,
            )?;
        }
        let invocation = PluginInvocation {
            api_version: validation::negotiated_api(&p.manifest)?,
            invocation_id: d.id.clone(),
            plugin_id: p.id.clone(),
            plugin_version: p.manifest.version.clone(),
            method: "event".into(),
            configuration: p.configuration.clone(),
            event: Some(e.clone()),
            input,
        };
        let work = dir.join("plugins").join(&p.id);
        let executable = p
            .executable
            .as_ref()
            .context("invalid: plugin executable unavailable")?;
        let response = tokio::select! {
            result=invoke_subprocess(executable,&invocation,&work)=>result?,
            _=async {
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    let Ok(current)=self.plugin(&p.id).await else {break};
                    if !current.enabled || current.version!=p.version {break;}
                }
            }=>return Ok(false),
        };
        if response.readiness.is_some() {
            bail!("invalid: readiness output requires an explicit setup check");
        }
        native_requests::validate_response(&response)?;
        if !response.native_commands.is_empty() {
            if response.contribution.is_some() {
                bail!("invalid: lifecycle native requests cannot return contribution output");
            }
            if !self.plugin_event_allowed(&p, &e).await? {
                return Ok(false);
            }
            let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
            let current = load(&mut tx, &p.id).await?;
            if current.version != p.version || current.installed_at != p.installed_at {
                return Ok(false);
            }
            let projects = e
                .project_ids
                .iter()
                .filter(|id| current.grant.project_ids.contains(id))
                .cloned()
                .collect::<Vec<_>>();
            Self::enqueue_plugin_native_tx(
                &mut tx,
                &current,
                "delivery",
                &d.id,
                &e.source_ids,
                &projects,
                response.native_commands,
            )
            .await?;
            tx.commit().await?;
            return Ok(true);
        }
        if !response.model_requests.is_empty() {
            self.stage_hook_model(&p, &e, &invocation, response).await?;
            return Ok(true);
        }
        if response.contribution.is_some()
            || !response.snapshots.is_empty()
            || !response.actions.is_empty()
            || !response.checks.is_empty()
            || response.observations.len() > 10
        {
            bail!("invalid: too many plugin effects");
        }
        for (index, proposal) in response.observations.into_iter().enumerate() {
            let current = self.plugin(&p.id).await?;
            if current.version != p.version || !self.plugin_event_allowed(&current, &e).await? {
                return Ok(false);
            }
            if !current
                .grant
                .operations
                .iter()
                .any(|op| op == "memory.propose")
            {
                bail!("forbidden: memory proposal permission required");
            }
            // An event cannot be used to smuggle unrelated granted context into
            // its downstream effects, and no plugin directly accepts a task.
            if proposal.source_ids.is_empty()
                || proposal
                    .source_ids
                    .iter()
                    .any(|id| !e.source_ids.contains(id))
            {
                bail!("forbidden: plugin effect must use this event's sources");
            }
            self.append_plugin_effect(&current, &e, index, proposal)
                .await?;
        }
        Ok(true)
    }
}

pub async fn invoke_subprocess(
    executable: &PluginExecutable,
    invocation: &PluginInvocation,
    work: &Path,
) -> Result<PluginResponse> {
    crate::private_dir(work)?;
    let payload = serde_json::to_vec(invocation)?;
    if payload.len() > 512_000 {
        bail!("invalid: plugin input exceeds bounds");
    }
    let mut command = Command::new(&executable.program);
    command
        .args(&executable.args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "en_US.UTF-8")
        .current_dir(work)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    let mut child = command
        .spawn()
        .context("invalid: plugin executable could not start")?;
    struct Group(Option<u32>);
    impl Drop for Group {
        fn drop(&mut self) {
            #[cfg(unix)]
            if let Some(pid) = self.0 {
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
            }
        }
    }
    let _group = Group(child.id());
    let mut stdin = child.stdin.take().context("plugin stdin unavailable")?;
    let stdout = child.stdout.take().context("plugin stdout unavailable")?;
    let stderr = child.stderr.take().context("plugin stderr unavailable")?;
    let result = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let send = async {
            stdin.write_all(&payload).await?;
            stdin.write_all(b"\n").await?;
            stdin.shutdown().await?;
            Ok::<_, anyhow::Error>(())
        };
        let output = async {
            let mut bytes = vec![];
            stdout.take(128_001).read_to_end(&mut bytes).await?;
            if bytes.len() > 128_000 {
                bail!("invalid: plugin response too large");
            }
            Ok::<_, anyhow::Error>(bytes)
        };
        let diagnostics = async {
            let mut bytes = vec![];
            stderr.take(8193).read_to_end(&mut bytes).await?;
            if bytes.len() > 8192 {
                bail!("invalid: plugin diagnostic limit exceeded");
            }
            Ok::<_, anyhow::Error>(())
        };
        let (_, bytes, _, status) = tokio::try_join!(send, output, diagnostics, async {
            Ok::<_, anyhow::Error>(child.wait().await?)
        })?;
        if !status.success() {
            bail!("invalid: plugin exited unsuccessfully");
        }
        let response: PluginResponse = serde_json::from_slice(&bytes)?;
        if response.api_version != invocation.api_version
            || response.invocation_id != invocation.invocation_id
        {
            bail!("invalid: plugin response identity mismatch");
        }
        Ok(response)
    })
    .await
    .context("invalid: plugin invocation timed out")?;
    let _ = child.kill().await;
    let _ = child.wait().await;
    result
}
