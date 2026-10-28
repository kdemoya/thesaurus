use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde_json::{Value, json};
use thesaurus_contracts::*;

fn lease(h: &HeaderMap) -> Result<&str, crate::server::ApiError> {
    h.get("x-thesaurus-plugin-host")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("unauthorized desktop host lease").into())
}
pub async fn register_host(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<PluginHostRegistration>,
) -> Api<PluginHostSession> {
    app.admin(&h).await?;
    Ok(Json(app.store.register_desktop_host(req).await?))
}
pub async fn poll_host(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<PluginHostHeartbeat>,
) -> Api<PluginHostPoll> {
    app.admin(&h).await?;
    Ok(Json(
        app.store.poll_desktop_host(&id, lease(&h)?, req).await?,
    ))
}
pub async fn reply_host(
    State(app): State<App>,
    h: HeaderMap,
    Path((id, command)): Path<(String, String)>,
    Json(req): Json<PluginNativeReply>,
) -> Api<Value> {
    app.admin(&h).await?;
    app.store
        .reply_desktop_command(&id, lease(&h)?, &command, req)
        .await?;
    Ok(Json(json!({"received":true})))
}
pub async fn invoke_native(
    State(app): State<App>,
    h: HeaderMap,
    Path(capability): Path<String>,
    Json(req): Json<PluginNativeRequest>,
) -> Api<Value> {
    app.admin(&h).await?;
    if capability == "native.browser" {
        return Ok(Json(app.store.open_local_browser(req).await?));
    }
    let id = app.store.enqueue_desktop_command(&capability, req).await?;
    for _ in 0..100 {
        if let Some(reply) = app.store.desktop_command_result(&id).await? {
            if let Some(error) = reply.error {
                return Err(anyhow::anyhow!("invalid: native provider: {error}").into());
            }
            return Ok(Json(reply.result));
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err(anyhow::anyhow!("conflict: desktop command timed out; outcome is unknown and will not be retried automatically").into())
}

pub async fn list(State(app): State<App>, h: HeaderMap) -> Api<Vec<PluginRecord>> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugins().await?))
}
pub async fn get(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<PluginRecord> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugin(&id).await?))
}
pub async fn capabilities(
    State(app): State<App>,
    h: HeaderMap,
) -> Api<Vec<PluginCapabilityStatus>> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugin_capabilities().await?))
}
pub async fn native_receipts(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<PluginNativeReceipt>> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugin_native_receipts(&id).await?))
}
pub async fn setup(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<PluginSetupStatus>> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugin_setup(&id).await?))
}
pub async fn check_setup(
    State(app): State<App>,
    h: HeaderMap,
    Path((id, step)): Path<(String, String)>,
    Json(req): Json<PluginSetupCheckRequest>,
) -> Api<PluginContributionRun> {
    app.admin(&h).await?;
    Ok(Json(
        app.store
            .request_plugin_setup_check(&id, &step, req)
            .await?,
    ))
}
pub async fn scope(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<String>> {
    app.admin(&h).await?;
    let p = app.store.plugin(&id).await?;
    let access = app.store.plugin_access(&p).await?;
    Ok(Json(
        app.store
            .sources(&access)
            .await?
            .into_iter()
            .map(|s| s.id)
            .collect(),
    ))
}
pub async fn evaluation_inputs(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<PluginEvaluationInputSummary>> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugin_evaluation_inputs(&id).await?))
}
pub async fn select_provider(
    State(app): State<App>,
    h: HeaderMap,
    Path(capability): Path<String>,
    Json(req): Json<PluginProviderSelectionRequest>,
) -> Api<Vec<PluginCapabilityStatus>> {
    app.admin(&h).await?;
    Ok(Json(
        app.store.select_plugin_provider(&capability, req).await?,
    ))
}
pub async fn install(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<PluginInstallRequest>,
) -> Api<PluginRecord> {
    app.admin(&h).await?;
    Ok(Json(app.store.install_plugin(req).await?))
}
pub async fn update(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<PluginUpdateRequest>,
) -> Api<PluginRecord> {
    app.admin(&h).await?;
    Ok(Json(app.store.update_plugin(&id, req).await?))
}
pub async fn uninstall(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<PluginUninstallRequest>,
) -> Api<Value> {
    app.admin(&h).await?;
    app.store.uninstall_plugin(&id, req).await?;
    Ok(Json(json!({"uninstalled":true})))
}
pub async fn deliveries(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<PluginDelivery>> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugin_deliveries(&id).await?))
}
pub async fn attempts(
    State(app): State<App>,
    h: HeaderMap,
    Path((plugin, delivery)): Path<(String, String)>,
) -> Api<Vec<PluginDeliveryAttempt>> {
    app.admin(&h).await?;
    Ok(Json(
        app.store
            .plugin_delivery_attempts(&plugin, &delivery)
            .await?,
    ))
}
pub async fn retry(
    State(app): State<App>,
    h: HeaderMap,
    Path((id, delivery)): Path<(String, String)>,
) -> Api<Value> {
    app.admin(&h).await?;
    app.store.retry_plugin_delivery(&id, &delivery).await?;
    Ok(Json(json!({"queued":true})))
}

pub async fn request_contribution(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<PluginContributionRequest>,
) -> Api<PluginContributionRun> {
    app.admin(&h).await?;
    Ok(Json(app.store.request_plugin_contribution(&id, req).await?))
}
pub async fn contribution_runs(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<PluginContributionRun>> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugin_contribution_runs(&id).await?))
}
pub async fn contribution_run(
    State(app): State<App>,
    h: HeaderMap,
    Path((id, run)): Path<(String, String)>,
) -> Api<PluginContributionRun> {
    app.admin(&h).await?;
    Ok(Json(app.store.plugin_contribution_run(&id, &run).await?))
}
pub async fn cancel_contribution(
    State(app): State<App>,
    h: HeaderMap,
    Path((id, run)): Path<(String, String)>,
) -> Api<PluginContributionRun> {
    app.admin(&h).await?;
    Ok(Json(app.store.cancel_plugin_contribution(&id, &run).await?))
}

pub fn start_workers(app: &App) -> Vec<tokio::task::JoinHandle<()>> {
    let pump = app.clone();
    let mut tasks = vec![tokio::spawn(async move {
        loop {
            if pump.store.pool.is_closed() {
                break;
            }
            let _ = pump.store.queue_plugin_deliveries().await;
            if let Err(error) = pump.store.collect_bragbook_events().await {
                eprintln!("Bragbook collection: {error}");
            }
            let _ = pump.store.reconcile_plugin_models().await;
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
    })];
    for _ in 0..4 {
        let app = app.clone();
        tasks.push(tokio::spawn(async move {
            let mut prefer_contribution = true;
            loop {
                if app.store.pool.is_closed() {
                    break;
                }
                let first = if prefer_contribution {
                    app.store.run_plugin_contribution(&app.data_dir).await
                } else {
                    app.store.run_plugin_delivery(&app.data_dir).await
                };
                let outcome = if matches!(first, Ok(true)) {
                    first
                } else if prefer_contribution {
                    app.store.run_plugin_delivery(&app.data_dir).await
                } else {
                    app.store.run_plugin_contribution(&app.data_dir).await
                };
                prefer_contribution = !prefer_contribution;
                match outcome {
                    Ok(true) => {}
                    _ => tokio::time::sleep(std::time::Duration::from_millis(250)).await,
                }
            }
        }));
    }
    tasks
}
