use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;
pub async fn list(State(a): State<App>, h: HeaderMap) -> Api<Vec<ConnectionUsage>> {
    let config = a.model_config().await?;
    a.admin(&h).await?;
    let mut result = vec![];
    for c in &config.connections {
        let billing = if c.kind == "acp" || c.kind.ends_with("cli") {
            "Selected runtime account; limits count attempts, not provider credits"
        } else {
            "Selected API or local endpoint; limits count attempts, not monetary spend"
        };
        result.push(
            a.store
                .connection_usage(&c.id, c.daily_limit, billing.into())
                .await?,
        );
    }
    Ok(Json(result))
}
pub async fn update(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UsagePolicyRequest>,
) -> Api<UsagePolicy> {
    let config = a.model_config().await?;
    a.admin(&h).await?;
    let c = config
        .connections
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| anyhow::anyhow!("unknown connection"))?;
    if let Some(fallback) = &req.fallback_connection {
        if c.kind != "acp"
            || !config
                .connections
                .iter()
                .any(|c| &c.id == fallback && c.kind == "acp")
        {
            return Err(anyhow::anyhow!(
                "invalid: fallback rules require two configured ACP connections"
            )
            .into());
        }
    }
    Ok(Json(
        a.store.set_usage_policy(&id, req, c.daily_limit).await?,
    ))
}
pub async fn fallback_preview(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<FallbackPreview> {
    a.admin(&h).await?;
    Ok(Json(a.store.fallback_preview(&id).await?))
}
pub async fn fallback_run(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<FallbackRequest>,
) -> Api<RuntimeRun> {
    let config = a.model_config().await?;
    a.admin(&h).await?;
    let p = a.store.fallback_preview(&id).await?;
    let c = config
        .connections
        .iter()
        .find(|c| c.id == p.fallback_connection && c.kind == "acp")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("unknown fallback ACP connection"))?;
    if !thesaurus_core::models::executable(c.command.as_deref().unwrap_or_default()) {
        return Err(anyhow::anyhow!("invalid fallback ACP executable").into());
    }
    let reserved = a
        .store
        .create_fallback_runtime(&id, req, c.daily_limit)
        .await?;
    Ok(Json(crate::runtime::launch(&a, c, reserved).await))
}
