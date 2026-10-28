use super::*;

async fn sync(app: &tauri::AppHandle, poll: &PluginHostPoll) -> Result<()> {
    let state = app.state::<Arc<DesktopHost>>().inner().clone();
    let selected: HashSet<_> = poll
        .selections
        .iter()
        .map(|s| s.provider_id.as_str())
        .collect();
    let records: HashMap<_, _> = poll.plugins.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut installed = state
        .installed
        .lock()
        .map_err(|_| anyhow::anyhow!("desktop lifecycle lock unavailable"))?
        .clone();
    let mut errors = HashMap::new();
    // Release dependents before their dependencies, then acquire in reverse.
    for kind in provider::KINDS.into_iter().rev() {
        let desired = records
            .get(kind.id())
            .is_some_and(|p| p.enabled && selected.contains(kind.id()));
        if installed.contains(kind.id()) && !desired {
            let result = async {
                state.registry.set_enabled(kind.id(), false)?;
                kind.set_installed(app, false).await
            }
            .await;
            match result {
                Ok(()) => {
                    installed.remove(kind.id());
                }
                Err(e) => {
                    errors.insert(kind.id(), e.to_string());
                }
            }
        }
    }
    for kind in provider::KINDS {
        let desired = records
            .get(kind.id())
            .is_some_and(|p| p.enabled && selected.contains(kind.id()));
        if !installed.contains(kind.id()) && desired {
            let result = async {
                if kind
                    .manifest()
                    .dependencies
                    .iter()
                    .any(|d| !installed.contains(d))
                {
                    bail!("desktop dependency unavailable");
                }
                if let Err(e) = kind.set_installed(app, true).await {
                    let _ = kind.set_installed(app, false).await;
                    return Err(e);
                }
                if let Err(e) = state.registry.set_enabled(kind.id(), true) {
                    let _ = kind.set_installed(app, false).await;
                    return Err(e);
                }
                Ok(())
            }
            .await;
            match result {
                Ok(()) => {
                    installed.insert(kind.id().to_owned());
                }
                Err(e) => {
                    errors.insert(kind.id(), e.to_string());
                }
            }
        }
    }
    for selection in &poll.selections {
        if installed.contains(&selection.provider_id) {
            state
                .registry
                .select(&selection.capability, &selection.provider_id)?;
        }
    }
    let health = provider::KINDS
        .into_iter()
        .filter_map(|kind| {
            records.get(kind.id()).map(|p| PluginProviderHealth {
                provider_id: kind.id().into(),
                version: p.version,
                available: installed.contains(kind.id())
                    && p.enabled
                    && !errors.contains_key(kind.id()),
                detail: errors.get(kind.id()).cloned().unwrap_or_else(|| {
                    if installed.contains(kind.id()) {
                        "Ready".into()
                    } else {
                        "Provider disabled or not selected".into()
                    }
                }),
            })
        })
        .collect();
    *state
        .installed
        .lock()
        .map_err(|_| anyhow::anyhow!("desktop lifecycle lock unavailable"))? = installed;
    *state
        .health
        .lock()
        .map_err(|_| anyhow::anyhow!("desktop health lock unavailable"))? = health;
    Ok(())
}

async fn run_command(
    app: &tauri::AppHandle,
    command: &PluginNativeCommand,
    poll: &PluginHostPoll,
) -> PluginNativeReply {
    let result: Result<Value> = async {
        if !poll.plugins.iter().any(|p| {
            p.id == command.provider_id && p.enabled && p.version == command.provider_version
        }) || !poll
            .selections
            .iter()
            .any(|s| s.capability == command.capability && s.provider_id == command.provider_id)
        {
            bail!("provider configuration changed before dispatch");
        }
        let registry = app.state::<Arc<DesktopHost>>().registry.clone();
        let lease = registry.acquire(&command.capability)?;
        if lease.id() != command.provider_id {
            bail!("selected provider changed before dispatch");
        }
        let manifest = lease.provider().manifest();
        if !manifest
            .capabilities
            .iter()
            .any(|c| c.id == command.capability && c.commands.contains(&command.command))
        {
            bail!("unknown provider command");
        }
        lease
            .provider()
            .invoke(&command.command, command.input.clone())
            .await
    }
    .await;
    match result {
        Ok(result) => PluginNativeReply {
            result,
            error: None,
        },
        Err(error) => PluginNativeReply {
            result: Value::Null,
            error: Some(error.to_string()),
        },
    }
}

