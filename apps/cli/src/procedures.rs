use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;
pub async fn suggestions(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<ProcedureQuery>,
) -> Api<ProcedurePage> {
    a.admin(&h).await?;
    Ok(Json(
        a.store
            .procedure_suggestions(&id, q, &Access::admin())
            .await?,
    ))
}
pub async fn get(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<ProcedureSuggestion> {
    a.admin(&h).await?;
    Ok(Json(
        a.store.procedure_suggestion(&id, &Access::admin()).await?,
    ))
}
pub async fn refresh(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<ProcedureRefresh> {
    a.admin(&h).await?;
    Ok(Json(a.store.refresh_procedures(&id).await?))
}
pub async fn reviews(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<ProcedureQuery>,
) -> Api<ProcedureReviewPage> {
    a.admin(&h).await?;
    Ok(Json(a.store.procedure_reviews(&id, q).await?))
}
pub async fn review(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<ProcedureReviewRequest>,
) -> Api<ProcedureRunReview> {
    a.admin(&h).await?;
    Ok(Json(a.store.review_procedure_run(r).await?))
}
pub async fn decide(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<ProcedureDecisionRequest>,
) -> Api<ProcedureSuggestion> {
    a.admin(&h).await?;
    Ok(Json(a.store.decide_procedure(&id, r).await?))
}
pub async fn install(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<ProcedureInstallRequest>,
) -> Api<ProcedureInstallation> {
    let config = a.model_config().await?;
    a.admin(&h).await?;
    if !config
        .connections
        .iter()
        .any(|c| c.id == r.connection && c.kind == "acp")
    {
        return Err(anyhow::anyhow!("invalid: choose a configured ACP connection").into());
    }
    Ok(Json(a.store.install_procedure(&id, r).await?))
}
pub async fn history(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<ProcedureQuery>,
) -> Api<ProcedureHistoryPage> {
    a.admin(&h).await?;
    Ok(Json(
        a.store.procedure_history(&id, q, &Access::admin()).await?,
    ))
}
