//! Tray and transient popover ownership. No windows are created by the bootstrap.
use super::*;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

pub(super) fn install(app: &tauri::AppHandle) -> Result<(), String> {
    if app.tray_by_id("meeting-tray").is_some() {
        return Ok(());
    }
    let item = |id, title| MenuItem::with_id(app, id, title, true, None::<&str>);
    let companion = item("meeting", "Meeting companion").map_err(|e| e.to_string())?;
    let workspace = item("workspace", "Open workspace").map_err(|e| e.to_string())?;
    let pause = item("pause", "Pause recording").map_err(|e| e.to_string())?;
    let stop = item("stop", "Stop recording").map_err(|e| e.to_string())?;
    let quit = item("quit", "Pause recording and quit").map_err(|e| e.to_string())?;
    let menu = Menu::with_items(app, &[&companion, &workspace, &pause, &stop, &quit])
        .map_err(|e| e.to_string())?;
    let mut tray = TrayIconBuilder::with_id("meeting-tray")
        .tooltip("Thesaurus · Meeting companion")
        .title("○")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click {
                rect,
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                popover::remember_anchor(rect);
                dispatch(tray.app_handle(), "native.tray", "toggle", json!({}));
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app).map_err(|e| e.to_string())?;
    popover::prepare(app)?;
    Ok(())
}
pub(super) fn teardown(app: &tauri::AppHandle) -> Result<(), String> {
    popover::teardown(app)?;
    app.remove_tray_by_id("meeting-tray");
    Ok(())
}
pub(super) fn callbacks(app: &tauri::AppHandle) {
    app.on_menu_event(|app, event| match event.id.as_ref() {
        "meeting" => dispatch(app, "native.tray", "show", json!({})),
        "workspace" => dispatch(app, "native.windows", "workspace", json!({})),
        "pause" | "stop" => {
            let handle = app.clone();
            let action = event.id.as_ref().to_owned();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = recording::active_control(handle.clone(), &action).await {
                    let _ = handle.emit("service-error", e);
                }
            });
        }
        "quit" => app.exit(0),
        _ => {}
    });
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let dir = handle.state::<Arc<Desktop>>().dir.clone();
            let status = if let Ok(client) = Client::connect(&dir, None) {
                client
                    .call::<Vec<thesaurus_contracts::LiveMeeting>>(
                        reqwest::Method::GET,
                        "/api/v1/live-meetings",
                        None,
                    )
                    .await
                    .ok()
            } else {
                None
            };
            if let Some(tray) = handle.tray_by_id("meeting-tray") {
                if let Some(rows) = status {
                    let active = rows.iter().find(|s| {
                        matches!(
                            s.state.as_str(),
                            "starting" | "recording" | "pausing" | "stopping"
                        )
                    });
                    let _ = tray.set_title(Some(if active.is_some() { "● REC" } else { "○" }));
                    let _ = tray.set_tooltip(Some(
                        active
                            .map(|s| format!("{} · {}", s.title, s.state))
                            .unwrap_or_else(|| "Thesaurus · No meeting recording".into()),
                    ));
                } else {
                    let _ = tray.set_title(Some("!"));
                    let _ = tray.set_tooltip(Some(
                        "Thesaurus · Service unavailable; check recording status",
                    ));
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    });
}
