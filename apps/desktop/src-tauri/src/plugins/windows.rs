use super::*;
use serde::{Deserialize, Serialize};
use tauri::{WebviewUrl, WebviewWindowBuilder};
pub(super) mod recovery;

pub(super) fn install(app: &tauri::AppHandle) -> Result<(), String> {
    if app.get_webview_window("main").is_some() {
        return Ok(());
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "main")
        .ok_or("Main window configuration missing")?;
    let window = WebviewWindowBuilder::from_config(app, config)
        .map_err(|e| e.to_string())?
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
pub(super) fn hide(app: &tauri::AppHandle) -> Result<(), String> {
    for label in ["main", "companion"] {
        if let Some(w) = app.get_webview_window(label) {
            w.hide().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
pub(super) fn teardown(app: &tauri::AppHandle) -> Result<(), String> {
    for label in ["companion", "main"] {
        if let Some(w) = app.get_webview_window(label) {
            w.destroy().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[derive(Default, Serialize, Deserialize)]
struct Position {
    mode: String,
    y: Option<i32>,
}
fn preferences(dir: &std::path::Path) -> Position {
    std::fs::read(dir.join("companion-window.json"))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn save_preferences(dir: &std::path::Path, p: &Position) {
    if let Ok(bytes) = serde_json::to_vec(p) {
        let _ = thesaurus_core::private_write(&dir.join("companion-window.json"), &bytes);
    }
}
pub(super) fn show(app: &tauri::AppHandle, mode: &str) -> Result<(), String> {
    if mode == "dismiss" {
        return Err("Use the tray provider to dismiss the popover".into());
    }
    if mode == "popover" {
        return Err("Use the tray provider to open the popover".into());
    }
    super::popover::dismiss(app);
    let setup = mode == "setup";
    let pin = mode == "pin";
    let pin_mode = if pin {
        let state = app.state::<Arc<Desktop>>();
        let saved = preferences(&state.dir);
        if ["left", "right"].contains(&saved.mode.as_str()) {
            saved.mode
        } else {
            "right".into()
        }
    } else {
        String::new()
    };
    let mode = if setup {
        "panel"
    } else if pin {
        pin_mode.as_str()
    } else {
        mode
    };
    if ["collapse", "expand"].contains(&mode) {
        if let Some(w) = app.get_webview_window("companion") {
            w.set_size(tauri::LogicalSize::new(
                420.,
                if mode == "collapse" { 460. } else { 650. },
            ))
            .map_err(|e| e.to_string())?;
        }
        return Ok(());
    }
    if mode == "workspace" {
        if let Some(w) = app.get_webview_window("main") {
            w.show().map_err(|e| e.to_string())?;
            w.set_focus().map_err(|e| e.to_string())?;
            let _ = app.emit_to("main", "meeting-workspace", json!({}));
        }
        return Ok(());
    }
    let state = app.state::<Arc<Desktop>>();
    let saved = preferences(&state.dir);
    let mode = if mode == "saved" {
        if ["left", "right", "panel"].contains(&saved.mode.as_str()) {
            saved.mode.as_str()
        } else {
            "panel"
        }
    } else {
        mode
    };
    if !["left", "right", "panel"].contains(&mode) {
        return Err("Unknown companion position".into());
    }
    let window = if let Some(w) = app.get_webview_window("companion") {
        w
    } else {
        let w = WebviewWindowBuilder::new(
            app,
            "companion",
            WebviewUrl::App(
                if setup {
                    "index.html?companion=1&setup=1"
                } else {
                    "index.html?companion=1"
                }
                .into(),
            ),
        )
        .title("Thesaurus · Meeting")
        .inner_size(420., 650.)
        .min_inner_size(350., 250.)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
        let handle = app.clone();
        let weak = w.clone();
        let movement = Arc::new(std::sync::atomic::AtomicU64::new(0));
        w.on_window_event(move |event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = weak.hide();
            }
            tauri::WindowEvent::Focused(false) => {
                let state = handle.state::<Arc<Desktop>>();
                if preferences(&state.dir).mode == "panel" {
                    let _ = weak.hide();
                }
            }
            // A docked widget keeps its edge and vertical position across openings.
            tauri::WindowEvent::Moved(p) => {
                use std::sync::atomic::Ordering;
                let ticket = movement.fetch_add(1, Ordering::SeqCst) + 1;
                let latest = movement.clone();
                let handle = handle.clone();
                let y = p.y;
                // Persist after dragging settles, keeping disk sync off the UI thread.
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                    if latest.load(Ordering::SeqCst) != ticket {
                        return;
                    }
                    let state = handle.state::<Arc<Desktop>>();
                    let mut pref = preferences(&state.dir);
                    if ["left", "right"].contains(&pref.mode.as_str()) {
                        pref.y = Some(y);
                        save_preferences(&state.dir, &pref);
                    }
                });
            }
            _ => {}
        });
        w
    };
    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .or(window.primary_monitor().map_err(|e| e.to_string())?);
    if let Some(m) = monitor {
        let pos = m.position();
        let size = m.size();
        let scale = m.scale_factor();
        let width = (420. * scale) as i32;
        let height = (650. * scale) as i32;
        let inset = (12. * scale) as i32;
        let x = if mode == "left" {
            pos.x + inset
        } else {
            pos.x + (size.width as i32 - width - inset).max(0)
        };
        let y = saved
            .y
            .filter(|_| mode != "panel")
            .unwrap_or(pos.y + (36. * scale) as i32)
            .clamp(pos.y, pos.y + (size.height as i32 - height).max(0));
        save_preferences(
            &state.dir,
            &Position {
                mode: mode.into(),
                y: Some(y),
            },
        );
        window
            .set_position(tauri::PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
    }
    if setup {
        let _ = window.emit("meeting-setup", json!({}));
    }
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}
