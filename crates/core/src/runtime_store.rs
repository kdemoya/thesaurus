use crate::{
    Store,
    store::{hash, now, token, validate_text},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sqlx::Row;
use thesaurus_contracts::*;

#[derive(Clone)]
enum RuntimeTarget {
    SetupCheck {
        signature: String,
    },
    MeetingSummary {
        id: String,
        request: MeetingSummaryRequest,
        external: bool,
        managed_generation: Option<i64>,
    },
    PluginModel {
        proposal_id: String,
        request: PluginModelApproveRequest,
        connection: crate::config::Connection,
    },
    EvalReplay {
        plan_id: String,
        case_id: String,
        request: EvalReplayStartRequest,
        config_hash: String,
    },
    EvalJudge {
        run_id: String,
        case_id: String,
        request: EvalJudgeRequest,
    },
    Fallback {
        primary: String,
        signature: String,
    },
    Recipe {
        id: String,
        request: RecipeRunRequest,
    },
    Brief {
        id: String,
        version: i64,
    },
    Thread {
        id: String,
        context_id: String,
        version: i64,
    },
}
impl Store {
    pub(crate) async fn reserve_setup_check(
        &self,
        req: SetupCheckRequest,
        c: &crate::config::Connection,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        let signature = hash(&serde_json::to_string(c)?);
        self.create_runtime_inner(
            RuntimeRequest {
                connection: c.id.clone(),
                prompt: format!("setup-check:{signature}"),
                source_ids: vec![],
                approve_usage: req.approve_usage,
                idempotency_key: req.idempotency_key,
            },
            c.daily_limit,
            Some(RuntimeTarget::SetupCheck { signature }),
        )
        .await
    }

    pub async fn create_meeting_summary_runtime(
        &self,
        id: &str,
        req: MeetingSummaryRequest,
        c: &crate::config::Connection,
        external: bool,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        self.create_runtime_inner(
            RuntimeRequest {
                connection: req.connection.clone(),
                prompt: format!("meeting-summary:{id}:{}", serde_json::to_string(&req)?),
                source_ids: vec![],
                approve_usage: req.approve_usage,
                idempotency_key: req.idempotency_key.clone(),
            },
            c.daily_limit,
            Some(RuntimeTarget::MeetingSummary {
                id: id.into(),
                request: req,
                external,
                managed_generation: None,
            }),
        )
        .await
    }

    pub async fn create_managed_meeting_summary_runtime(
        &self,
        id: &str,
        generation: i64,
        revision: i64,
        c: &crate::config::Connection,
        external: bool,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        let request = MeetingSummaryRequest {
            connection: c.id.clone(),
            expected_revision: revision,
            approve_usage: true,
            idempotency_key: format!("meeting-auto:{id}:{generation}"),
        };
        self.create_runtime_inner(
            RuntimeRequest {
                connection: c.id.clone(),
                prompt: format!("meeting-summary:{id}:{}", serde_json::to_string(&request)?),
                source_ids: vec![],
                approve_usage: true,
                idempotency_key: request.idempotency_key.clone(),
            },
            c.daily_limit,
            Some(RuntimeTarget::MeetingSummary {
                id: id.into(),
                request,
                external,
                managed_generation: Some(generation),
            }),
        )
        .await
    }

    pub async fn create_plugin_model_runtime(
        &self,
        proposal: &str,
        req: PluginModelApproveRequest,
        c: &crate::config::Connection,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        self.create_runtime_inner(
            RuntimeRequest {
                connection: c.id.clone(),
                prompt: format!("plugin-model:{proposal}:{}", serde_json::to_string(&req)?),
                source_ids: vec![],
                approve_usage: req.approve_usage,
                idempotency_key: req.idempotency_key.clone(),
            },
            c.daily_limit,
            Some(RuntimeTarget::PluginModel {
                proposal_id: proposal.into(),
                request: req,
                connection: c.clone(),
            }),
        )
        .await
    }
    pub async fn migrate_runtime(&self) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS runtime_runs (id TEXT PRIMARY KEY, connection TEXT NOT NULL, status TEXT NOT NULL, idempotency_key TEXT NOT NULL UNIQUE, request_hash TEXT NOT NULL, grant_id TEXT NOT NULL REFERENCES grants(id), record TEXT NOT NULL, created_at TEXT NOT NULL); CREATE TABLE IF NOT EXISTS runtime_permissions (id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES runtime_runs(id), record TEXT NOT NULL, decision TEXT); PRAGMA user_version=2;").execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn recover_runtime(&self) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("UPDATE grants SET revoked=1 WHERE id IN (SELECT grant_id FROM runtime_runs)")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE runtime_permissions SET decision='cancelled' WHERE decision IS NULL")
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE runtime_runs SET status='interrupted',record=json_set(record,'$.status','interrupted','$.error','Service restarted; explicitly start a new run. No automatic retry.','$.updated_at',?) WHERE status IN ('starting','running','waiting_permission','cancelling')").bind(now()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn create_eval_judge_runtime(
        &self,
        run_id: &str,
        case_id: &str,
        req: EvalJudgeRequest,
        limit: i64,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        self.create_runtime_inner(
            RuntimeRequest {
                connection: req.connection.clone(),
                prompt: format!(
                    "eval-judge:{run_id}:{case_id}:{}",
                    serde_json::to_string(&req)?
                ),
                source_ids: vec![],
                approve_usage: req.approve_usage,
                idempotency_key: req.idempotency_key.clone(),
            },
            limit,
            Some(RuntimeTarget::EvalJudge {
                run_id: run_id.into(),
                case_id: case_id.into(),
                request: req,
            }),
        )
        .await
    }
    pub async fn create_eval_replay_runtime(
        &self,
        plan_id: &str,
        case_id: &str,
        req: EvalReplayStartRequest,
        connection: &str,
        config_hash: &str,
        limit: i64,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        self.create_runtime_inner(
            RuntimeRequest {
                connection: connection.into(),
                prompt: format!(
                    "eval-replay:{plan_id}:{case_id}:{}",
                    serde_json::to_string(&req)?
                ),
                source_ids: vec![],
                approve_usage: req.approve_usage,
                idempotency_key: req.idempotency_key.clone(),
            },
            limit,
            Some(RuntimeTarget::EvalReplay {
                plan_id: plan_id.into(),
                case_id: case_id.into(),
                request: req,
                config_hash: config_hash.into(),
            }),
        )
        .await
    }
    pub async fn create_runtime(
        &self,
        req: RuntimeRequest,
        limit: i64,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        self.create_runtime_inner(req, limit, None).await
    }
    pub async fn create_fallback_runtime(
        &self,
        primary: &str,
        req: FallbackRequest,
        limit: i64,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        let preview = self.fallback_preview(primary).await?;
        self.create_runtime_inner(
            RuntimeRequest {
                connection: preview.fallback_connection,
                prompt: format!("fallback:{primary}:{}", req.signature),
                source_ids: vec![],
                approve_usage: req.approve_usage,
                idempotency_key: req.idempotency_key,
            },
            limit,
            Some(RuntimeTarget::Fallback {
                primary: primary.into(),
                signature: req.signature,
            }),
        )
        .await
    }
    pub async fn create_brief_runtime(
        &self,
        id: &str,
        req: BriefGenerateRequest,
        limit: i64,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        let version = req.expected_version;
        self.create_runtime_inner(
            RuntimeRequest {
                connection: req.connection,
                prompt: format!("brief:{id}:v{version}"),
                source_ids: vec![],
                approve_usage: req.approve_usage,
                idempotency_key: req.idempotency_key,
            },
            limit,
            Some(RuntimeTarget::Brief {
                id: id.to_string(),
                version,
            }),
        )
        .await
    }
    pub async fn create_thread_runtime(
        &self,
        id: &str,
        req: ThreadResumeRequest,
        limit: i64,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        let version = req.expected_version;
        let context_id = req.context_id;
        self.create_runtime_inner(
            RuntimeRequest {
                connection: req.connection,
                prompt: format!("thread:{id}:context:{context_id}:v{version}"),
                source_ids: vec![],
                approve_usage: req.approve_usage,
                idempotency_key: req.idempotency_key,
            },
            limit,
            Some(RuntimeTarget::Thread {
                id: id.into(),
                context_id,
                version,
            }),
        )
        .await
    }
    pub async fn create_recipe_runtime(
        &self,
        id: &str,
        req: RecipeRunRequest,
        limit: i64,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        let recipe = self
            .recipe(id, Some(req.expected_version), &Access::admin())
            .await?;
        self.create_runtime_inner(
            RuntimeRequest {
                connection: recipe.connection,
                prompt: format!("recipe:{id}:{}", serde_json::to_string(&req)?),
                source_ids: vec![],
                approve_usage: req.approve_usage,
                idempotency_key: req.idempotency_key.clone(),
            },
            limit,
            Some(RuntimeTarget::Recipe {
                id: id.into(),
                request: req,
            }),
        )
        .await
    }
    async fn create_runtime_inner(
        &self,
        req: RuntimeRequest,
        limit: i64,
        target: Option<RuntimeTarget>,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        // Retrying only a rolled-back reservation cannot repeat an agent request:
        // runtime launch happens after this method returns its committed receipt.
        for attempt in 0..5 {
            match self
                .reserve_runtime_once(req.clone(), limit, target.clone())
                .await
            {
                Err(error)
                    if error
                        .downcast_ref::<sqlx::Error>()
                        .and_then(sqlx::Error::as_database_error)
                        .is_some_and(|db| {
                            db.code()
                                .and_then(|c| c.parse::<u32>().ok())
                                .is_some_and(|code| matches!(code & 255, 5 | 6))
                        }) =>
                {
                    if attempt == 4 {
                        bail!(
                            "conflict: concurrent runtime reservation changed; retry this request using the same idempotency key"
                        );
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(10u64 << attempt)).await;
                }
                result => return result,
            }
        }
        unreachable!("bounded runtime reservation retries return above")
    }
    async fn reserve_runtime_once(
        &self,
        mut req: RuntimeRequest,
        limit: i64,
        target: Option<RuntimeTarget>,
    ) -> Result<(RuntimeRun, Option<String>, String)> {
        if !req.approve_usage {
            bail!("forbidden: approve this runtime usage request");
        }
        validate_text(&req.prompt, 16000, "runtime prompt")?;
        validate_text(&req.idempotency_key, 200, "idempotency key")?;
        req.source_ids.sort();
        req.source_ids.dedup();
        if target.is_none() && (req.source_ids.is_empty() || req.source_ids.len() > 50) {
            bail!("invalid runtime source selection");
        }
        let digest = hash(&serde_json::to_string(&req)?);
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        if let Some(row) =
            sqlx::query("SELECT id,request_hash,grant_id FROM runtime_runs WHERE idempotency_key=?")
                .bind(&req.idempotency_key)
                .fetch_optional(&mut *tx)
                .await?
        {
            if row.get::<String, _>("request_hash") != digest {
                bail!("conflict: idempotency key already used with different input");
            }
            let id: String = row.get("id");
            let grant_id: String = row.get("grant_id");
            tx.commit().await?;
            return Ok((self.runtime(&id).await?, None, grant_id));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut brief_revision = None;
        let mut recipe_preview = None;
        let mut fallback_preview = None;
        let mut eval_judge_preview = None;
        let mut eval_replay_preview = None;
        let mut plugin_preview = None;
        let mut meeting_summary = None;
        match &target {
            Some(RuntimeTarget::SetupCheck { .. }) => {
                req.prompt = crate::onboarding::CHECK_PROMPT.into();
                req.source_ids.clear();
            }
            Some(RuntimeTarget::MeetingSummary {
                id: session_id,
                request,
                managed_generation,
                ..
            }) => {
                if let Some(generation) = managed_generation {
                    crate::capture::processing::claim_tx(
                        &mut tx,
                        session_id,
                        &req.connection,
                        *generation,
                        &id,
                    )
                    .await?;
                }
                let (summary, prompt) =
                    crate::capture::summaries::prepare(&mut tx, session_id, request, &id).await?;
                req.source_ids = vec![summary.source_id.clone()];
                req.prompt = prompt;
                meeting_summary = Some(summary);
            }

            Some(RuntimeTarget::PluginModel {
                proposal_id,
                request,
                connection,
            }) => {
                let p = crate::plugins::contributions::models::prepare_tx(
                    &mut tx,
                    proposal_id,
                    request,
                    connection,
                )
                .await?;
                req.prompt = p.prompt.clone();
                req.source_ids = p.source_ids.clone();
                plugin_preview = Some(p);
            }
            Some(RuntimeTarget::EvalReplay {
                plan_id,
                case_id,
                request,
                config_hash,
            }) => {
                let (p, c) = self
                    .prepare_eval_replay(
                        &mut tx,
                        plan_id,
                        case_id,
                        request,
                        &req.connection,
                        config_hash,
                    )
                    .await?;
                req.prompt = c.prompt.clone();
                req.source_ids = p.source_ids.clone();
                eval_replay_preview = Some((p, c));
            }
            Some(RuntimeTarget::EvalJudge {
                run_id,
                case_id,
                request,
            }) => {
                let p = self
                    .prepare_eval_judge(&mut tx, run_id, case_id, request)
                    .await?;
                req.prompt = p.prompt.clone();
                req.source_ids = p.source_ids.clone();
                eval_judge_preview = Some(p);
            }
            Some(RuntimeTarget::Fallback { primary, signature }) => {
                let p = self.prepare_fallback(&mut tx, primary).await?;
                if &p.signature != signature || p.fallback_connection != req.connection {
                    bail!("conflict: fallback rule or source destinations changed; preview again");
                }
                req.prompt = p.prompt.clone();
                req.source_ids = p.source_ids.clone();
                fallback_preview = Some(p);
            }
            Some(RuntimeTarget::Recipe { id, request }) => {
                let (preview, sources) = self.prepare_recipe(&mut tx, id, request).await?;
                if req.connection != preview.connection {
                    bail!("conflict: recipe connection changed; preview again");
                }
                req.prompt = preview.prompt.clone();
                req.source_ids = sources;
                recipe_preview = Some(preview);
            }
            Some(RuntimeTarget::Brief {
                id: brief_id,
                version,
            }) => {
                let (revision, prompt, sources) = self
                    .prepare_brief(&mut tx, brief_id, *version, &id, &req.connection)
                    .await?;
                req.prompt = prompt;
                req.source_ids = sources;
                brief_revision = Some(revision);
            }
            Some(RuntimeTarget::Thread {
                id: thread_id,
                context_id,
                version,
            }) => {
                let (prompt, sources) = self
                    .prepare_thread_resume(&mut tx, thread_id, context_id, *version)
                    .await?;
                req.prompt = prompt;
                req.source_ids = sources;
            }
            None => {}
        }
        let external = if let Some(RuntimeTarget::MeetingSummary { external, .. }) = &target {
            *external
        } else {
            plugin_preview.as_ref().map(|p| p.external).unwrap_or(true)
        };
        for source in &req.source_ids {
            if let Some(summary) = &meeting_summary {
                crate::capture::processing::check_destination_tx(&mut tx, summary, external)
                    .await?;
            } else {
                crate::source_controls::check_connection_tx(
                    &mut tx,
                    source,
                    &req.connection,
                    external,
                )
                .await?;
            }
            let allowed: Option<bool> =
                sqlx::query_scalar("SELECT allow_external FROM sources WHERE id=?")
                    .bind(source)
                    .fetch_optional(&mut *tx)
                    .await?;
            let managed_summary = matches!(
                &target,
                Some(RuntimeTarget::MeetingSummary {
                    managed_generation: Some(_),
                    ..
                })
            );
            if allowed.is_none() || (external && allowed != Some(true) && !managed_summary) {
                bail!("forbidden: each runtime source must allow external agents");
            }
        }
        let used = crate::usage_controls::used_tx(&mut tx, &req.connection).await?;
        let limit =
            crate::usage_controls::effective_limit_tx(&mut tx, &req.connection, limit, false)
                .await?;
        if used >= limit && !(meeting_summary.is_some() && req.connection == "local") {
            bail!("forbidden: connection daily limit reached; no fallback was used");
        }
        let active:i64=sqlx::query_scalar("SELECT COUNT(*) FROM runtime_runs WHERE status IN ('starting','running','waiting_permission','cancelling')").fetch_one(&mut *tx).await?;
        if active >= 2 {
            bail!("conflict: two runtime runs are already active");
        }
        let grant_id = uuid::Uuid::new_v4().to_string();
        let secret = token();
        let run = RuntimeRun {
            provenance: None,
            id: id.clone(),
            connection: req.connection,
            prompt: req.prompt,
            source_ids: req.source_ids,
            status: "starting".into(),
            session_id: None,
            output: String::new(),
            progress: if meeting_summary.is_some() {
                "Preparing meeting summary"
            } else if plugin_preview
                .as_ref()
                .is_some_and(|p| p.connection_kind == "openai_compatible")
            {
                "Starting selected model API"
            } else {
                "Starting ACP runtime"
            }
            .into(),
            stop_reason: None,
            error: None,
            permissions: vec![],
            created_at: now(),
            updated_at: now(),
        };
        sqlx::query("INSERT INTO grants(id,name,token_hash,source_ids,allow_write,allow_external) VALUES(?,?,?,?,0,?)").bind(&grant_id).bind(if plugin_preview.is_some(){format!("plugin-model-run:{id}")}else{format!("acp-run:{id}")}).bind(hash(&secret)).bind(serde_json::to_string(&run.source_ids)?).bind(external).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO runtime_runs(id,connection,status,idempotency_key,request_hash,grant_id,record,created_at) VALUES(?,?,'starting',?,?,?,?,?)").bind(&id).bind(&run.connection).bind(req.idempotency_key).bind(digest).bind(&grant_id).bind(serde_json::to_string(&run)?).bind(&run.created_at).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO events(kind,record_id) VALUES('runtime',?)")
            .bind(&id)
            .execute(&mut *tx)
            .await?;
        match target {
            Some(RuntimeTarget::SetupCheck { signature }) => {
                sqlx::query("INSERT INTO onboarding_checks(run_id,connection,signature,created_at) VALUES(?,?,?,?)").bind(&id).bind(&run.connection).bind(signature).bind(now()).execute(&mut *tx).await?;
            }
            Some(RuntimeTarget::MeetingSummary { .. }) => {
                crate::capture::summaries::link(
                    &mut tx,
                    &meeting_summary.context("missing meeting summary receipt")?,
                )
                .await?;
            }

            Some(RuntimeTarget::PluginModel { .. }) => {
                crate::plugins::contributions::models::link_tx(
                    &mut tx,
                    &plugin_preview.context("missing plugin preview")?,
                    &id,
                )
                .await?;
            }
            Some(RuntimeTarget::EvalReplay { .. }) => {
                let (p, c) = eval_replay_preview.context("missing replay preview")?;
                self.link_eval_replay(&mut tx, &id, &p, &c).await?;
            }
            Some(RuntimeTarget::EvalJudge { .. }) => {
                self.link_eval_judge(
                    &mut tx,
                    &id,
                    &eval_judge_preview.context("missing judge preview")?,
                )
                .await?;
            }
            Some(RuntimeTarget::Fallback { .. }) => {
                self.link_fallback(
                    &mut tx,
                    &id,
                    &fallback_preview.context("missing fallback preview")?,
                )
                .await?;
            }
            Some(RuntimeTarget::Recipe {
                id: recipe_id,
                request,
            }) => {
                self.link_recipe_run(
                    &mut tx,
                    &id,
                    &recipe_id,
                    &request,
                    recipe_preview.context("missing recipe preview")?,
                )
                .await?;
            }
            Some(RuntimeTarget::Brief { id: brief_id, .. }) => {
                self.insert_brief_revision(
                    &mut tx,
                    &brief_id,
                    &brief_revision.context("missing brief revision")?,
                )
                .await?
            }
            Some(RuntimeTarget::Thread {
                id: thread_id,
                context_id,
                ..
            }) => {
                self.link_thread_resume(&mut tx, &thread_id, &context_id, &id)
                    .await?
            }
            None => {}
        }
        tx.commit().await?;
        Ok((run, Some(secret), grant_id))
    }
    pub async fn runtime(&self, id: &str) -> Result<RuntimeRun> {
        let record: String = sqlx::query_scalar("SELECT record FROM runtime_runs WHERE id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .context("record not found")?;
        let mut run: RuntimeRun = serde_json::from_str(&record)?;
        for row in sqlx::query(
            "SELECT record,decision FROM runtime_permissions WHERE run_id=? ORDER BY rowid",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?
        {
            let mut permission: RuntimePermission = serde_json::from_str(row.get("record"))?;
            permission.decision = row.get("decision");
            run.permissions.push(permission);
        }
        Ok(run)
    }
    pub async fn runtimes(&self) -> Result<Vec<RuntimeRun>> {
        let ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM runtime_runs ORDER BY created_at DESC LIMIT 30")
                .fetch_all(&self.pool)
                .await?;
        let mut runs = vec![];
        for id in ids {
            runs.push(self.runtime(&id).await?);
        }
        Ok(runs)
    }
    pub async fn runtime_patch(&self, id: &str, mut patch: Value) -> Result<()> {
        let terminal = patch
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|s| matches!(s, "completed" | "cancelled" | "failed" | "interrupted"));
        patch["updated_at"] = json!(now());
        let patch = serde_json::to_string(&patch)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let result=sqlx::query("UPDATE runtime_runs SET record=json_patch(record,?),status=COALESCE(json_extract(?,'$.status'),status) WHERE id=? AND status IN ('starting','running','waiting_permission','cancelling') AND (status!='cancelling' OR json_extract(?,'$.status') IS NULL OR json_extract(?,'$.status') IN ('completed','cancelled','failed','interrupted'))").bind(&patch).bind(&patch).bind(id).bind(&patch).bind(&patch).execute(&mut *tx).await?;
        if result.rows_affected() > 0 {
            sqlx::query("INSERT INTO events(kind,record_id) VALUES('runtime',?)")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        if terminal {
            self.finish_brief_runtime(id).await?;
            self.finish_meeting_summary(id).await?;
        }
        Ok(())
    }
    pub async fn runtime_permission(&self, id: &str, permission: RuntimePermission) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("INSERT INTO runtime_permissions(id,run_id,record) SELECT ?,?,? WHERE EXISTS(SELECT 1 FROM runtime_runs WHERE id=? AND status IN ('starting','running','waiting_permission'))").bind(&permission.id).bind(id).bind(serde_json::to_string(&permission)?).bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        self.runtime_patch(id,json!({"status":"waiting_permission","progress":"Waiting for a one-time tool permission"})).await
    }
    pub async fn runtime_decide(&self, id: &str, permission_id: &str, option: &str) -> Result<()> {
        let run = self.runtime(id).await?;
        let permission = run
            .permissions
            .iter()
            .find(|p| p.id == permission_id && p.decision.is_none())
            .context("conflict: permission is no longer pending")?;
        if !permission
            .options
            .iter()
            .any(|o| o.id == option && matches!(o.kind.as_str(), "allow_once" | "reject_once"))
        {
            bail!("invalid one-time permission option");
        }
        let result=sqlx::query("UPDATE runtime_permissions SET decision=? WHERE id=? AND run_id=? AND decision IS NULL AND EXISTS(SELECT 1 FROM runtime_runs WHERE id=? AND status IN ('running','waiting_permission'))").bind(option).bind(permission_id).bind(id).bind(id).execute(&self.pool).await?;
        if result.rows_affected() != 1 {
            bail!("conflict: permission is no longer pending");
        }
        Ok(())
    }
    pub async fn runtime_cleanup(&self, id: &str, grant_id: &str) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("UPDATE grants SET revoked=1 WHERE id=?")
            .bind(grant_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE runtime_permissions SET decision='cancelled' WHERE run_id=? AND decision IS NULL").bind(id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
