use super::*;

pub(super) async fn sync_event_tx(
    tx: &mut Transaction<'_, Sqlite>,
    run: &PluginContributionRun,
    kind: &str,
    sources: &[String],
    projects: &[String],
) -> Result<()> {
    sqlx::query("INSERT INTO plugin_events(id,kind,record_kind,record_id,record_revision,source_ids,project_ids,correlation_id,causation_id,origin_plugin,depth) VALUES(?,?,'plugin_run',?,?,?,?,?,?,?,1)")
        .bind(uuid::Uuid::new_v4().to_string()).bind(kind).bind(&run.id).bind(run.configuration_version).bind(serde_json::to_string(sources)?).bind(serde_json::to_string(projects)?).bind(&run.id).bind(&run.event_id).bind(&run.plugin_id).execute(&mut **tx).await?;
    Ok(())
}

pub(super) async fn import_snapshot_tx(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    run: &PluginContributionRun,
    snapshot: PluginSourceSnapshot,
) -> Result<PluginImportedDocument> {
    let uri = format!(
        "plugin://{}/{}/{}",
        p.id,
        &crate::store::hash(&p.installed_at)[..24],
        snapshot.key
    );
    let owned:Option<String>=sqlx::query_scalar("SELECT source_id FROM plugin_sources WHERE plugin_id=? AND installed_at=? AND snapshot_key=?").bind(&p.id).bind(&p.installed_at).bind(&snapshot.key).fetch_optional(&mut **tx).await?;
    let existing = sqlx::query("SELECT id,allow_external FROM sources WHERE uri=?")
        .bind(&uri)
        .fetch_optional(&mut **tx)
        .await?;
    if existing
        .as_ref()
        .is_some_and(|r| owned.as_deref() != Some(r.get::<&str, _>("id")))
    {
        bail!("forbidden: connector cannot overwrite a source it does not own");
    }
    if owned.is_some() && existing.is_none() {
        bail!("forbidden: an owned source was removed; choose a new snapshot identity explicitly");
    }
    let allow_external = existing
        .as_ref()
        .is_some_and(|r| r.get::<bool, _>("allow_external"));
    let (document_id, changed) = Store::import_tx(
        tx,
        &ImportRequest {
            source_name: snapshot.source_name,
            kind: snapshot.kind,
            uri,
            title: snapshot.title,
            body: snapshot.body,
            allow_external,
        },
        run.destination_project_id.as_deref(),
    )
    .await?;
    let row = sqlx::query("SELECT source_id,revision FROM documents WHERE id=?")
        .bind(&document_id)
        .fetch_one(&mut **tx)
        .await?;
    let source_id: String = row.get("source_id");
    sqlx::query("INSERT INTO plugin_sources(plugin_id,installed_at,snapshot_key,source_id,document_id) VALUES(?,?,?,?,?) ON CONFLICT(plugin_id,installed_at,snapshot_key) DO NOTHING").bind(&p.id).bind(&p.installed_at).bind(&snapshot.key).bind(&source_id).bind(&document_id).execute(&mut **tx).await?;
    Ok(PluginImportedDocument {
        source_id,
        document_id,
        revision: row.get("revision"),
        changed,
    })
}
pub(super) async fn propose_action_tx(
    tx: &mut Transaction<'_, Sqlite>,
    p: &PluginRecord,
    run: &PluginContributionRun,
    index: usize,
    proposal: PluginActionProposal,
) -> Result<ActionPreview> {
    if !p.grant.operations.iter().any(|op| op == "actions.propose") {
        bail!("forbidden: action proposal permission required");
    }
    let task = crate::store::task_row(
        &sqlx::query("SELECT * FROM tasks WHERE id=?")
            .bind(&proposal.target_id)
            .fetch_optional(&mut **tx)
            .await?
            .context("invalid: action target no longer exists")?,
    );
    if !proposal.evidence.contains(&task.evidence) {
        bail!("invalid: action must cite the target task's exact evidence");
    }
    Store::preview_action_tx(
        tx,
        ActionPreviewRequest {
            provider: proposal.provider,
            target_id: proposal.target_id,
            expected_version: Some(proposal.expected_version),
            status: Some(proposal.status),
            idempotency_key: format!("plugin-action:{}:{index}", run.id),
        },
        Some(PluginActionOrigin {
            plugin_id: p.id.clone(),
            installed_at: p.installed_at.clone(),
            configuration_version: p.version,
            run_id: run.id.clone(),
            contribution_id: run.contribution.id.clone(),
            rationale: proposal.rationale,
            evidence: proposal.evidence,
        }),
    )
    .await
}

/// Explicit user approval remains necessary. A stale or revoked plugin proposal
/// cannot acquire fresh authority merely because its preview is still visible.
pub(crate) async fn validate_action_origin_tx(
    tx: &mut Transaction<'_, Sqlite>,
    origin: &PluginActionOrigin,
    preview: &ActionPreview,
) -> Result<()> {
    let p = load(tx, &origin.plugin_id).await?;
    let run = run_tx(tx, &origin.plugin_id, &origin.run_id).await?;
    if !p.enabled
        || p.installed_at != origin.installed_at
        || p.version != origin.configuration_version
        || !p.grant.operations.iter().any(|op| op == "actions.propose")
        || run.status != "completed"
        || run.installed_at != origin.installed_at
        || run.configuration_version != origin.configuration_version
        || run.contribution.kind != PluginContributionKind::Action
        || !run.action_preview_ids.contains(&preview.id)
        || run.contribution.id != origin.contribution_id
    {
        bail!("forbidden: plugin proposal changed or lost its grant; create a fresh preview");
    }
    scope_tx(tx, &p, &run.source_ids, &run.project_ids).await?;
    if preview.provider != "task_status"
        || !run.tasks.iter().any(|t| {
            t.task_id == preview.target_id && Some(t.version) == preview.before["version"].as_i64()
        })
    {
        bail!("forbidden: action exceeds its recorded task scope");
    }
    sqlx::query(
        "INSERT INTO plugin_effect_context(contributor,record) VALUES('__host_transaction__',?)",
    )
    .bind(
        json!({"plugin_id":p.id,"event_id":run.event_id,"correlation_id":run.id,"depth":1})
            .to_string(),
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