pub(super) fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let dir = app.state::<Arc<Desktop>>().dir.clone();
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let host_id = format!("desktop-{}-{epoch}", std::process::id());
        let registration = PluginHostRegistration {
            host_id: host_id.clone(),
            manifests: provider::KINDS.into_iter().map(|k| k.manifest()).collect(),
        };
        let mut session: Option<PluginHostSession> = None;
        loop {
            let mut registration_failure = None;
            let result: Result<()> = async {
                let client = Client::connect(&dir, None)?;
                if session.is_none() {
                    let response = client
                        .request(reqwest::Method::POST, "/api/v1/plugin-host/register")
                        .timeout(std::time::Duration::from_secs(2))
                        .json(&registration)
                        .send()
                        .await?;
                    if !response.status().is_success() {
                        registration_failure =
                            Some(crate::service::registration_status(response.status()));
                        bail!("desktop host registration was rejected");
                    }
                    match response.json().await {
                        Ok(value) => session = Some(value),
                        Err(error) => {
                            registration_failure = Some(crate::service::Status::new(
                                crate::service::Phase::Incompatible,
                            ));
                            return Err(error.into());
                        }
                    }
                }
                let current = session
                    .as_ref()
                    .context("desktop host registration missing")?;
                let health = app
                    .state::<Arc<DesktopHost>>()
                    .health
                    .lock()
                    .map_err(|_| anyhow::anyhow!("desktop health lock unavailable"))?
                    .clone();
                let response = client
                    .request(
                        reqwest::Method::POST,
                        &format!("/api/v1/plugin-host/{host_id}/poll"),
                    )
                    .header("x-thesaurus-plugin-host", &current.lease_token)
                    .timeout(std::time::Duration::from_secs(2))
                    .json(&PluginHostHeartbeat { providers: health })
                    .send()
                    .await?;
                if response.status() == reqwest::StatusCode::UNAUTHORIZED {
                    session = None;
                    registration_failure = Some(crate::service::Status::new(
                        crate::service::Phase::Connecting,
                    ));
                    bail!("desktop host lease expired; reconnecting");
                }
                if !response.status().is_success() {
                    bail!("desktop host poll failed: {}", response.status());
                }
                let poll: PluginHostPoll = response.json().await?;
                sync(&app, &poll).await?;
                let window = poll
                    .plugins
                    .iter()
                    .find(|p| p.id == "thesaurus.native.windows");
                let selected = poll.selections.iter().any(|s| {
                    s.capability == "native.windows" && s.provider_id == "thesaurus.native.windows"
                });
                let window_installed = app
                    .state::<Arc<DesktopHost>>()
                    .installed
                    .lock()
                    .map_err(|_| anyhow::anyhow!("desktop lifecycle lock unavailable"))?
                    .contains("thesaurus.native.windows");
                let mut status = crate::service::Status::new(
                    if !selected || window.is_none_or(|p| !p.enabled) {
                        crate::service::Phase::WindowDisabled
                    } else if !window_installed {
                        crate::service::Phase::ProviderFailed
                    } else {
                        crate::service::Phase::Ready
                    },
                );
                status.can_enable_windows = selected && window.is_some_and(|p| !p.enabled);
                crate::service::report(&app, status).await;
                for command in &poll.commands {
                    let reply = run_command(&app, command, &poll).await;
                    // A failed receipt is never a reason to execute a UI command
                    // twice. The service eventually reports an unknown outcome.
                    let _ = client
                        .request(
                            reqwest::Method::POST,
                            &format!("/api/v1/plugin-host/{host_id}/commands/{}", command.id),
                        )
                        .header("x-thesaurus-plugin-host", &current.lease_token)
                        .timeout(std::time::Duration::from_secs(2))
                        .json(&reply)
                        .send()
                        .await;
                }
                Ok(())
            }
            .await;
            if let Err(error) = result {
                let _ = app.emit("plugin-host-error", error.to_string());
                if let Some(status) = registration_failure {
                    crate::service::report(&app, status).await;
                } else {
                    match crate::service::probe(&dir).await {
                        crate::service::Probe::Unavailable(status) => {
                            crate::service::report_probe(&app, status).await
                        }
                        crate::service::Probe::Ready => {
                            crate::service::report(
                                &app,
                                crate::service::Status::new(crate::service::Phase::ProviderFailed),
                            )
                            .await
                        }
                    }
                }
                let data = app.state::<Arc<Desktop>>();
                tokio::select! {_=tokio::time::sleep(std::time::Duration::from_secs(1))=>{},_=data.recovery.retry.notified()=>{}}
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    });
}
