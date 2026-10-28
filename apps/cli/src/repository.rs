use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;
pub async fn list(State(app): State<App>, headers: HeaderMap) -> Api<Vec<RepositoryConnection>> {
    Ok(Json(
        app.store.repositories(&app.access(&headers).await?).await?,
    ))
}
pub async fn get(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<RepositoryConnection> {
    Ok(Json(
        app.store
            .repository(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<RepositoryRequest>,
) -> Api<RepositoryConnection> {
    app.admin(&headers).await?;
    Ok(Json(app.store.save_repository(None, req).await?))
}
pub async fn update(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<RepositoryRequest>,
) -> Api<RepositoryConnection> {
    app.admin(&headers).await?;
    Ok(Json(app.store.save_repository(Some(&id), req).await?))
}
pub async fn reports(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<RepositoryReport>> {
    Ok(Json(
        app.store
            .repository_reports(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn report(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<RepositoryReportRequest>,
) -> Api<RepositoryReport> {
    app.admin(&headers).await?;
    Ok(Json(app.store.repository_report(&id, req).await?))
}
pub async fn symbols(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<SymbolQueryRequest>,
) -> Api<SymbolQueryResult> {
    app.admin(&headers).await?;
    Ok(Json(app.store.repository_symbols(&id, req).await?))
}
pub async fn symbol_history(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<SymbolQueryResult>> {
    Ok(Json(
        app.store
            .repository_symbol_history(&id, &app.access(&headers).await?)
            .await?,
    ))
}
