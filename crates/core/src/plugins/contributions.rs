//! Durable, explicitly requested extension work. User code runs outside database
//! transactions; validated host effects and the result commit together.
use super::*;
mod effects;
mod evaluation;
pub mod models;
pub(crate) use effects::validate_action_origin_tx;
use effects::{import_snapshot_tx, propose_action_tx, sync_event_tx};

fn supported(kind: &PluginContributionKind) -> bool {
    matches!(
        kind,
        PluginContributionKind::Command
            | PluginContributionKind::Processor
            | PluginContributionKind::Panel
            | PluginContributionKind::Connector
            | PluginContributionKind::Action
            | PluginContributionKind::Evaluation
    )
}
fn permission(kind: &PluginContributionKind) -> &'static str {
    match kind {
        PluginContributionKind::Connector => "connectors.sync",
        PluginContributionKind::Action => "actions.propose",
        PluginContributionKind::Evaluation => "eval.check",
        _ => "commands.invoke",
    }
}
pub(super) async fn run_tx(
    tx: &mut Transaction<'_, Sqlite>,
    plugin: &str,
    id: &str,
) -> Result<PluginContributionRun> {
    let record: String =
        sqlx::query_scalar("SELECT record FROM plugin_runs WHERE plugin_id=? AND id=?")
            .bind(plugin)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await?
            .context("record not found")?;
    Ok(serde_json::from_str(&record)?)
}
pub(super) async fn save_run(
    tx: &mut Transaction<'_, Sqlite>,
    run: &PluginContributionRun,
) -> Result<()> {
    sqlx::query("INSERT INTO plugin_runs(id,plugin_id,status,created_at,record) VALUES(?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET status=excluded.status,record=excluded.record")
        .bind(&run.id).bind(&run.plugin_id).bind(&run.status).bind(&run.created_at).bind(serde_json::to_string(run)?).execute(&mut **tx).await?;
    event(tx, "plugin_run", &run.id).await
}
pub(super) async fn scope_tx(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    sources: &[String],
    projects: &[String],
) -> Result<()> {
    if sources.len() > 100
        || projects.len() > 50
        || projects.iter().any(|id| !p.grant.project_ids.contains(id))
    {
        bail!("forbidden: contribution scope exceeds its grant");
    }
    let mut allowed = p.grant.source_ids.clone();
    allowed.extend(owned_sources_tx(tx, p).await?);
    for project in &p.grant.project_ids {
        allowed.extend(crate::continuity::project_sources(tx, project).await?);
    }
    for source in sources {
        let external: Option<bool> =
            sqlx::query_scalar("SELECT allow_external FROM sources WHERE id=?")
                .bind(source)
                .fetch_optional(&mut **tx)
                .await?;
        if !allowed.contains(source) || !external.is_some_and(|v| !p.grant.external || v) {
            bail!("forbidden: current source policy does not permit this contribution");
        }
        crate::source_controls::check_connection_tx(
            tx,
            source,
            &format!("plugin:{}", p.id),
            p.grant.external,
        )
        .await?;
    }
    Ok(())
}
async fn current_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run: &PluginContributionRun,
) -> Result<PluginRecord> {
    let p = load(tx, &run.plugin_id).await?;
    let current = run_tx(tx, &run.plugin_id, &run.id).await?;
    if !p.enabled
        || p.installed_at != run.installed_at
        || p.version != run.configuration_version
        || current.status != "running"
    {
        bail!("forbidden: plugin changed or contribution was cancelled");
    }
    scope_tx(tx, &p, &run.source_ids, &run.project_ids).await?;
    if let Some(target) = &run.evaluation {
        let (_, input) = evaluation::input_tx(tx, &p, target).await?;
        if Some(input.signature) != run.evaluation_signature {
            bail!("forbidden: saved evaluation changed before execution");
        }
    }
    if let Some(project) = &run.destination_project_id {
        if !p.grant.project_ids.contains(project) {
            bail!("forbidden: connector destination is no longer granted");
        }
        crate::continuity::project_sources(tx, project).await?;
    }
    Ok(p)
}
pub(super) async fn owned_sources_tx(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
) -> Result<Vec<String>> {
    if !p.grant.operations.iter().any(|op| op == "connectors.sync") {
        return Ok(vec![]);
    }
    Ok(sqlx::query_scalar("SELECT source_id FROM plugin_sources WHERE plugin_id=? AND installed_at=? ORDER BY source_id")
        .bind(&p.id).bind(&p.installed_at).fetch_all(&mut **tx).await?)
}
fn evidence_valid(e: &Evidence, docs: &[Document]) -> bool {
    e.line > 0
        && docs.iter().any(|d| {
            d.id == e.document_id
                && d.revision == e.revision
                && d.body.lines().nth(e.line as usize - 1) == Some(e.quote.as_str())
        })
}
fn validate_output(
    run: &PluginContributionRun,
    response: &PluginResponse,
    docs: &[Document],
) -> Result<()> {
    native_requests::validate_response(response)?;
    if !response.native_commands.is_empty()
        && run.invocation_method != "event"
        && !matches!(
            run.contribution.kind,
            PluginContributionKind::Command | PluginContributionKind::Processor
        )
    {
        bail!(
            "invalid: only commands/processors and lifecycle continuations may request native commands"
        );
    }
    if response.readiness.is_some() {
        bail!("invalid: readiness output is only valid for an explicit setup check");
    }
    if !response.model_requests.is_empty() {
        models::validate_requests(response)?;
        return Ok(());
    }
    if run.invocation_method == "event" {
        if response.contribution.is_some()
            || !response.snapshots.is_empty()
            || !response.actions.is_empty()
            || !response.checks.is_empty()
            || response.observations.len() > 10
        {
            bail!("invalid: lifecycle model continuation returned unsupported effects");
        }
        for observation in &response.observations {
            if observation.source_ids.is_empty()
                || observation
                    .source_ids
                    .iter()
                    .any(|id| !run.source_ids.contains(id))
                || observation
                    .evidence
                    .iter()
                    .any(|e| !evidence_valid(e, docs))
            {
                bail!("invalid: lifecycle model effects exceed pinned event input");
            }
        }
        return Ok(());
    }
    let output = response
        .contribution
        .as_ref()
        .context("invalid: contribution output is required")?;
    validate_text(&output.text, 16000, "contribution result")?;
    if !response.result.is_null()
        || output.evidence.len() > 50
        || output.evidence.iter().any(|e| !evidence_valid(e, docs))
        || response.observations.len() > 10
        || response.snapshots.len() > 10
        || response.actions.len() > 10
        || response.checks.len() > 50
    {
        bail!("invalid: contribution output or evidence exceeds its pinned input");
    }
    if !response.checks.is_empty() && run.contribution.kind != PluginContributionKind::Evaluation {
        bail!("invalid: only evaluation contributions may return checks");
    }
    if !response.snapshots.is_empty() && run.contribution.kind != PluginContributionKind::Connector
    {
        bail!("invalid: only connector contributions may import snapshots");
    }
    if !response.actions.is_empty() && run.contribution.kind != PluginContributionKind::Action {
        bail!("invalid: only action contributions may propose actions");
    }
    if matches!(
        run.contribution.kind,
        PluginContributionKind::Connector
            | PluginContributionKind::Action
            | PluginContributionKind::Evaluation
    ) && !response.observations.is_empty()
    {
        bail!("invalid: connector/action contributions cannot propose unrelated memory effects");
    }
    let mut keys = HashSet::new();
    for snapshot in &response.snapshots {
        if !validation::identifier(&snapshot.key) || !keys.insert(&snapshot.key) {
            bail!("invalid: snapshot keys must be unique identifiers");
        }
    }
    for action in &response.actions {
        validate_text(&action.rationale, 2000, "action rationale")?;
        if action.provider != "task_status"
            || action.evidence.is_empty()
            || action.evidence.len() > 10
            || action.evidence.iter().any(|e| !evidence_valid(e, docs))
            || !run
                .tasks
                .iter()
                .any(|t| t.task_id == action.target_id && t.version == action.expected_version)
        {
            bail!("invalid: action must target a supplied task version and cite pinned input");
        }
    }
    for observation in &response.observations {
        if observation.source_ids.is_empty()
            || observation
                .source_ids
                .iter()
                .any(|id| !run.source_ids.contains(id))
            || observation.evidence.is_empty()
            || observation
                .evidence
                .iter()
                .any(|e| !evidence_valid(e, docs))
        {
            bail!("invalid: contribution observations require exact pinned evidence");
        }
    }
    if run.contribution.kind == PluginContributionKind::Panel {
        if !response.observations.is_empty() {
            bail!("invalid: a panel cannot mutate memory");
        }
        let panel = output
            .panel
            .as_ref()
            .context("invalid: panel output is required")?;
        validate_text(&panel.title, 120, "panel title")?;
        if panel.sections.len() > 12 {
            bail!("invalid: too many panel sections");
        }
        for section in &panel.sections {
            validate_text(&section.heading, 120, "panel heading")?;
            validate_text(&section.text, 8000, "panel text")?;
            if section.evidence.len() > 20
                || section.evidence.iter().any(|e| !evidence_valid(e, docs))
            {
                bail!("invalid: panel evidence does not match pinned input");
            }
        }
    } else if output.panel.is_some() {
        bail!("invalid: only a declared panel can return panel content");
    }
    Ok(())
}

