use super::*;

pub const HOOKS: &[&str] = &[
    "service.ready",
    "service.stopping",
    "plugin.enabled",
    "plugin.disabled",
    "recording.started",
    "recording.paused",
    "recording.resumed",
    "recording.stopped",
    "recording.failed",
    "recording.state.changed",
    "transcript.chunk.committed",
    "document.revision.created",
    "source.sync.completed",
    "source.sync.failed",
    "source.policy.changed",
    "memory.observation.created",
    "memory.observation.superseded",
    "task.accepted",
    "task.completed",
    "task.changed",
    "delivery.changed",
    "priority.changed",
    "context.bundle.created",
    "agent.run.started",
    "agent.run.completed",
    "agent.run.failed",
    "agent.run.cancelled",
    "agent.run.state.changed",
    "handoff.acknowledged",
    "eval.run.completed",
];
pub const OPERATIONS: &[&str] = &[
    "events.lifecycle",
    "events.workspace",
    "sources.read",
    "memory.propose",
    "commands.invoke",
    "actions.propose",
    "eval.check",
    "models.request",
    "connectors.sync",
    "setup.check",
    "native.windows.workspace",
    "native.windows.companion",
    "native.tray.show",
    "native.tray.dismiss",
    "native.notifications.show",
    "native.notifications.dismiss",
];

pub fn identifier(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 120
        && s.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-' || b == b'_'
        })
        && s.as_bytes()[0].is_ascii_lowercase()
}
pub fn validate_manifest(m: &PluginManifest) -> Result<()> {
    if !identifier(&m.id)
        || m.api_min > PLUGIN_API_VERSION
        || m.api_min < PLUGIN_API_MIN_VERSION
        || m.api_max < PLUGIN_API_MIN_VERSION
        || m.api_min > m.api_max
    {
        bail!("invalid: plugin identifier or host API range is incompatible");
    }
    validate_text(&m.name, 120, "plugin name")?;
    if m.api_max < 2 && m.requests.iter().any(|op| op.starts_with("native.")) {
        bail!("invalid: native command requests require API 2");
    }
    let version = m.version.split('.').collect::<Vec<_>>();
    if version.len() != 3
        || version
            .iter()
            .any(|s| s.is_empty() || s.parse::<u32>().is_err())
    {
        bail!("invalid: plugin version must be major.minor.patch");
    }
    if m.description.len() > 4000
        || m.hooks.len() > 50
        || m.capabilities.len() > 30
        || m.configuration.len() > 40
        || m.dependencies.len() > 20
        || m.contributions.len() > 50
        || m.setup.len() > 20
    {
        bail!("invalid: plugin manifest exceeds limits");
    }
    if m.dependencies.iter().any(|d| !identifier(d) || d == &m.id)
        || m.hooks.iter().any(|h| !HOOKS.contains(&h.as_str()))
        || m.requests
            .iter()
            .any(|op| !OPERATIONS.contains(&op.as_str()))
    {
        bail!("invalid: unknown hook/operation or invalid dependency");
    }
    let mut ids = HashSet::new();
    for c in &m.capabilities {
        if !identifier(&c.id)
            || !ids.insert(&c.id)
            || c.commands.is_empty()
            || c.commands.len() > 30
            || c.commands.iter().any(|v| !identifier(v))
        {
            bail!("invalid: capability identifiers or commands");
        }
    }
    ids.clear();
    for f in &m.configuration {
        if !identifier(&f.key)
            || !ids.insert(&f.key)
            || f.label.trim().is_empty()
            || f.label.len() > 120
            || f.description.len() > 2000
            || f.choices.len() > 30
            || f.choices.iter().any(|s| s.len() > 200)
        {
            bail!("invalid: configuration schema");
        }
        if f.kind == PluginFieldKind::Choice && f.choices.is_empty() {
            bail!("invalid: choice fields require options");
        }
    }
    ids.clear();
    for c in &m.contributions {
        if !identifier(&c.id)
            || !ids.insert(&c.id)
            || c.title.trim().is_empty()
            || c.title.len() > 120
            || c.description.len() > 2000
        {
            bail!("invalid: contribution schema");
        }
    }
    ids.clear();
    for step in &m.setup {
        if !identifier(&step.id)
            || !ids.insert(&step.id)
            || step.title.trim().is_empty()
            || step.title.len() > 120
            || step.description.len() > 2000
            || step.prerequisites.len() > 20
            || step.applies_to.len() > 50
            || step.applies_to.iter().any(|id| {
                !m.contributions.iter().any(|c| &c.id == id)
                    && !m.capabilities.iter().any(|c| &c.id == id)
            })
            || step.check && m.api_max < 2
        {
            bail!("invalid: setup step schema or API version");
        }
        for prerequisite in &step.prerequisites {
            let valid = match prerequisite {
                PluginPrerequisite::Configuration { key } => {
                    m.configuration.iter().any(|f| &f.key == key)
                }
                PluginPrerequisite::Operation { operation } => m.requests.contains(operation),
                PluginPrerequisite::Dependency { plugin_id } => m.dependencies.contains(plugin_id),
                PluginPrerequisite::Capability { capability } => identifier(capability),
            };
            if !valid {
                bail!("invalid: undeclared setup prerequisite");
            }
        }
        let valid = match &step.repair {
            Some(PluginSetupRepair::Configuration { key }) => {
                m.configuration.iter().any(|f| &f.key == key)
            }
            Some(PluginSetupRepair::Provider { capability }) => identifier(capability),
            _ => true,
        };
        if !valid {
            bail!("invalid: setup repair target");
        }
        if step.check
            && !m.id.starts_with("thesaurus.")
            && !m.requests.iter().any(|r| r == "setup.check")
        {
            bail!("invalid: executable setup checks must request setup.check");
        }
    }
    Ok(())
}

