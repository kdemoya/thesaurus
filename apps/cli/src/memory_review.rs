use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use thesaurus_contracts::*;

#[derive(Default, Deserialize)]
pub struct Page {
    #[serde(default)]
    pub offset: u32,
}
pub async fn catalog(
    State(app): State<App>,
    headers: HeaderMap,
    Query(page): Query<Page>,
) -> Api<Vec<MemoryInspection>> {
    let access = app.access(&headers).await?;
    let mut list = app.store.memory_catalog(page.offset, &access).await?;
    if access.source_ids.is_some() {
        while serde_json::to_vec(&list)
            .map_err(anyhow::Error::from)?
            .len()
            > 250000
        {
            list.pop();
        }
    }
    Ok(Json(list))
}
pub async fn inspect(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<MemoryInspection> {
    Ok(Json(
        app.store
            .inspect_memory(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn revision(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, version)): Path<(String, i64)>,
) -> Api<MemoryRevision> {
    Ok(Json(
        app.store
            .memory_revision(&id, version, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn revise(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<MemoryReviseRequest>,
) -> Api<MemoryInspection> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store.revise_memory(&id, req, &Access::admin()).await?,
    ))
}
pub async fn conflicts(State(app): State<App>, headers: HeaderMap) -> Api<Vec<MemoryConflict>> {
    let access = app.access(&headers).await?;
    let mut list = app.store.memory_conflicts(&access).await?;
    if access.source_ids.is_some() {
        while serde_json::to_vec(&list)
            .map_err(anyhow::Error::from)?
            .len()
            > 250000
        {
            if list.pop().is_none() {
                break;
            }
        }
    }
    Ok(Json(list))
}
pub async fn get_conflict(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<MemoryConflict> {
    let result = app
        .store
        .memory_conflict(&id, &app.access(&headers).await?)
        .await?;
    if serde_json::to_vec(&result)
        .map_err(anyhow::Error::from)?
        .len()
        > 250000
    {
        return Err(anyhow::anyhow!("invalid: conflict evidence exceeds shared response budget; inspect each memory revision and source document separately").into());
    }
    Ok(Json(result))
}
pub async fn create_conflict(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<MemoryConflictCreate>,
) -> Api<MemoryConflict> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .create_memory_conflict(req, &Access::admin())
            .await?,
    ))
}
pub async fn resolve_conflict(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<MemoryConflictResolve>,
) -> Api<MemoryConflict> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .resolve_memory_conflict(&id, req, &Access::admin())
            .await?,
    ))
}
pub async fn conflict_history(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(page): Query<Page>,
) -> Api<MemoryConflictHistory> {
    let mut result = app
        .store
        .memory_conflict_history(&id, page.offset, &app.access(&headers).await?)
        .await?;
    while serde_json::to_vec(&result)
        .map_err(anyhow::Error::from)?
        .len()
        > 250000
    {
        if result.records.pop().is_none() {
            break;
        }
    }
    Ok(Json(result))
}
