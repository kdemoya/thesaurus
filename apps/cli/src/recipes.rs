use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use serde::Deserialize;
use thesaurus_contracts::*;

#[derive(Deserialize)]
pub struct Version {
    version: Option<i64>,
}
pub async fn list(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
) -> Api<Vec<WorkRecipe>> {
    Ok(Json(
        app.store
            .recipes(&project, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn get(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<Version>,
) -> Api<WorkRecipe> {
    Ok(Json(
        app.store
            .recipe(&id, q.version, &app.access(&headers).await?)
            .await?,
    ))
}
async fn connection(app: &App, id: &str) -> anyhow::Result<thesaurus_core::config::Connection> {
    let config = app.model_config().await?;
    config
        .connections
        .iter()
        .find(|c| c.id == id && c.kind == "acp")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("invalid: select a configured ACP connection"))
}
pub async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<RecipeSaveRequest>,
) -> Api<WorkRecipe> {
    app.admin(&headers).await?;
    connection(&app, &req.connection).await?;
    Ok(Json(app.store.save_recipe(None, req).await?))
}
pub async fn save(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<RecipeSaveRequest>,
) -> Api<WorkRecipe> {
    app.admin(&headers).await?;
    connection(&app, &req.connection).await?;
    Ok(Json(app.store.save_recipe(Some(&id), req).await?))
}
pub async fn preview(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<RecipePreviewRequest>,
) -> Api<RecipePreview> {
    app.admin(&headers).await?;
    Ok(Json(app.store.preview_recipe(&id, req).await?))
}
pub async fn runs(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<RecipeRun>> {
    app.admin(&headers).await?;
    Ok(Json(app.store.recipe_runs(&id).await?))
}
pub async fn run(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<RecipeRunRequest>,
) -> Api<RuntimeRun> {
    app.admin(&headers).await?;
    let recipe = app
        .store
        .recipe(&id, Some(req.expected_version), &Access::admin())
        .await?;
    let c = connection(&app, &recipe.connection).await?;
    if !thesaurus_core::models::executable(c.command.as_deref().unwrap_or_default()) {
        return Err(anyhow::anyhow!("invalid ACP executable").into());
    }
    let reserved = app
        .store
        .create_recipe_runtime(&id, req, c.daily_limit)
        .await?;
    Ok(Json(crate::runtime::launch(&app, c, reserved).await))
}
