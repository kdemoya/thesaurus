//! Bounded service discovery/bootstrap. Never signal an existing workspace owner.
//! Recovery window behavior belongs to the native window provider.
use crate::*;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::OpenOptions,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use thesaurus_contracts::*;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Connecting,
    Starting,
    Disconnected,
    Incompatible,
    OtherDesktop,
    WindowDisabled,
    ProviderFailed,
    Ready,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Status {
    pub phase: Phase,
    pub headline: String,
    pub detail: String,
    pub can_start: bool,
    pub can_enable_windows: bool,
}
impl Status {
    pub fn new(phase: Phase) -> Self {
        let (headline, detail) = match phase {
            Phase::Connecting => (
                "Connecting to your workspace",
                "Checking the local service and desktop providers.",
            ),
            Phase::Starting => (
                "Starting your workspace",
                "Waiting for the local service. Your data stays on this computer.",
            ),
            Phase::Disconnected => (
                "The local service is unavailable",
                "Retry after the service is ready. If no service owns this workspace, you can start the bundled service.",
            ),
            Phase::Incompatible => (
                "The service needs an update",
                "A running service uses a different protocol or plugin interface. Save current work, stop that older service, then start the service bundled with this app. Retrying does not stop it.",
            ),
            Phase::OtherDesktop => (
                "Another desktop is connected",
                "This workspace already has a desktop host. Close that desktop connection or use its window, then retry here.",
            ),
            Phase::WindowDisabled => (
                "The workspace window is disabled",
                "Your native window provider is disabled or not selected. Enable the selected window provider to open the workspace. Other saved settings are preserved.",
            ),
            Phase::ProviderFailed => (
                "The desktop provider could not start",
                "The service is connected, but a native provider could not initialize. Retry the connection or reopen the app after resolving the platform error.",
            ),
            Phase::Ready => (
                "Workspace connected",
                "The desktop window provider is ready.",
            ),
        };
        Self {
            phase,
            headline: headline.into(),
            detail: detail.into(),
            can_start: false,
            can_enable_windows: false,
        }
    }
}
pub struct Recovery {
    pub status: Mutex<Status>,
    pub retry: tokio::sync::Notify,
    starting: AtomicBool,
}
impl Default for Recovery {
    fn default() -> Self {
        Self {
            status: Mutex::new(Status::new(Phase::Connecting)),
            retry: Default::default(),
            starting: AtomicBool::new(false),
        }
    }
}
pub enum Probe {
    Ready,
    Unavailable(Status),
}
pub fn registration_status(code: reqwest::StatusCode) -> Status {
    Status::new(if code == reqwest::StatusCode::CONFLICT {
        Phase::OtherDesktop
    } else if matches!(code.as_u16(), 400 | 404 | 405 | 415 | 422 | 501) {
        Phase::Incompatible
    } else {
        Phase::Disconnected
    })
}

