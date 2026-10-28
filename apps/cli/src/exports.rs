use crate::server::{Api, App};
use axum::{Json, extract::State, http::HeaderMap};
use serde::Deserialize;
use serde_json::Value;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRequest {
    pub source_ids: Vec<String>,
    pub format: String,
}
pub async fn export(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<ExportRequest>,
) -> Api<Value> {
    Ok(Json(
        app.store
            .export_context(req.source_ids, &req.format, &app.access(&headers).await?)
            .await?,
    ))
}
