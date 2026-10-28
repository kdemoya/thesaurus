use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;

pub async fn list(
    State(app): State<App>,
    h: HeaderMap,
    Query(q): Query<BragbookQuery>,
) -> Api<BragbookList> {
    app.admin(&h).await?;
    Ok(Json(app.store.bragbook_entries(q, &Access::admin()).await?))
}
pub async fn review(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<BragbookReview>,
) -> Api<BragbookEntry> {
    app.admin(&h).await?;
    Ok(Json(
        app.store
            .review_bragbook_entry(&id, req, &Access::admin())
            .await?,
    ))
}
pub async fn collect(State(app): State<App>, h: HeaderMap) -> Api<BragbookCollection> {
    app.admin(&h).await?;
    Ok(Json(
        app.store
            .collect_existing_bragbook(&Access::admin())
            .await?,
    ))
}
pub async fn export(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<BragbookExportRequest>,
) -> Api<serde_json::Value> {
    app.admin(&h).await?;
    Ok(Json(
        app.store.export_bragbook(req, &Access::admin()).await?,
    ))
}
