use crate::server::{Api, App};
use anyhow::Context;
use axum::{Json, extract::State, http::HeaderMap};
use serde_json::{Value, json};
use thesaurus_contracts::*;
pub async fn status(State(a): State<App>, h: HeaderMap) -> Api<OnboardingStatus> {
    a.admin(&h).await?;
    Ok(Json(a.store.onboarding_status(&a.config).await?))
}
pub async fn save(
    State(a): State<App>,
    h: HeaderMap,
    Json(req): Json<OnboardingSave>,
) -> Api<OnboardingProgress> {
    a.admin(&h).await?;
    let config = a.model_config().await?;
    Ok(Json(a.store.save_onboarding(req, &config).await?))
}
pub async fn connection(
    State(a): State<App>,
    h: HeaderMap,
    Json(req): Json<SetupConnectionRequest>,
) -> Api<ConnectionInfo> {
    a.admin(&h).await?;
    Ok(Json(a.store.create_setup_connection(req, &a.config).await?))
}
pub async fn check(
    State(a): State<App>,
    h: HeaderMap,
    Json(req): Json<SetupCheckRequest>,
) -> Api<RuntimeRun> {
    a.admin(&h).await?;
    let config = a.model_config().await?;
    let c = config
        .connections
        .iter()
        .find(|c| c.id == req.connection)
        .context("invalid: select a configured model connection")?
        .clone();
    if c.kind == "acp"
        && !thesaurus_core::models::executable(c.command.as_deref().unwrap_or_default())
    {
        return Err(anyhow::anyhow!("invalid: selected agent executable is unavailable").into());
    }
    let reserved = a.store.create_onboarding_check(req, &c).await?;
    Ok(Json(crate::runtime::launch(&a, c, reserved).await))
}
pub async fn memory(
    State(a): State<App>,
    h: HeaderMap,
    Json(req): Json<SetupMemoryRequest>,
) -> Api<Value> {
    a.admin(&h).await?;
    Ok(Json(a.store.setup_memory(req).await?))
}
pub async fn example(State(a): State<App>, h: HeaderMap) -> Api<Value> {
    a.admin(&h).await?;
    let r=a.store.import(ImportRequest {source_name:"Onboarding example".into(),kind:"transcript".into(),uri:"onboarding://first-example".into(),title:"A small meeting, with clear next steps".into(),body:"Maya: I'll share the pilot checklist.\nAction: Review the deployment notes.\nDecision: Start with a staged rollout.".into(),allow_external:false}).await?;
    Ok(Json(json!({"document":r.document})))
}

pub async fn memory_config(
    State(a): State<App>,
    h: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Api<Value> {
    a.admin(&h).await?;
    Ok(Json(a.store.setup_memory_config(&id).await?))
}
