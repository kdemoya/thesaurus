//! Host-rendered setup and read-only checks share the durable plugin queue.
//! A remembered check is evidence for one configuration/host, not permission to
//! record, sync, spend model allowance or mutate work data.
use super::*;
use contributions::{run_tx, save_run};

impl Store {
    async fn setup_signature(&self, p: &PluginRecord) -> Result<String> {
        let epoch: String = sqlx::query_scalar("SELECT epoch FROM plugin_setup_epoch WHERE id=1")
            .fetch_one(&self.pool)
            .await?;
        let capabilities = self.plugin_capabilities().await?;
        let mut dependencies = vec![];
        for id in &p.manifest.dependencies {
            let dependency = self.plugin(id).await?;
            dependencies.push((
                dependency.id,
                dependency.installed_at,
                dependency.version,
                dependency.enabled,
            ));
        }
        let hosts: Vec<(String,String)>=sqlx::query_as("SELECT host_id,token_hash FROM plugin_hosts WHERE julianday(expires_at)>julianday('now') ORDER BY host_id")
            .fetch_all(&self.pool).await?;
        // Lease secrets themselves never enter a check invocation or receipt.
        Ok(crate::store::hash(&serde_json::to_string(&(
            epoch,
            &p.installed_at,
            p.version,
            capabilities,
            hosts,
            dependencies,
        ))?))
    }
    async fn setup_prerequisites(
        &self,
        p: &PluginRecord,
        step: &PluginSetupStep,
    ) -> Result<Option<String>> {
        if !p.enabled {
            return Ok(Some(
                "Enable this plugin to use its features or run a check".into(),
            ));
        }
        if step.check && !p.builtin && !p.grant.operations.iter().any(|op| op == "setup.check") {
            return Ok(Some(
                "Review and grant setup.check before running this executable check".into(),
            ));
        }
        let caps = self.plugin_capabilities().await?;
        for prerequisite in &step.prerequisites {
            let missing=match prerequisite {
                PluginPrerequisite::Configuration{key}=>{
                    let value=p.configuration.get(key).or_else(||p.manifest.configuration.iter().find(|f|&f.key==key).and_then(|f|f.default.as_ref()));
                    (!value.is_some_and(|v|!v.is_null()&&v.as_str().is_none_or(|s|!s.trim().is_empty())))
                        .then(||format!("Configure {key}; saving a value does not verify authentication"))
                },
                PluginPrerequisite::Operation{operation}=>
                    (!p.grant.operations.contains(operation)).then(||format!("Review and grant {operation} if you want this feature")),
                PluginPrerequisite::Dependency{plugin_id}=>
                    (!self.plugin(plugin_id).await.is_ok_and(|d|d.enabled)).then(||format!("Enable dependency {plugin_id}")),
                PluginPrerequisite::Capability{capability}=>
                    (!caps.iter().any(|c|&c.capability==capability&&c.available&&(!p.builtin||!p.manifest.capabilities.iter().any(|c|&c.id==capability)||c.provider_id==p.id))).then(||format!("The selected {capability} provider is unavailable or this provider is not selected; check its configuration or connected host")),
            };
            if missing.is_some() {
                return Ok(missing);
            }
        }
        Ok(None)
    }
    pub async fn plugin_setup(&self, id: &str) -> Result<Vec<PluginSetupStatus>> {
        let p = self.plugin(id).await?;
        let signature = self.setup_signature(&p).await?;
        let mut statuses = vec![];
        for step in &p.manifest.setup {
            let mut status = PluginSetupStatus {
                plugin_id: p.id.clone(),
                configuration_version: p.version,
                step: step.clone(),
                state: PluginSetupState::Unverified,
                detail: "Run a check to verify this setup".into(),
                checked_at: None,
                check_run_id: None,
            };
            if step.check && !p.builtin && !p.grant.operations.iter().any(|op| op == "setup.check")
            {
                status.step.repair = Some(PluginSetupRepair::Grants);
            }
            if let Some(reason) = self.setup_prerequisites(&p, step).await? {
                status.state = if p.enabled {
                    PluginSetupState::NeedsSetup
                } else {
                    PluginSetupState::Unavailable
                };
                status.detail = reason;
            } else if !step.check {
                status.state = PluginSetupState::Ready;
                status.detail="Declared prerequisites are satisfied. This step does not verify invocation, authentication or input permissions".into();
            } else {
                let record:Option<String>=sqlx::query_scalar("SELECT record FROM plugin_runs WHERE plugin_id=? AND json_extract(record,'$.installed_at')=? AND json_extract(record,'$.invocation_method')='readiness' AND json_extract(record,'$.contribution.id')=? ORDER BY created_at DESC,id DESC LIMIT 1")
                    .bind(id).bind(&p.installed_at).bind(&step.id).fetch_optional(&self.pool).await?;
                if let Some(record) = record {
                    let run: PluginContributionRun = serde_json::from_str(&record)?;
                    status.check_run_id = Some(run.id);
                    if run.configuration_version != p.version
                        || run.input["setup_signature"].as_str() != Some(&signature)
                    {
                        status.detail =
                            "Configuration, provider or host changed; run a new check".into();
                    } else if matches!(run.status.as_str(), "queued" | "running" | "cancelling") {
                        status.state = PluginSetupState::Checking;
                        status.detail = format!("Setup check {}", run.status);
                    } else if run.status == "completed" {
                        let recent = run
                            .finished_at
                            .as_deref()
                            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                            .is_some_and(|t| {
                                (0..600).contains(
                                    &(chrono::Utc::now() - t.with_timezone(&chrono::Utc))
                                        .num_seconds(),
                                )
                            });
                        status.checked_at = run.finished_at;
                        if recent {
                            if let Some(result) = run.readiness {
                                status.state = result.state;
                                status.detail = result.detail;
                            }
                        } else {
                            status.detail = "The last setup check expired; run it again".into();
                        }
                    } else {
                        status.detail = format!(
                            "Check {}. Run another check when ready; no automatic retry is used",
                            run.status
                        );
                    }
                }
            }
            statuses.push(status);
        }
        Ok(statuses)
    }
    pub async fn request_plugin_setup_check(
        &self,
        id: &str,
        step_id: &str,
        req: PluginSetupCheckRequest,
    ) -> Result<PluginContributionRun> {
        let p = self.plugin(id).await?;
        let step = p
            .manifest
            .setup
            .iter()
            .find(|s| s.id == step_id && s.check)
            .context("invalid: setup check not declared")?
            .clone();
        let signature = self.setup_signature(&p).await?;
        let missing = self.setup_prerequisites(&p, &step).await?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, replay) = operation(
            &mut tx,
            "plugin-setup-check",
            &format!("{id}:{step_id}"),
            &req.idempotency_key,
            &req,
        )
        .await?;
        let current = load(&mut tx, id).await?;
        if let Some(run) = replay {
            let run = run_tx(&mut tx, id, &run).await?;
            if run.installed_at != current.installed_at {
                bail!("conflict: check belongs to an earlier installation");
            }
            return Ok(run);
        }
        if current.version != req.expected_version
            || p.version != current.version
            || !current.enabled
        {
            bail!("conflict: enable the current plugin and refresh setup first");
        }
        if let Some(reason) = missing {
            bail!("invalid: setup prerequisite missing: {reason}");
        }
        if !p.builtin && !p.grant.operations.iter().any(|op| op == "setup.check") {
            bail!("forbidden: grant setup.check before running the executable check");
        }
        let pending:i64=sqlx::query_scalar("SELECT COUNT(*) FROM plugin_runs WHERE plugin_id=? AND status IN ('queued','running','cancelling')").bind(id).fetch_one(&mut *tx).await?;
        if pending >= 20 {
            bail!("conflict: plugin queue is full");
        }
        let run = PluginContributionRun {
            id: uuid::Uuid::new_v4().to_string(),
            plugin_id: id.into(),
            installed_at: p.installed_at.clone(),
            configuration_version: p.version,
            package_version: p.manifest.version.clone(),
            api_version: 2,
            contribution: PluginContribution {
                id: step.id.clone(),
                kind: PluginContributionKind::Command,
                title: step.title.clone(),
                description: step.description.clone(),
            },
            invocation_method: "readiness".into(),
            trigger_event: None,
            source_ids: vec![],
            project_ids: vec![],
            documents: vec![],
            tasks: vec![],
            destination_project_id: None,
            imported_documents: vec![],
            action_preview_ids: vec![],
            evaluation: None,
            evaluation_signature: None,
            evaluation_receipt: None,
            model_proposal_ids: vec![],
            model_results: vec![],
            readiness: None,
            grant: p.grant.clone(),
            configuration_hash: crate::store::hash(&serde_json::to_string(&p.configuration)?),
            executable_definition_hash: crate::store::hash(&serde_json::to_string(&p.executable)?),
            input: json!({"step_id":step.id,"setup_signature":signature}),
            event_id: uuid::Uuid::new_v4().to_string(),
            status: "queued".into(),
            created_at: now(),
            started_at: None,
            finished_at: None,
            output: None,
            error: None,
        };
        save_run(&mut tx, &run).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &run.id).await?;
        tx.commit().await?;
        Ok(run)
    }
    pub(super) async fn execute_plugin_setup_check(
        &self,
        run: &PluginContributionRun,
        p: &PluginRecord,
        dir: &Path,
    ) -> Result<PluginReadinessResult> {
        if run.input["setup_signature"].as_str() != Some(&self.setup_signature(p).await?) {
            bail!("forbidden: setup prerequisites or host changed");
        }
        let step = p
            .manifest
            .setup
            .iter()
            .find(|s| s.id == run.contribution.id && s.check)
            .context("invalid: setup step no longer declared")?;
        if self.setup_prerequisites(p, step).await?.is_some() {
            bail!("forbidden: setup prerequisites changed");
        }
        let result = if p.builtin {
            let cap = p
                .manifest
                .capabilities
                .iter()
                .find(|c| {
                    step.applies_to.contains(&c.id) && c.commands.iter().any(|c| c == "setup_check")
                })
                .context("invalid: native setup check unavailable")?;
            let selected = self
                .plugin_capabilities()
                .await?
                .into_iter()
                .find(|c| c.capability == cap.id)
                .context("unsupported: native provider unavailable")?;
            if selected.provider_id != p.id {
                bail!("forbidden: select this native provider before checking it");
            }
            let input = json!({"step_id":step.id});
            let value = if p.manifest.host == PluginHost::Service {
                self.native_invoke(&cap.id, "setup_check", input).await?
            } else {
                let command = self
                    .enqueue_desktop_command(
                        &cap.id,
                        PluginNativeRequest {
                            command: "setup_check".into(),
                            input,
                            idempotency_key: format!("setup-check:{}", run.id),
                        },
                    )
                    .await?;
                let mut result = None;
                for _ in 0..100 {
                    if let Some(reply) = self.desktop_command_result(&command).await? {
                        if reply.error.is_some() {
                            bail!("invalid: native setup check failed");
                        }
                        result = Some(reply.result);
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                result.context("invalid: native setup check timed out")?
            };
            serde_json::from_value::<PluginReadinessResult>(value)?
        } else {
            if !p.grant.operations.iter().any(|op| op == "setup.check") {
                bail!("forbidden: setup.check is no longer granted");
            }
            let invocation = PluginInvocation {
                api_version: 2,
                invocation_id: run.id.clone(),
                plugin_id: p.id.clone(),
                plugin_version: p.manifest.version.clone(),
                method: "readiness".into(),
                configuration: p.configuration.clone(),
                event: None,
                input: json!({"step":step}),
            };
            let response = worker::invoke_subprocess(
                p.executable
                    .as_ref()
                    .context("invalid: plugin executable unavailable")?,
                &invocation,
                &dir.join("plugins").join(&p.id),
            )
            .await?;
            if !response.result.is_null()
                || response.contribution.is_some()
                || !response.observations.is_empty()
                || !response.snapshots.is_empty()
                || !response.actions.is_empty()
                || !response.checks.is_empty()
                || !response.native_commands.is_empty()
                || !response.model_requests.is_empty()
            {
                bail!("invalid: readiness checks cannot return host effects or model requests");
            }
            response
                .readiness
                .context("invalid: readiness result required")?
        };
        if result.step_id != step.id
            || result.state == PluginSetupState::Checking
            || result.detail.trim().is_empty()
            || result.detail.len() > 2000
        {
            bail!("invalid: readiness result identity, state or detail");
        }
        if run.input["setup_signature"].as_str() != Some(&self.setup_signature(p).await?) {
            bail!("forbidden: setup host or prerequisites changed during check");
        }
        Ok(result)
    }
    pub(super) async fn require_plugin_setup(&self, id: &str, contribution: &str) -> Result<()> {
        if let Some(step) = self.plugin_setup(id).await?.into_iter().find(|s| {
            !s.step.optional
                && (s.step.applies_to.is_empty()
                    || s.step.applies_to.iter().any(|c| c == contribution))
                && s.state != PluginSetupState::Ready
        }) {
            bail!(
                "invalid: complete plugin setup {} first: {}",
                step.step.id,
                step.detail
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
