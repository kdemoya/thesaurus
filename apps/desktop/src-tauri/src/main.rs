#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod notices;
mod plugins;
mod service;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use tauri::{Emitter, Manager, State};
use thesaurus_client::Client;

struct Desktop {
    dir: PathBuf,
    quitting: std::sync::atomic::AtomicBool,
    recovery: service::Recovery,
}

#[tauri::command]
async fn api_request(
    state: State<'_, Arc<Desktop>>,
    path: String,
    method: String,
    body: Option<Value>,
) -> Result<Value, String> {
    let route = path.split('?').next().unwrap_or_default();
    if !path.starts_with("/api/v1/")
        || path.contains("..")
        || path.contains('#')
        || !["GET", "POST", "PATCH", "PUT", "DELETE"].contains(&method.as_str())
        || route.contains('%')
        || route
            .split('/')
            .any(|segment| matches!(segment, "grants" | "pairing" | "pair" | "plugin-host"))
    {
        return Err("This request is not available in the desktop interface".into());
    }
    let client = Client::connect(&state.dir, None)
        .map_err(|_| "The background service is unavailable. Restart Thesaurus.".to_string())?;
    client
        .call(
            method.parse().map_err(|_| "Invalid method".to_string())?,
            &path,
            body.filter(|b| !b.is_null()),
        )
        .await
        .map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            api_request,
            plugins::meeting_window,
            plugins::meeting_popover_height,
            service::recovery_action
        ])
        .setup(|app| {
            let dir = thesaurus_core::data_dir();
            let data = Arc::new(Desktop {
                dir: dir.clone(),
                quitting: Default::default(),
                recovery: Default::default(),
            });
            app.manage(data);
            notices::install(app.handle())?;
            plugins::install(app)?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                service::bootstrap(&handle).await;
                let _ = handle.emit("data-changed", json!({}));
                plugins::connect(handle.clone());
                // The service persists independently. Reconnect subscriptions after a service restart.
                loop {
                    if let Ok(client) = Client::connect(&dir, None) {
                        if let Ok(response) = client
                            .request(reqwest::Method::GET, "/api/v1/events")
                            .send()
                            .await
                        {
                            use futures_util::StreamExt;
                            let mut stream = response.bytes_stream();
                            while let Some(Ok(_)) = stream.next().await {
                                let _ = handle.emit("data-changed", json!({}));
                            }
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("unable to start Thesaurus desktop")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if matches!(&event, tauri::RunEvent::Reopen { .. }) {
                plugins::reopen(app);
            }
            if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                } else {
                    plugins::on_exit(app, &api);
                }
            }
        });
}