impl Store {
    pub async fn request_plugin_contribution(
        &self,
        plugin: &str,
        req: PluginContributionRequest,
    ) -> Result<PluginContributionRun> {
        if req.source_ids.len() > 100
            || req.project_ids.len() > 50
            || serde_json::to_vec(&req.input)?.len() > 32_000
            || !req.input.is_object()
        {
            bail!("invalid: contribution parameters must be an object of at most 32 KB");
        }
        let setup_error = self
            .require_plugin_setup(plugin, &req.contribution_id)
            .await
            .err();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, replay) = operation(
            &mut tx,
            "plugin-contribution",
            plugin,
            &req.idempotency_key,
            &req,
        )
        .await?;
        let p = load(&mut tx, plugin).await?;
        if let Some(id) = replay {
            let run = run_tx(&mut tx, plugin, &id).await?;
            if run.installed_at != p.installed_at {
                bail!("conflict: request belongs to an earlier plugin installation");
            }
            return Ok(run);
        }
        if let Some(error) = setup_error {
            return Err(error);
        }
        if !p.enabled
            || p.builtin
            || p.manifest.host != PluginHost::Service
            || p.version != req.expected_version
        {
            bail!("conflict: enable a current service plugin before running its contribution");
        }
        if validation::negotiated_api(&p.manifest)? < 2 {
            bail!("invalid: contribution invocation requires plugin API 2");
        }
        let contribution = p
            .manifest
            .contributions
            .iter()
            .find(|c| c.id == req.contribution_id)
            .context("invalid: contribution not declared")?
            .clone();
        if !supported(&contribution.kind) {
            bail!("invalid: this contribution type is not executable by this host yet");
        }
        if let Some(project) = &req.destination_project_id {
            if contribution.kind != PluginContributionKind::Connector
                || !p.grant.project_ids.contains(project)
            {
                bail!("forbidden: choose a granted connector destination project");
            }
            crate::continuity::project_sources(&mut tx, project).await?;
        }
        if (contribution.kind == PluginContributionKind::Evaluation) != req.evaluation.is_some() {
            bail!(
                "invalid: evaluation contributions require a saved evaluation run, and other contributions cannot select one"
            );
        }
        if !p
            .grant
            .operations
            .iter()
            .any(|op| op == permission(&contribution.kind))
        {
            bail!("forbidden: contribution operation has not been granted");
        }
        let evaluation_input = if let Some(target) = &req.evaluation {
            if !req.source_ids.is_empty() || !req.project_ids.is_empty() {
                bail!("invalid: evaluation input uses the saved run's exact source scope");
            }
            Some(evaluation::input_tx(&mut tx, &p, target).await?)
        } else {
            None
        };
        let mut sources = if let Some((base, _)) = &evaluation_input {
            base.source_ids.clone()
        } else {
            req.source_ids
        };
        for project in &req.project_ids {
            sources.extend(crate::continuity::project_sources(&mut tx, project).await?);
        }
        sources.sort();
        sources.dedup();
        scope_tx(&mut tx, &p, &sources, &req.project_ids).await?;
        if !sources.is_empty() && !p.grant.operations.iter().any(|op| op == "sources.read") {
            bail!("forbidden: sources.read is required for source input");
        }
        let mut documents = if let Some((base, _)) = &evaluation_input {
            base.source_snapshots
                .iter()
                .map(|d| PluginDocumentRef {
                    document_id: d.id.clone(),
                    revision: d.revision,
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let mut input_bytes = 0i64;
        for source in sources.iter().filter(|_| evaluation_input.is_none()) {
            let rows = sqlx::query(
                "SELECT id,revision,length(CAST(body AS BLOB))+length(CAST(title AS BLOB)) AS input_bytes FROM documents WHERE source_id=? ORDER BY id LIMIT 21",
            )
            .bind(source)
            .fetch_all(&mut *tx)
            .await?;
            input_bytes += rows
                .iter()
                .map(|r| r.get::<i64, _>("input_bytes"))
                .sum::<i64>();
            if input_bytes > 400_000 {
                bail!("invalid: select a smaller input scope (document text exceeds 400 KB)");
            }
            documents.extend(rows.into_iter().map(|r| PluginDocumentRef {
                document_id: r.get("id"),
                revision: r.get("revision"),
            }));
            if documents.len() > 20 {
                bail!("invalid: choose a scope containing at most twenty documents");
            }
        }
        if documents.len() > 20 {
            bail!("invalid: evaluation input exceeds twenty pinned documents");
        }
        let pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM plugin_runs WHERE plugin_id=? AND status IN ('queued','running','cancelling')",
        )
        .bind(plugin)
        .fetch_one(&mut *tx)
        .await?;
        if pending >= 20 {
            bail!("conflict: plugin contribution queue is full");
        }
        let mut tasks = vec![];
        if contribution.kind == PluginContributionKind::Action {
            for source in &sources {
                for row in sqlx::query("SELECT t.* FROM tasks t JOIN documents d ON d.id=t.document_id WHERE d.source_id=? AND t.status!='superseded' ORDER BY t.id LIMIT 31").bind(source).fetch_all(&mut *tx).await? {
                    let task=crate::store::task_row(&row);
                    if documents.iter().any(|d|d.document_id==task.evidence.document_id&&d.revision==task.evidence.revision) {tasks.push(PluginTaskRef{task_id:task.id,version:task.version});}
                }
            }
            if tasks.len() > 30 {
                bail!("invalid: choose a scope containing at most thirty current tasks");
            }
        }
        let run = PluginContributionRun {
            id: uuid::Uuid::new_v4().to_string(),
            plugin_id: p.id.clone(),
            installed_at: p.installed_at.clone(),
            configuration_version: p.version,
            package_version: p.manifest.version.clone(),
            api_version: 2,
            contribution,
            invocation_method: "contribution".into(),
            trigger_event: None,
            source_ids: sources,
            project_ids: req.project_ids,
            documents,
            tasks,
            destination_project_id: req.destination_project_id,
            imported_documents: vec![],
            action_preview_ids: vec![],
            evaluation: req.evaluation,
            evaluation_signature: evaluation_input.as_ref().map(|(_, i)| i.signature.clone()),
            evaluation_receipt: None,
            model_proposal_ids: vec![],
            model_results: vec![],
            readiness: None,
            grant: p.grant.clone(),
            configuration_hash: crate::store::hash(&serde_json::to_string(&p.configuration)?),
            executable_definition_hash: crate::store::hash(&serde_json::to_string(&p.executable)?),
            input: req.input,
            event_id: uuid::Uuid::new_v4().to_string(),
            status: "queued".into(),
            created_at: now(),
            started_at: None,
            finished_at: None,
            output: None,
            error: None,
        };
        sqlx::query("INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id,origin_plugin) VALUES(?,'plugin.contribution.requested','plugin_run',?,?,?,?,?,?)")
            .bind(&run.event_id).bind(&run.id).bind(p.version).bind(serde_json::to_string(&run.source_ids)?).bind(serde_json::to_string(&run.project_ids)?).bind(&run.id).bind(plugin).execute(&mut *tx).await?;
        save_run(&mut tx, &run).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &run.id).await?;
        tx.commit().await?;
        Ok(run)
    }
    pub async fn plugin_contribution_runs(
        &self,
        plugin: &str,
    ) -> Result<Vec<PluginContributionRun>> {
        let p = self.plugin(plugin).await?;
        let rows: Vec<String> = sqlx::query_scalar("SELECT record FROM plugin_runs WHERE plugin_id=? AND json_extract(record,'$.installed_at')=? ORDER BY created_at DESC,id DESC LIMIT 100")
            .bind(plugin).bind(p.installed_at).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|s| Ok(serde_json::from_str(&s)?))
            .collect()
    }
    pub async fn plugin_contribution_run(
        &self,
        plugin: &str,
        id: &str,
    ) -> Result<PluginContributionRun> {
        let p = self.plugin(plugin).await?;
        let mut tx = self.pool.begin().await?;
        let run = run_tx(&mut tx, plugin, id).await?;
        if p.installed_at != run.installed_at {
            bail!("record not found");
        }
        Ok(run)
    }
    pub async fn cancel_plugin_contribution(
        &self,
        plugin: &str,
        id: &str,
    ) -> Result<PluginContributionRun> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let p = load(&mut tx, plugin).await?;
        let mut run = run_tx(&mut tx, plugin, id).await?;
        if p.installed_at != run.installed_at {
            bail!("record not found");
        }
        if matches!(run.status.as_str(), "queued" | "running" | "awaiting_model") {
            if run.status == "awaiting_model" {
                models::cancel_run_tx(&mut tx, &run.id).await?;
            }
            run.status = if run.status == "running" {
                "cancelling"
            } else {
                "cancelled"
            }
            .into();
            if run.status == "cancelled" {
                run.finished_at = Some(now());
            }
            run.error = Some("Cancellation requested; no host effects can commit. A running executable must stop before the next invocation starts".into());
            save_run(&mut tx, &run).await?;
        }
        tx.commit().await?;
        Ok(run)
    }
    pub async fn run_plugin_contribution(&self, dir: &Path) -> Result<bool> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let record: Option<String> = sqlx::query_scalar("SELECT r.record FROM plugin_runs r JOIN plugins p ON p.id=r.plugin_id WHERE r.status='queued' AND NOT EXISTS(SELECT 1 FROM plugin_runs active WHERE active.plugin_id=r.plugin_id AND active.status IN ('running','cancelling')) AND NOT EXISTS(SELECT 1 FROM plugin_deliveries d WHERE d.plugin_id=r.plugin_id AND d.status='running') ORDER BY r.created_at,r.id LIMIT 1").fetch_optional(&mut *tx).await?;
        let Some(record) = record else {
            return Ok(false);
        };
        let mut run: PluginContributionRun = serde_json::from_str(&record)?;
        run.status = "running".into();
        run.started_at = Some(now());
        save_run(&mut tx, &run).await?;
        tx.commit().await?;
        let result = self.execute_plugin_contribution(&run, dir).await;
        if let Err(error) = result {
            let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
            let mut current = run_tx(&mut tx, &run.plugin_id, &run.id).await?;
            if matches!(current.status.as_str(), "running" | "cancelling") {
                let denied = error.to_string().starts_with("forbidden:");
                current.status = if denied { "cancelled" } else { "failed" }.into();
                current.finished_at = Some(now());
                current.error=Some(if denied {"Plugin, scope or policy changed before completion; no host effects committed"}else{"Contribution failed or returned invalid output; inspect the plugin locally. It will not be retried automatically"}.into());
                save_run(&mut tx, &current).await?;
                if run.contribution.kind == PluginContributionKind::Connector {
                    let projects = run
                        .destination_project_id
                        .clone()
                        .into_iter()
                        .collect::<Vec<_>>();
                    sync_event_tx(
                        &mut tx,
                        &run,
                        "source.sync.failed",
                        &run.source_ids,
                        &projects,
                    )
                    .await?;
                }
            }
            tx.commit().await?;
        }
        Ok(true)
    }
    async fn execute_plugin_contribution(
        &self,
        run: &PluginContributionRun,
        dir: &Path,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let p = current_tx(&mut tx, run).await?;
        tx.commit().await?;
        if run.invocation_method == "readiness" {
            let result = tokio::select! {
                result=self.execute_plugin_setup_check(run,&p,dir)=>result?,
                _=async {loop {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    let Ok(mut tx)=self.pool.begin().await else {break};
                    if current_tx(&mut tx,run).await.is_err() {break;}
                }}=>bail!("forbidden: plugin setup check was cancelled or configuration changed"),
            };
            let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
            current_tx(&mut tx, run).await?;
            let mut completed = run.clone();
            completed.status = "completed".into();
            completed.finished_at = Some(now());
            completed.readiness = Some(result);
            save_run(&mut tx, &completed).await?;
            tx.commit().await?;
            return Ok(());
        }
        if run.invocation_method == "contribution" {
            self.require_plugin_setup(&p.id, &run.contribution.id)
                .await?;
        }
        let access = Access {
            contributor: format!("plugin:{}", p.id),
            source_ids: Some(run.source_ids.clone()),
            allow_write: false,
            external: p.grant.external,
        };
        let mut documents = vec![];
        for r in &run.documents {
            documents.push(
                self.document(&r.document_id, Some(r.revision), &access)
                    .await?,
            );
        }
        let mut tasks = vec![];
        for reference in &run.tasks {
            let task = crate::store::task_row(
                &sqlx::query("SELECT * FROM tasks WHERE id=?")
                    .bind(&reference.task_id)
                    .fetch_optional(&self.pool)
                    .await?
                    .context("forbidden: action input task no longer exists")?,
            );
            if task.version != reference.version {
                bail!("forbidden: action input task changed before invocation");
            }
            tasks.push(task);
        }
        let evaluation_input = if let Some(target) = &run.evaluation {
            let mut tx = self.pool.begin().await?;
            let (_, input) = evaluation::input_tx(&mut tx, &p, target).await?;
            tx.commit().await?;
            Some(input)
        } else {
            None
        };
        let invocation = PluginInvocation {
            api_version: 2,
            invocation_id: run.id.clone(),
            plugin_id: p.id.clone(),
            plugin_version: p.manifest.version.clone(),
            method: run.invocation_method.clone(),
            configuration: p.configuration.clone(),
            event: run.trigger_event.clone(),
            input: if run.invocation_method == "event" {
                json!({"document":documents.first(),"models":run.model_results})
            } else {
                json!({"contribution":run.contribution,"parameters":run.input,"documents":documents,"tasks":tasks,"destination_project_id":run.destination_project_id,"evaluation":evaluation_input,"models":run.model_results})
            },
        };
        let executable = p
            .executable
            .as_ref()
            .context("invalid: plugin executable unavailable")?;
        let work = dir.join("plugins").join(&p.id);
        let response = tokio::select! {
            response=worker::invoke_subprocess(executable,&invocation,&work)=>response?,
            _=async {loop {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                let Ok(mut tx)=self.pool.begin().await else {break};
                if current_tx(&mut tx,run).await.is_err() {break;}
            }}=>bail!("forbidden: contribution permission or state changed"),
        };
        validate_output(run, &response, &documents)?;
        let e = self.plugin_event(&run.event_id).await?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let p = current_tx(&mut tx, run).await?;
        let mut completed = run.clone();
        Self::enqueue_plugin_native_tx(
            &mut tx,
            &p,
            "run",
            &run.id,
            &run.source_ids,
            &run.project_ids,
            response.native_commands.clone(),
        )
        .await?;
        if !response.model_requests.is_empty() {
            models::stage_requests_tx(&mut tx, &p, &mut completed, response.model_requests).await?;
            completed.status = "awaiting_model".into();
            completed.output = response.contribution;
            save_run(&mut tx, &completed).await?;
            tx.commit().await?;
            return Ok(());
        }
        if let Some(input) = &evaluation_input {
            completed.evaluation_receipt = Some(
                evaluation::record_checks_tx(&mut tx, &p, run, input, response.checks, &documents)
                    .await?,
            );
        }
        if run.contribution.kind == PluginContributionKind::Connector {
            if !p.grant.operations.iter().any(|op| op == "connectors.sync") {
                bail!("forbidden: connector sync permission required");
            }
            sqlx::query("INSERT INTO plugin_effect_context(contributor,record) VALUES('__host_transaction__',?)").bind(json!({"plugin_id":p.id,"event_id":e.id,"correlation_id":e.correlation_id,"depth":e.depth+1}).to_string()).execute(&mut *tx).await?;
            for snapshot in response.snapshots {
                completed
                    .imported_documents
                    .push(import_snapshot_tx(&mut tx, &p, run, snapshot).await?);
            }
            let sources = completed
                .imported_documents
                .iter()
                .map(|d| d.source_id.clone())
                .collect::<Vec<_>>();
            let projects = run
                .destination_project_id
                .clone()
                .into_iter()
                .collect::<Vec<_>>();
            sync_event_tx(&mut tx, run, "source.sync.completed", &sources, &projects).await?;
            sqlx::query(
                "DELETE FROM plugin_effect_context WHERE contributor='__host_transaction__'",
            )
            .execute(&mut *tx)
            .await?;
        }
        for (index, proposal) in response.actions.into_iter().enumerate() {
            completed.action_preview_ids.push(
                propose_action_tx(&mut tx, &p, run, index, proposal)
                    .await?
                    .id,
            );
        }
        for (index, proposal) in response.observations.into_iter().enumerate() {
            Self::append_plugin_effect_tx(&mut tx, &p, &e, index, proposal).await?;
        }
        completed.status = "completed".into();
        completed.finished_at = Some(now());
        completed.output = response.contribution;
        save_run(&mut tx, &completed).await?;
        tx.commit().await?;
        for document in &completed.imported_documents {
            // Canonical imports and provenance are already committed. Derived
            // discovery refresh failure must never replay the connector.
            let _ = self.refresh_discovery_document(&document.document_id).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod evaluation_tests;
#[cfg(test)]
mod model_tests;
#[cfg(test)]
mod tests;
