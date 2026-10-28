//! The desktop host owns registration and dispatch. Product OS behavior lives
//! in the compiled providers below, sharing the service's capability contracts.
use crate::*;
use anyhow::{Context, Result, bail};
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};
use thesaurus_contracts::*;
use thesaurus_core::plugins::registry::{CapabilityProvider, ProviderFuture, ProviderRegistry};
mod host;
mod notifications;
mod popover;
mod provider;
mod recording;
mod shortcuts;
mod tray;
mod windows;

struct DesktopHost {
    registry: ProviderRegistry,
    installed: Mutex<HashSet<String>>,
    health: Mutex<Vec<PluginProviderHealth>>,
}

pub fn install(app: &tauri::App) -> Result<()> {
    let registry = ProviderRegistry::default();
    for kind in provider::KINDS {
        registry.register(Arc::new(provider::Native {
            app: app.handle().clone(),
            kind,
        }))?;
    }
    // Until the service supplies persisted configuration, no provider is live.
    for kind in provider::KINDS.into_iter().rev() {
        registry.set_enabled(kind.id(), false)?;
    }
    app.manage(Arc::new(DesktopHost {
        registry,
        installed: Default::default(),
        health: Default::default(),
    }));
    tray::callbacks(app.handle());
    Ok(())
}
pub fn connect(app: tauri::AppHandle) {
    host::start(app);
}
pub async fn update_recovery(app: &tauri::AppHandle, status: crate::service::Status) {
    let _ = on_main(app.clone(), move |app| {
        windows::recovery::update(app, &status, false)
    })
    .await;
}
pub fn reopen(app: &tauri::AppHandle) {
    let status = app
        .state::<Arc<Desktop>>()
        .recovery
        .status
        .lock()
        .ok()
        .map(|s| s.clone());
    if let Some(status) = status.filter(|s| s.phase != crate::service::Phase::Ready) {
        let _ = windows::recovery::update(app, &status, true);
    } else if app.get_webview_window("main").is_some() {
        dispatch(app, "native.windows", "workspace", json!({}));
    } else {
        let state = app.state::<Arc<Desktop>>();
        if let Ok(status) = state.recovery.status.lock() {
            let _ = windows::recovery::update(app, &status, true);
        }
    }
}
pub fn on_exit(app: &tauri::AppHandle, api: &tauri::ExitRequestApi) {
    recording::on_exit(app, api);
}

/// Native callbacks take the same broker route as browser commands, so persisted
/// enablement/selection is checked before dispatch, rather than a stale UI flag.
fn dispatch(app: &tauri::AppHandle, capability: &str, command: &str, input: Value) {
    let app = app.clone();
    let capability = capability.to_owned();
    let command = command.to_owned();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = request(&app, &capability, &command, input).await {
            let _ = app.emit("service-error", error);
        }
    });
}
async fn request(
    app: &tauri::AppHandle,
    capability: &str,
    command: &str,
    input: Value,
) -> std::result::Result<Value, String> {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = app.state::<Arc<Desktop>>().dir.clone();
    let client = Client::connect(&dir, None).map_err(|e| e.to_string())?;
    let nonce = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    client.call(reqwest::Method::POST, &format!("/api/v1/plugin-capabilities/{capability}/invoke"), Some(json!({"command":command,"input":input,"idempotency_key":format!("desktop-{}-{epoch}-{nonce}",std::process::id())}))).await.map_err(|e|e.to_string())
}
#[tauri::command]
pub async fn meeting_window(
    app: tauri::AppHandle,
    mode: String,
) -> std::result::Result<(), String> {
    let (cap, command, input) = match mode.as_str() {
        "dismiss" => ("native.tray", "dismiss", json!({})),
        "popover" => ("native.tray", "show", json!({})),
        "workspace" => ("native.windows", "workspace", json!({})),
        _ => ("native.windows", "companion", json!({"mode":mode})),
    };
    request(&app, cap, command, input).await.map(|_| ())
}
#[tauri::command]
pub async fn meeting_popover_height(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    height: f64,
) -> std::result::Result<(), String> {
    if window.label() != "meeting-popover" || !height.is_finite() {
        return Err("Invalid popover size".into());
    }
    // This internal resize path only updates the already-owned transient view.
    // It acquires a registry lease and cannot create/show a disabled provider.
    let registry = app.state::<Arc<DesktopHost>>().registry.clone();
    registry
        .invoke("native.tray", "height", json!({"height":height}))
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

async fn on_main<T: Send + 'static>(
    app: tauri::AppHandle,
    f: impl FnOnce(&tauri::AppHandle) -> std::result::Result<T, String> + Send + 'static,
) -> Result<T> {
    let (send, recv) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = send.send(f(&handle));
    })?;
    tokio::time::timeout(std::time::Duration::from_secs(5), recv)
        .await
        .context("desktop main thread did not respond; command outcome unknown")?
        .context("desktop main thread closed")?
        .map_err(anyhow::Error::msg)
}
