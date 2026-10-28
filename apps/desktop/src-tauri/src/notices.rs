use tauri::{
    Emitter, Manager,
    menu::{HELP_SUBMENU_ID, Menu, MenuItem},
};

fn index(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .resource_dir()
        .ok()
        .map(|root| root.join("resources/notices/index.html"))
        .filter(|path| path.is_file())
        .or_else(|| {
            std::env::current_exe()
                .ok()?
                .parent()
                .map(|root| root.join("notices/index.html"))
                .filter(|path| path.is_file())
        })
}

pub fn install(app: &tauri::AppHandle) -> tauri::Result<()> {
    let menu = match app.menu() {
        Some(menu) => menu,
        None => Menu::default(app)?,
    };
    let item = MenuItem::with_id(
        app,
        "third-party-notices",
        "Third-party licenses",
        index(app).is_some(),
        None::<&str>,
    )?;
    if let Some(help) = menu
        .get(HELP_SUBMENU_ID)
        .and_then(|item| item.as_submenu().cloned())
    {
        help.append(&item)?;
    } else {
        menu.append(&item)?;
    }
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        if event.id.as_ref() == "third-party-notices" {
            if let Some(path) = index(app) {
                // This fixed, bundled file is the only target; no user URL or
                // shell text is accepted by this native menu action.
                if let Err(error) = std::process::Command::new("/usr/bin/open")
                    .arg(path)
                    .spawn()
                {
                    let _ = app.emit(
                        "service-error",
                        format!("Could not open bundled licenses: {error}"),
                    );
                }
            }
        }
    });
    Ok(())
}
