use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use thesaurus_contracts::*;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Discover {
    pub topic: String,
}
pub async fn discover(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
    Json(req): Json<Discover>,
) -> Api<OpportunityDiscovery> {
    Ok(Json(
        app.store
            .discover_opportunity(&project, &req.topic, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn opportunities(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
) -> Api<Vec<Opportunity>> {
    Ok(Json(
        app.store
            .opportunities(&project, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn create_opportunity(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
    Json(req): Json<OpportunityCreate>,
) -> Api<Opportunity> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .create_opportunity(&project, req, &Access::admin())
            .await?,
    ))
}
pub async fn review_opportunity(
    State(app): State<App>,
    headers: HeaderMap,
    Path((project, id)): Path<(String, String)>,
    Json(req): Json<InsightReview>,
) -> Api<Opportunity> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .review_opportunity(&project, &id, req, &Access::admin())
            .await?,
    ))
}
pub async fn weekly(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
) -> Api<Vec<WeeklyOutcomes>> {
    Ok(Json(
        app.store
            .weekly_outcomes(&project, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn create_weekly(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
    Json(req): Json<WeeklyCreate>,
) -> Api<WeeklyOutcomes> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .create_weekly_outcomes(&project, req, &Access::admin())
            .await?,
    ))
}
pub async fn get_weekly(
    State(app): State<App>,
    headers: HeaderMap,
    Path((project, id)): Path<(String, String)>,
) -> Api<WeeklyOutcomes> {
    Ok(Json(
        app.store
            .weekly_outcome(&project, &id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn review_weekly(
    State(app): State<App>,
    headers: HeaderMap,
    Path((project, id)): Path<(String, String)>,
    Json(req): Json<InsightReview>,
) -> Api<WeeklyOutcomes> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .review_weekly_outcomes(&project, &id, req, &Access::admin())
            .await?,
    ))
}
pub async fn efforts(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
) -> Api<Vec<TaskEffort>> {
    Ok(Json(
        app.store
            .task_efforts(&project, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn effort(
    State(app): State<App>,
    headers: HeaderMap,
    Path((project, task)): Path<(String, String)>,
    Json(req): Json<TaskEffortUpdate>,
) -> Api<TaskEffort> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .set_task_effort(&project, &task, req, &Access::admin())
            .await?,
    ))
}
pub async fn capacity(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
) -> Api<Vec<CapacityPlan>> {
    Ok(Json(
        app.store
            .capacity_plans(&project, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn save_capacity(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
    Json(req): Json<CapacitySave>,
) -> Api<CapacityPlan> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .save_capacity_plan(&project, req, &Access::admin())
            .await?,
    ))
}
pub async fn import_calendar(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<CalendarImport>,
) -> Api<CalendarImportPreview> {
    app.admin(&headers).await?;
    Ok(Json(thesaurus_core::Store::import_capacity_calendar(req)?))
}
