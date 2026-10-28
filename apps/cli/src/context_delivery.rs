use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;
#[derive(serde::Deserialize)]
pub struct Cursor {
    pub cursor: Option<String>,
}
pub async fn build(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<ContextBuildRequest>,
) -> Api<ContextBundle> {
    Ok(Json(
        app.store
            .context_budgeted(req, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn receipt(
    State(app): State<App>,
    headers: HeaderMap,
    Path((thread, context)): Path<(String, String)>,
) -> Api<HandoffReceipt> {
    Ok(Json(
        app.store
            .handoff_receipt(&thread, &context, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn receipt_update(
    State(app): State<App>,
    headers: HeaderMap,
    Path((thread, context)): Path<(String, String)>,
    Json(req): Json<HandoffUpdateRequest>,
) -> Api<HandoffReceipt> {
    Ok(Json(
        app.store
            .update_handoff(&thread, &context, req, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn observe(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<Cursor>,
) -> Api<RuntimeObservation> {
    app.admin(&headers).await?;
    if query.cursor.as_ref().is_some_and(|c| c.len() > 100) {
        return Err(anyhow::anyhow!("invalid observation cursor").into());
    }
    Ok(Json(
        app.store
            .observe_runtime(&id, query.cursor.as_deref())
            .await?,
    ))
}
