use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;
#[derive(serde::Deserialize)]
pub struct Version {
    pub version: Option<i64>,
}
pub async fn suites(State(app): State<App>, headers: HeaderMap) -> Api<Vec<EvalSuite>> {
    Ok(Json(
        app.store.eval_suites(&app.access(&headers).await?).await?,
    ))
}
pub async fn suite(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<Version>,
) -> Api<EvalSuite> {
    Ok(Json(
        app.store
            .eval_suite(&id, q.version, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn create(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<EvalSuiteRequest>,
) -> Api<EvalSuite> {
    app.admin(&headers).await?;
    Ok(Json(app.store.save_eval_suite(None, req).await?))
}
pub async fn update(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<EvalSuiteRequest>,
) -> Api<EvalSuite> {
    app.admin(&headers).await?;
    Ok(Json(app.store.save_eval_suite(Some(&id), req).await?))
}
pub async fn preview(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<EvalRunRequest>,
) -> Api<EvalPreview> {
    app.admin(&headers).await?;
    Ok(Json(app.store.eval_preview(&id, &req).await?))
}
pub async fn execute(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<EvalRunRequest>,
) -> Api<EvalRun> {
    app.admin(&headers).await?;
    Ok(Json(app.store.execute_eval(&id, req).await?))
}
pub async fn runs(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
) -> Api<Vec<EvalRunSummary>> {
    Ok(Json(
        app.store
            .eval_runs(&project, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn run(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<EvalRun> {
    Ok(Json(
        app.store
            .eval_run(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn reviews(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<EvalReview>> {
    Ok(Json(
        app.store
            .eval_reviews(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn review(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<EvalReviewRequest>,
) -> Api<EvalReview> {
    app.admin(&headers).await?;
    Ok(Json(app.store.review_eval(&id, req).await?))
}
pub async fn judge(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, case)): Path<(String, String)>,
) -> Api<EvalJudgePreview> {
    app.admin(&headers).await?;
    Ok(Json(app.store.eval_judge_preview(&id, &case).await?))
}
/// Exactly one user approved judge request; existing runtime enforces the connection daily limit.
pub async fn start_judge(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, case)): Path<(String, String)>,
    Json(req): Json<EvalJudgeRequest>,
) -> Api<RuntimeRun> {
    let config = app.model_config().await?;
    app.admin(&headers).await?;
    let preview = app.store.eval_judge_preview(&id, &case).await?;
    if !req.approve_usage
        || req.max_approximate_input_tokens == 0
        || preview.approximate_input_tokens > req.max_approximate_input_tokens
    {
        return Err(anyhow::anyhow!(
            "invalid: explicitly approve judge usage within the input estimate budget"
        )
        .into());
    }
    let c = config
        .connections
        .iter()
        .find(|c| c.id == req.connection && c.kind == "acp")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("unknown ACP connection"))?;
    if !thesaurus_core::models::executable(c.command.as_deref().unwrap_or_default()) {
        return Err(anyhow::anyhow!("invalid ACP executable").into());
    }
    let reserved = app
        .store
        .create_eval_judge_runtime(&id, &case, req, c.daily_limit)
        .await?;
    Ok(Json(crate::runtime::launch(&app, c, reserved).await))
}

async fn replay_connection(
    app: &App,
    id: &str,
) -> Result<thesaurus_core::config::Connection, anyhow::Error> {
    let config = app.model_config().await?;
    config
        .connections
        .iter()
        .find(|c| c.id == id && c.kind == "acp")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("unknown ACP connection"))
}
pub async fn replay_preview(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<EvalReplayPlanRequest>,
) -> Api<EvalReplayPlan> {
    app.admin(&headers).await?;
    let c = replay_connection(&app, &req.connection).await?;
    Ok(Json(app.store.eval_replay_preview(&id, &req, &c).await?))
}
pub async fn replay_create(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<EvalReplayPlanRequest>,
) -> Api<EvalReplayPlan> {
    app.admin(&headers).await?;
    let c = replay_connection(&app, &req.connection).await?;
    Ok(Json(app.store.save_eval_replay(&id, req, &c).await?))
}
pub async fn replay_get(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<EvalReplayPlan> {
    Ok(Json(
        app.store
            .eval_replay(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn replay_start(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, case)): Path<(String, String)>,
    Json(req): Json<EvalReplayStartRequest>,
) -> Api<RuntimeRun> {
    app.admin(&headers).await?;
    let p = app.store.eval_replay(&id, &Access::admin()).await?;
    let c = replay_connection(&app, &p.connection).await?;
    if !thesaurus_core::models::executable(c.command.as_deref().unwrap_or_default()) {
        return Err(anyhow::anyhow!("invalid ACP executable").into());
    }
    let config = thesaurus_core::evaluations::eval_connection_config_hash(&c)?;
    let reserved = app
        .store
        .create_eval_replay_runtime(&id, &case, req, &c.id, &config, c.daily_limit)
        .await?;
    Ok(Json(crate::runtime::launch(&app, c, reserved).await))
}
pub async fn replay_finalize(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, case)): Path<(String, String)>,
    Json(req): Json<EvalReplayFinalizeRequest>,
) -> Api<EvalRun> {
    app.admin(&headers).await?;
    Ok(Json(app.store.finalize_eval_replay(&id, &case, req).await?))
}
