use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;
pub async fn cues(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<DiscoveryQuery>,
) -> Api<CuePage> {
    Ok(Json(
        a.store.question_cues(&id, q, &a.access(&h).await?).await?,
    ))
}
pub async fn cue(State(a): State<App>, h: HeaderMap, Path(id): Path<String>) -> Api<QuestionCue> {
    Ok(Json(a.store.question_cue(&id, &a.access(&h).await?).await?))
}
pub async fn create_cue(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<CueCreateRequest>,
) -> Api<QuestionCue> {
    a.admin(&h).await?;
    Ok(Json(a.store.create_question_cue(r).await?))
}
pub async fn update_cue(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<CueUpdateRequest>,
) -> Api<QuestionCue> {
    a.admin(&h).await?;
    Ok(Json(a.store.update_question_cue(&id, r).await?))
}
pub async fn gaps(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<DiscoveryQuery>,
) -> Api<GapPage> {
    Ok(Json(
        a.store.knowledge_gaps(&id, q, &a.access(&h).await?).await?,
    ))
}
pub async fn gap(State(a): State<App>, h: HeaderMap, Path(id): Path<String>) -> Api<KnowledgeGap> {
    Ok(Json(
        a.store.knowledge_gap(&id, &a.access(&h).await?).await?,
    ))
}
pub async fn create_gap(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<GapCreateRequest>,
) -> Api<KnowledgeGap> {
    a.admin(&h).await?;
    Ok(Json(a.store.create_knowledge_gap(r).await?))
}
pub async fn update_gap(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<GapUpdateRequest>,
) -> Api<KnowledgeGap> {
    a.admin(&h).await?;
    Ok(Json(a.store.update_knowledge_gap(&id, r).await?))
}
pub async fn refresh(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<DiscoveryRefresh> {
    a.admin(&h).await?;
    Ok(Json(a.store.refresh_discovery(&id).await?))
}
pub async fn mute(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<DiscoveryMuteRequest>,
) -> Api<DiscoverySettings> {
    a.admin(&h).await?;
    Ok(Json(a.store.mute_question_cues(&id, r).await?))
}
pub async fn history(
    State(a): State<App>,
    h: HeaderMap,
    Path((kind, id)): Path<(String, String)>,
    Query(q): Query<DiscoveryQuery>,
) -> Api<DiscoveryHistoryPage> {
    Ok(Json(
        a.store
            .discovery_history(&kind, &id, q, &a.access(&h).await?)
            .await?,
    ))
}
