use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;
pub async fn previews(State(a): State<App>, h: HeaderMap) -> Api<Vec<ActionPreview>> {
    a.admin(&h).await?;
    Ok(Json(a.store.action_previews().await?))
}
pub async fn get_preview(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<ActionPreview> {
    a.admin(&h).await?;
    Ok(Json(a.store.action_preview(&id).await?))
}
pub async fn preview_receipt(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Option<ActionReceipt>> {
    a.admin(&h).await?;
    Ok(Json(a.store.action_preview_receipt(&id).await?))
}
pub async fn preview(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<ActionPreviewRequest>,
) -> Api<ActionPreview> {
    a.admin(&h).await?;
    Ok(Json(a.store.preview_action(r).await?))
}
pub async fn execute(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<ActionExecuteRequest>,
) -> Api<ActionReceipt> {
    a.admin(&h).await?;
    Ok(Json(a.store.execute_action(&id, r).await?))
}
pub async fn receipts(State(a): State<App>, h: HeaderMap) -> Api<Vec<ActionReceipt>> {
    a.admin(&h).await?;
    Ok(Json(a.store.action_receipts().await?))
}
pub async fn receipt(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<ActionReceipt> {
    a.admin(&h).await?;
    Ok(Json(a.store.action_receipt(&id).await?))
}
pub async fn undo(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<ActionUndoRequest>,
) -> Api<ActionReceipt> {
    a.admin(&h).await?;
    Ok(Json(a.store.undo_action(&id, r).await?))
}
