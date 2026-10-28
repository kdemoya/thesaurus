use super::*;

pub(super) async fn active_control(app: tauri::AppHandle, action: &str) -> Result<(), String> {
    let dir = app.state::<Arc<Desktop>>().dir.clone();
    let client = match Client::connect(&dir, None) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let response = match client
        .request(reqwest::Method::GET, "/api/v1/live-meetings")
        .send()
        .await
    {
        Ok(r) => r,
        // A dead daemon closes the recorder's lifetime pipe.
        Err(e) if e.is_connect() => return Ok(()),
        Err(_) => {
            return Err(
                "Service did not confirm recording status; retry Pause before quitting".into(),
            );
        }
    };
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(());
    }
    if !response.status().is_success() {
        return Err("Unable to read current recording status".into());
    }
    let sessions: Vec<thesaurus_contracts::LiveMeeting> =
        response.json().await.map_err(|e| e.to_string())?;
    let target = sessions
        .iter()
        .find(|s| {
            matches!(
                s.state.as_str(),
                "starting" | "recording" | "pausing" | "stopping"
            )
        })
        .or_else(|| {
            if action == "stop" {
                sessions
                    .iter()
                    .filter(|s| s.state == "paused")
                    .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
            } else {
                None
            }
        });
    if let Some(s) = target {
        let _:Value=client.call(reqwest::Method::POST,&format!("/api/v1/live-meetings/{}/control",s.id),Some(json!({"expected_version":s.version,"action":action,"approve_recording":false,"idempotency_key":format!("desktop-{action}-{}-{}",s.id,s.version)}))).await.map_err(|e|e.to_string())?;
    }
    Ok(())
}
pub(super) fn on_exit(app: &tauri::AppHandle, api: &tauri::ExitRequestApi) {
    use std::sync::atomic::Ordering;
    let state = app.state::<Arc<Desktop>>();
    if state.quitting.load(Ordering::SeqCst) {
        return;
    }
    api.prevent_exit();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        match active_control(handle.clone(), "pause").await {
            Ok(()) => {
                handle
                    .state::<Arc<Desktop>>()
                    .quitting
                    .store(true, Ordering::SeqCst);
                handle.exit(0);
            }
            Err(e) => {
                // With a live service, require a confirmed pause before removing the tray.
                let _ = handle.emit(
                    "service-error",
                    format!("Unable to confirm recording paused: {e}"),
                );
                let _ = super::popover::show(&handle);
            }
        }
    });
}
