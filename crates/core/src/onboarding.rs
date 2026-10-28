//! Workspace-backed setup. Saved progress never substitutes for current readiness.
use crate::{
    Config, Store,
    config::Connection,
    continuity::{event, operation, remember},
    private_dir, private_write,
    store::{hash, now, validate_text},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use thesaurus_contracts::*;

pub const CHECK_PROMPT: &str = "This is a synthetic Thesaurus connection check. Reply with READY. No work data is included. Do not use tools, read files, execute commands or contact other services.";
impl Store {
    pub async fn migrate_onboarding(&self) -> Result<()> {
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS onboarding_progress(id INTEGER PRIMARY KEY CHECK(id=1),record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS setup_connections(id TEXT PRIMARY KEY,record TEXT NOT NULL); CREATE TABLE IF NOT EXISTS onboarding_checks(run_id TEXT PRIMARY KEY REFERENCES runtime_runs(id),connection TEXT NOT NULL,signature TEXT NOT NULL,created_at TEXT NOT NULL);").execute(&self.pool).await?;
        Ok(())
    }
    pub async fn onboarding_progress(&self) -> Result<OnboardingProgress> {
        let s: Option<String> =
            sqlx::query_scalar("SELECT record FROM onboarding_progress WHERE id=1")
                .fetch_optional(&self.pool)
                .await?;
        s.map(|s| serde_json::from_str(&s).map_err(Into::into))
            .unwrap_or_else(|| Ok(OnboardingProgress::default()))
    }
    /// Managed connections are append-only: changing primary never changes an in-flight account.
    pub async fn model_config(&self, base: &Config) -> Result<Config> {
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT record FROM setup_connections ORDER BY id")
                .fetch_all(&self.pool)
                .await?;
        let mut config = base.clone();
        for row in rows {
            let c: Connection = serde_json::from_str(&row)?;
            if config.connections.iter().any(|b| b.id == c.id) {
                bail!(
                    "invalid: a config.toml connection conflicts with a connection saved in Settings; use distinct IDs"
                );
            }
            config.connections.push(c);
        }
        config.validate()?;
        Ok(config)
    }
    pub async fn save_onboarding(
        &self,
        req: OnboardingSave,
        config: &Config,
    ) -> Result<OnboardingProgress> {
        if !["record", "context", "memory"].contains(&req.goal.as_str())
            || req.step > 4
            || !["active", "deferred", "complete"].contains(&req.status.as_str())
        {
            bail!("invalid: onboarding goal, step or status");
        }
        if req.status == "complete"
            && req.primary_connection != "local"
            && !config
                .connections
                .iter()
                .any(|c| c.id == req.primary_connection)
        {
            bail!("invalid: select a configured primary connection");
        }
        if req.status == "complete" && req.goal == "record" {
            let r = self.recording_setup_readiness().await;
            if !["ready", "configured"].contains(&r.state.as_str()) {
                bail!("invalid: prepare the selected audio configuration, or choose Set up later");
            }
        }
        if let Some(id) = &req.memory_grant_id {
            if req.status == "complete" && req.goal == "memory" {
                self.setup_memory_config(id).await?;
            }
        }
        if req.status == "complete" && req.goal == "memory" && req.memory_grant_id.is_none() {
            bail!("invalid: prepare scoped memory access, or choose Set up later");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "onboarding",
            "workspace",
            &req.idempotency_key,
            &req,
        )
        .await?;
        let saved: Option<String> =
            sqlx::query_scalar("SELECT record FROM onboarding_progress WHERE id=1")
                .fetch_optional(&mut *tx)
                .await?;
        let previous: OnboardingProgress = saved
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        if old.is_some() {
            tx.commit().await?;
            return Ok(previous);
        }
        if previous.version != req.expected_version {
            bail!("conflict: setup changed in another window; reload its progress before saving");
        }
        let p = OnboardingProgress {
            version: previous.version + 1,
            goal: req.goal,
            step: req.step,
            primary_connection: req.primary_connection,
            status: req.status,
            memory_grant_id: req.memory_grant_id,
            updated_at: Some(now()),
        };
        sqlx::query("INSERT INTO onboarding_progress(id,record) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record").bind(serde_json::to_string(&p)?).execute(&mut *tx).await?;
        remember(&mut tx, &req.idempotency_key, &digest, "workspace").await?;
        event(&mut tx, "onboarding", "workspace").await?;
        tx.commit().await?;
        Ok(p)
    }
    pub async fn create_setup_connection(
        &self,
        req: SetupConnectionRequest,
        base: &Config,
    ) -> Result<ConnectionInfo> {
        validate_text(&req.id, 80, "connection ID")?;
        let current = self.model_config(base).await?;
        let fingerprint = hash(&serde_json::to_string(
            &json!({"id":req.id,"kind":req.kind,"model":req.model,"endpoint":req.endpoint,"command":req.command,"args":req.args,"credential_ref":req.credential_ref,"key_digest":req.api_key.as_deref().map(hash),"daily_limit":req.daily_limit}),
        )?);
        let reference = if req.api_key.as_ref().is_some_and(|s| !s.is_empty()) {
            if req.kind != "openai_compatible"
                || req.credential_ref.as_ref().is_some_and(|r| !r.is_empty())
            {
                bail!(
                    "invalid: use either an API key or a credential reference for an API connection"
                );
            }
            Some(format!(
                "keychain:models.{}",
                &hash(&req.idempotency_key)[..24]
            ))
        } else {
            req.credential_ref.filter(|r| !r.is_empty())
        };
        let c = Connection {
            id: req.id.clone(),
            kind: req.kind,
            model: req.model,
            endpoint: req.endpoint,
            command: req.command,
            args: req.args,
            token_env: None,
            credential_ref: reference.clone(),
            daily_limit: req.daily_limit,
        };
        if c.model.len() > 200
            || c.endpoint.as_ref().is_some_and(|s| s.len() > 2000)
            || c.command
                .as_ref()
                .is_some_and(|s| s.len() > 4096 || !Path::new(s).is_absolute())
            || c.args.len() > 32
            || c.args.iter().any(|s| s.len() > 4096 || s.contains('\0'))
        {
            bail!(
                "invalid: connection fields exceed supported bounds; executable paths must be absolute"
            );
        }
        let intent = json!({"fingerprint":fingerprint});
        // Check idempotency before Keychain writes; never persist a plaintext key.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "setup-connection",
            "workspace",
            &req.idempotency_key,
            &intent,
        )
        .await?;
        if old.is_some() {
            tx.commit().await?;
            return crate::models::connections(&current)
                .into_iter()
                .find(|x| x.id == req.id)
                .context("invalid: saved connection is unavailable");
        }
        if current.connections.len() >= 50 || current.connections.iter().any(|x| x.id == c.id) {
            bail!("invalid: use a new connection ID; configured accounts are preserved");
        }
        let mut candidate = current.clone();
        candidate.connections.push(c.clone());
        candidate.validate()?;
        tx.commit().await?;
        if let Some(secret) = req.api_key.filter(|s| !s.is_empty()) {
            self.native_invoke(
                "native.credentials",
                "write",
                json!({"reference":reference,"secret":secret}),
            )
            .await?;
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (_, old) = operation(
            &mut tx,
            "setup-connection",
            "workspace",
            &req.idempotency_key,
            &intent,
        )
        .await?;
        if old.is_none() {
            sqlx::query("INSERT INTO setup_connections(id,record) VALUES(?,?)")
                .bind(&c.id)
                .bind(serde_json::to_string(&c)?)
                .execute(&mut *tx)
                .await
                .context(
                    "conflict: connection ID was saved in another window; reload connections",
                )?;
            remember(&mut tx, &req.idempotency_key, &digest, &c.id).await?;
            event(&mut tx, "connection", &c.id).await?;
        }
        tx.commit().await?;
        crate::models::connections(&candidate)
            .into_iter()
            .find(|x| x.id == c.id)
            .context("invalid: saved connection is unavailable")
    }
    pub async fn recording_setup_readiness(&self) -> SetupReadiness {
        let result=async {
            self.native_provider("native.audio").await?;
            let mut s=self.live_recording_settings().await?.context("invalid: audio defaults unavailable")?;
            self.resolve_transcription_settings(&mut s).await?;
            crate::capture::live::validate_settings(&s).await?;
            let remote=s.transcription.as_ref().is_some_and(|t|t.provider.is_remote());
            if remote {self.require_audio_grant(s.transcription.as_ref().unwrap()).await?;} else {
                self.native_provider("native.transcription").await?;
                if !crate::models::executable(&s.whisper_command){bail!("The selected transcription executable is missing or cannot run; repair Audio setup");}
            }
            let inputs:MeetingAudioInputs=serde_json::from_value(self.native_invoke("native.audio","inputs",json!({})).await?)?;
            if !inputs.available {bail!("Audio inputs are unavailable on the connected host");}
            if let Some(id)=s.microphone_id.as_ref(){if id!=DEFAULT_MEETING_MICROPHONE && !inputs.microphones.iter().any(|m|&m.id==id){bail!("The saved microphone is disconnected; choose another input");}}
            if s.system_audio&&!inputs.system_audio_available {bail!("System audio is unavailable; choose a microphone or another host");}
            if s.microphone_id.is_some()&&["denied","restricted"].contains(&inputs.microphone_permission.as_str()){bail!("Enable Microphone access for Thesaurus in macOS Settings, then recheck this step");}
            let system_granted=if s.system_audio {
                let check:PluginReadinessResult=serde_json::from_value(self.native_invoke("native.audio","setup_check",json!({"step_id":"system-audio"})).await?)?;
                check.state==PluginSetupState::Ready
            }else{true};
            let permission=(s.microphone_id.is_none()||inputs.microphone_permission=="granted")&&system_granted;
            Ok::<_,anyhow::Error>((if permission&&!remote {"ready"}else{"configured"},format!("Audio configuration is available. Microphone permission: {}. System-audio permission: {}. OS grants are requested only on explicit Start; cloud credentials and provider access are checked on explicit use. No audio has been captured by this check",inputs.microphone_permission,if system_granted{"granted or not required"}else{"not granted"})))
        }.await;
        let (state, detail) = match result {
            Ok((s, d)) => (s.to_string(), d),
            Err(e) => ("needs_setup".into(), e.to_string()),
        };
        SetupReadiness {
            capability: "audio".into(),
            state,
            detail,
            checked_at: now(),
            repair_step: 2,
        }
    }
    pub async fn onboarding_status(&self, base: &Config) -> Result<OnboardingStatus> {
        let config = self.model_config(base).await?;
        let progress = self.onboarding_progress().await?;
        let mut connections = vec![];
        for info in crate::models::connections(&config) {
            let c = config.connections.iter().find(|c| c.id == info.id);
            let credential_state = match c.and_then(|c| {
                c.credential_ref
                    .clone()
                    .or_else(|| c.token_env.as_ref().map(|e| format!("env:{e}")))
            }) {
                Some(r) => {
                    if let Some(name) = r.strip_prefix("env:") {
                        if std::env::var_os(name).is_some() {
                            "environment reference available; authentication unverified"
                        } else {
                            "credential unavailable"
                        }
                    } else {
                        "Keychain reference configured; checked only on explicit use"
                    }
                }
                None => {
                    if info.kind == "local_rules" {
                        "not required"
                    } else {
                        "managed by runtime / not checked"
                    }
                }
            }
            .to_string();
            let row:Option<(String,String)>=sqlx::query_as("SELECT run_id,signature FROM onboarding_checks WHERE connection=? ORDER BY created_at DESC,rowid DESC LIMIT 1").bind(&info.id).fetch_optional(&self.pool).await?;
            let check = if let (Some(c), Some((id, signature))) = (c, row) {
                if signature == hash(&serde_json::to_string(c)?) {
                    self.runtime(&id).await.ok()
                } else {
                    None
                }
            } else {
                None
            };
            connections.push(SetupConnection {
                destination: c
                    .and_then(|c| c.endpoint.clone().or_else(|| c.command.clone()))
                    .unwrap_or_else(|| "On this computer".into()),
                connection: info,
                credential_state,
                check,
            });
        }
        let mut plugin_setup = vec![];
        for p in self.plugins().await? {
            if p.enabled {
                if let Ok(mut rows) = self.plugin_setup(&p.id).await {
                    plugin_setup.append(&mut rows);
                }
            }
        }
        plugin_setup.truncate(100);
        let sources = self.sources(&Access::admin()).await?;
        let memory_ready = if let Some(id) = &progress.memory_grant_id {
            self.setup_memory_config(id).await.is_ok()
        } else {
            false
        };
        let readiness = vec![
            self.recording_setup_readiness().await,
            SetupReadiness {
                capability: "context".into(),
                state: "ready".into(),
                detail: format!(
                    "{} local sources. Local commitment extraction requires no model account",
                    sources.len()
                ),
                checked_at: now(),
                repair_step: 3,
            },
            SetupReadiness {
                capability: "memory".into(),
                state: if memory_ready {
                    "configured"
                } else {
                    "needs_setup"
                }
                .into(),
                detail: if memory_ready {
                    "Scoped access is prepared; configuring your agent is a separate explicit step"
                } else {
                    "Choose sources and create a separate read-only grant for your agent"
                }
                .into(),
                checked_at: now(),
                repair_step: 2,
            },
        ];
        Ok(OnboardingStatus {
            existing_workspace: !sources.is_empty() || !config.connections.is_empty(),
            progress,
            connections,
            detected_agents: detect_agents(),
            readiness,
            plugin_setup,
        })
    }
    pub async fn create_onboarding_check(
        &self,
        req: SetupCheckRequest,
        c: &Connection,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        if !["acp", "openai_compatible"].contains(&c.kind.as_str()) {
            bail!(
                "invalid: live connection checks support ACP and compatible API connections; CLI extraction remains unverified"
            );
        }
        self.reserve_setup_check(req, c).await
    }
    pub async fn onboarding_check_authority(
        &self,
        run: &RuntimeRun,
        c: &Connection,
    ) -> Result<bool> {
        let signature: Option<String> = sqlx::query_scalar(
            "SELECT signature FROM onboarding_checks WHERE run_id=? AND connection=?",
        )
        .bind(&run.id)
        .bind(&c.id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(signature) = signature else {
            return Ok(false);
        };
        if signature != hash(&serde_json::to_string(c)?)
            || run.prompt != CHECK_PROMPT
            || !run.source_ids.is_empty()
        {
            bail!("forbidden: connection check authority changed");
        }
        Ok(true)
    }
    pub async fn setup_memory(&self, req: SetupMemoryRequest) -> Result<Value> {
        if !req.approve_access {
            bail!("forbidden: explicitly approve these source grants");
        }
        validate_text(&req.name, 100, "agent name")?;
        if req.source_ids.is_empty() || req.source_ids.len() > 50 {
            bail!("invalid: choose 1–50 sources");
        }
        let sources = self.sources(&Access::admin()).await?;
        if req.source_ids.iter().any(|id| {
            !sources
                .iter()
                .any(|s| &s.id == id && (req.local_agent || s.allow_external))
        }) {
            bail!("invalid: sources must exist and allow external sharing for an external agent");
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "onboarding-memory",
            "workspace",
            &req.idempotency_key,
            &req,
        )
        .await?;
        let id = if let Some(id) = old {
            id
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            let secret = crate::store::token();
            sqlx::query("INSERT INTO grants(id,name,token_hash,source_ids,allow_write,allow_external) VALUES(?,?,?,?,0,?)").bind(&id).bind(&req.name).bind(hash(&secret)).bind(serde_json::to_string(&req.source_ids)?).bind(!req.local_agent).execute(&mut *tx).await?;
            let folder = self.data_dir.join("grants");
            private_dir(&folder)?;
            private_write(
                &folder.join(format!("{id}.json")),
                &serde_json::to_vec(&json!({"token":secret}))?,
            )?;
            remember(&mut tx, &req.idempotency_key, &digest, &id).await?;
            event(&mut tx, "grant", &id).await?;
            id
        };
        tx.commit().await?;
        self.setup_memory_config(&id).await
    }
    pub async fn setup_memory_config(&self, id: &str) -> Result<Value> {
        let active = self
            .grants()
            .await?
            .into_iter()
            .find(|g| g.id == id && !g.revoked && !g.allow_write && !g.source_ids.is_empty())
            .context(
                "invalid: memory access is missing, revoked or not a scoped read-only grant",
            )?;
        let allowed = self
            .filter_grant_destinations(&active.id, active.source_ids.clone(), active.allow_external)
            .await?;
        if allowed.len() != active.source_ids.len() {
            bail!(
                "invalid: one or more selected sources no longer allow this grant; review access before continuing"
            );
        }
        let path = self.data_dir.join("grants").join(format!("{id}.json"));
        use std::os::unix::fs::PermissionsExt;
        if !std::fs::symlink_metadata(&path)
            .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o077 == 0)
        {
            bail!(
                "invalid: restore a private regular grant file (0600), or revoke this grant and create a new one"
            );
        }
        Ok(
            json!({"grant":active,"mcpServers":{"thesaurus":{"command":std::env::current_exe()?,"args":["--data-dir",self.data_dir,"mcp","serve","--grant-file",path]}}}),
        )
    }
}
fn detect_agents() -> Vec<DetectedAgent> {
    let mut paths: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(Path::new(&home).join(".local/bin"));
        paths.push(Path::new(&home).join(".npm-global/bin"));
    }
    paths.extend([
        PathBuf::from("/opt/homebrew/bin"),
        PathBuf::from("/usr/local/bin"),
    ]);
    [("Codex ACP","codex-acp","acp"),("Claude ACP","claude-agent-acp","acp"),("Codex CLI","codex","codex_cli"),("Claude CLI","claude","claude_cli")].into_iter().filter_map(|(name,binary,kind)|paths.iter().map(|p|p.join(binary)).find(|p|crate::models::executable(&p.to_string_lossy())).map(|p|DetectedAgent {name:name.into(),kind:kind.into(),command:p.to_string_lossy().into_owned(),detail:if kind=="acp"{"Installed; authentication and model access are unverified"}else{"Installed; supports document extraction. Use an ACP adapter for meeting summaries and agent runs"}.into()})).collect()
}
