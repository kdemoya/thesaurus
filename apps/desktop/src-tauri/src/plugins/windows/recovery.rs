//! Recovery is a window-provider surface, available before service-backed
//! enablement. It never enables workspace/tray/capture providers implicitly.
use super::*;
use crate::service::{Phase, Status};
pub(in crate::plugins) fn update(
    app: &tauri::AppHandle,
    status: &Status,
    force: bool,
) -> Result<(), String> {
    if status.phase == Phase::Ready {
        if let Some(w) = app.get_webview_window("recovery") {
            w.destroy().map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if let Some(w) = app.get_webview_window("recovery") {
        if force {
            w.show().map_err(|e| e.to_string())?;
            w.set_focus().map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if !force && status.phase == Phase::Connecting && app.get_webview_window("main").is_some() {
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(
        app,
        "recovery",
        WebviewUrl::App("recovery/recovery.html".into()),
    )
    .title("Thesaurus · Workspace connection")
    .inner_size(560., 410.)
    .min_inner_size(380., 300.)
    .resizable(true)
    .build()
    .map_err(|e| e.to_string())?;
    let handle = window.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = handle.hide();
        }
    });
    Ok(())
}
