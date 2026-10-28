use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde_json::{Value, json};
use thesaurus_contracts::*;
pub async fn list(State(app): State<App>, h: HeaderMap) -> Api<Vec<UpstreamConnection>> {
    Ok(Json(app.store.upstreams(&app.access(&h).await?).await?))
}
pub async fn get(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<UpstreamConnection> {
    Ok(Json(app.store.upstream(&id, &app.access(&h).await?).await?))
}
pub async fn create(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<UpstreamRequest>,
) -> Api<UpstreamConnection> {
    app.admin(&h).await?;
    Ok(Json(app.store.save_upstream(None, req).await?))
}
pub async fn update(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<UpstreamRequest>,
) -> Api<UpstreamConnection> {
    app.admin(&h).await?;
    Ok(Json(app.store.save_upstream(Some(&id), req).await?))
}
pub async fn retrieve(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<FederatedRequest>,
) -> Api<FederatedResponse> {
    Ok(Json(
        app.store
            .federated_retrieve(req, &app.access(&h).await?)
            .await?,
    ))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refresh {
    pub query: String,
    pub approve_upstream: bool,
    pub idempotency_key: String,
}
pub async fn refresh(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<Refresh>,
) -> Api<Value> {
    app.admin(&h).await?;
    let (doc, omissions, fetched_at, fresh) = app
        .store
        .fetch_upstream(
            &id,
            &req.query,
            req.approve_upstream,
            &req.idempotency_key,
            &Access::admin(),
        )
        .await?;
    Ok(Json(
        json!({"document":doc,"omissions":omissions,"fetched_at":fetched_at,"fresh":fresh,"path":if fresh{"live_refresh"}else{"receipt_replay"}}),
    ))
}