pub fn negotiated_api(manifest: &PluginManifest) -> Result<u32> {
    validate_manifest(manifest)?;
    Ok(manifest.api_max.min(PLUGIN_API_VERSION))
}
pub fn validate_configuration(m: &PluginManifest, configuration: &Value) -> Result<()> {
    let values = configuration
        .as_object()
        .context("invalid: plugin configuration must be an object")?;
    if serde_json::to_vec(configuration)?.len() > 32_000
        || values
            .keys()
            .any(|k| !m.configuration.iter().any(|f| &f.key == k))
    {
        bail!("invalid: unknown or oversized plugin configuration");
    }
    for field in &m.configuration {
        let Some(value) = values.get(&field.key).or(field.default.as_ref()) else {
            if field.required {
                bail!("invalid: missing plugin setting {}", field.key);
            }
            continue;
        };
        let valid = match field.kind {
            PluginFieldKind::String => value.as_str().is_some_and(|v| {
                v.len() <= 4096 && !v.contains('\0') && (!field.required || !v.trim().is_empty())
            }),
            PluginFieldKind::Boolean => value.is_boolean(),
            PluginFieldKind::Integer => value.as_i64().is_some(),
            PluginFieldKind::Choice => value
                .as_str()
                .is_some_and(|s| field.choices.iter().any(|v| v == s)),
            PluginFieldKind::CredentialReference => value
                .as_str()
                .is_some_and(|s| super::native::credentials::reference(s).is_ok()),
        };
        if !valid {
            bail!("invalid: wrong value for plugin setting {}", field.key);
        }
    }
    Ok(())
}

pub fn configured_values(m: &PluginManifest, mut configuration: Value) -> Result<Value> {
    validate_configuration(m, &configuration)?;
    let values = configuration.as_object_mut().unwrap();
    for field in &m.configuration {
        if let Some(default) = &field.default {
            values
                .entry(field.key.clone())
                .or_insert_with(|| default.clone());
        }
    }
    Ok(configuration)
}

/// Check the proposed package against the installed graph before any flags or
/// manifests commit. Disabled packages may depend on installed disabled packages;
/// enablement separately requires every dependency to be enabled.
pub async fn validate_dependency_graph(
    tx: &mut Transaction<'_, Sqlite>,
    proposed: &PluginManifest,
) -> Result<()> {
    let mut graph = std::collections::HashMap::new();
    for record in sqlx::query_scalar::<_, String>("SELECT record FROM plugins")
        .fetch_all(&mut **tx)
        .await?
    {
        let plugin: PluginRecord = serde_json::from_str(&record)?;
        graph.insert(plugin.id, plugin.manifest.dependencies);
    }
    graph.insert(proposed.id.clone(), proposed.dependencies.clone());
    fn visit(
        id: &str,
        graph: &std::collections::HashMap<String, Vec<String>>,
        visiting: &mut HashSet<String>,
        done: &mut HashSet<String>,
    ) -> Result<()> {
        if done.contains(id) {
            return Ok(());
        }
        if !visiting.insert(id.into()) {
            bail!("invalid: plugin dependency cycle includes {id}");
        }
        let dependencies = graph
            .get(id)
            .with_context(|| format!("invalid: plugin dependency {id} is not installed"))?;
        for dependency in dependencies {
            visit(dependency, graph, visiting, done)?;
        }
        visiting.remove(id);
        done.insert(id.into());
        Ok(())
    }
    visit(
        &proposed.id,
        &graph,
        &mut HashSet::new(),
        &mut HashSet::new(),
    )
}
