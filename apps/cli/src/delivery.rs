use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;

pub async fn list(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
    Query(query): Query<CommitmentQuery>,
) -> Api<CommitmentList> {
    Ok(Json(
        app.store
            .commitment_list(&project, query, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn get(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<Commitment> {
    Ok(Json(
        app.store
            .commitment(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn history(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<CommitmentQuery>,
) -> Api<CommitmentHistory> {
    Ok(Json(
        app.store
            .commitment_history(&id, query, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<CommitmentCreateRequest>,
) -> Api<Commitment> {
    app.admin(&headers).await?;
    Ok(Json(app.store.create_commitment(req).await?))
}
pub async fn update(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<CommitmentUpdateRequest>,
) -> Api<Commitment> {
    app.admin(&headers).await?;
    Ok(Json(app.store.update_commitment(&id, req).await?))
}

pub async fn repository_link(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<DeliveryRepositoryRequest>,
) -> Api<Commitment> {
    app.admin(&headers).await?;
    Ok(Json(app.store.link_commitment_repository(&id, req).await?))
}