/// The descriptor PID is not proof of ownership. Probe the same OS lock as the
/// daemon, without creating/truncating it or following a symlink.
pub fn owned(dir: &Path) -> std::io::Result<bool> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(dir.join("daemon.lock"))
    {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::other("Invalid workspace lock"));
    }
    match file.try_lock_exclusive() {
        Ok(()) => {
            FileExt::unlock(&file)?;
            Ok(false)
        }
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(true),
        Err(e) => Err(e),
    }
}
async fn bounded_json<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
) -> Result<T, String> {
    tokio::time::timeout(Duration::from_secs(2), async move {
        let mut bytes = vec![];
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| "Service reply unavailable")?
        {
            if bytes.len() + chunk.len() > 2_000_000 {
                return Err("Service reply exceeds limits".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| "Service reply is incompatible".into())
    })
    .await
    .map_err(|_| "Service reply timed out".to_string())?
}
pub async fn probe(dir: &Path) -> Probe {
    let ownership = owned(dir);
    let mut disconnected = Status::new(Phase::Disconnected);
    disconnected.can_start = matches!(ownership, Ok(false));
    if ownership.is_err() {
        disconnected.detail="The workspace lock cannot be checked. Check this workspace's file permissions before starting a service.".into();
    }
    let descriptor = (|| -> Option<Instance> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.join("instance.json"))
            .ok()?;
        let mut bytes = vec![];
        file.take(32_001).read_to_end(&mut bytes).ok()?;
        if bytes.len() > 32_000 {
            return None;
        }
        serde_json::from_slice(&bytes).ok()
    })();
    let Some(instance) = descriptor else {
        return Probe::Unavailable(disconnected);
    };
    if instance.protocol_version != PROTOCOL_VERSION {
        if disconnected.can_start {
            return Probe::Unavailable(disconnected);
        }
        return Probe::Unavailable(Status::new(Phase::Incompatible));
    }
    let Ok(client) = Client::connect(dir, None) else {
        return Probe::Unavailable(disconnected);
    };
    let health = client
        .request(reqwest::Method::GET, "/health")
        .timeout(Duration::from_secs(2))
        .send()
        .await;
    let Ok(health) = health else {
        return Probe::Unavailable(disconnected);
    };
    if !health.status().is_success() {
        return Probe::Unavailable(disconnected);
    }
    let health = bounded_json::<Value>(health).await;
    if !matches!(health,Ok(ref value) if value["status"]=="ready"&&value["protocol_version"].as_u64()==Some(PROTOCOL_VERSION as u64))
    {
        return Probe::Unavailable(Status::new(Phase::Incompatible));
    }
    // Same service protocol does not prove that it supports the plugin host.
    let response = client
        .request(reqwest::Method::GET, "/api/v1/plugin-capabilities")
        .timeout(Duration::from_secs(2))
        .send()
        .await;
    let Ok(response) = response else {
        return Probe::Unavailable(disconnected);
    };
    if !response.status().is_success() {
        return Probe::Unavailable(Status::new(Phase::Incompatible));
    }
    match bounded_json::<Vec<PluginCapabilityStatus>>(response).await {
        Ok(_) => Probe::Ready,
        _ => Probe::Unavailable(Status::new(Phase::Incompatible)),
    }
}
pub async fn report(app: &tauri::AppHandle, status: Status) {
    let changed = {
        let state = app.state::<Arc<Desktop>>();
        if let Ok(mut current) = state.recovery.status.lock() {
            let changed = *current != status;
            *current = status.clone();
            changed
        } else {
            false
        }
    };
    if changed {
        let _ = app.emit("desktop-recovery", &status);
        if !matches!(
            status.phase,
            Phase::Ready | Phase::Connecting | Phase::Starting
        ) {
            let _ = app.emit(
                "service-error",
                format!("{}: {}", status.headline, status.detail),
            );
        }
    }
    plugins::update_recovery(app, status).await;
}
fn binary(app: &tauri::AppHandle) -> Option<(PathBuf, Option<PathBuf>)> {
    if let Ok(root) = app.path().resource_dir() {
        let exe = root.join("resources/thesaurus");
        if exe.is_file() {
            let web = root.join("resources/web");
            return Some((exe, web.is_dir().then_some(web)));
        }
    }
    let exe = std::env::current_exe().ok()?.parent()?.join("thesaurus");
    exe.is_file().then_some((exe, None))
}
pub async fn report_probe(app: &tauri::AppHandle, mut status: Status) {
    if status.can_start && binary(app).is_none() {
        status.can_start = false;
        status.detail =
            "The bundled local service is missing. Reinstall or rebuild the app, then retry."
                .into();
    }
    report(app, status).await;
}
pub async fn bootstrap(app: &tauri::AppHandle) {
    report(app, Status::new(Phase::Connecting)).await;
    let dir = app.state::<Arc<Desktop>>().dir.clone();
    if let Probe::Unavailable(status) = probe(&dir).await {
        if status.can_start {
            let _ = start_service(app).await;
        } else {
            report_probe(app, status).await;
        }
    }
}
async fn start_service(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<Arc<Desktop>>().inner().clone();
    if state.recovery.starting.swap(true, Ordering::SeqCst) {
        return Err("The local service is already starting".into());
    }
    struct Guard(Arc<Desktop>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.recovery.starting.store(false, Ordering::SeqCst);
        }
    }
    let _guard = Guard(state.clone());
    match probe(&state.dir).await {
        Probe::Ready => {
            state.recovery.retry.notify_one();
            return Ok(());
        }
        Probe::Unavailable(status) if !status.can_start => {
            report_probe(app, status).await;
            return Err(
                "An existing workspace owner or lock error prevents starting another service"
                    .into(),
            );
        }
        _ => {}
    }
    let Some((exe, web)) = binary(app) else {
        let mut status = Status::new(Phase::Disconnected);
        status.detail =
            "The bundled local service is missing. Reinstall or rebuild the app, then retry."
                .into();
        report(app, status).await;
        return Err("Bundled service unavailable".into());
    };
    report(app, Status::new(Phase::Starting)).await;
    let mut command = tokio::process::Command::new(exe);
    command
        .arg("--data-dir")
        .arg(&state.dir)
        .arg("serve")
        .arg("--port")
        .arg("0")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(web) = web {
        command.arg("--web-dir").arg(web);
    }
    // Keep the independently owned daemon out of the desktop's process group.
    // Closing a launcher/terminal must not deliver its group signal to capture.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    let child = command.spawn();
    let Ok(mut child) = child else {
        let mut status = Status::new(Phase::Disconnected);
        status.detail="The bundled service could not launch. Check the app installation and workspace permissions, then retry.".into();
        report(app, status).await;
        return Err("Local service launch failed".into());
    };
    // The daemon owns its own lock and survives the desktop. Reap our child but
    // never kill it when this future or desktop window is dropped.
    tauri::async_runtime::spawn(async move {
        let _ = child.wait().await;
    });
    if tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if matches!(probe(&state.dir).await, Probe::Ready) {
                break;
            }
        }
    })
    .await
    .is_ok()
    {
        state.recovery.retry.notify_one();
        return Ok(());
    }
    if let Probe::Unavailable(status) = probe(&state.dir).await {
        report_probe(app, status).await;
    }
    Err("Service has not become ready; retry when it is available".into())
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Status,
    Retry,
    StartService,
    EnableWindows,
    Quit,
}
#[tauri::command]
pub async fn recovery_action(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    action: Action,
) -> Result<Status, String> {
    if window.label() != "recovery" {
        return Err("Recovery controls are only available in the recovery window".into());
    }
    let state = app.state::<Arc<Desktop>>().inner().clone();
    match action {
        Action::Status => {}
        Action::Retry => state.recovery.retry.notify_one(),
        Action::StartService => start_service(&app).await?,
        Action::EnableWindows => {
            let client =
                Client::connect(&state.dir, None).map_err(|_| "The service is unavailable")?;
            let caps: Vec<PluginCapabilityStatus> = client
                .call(reqwest::Method::GET, "/api/v1/plugin-capabilities", None)
                .await
                .map_err(|_| "Check the service connection and retry")?;
            if !caps.iter().any(|c| {
                c.capability == "native.windows" && c.provider_id == "thesaurus.native.windows"
            }) {
                return Err(
                    "The selected window provider cannot be enabled by this recovery action".into(),
                );
            }
            let p: PluginRecord = client
                .call(
                    reqwest::Method::GET,
                    "/api/v1/plugins/thesaurus.native.windows",
                    None,
                )
                .await
                .map_err(|_| "Window provider unavailable")?;
            if !p.enabled {
                let key = format!(
                    "desktop-window-recovery-{}-{}",
                    std::process::id(),
                    p.version
                );
                let _:PluginRecord=client.call(reqwest::Method::PATCH,"/api/v1/plugins/thesaurus.native.windows",Some(json!({"expected_version":p.version,"enabled":true,"grant":p.grant,"configuration":p.configuration,"idempotency_key":key}))).await.map_err(|_|"The provider changed or could not be enabled; retry the connection")?;
            }
            state.recovery.retry.notify_one();
        }
        Action::Quit => app.exit(0),
    }
    let status = state
        .recovery
        .status
        .lock()
        .map_err(|_| "Recovery status unavailable")?
        .clone();
    Ok(status)
}

#[cfg(test)]
mod tests;
