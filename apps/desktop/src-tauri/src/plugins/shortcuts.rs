//! Application menu accelerators are an independently configured provider.
use super::*;
use tauri::menu::{Menu, MenuItem, Submenu};

pub(super) fn install(app: &tauri::AppHandle) -> Result<(), String> {
    let menu = Menu::default(app).map_err(|e| e.to_string())?;
    let meeting = MenuItem::with_id(
        app,
        "meeting",
        "Meeting companion",
        true,
        Some("CmdOrCtrl+Shift+M"),
    )
    .map_err(|e| e.to_string())?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        "Pause recording",
        true,
        Some("CmdOrCtrl+Shift+P"),
    )
    .map_err(|e| e.to_string())?;
    let stop = MenuItem::with_id(
        app,
        "stop",
        "Stop recording",
        true,
        Some("CmdOrCtrl+Shift+S"),
    )
    .map_err(|e| e.to_string())?;
    let submenu = Submenu::with_items(app, "Meeting", true, &[&meeting, &pause, &stop])
        .map_err(|e| e.to_string())?;
    menu.append(&submenu).map_err(|e| e.to_string())?;
    app.set_menu(menu).map_err(|e| e.to_string())?;
    Ok(())
}
pub(super) fn teardown(app: &tauri::AppHandle) -> Result<(), String> {
    app.set_menu(Menu::default(app).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    Ok(())
}
