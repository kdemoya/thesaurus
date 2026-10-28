use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;
pub async fn answer(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<KnowledgeAnswerRequest>,
) -> Api<KnowledgeAnswer> {
    Ok(Json(
        a.store.answer_knowledge(r, &a.access(&h).await?).await?,
    ))
}
pub async fn compare(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<KnowledgeAnswerRequest>,
) -> Api<KnowledgeComparison> {
    Ok(Json(
        a.store.compare_knowledge(r, &a.access(&h).await?).await?,
    ))
}
pub async fn claims(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<KnowledgeQuery>,
) -> Api<ClaimPage> {
    Ok(Json(
        a.store.dated_claims(&id, q, &a.access(&h).await?).await?,
    ))
}
pub async fn claim(State(a): State<App>, h: HeaderMap, Path(id): Path<String>) -> Api<DatedClaim> {
    Ok(Json(a.store.dated_claim(&id, &a.access(&h).await?).await?))
}
pub async fn create_claim(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<ClaimCreateRequest>,
) -> Api<DatedClaim> {
    a.admin(&h).await?;
    Ok(Json(a.store.create_dated_claim(r).await?))
}
pub async fn update_claim(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<ClaimUpdateRequest>,
) -> Api<DatedClaim> {
    a.admin(&h).await?;
    Ok(Json(a.store.update_dated_claim(&id, r).await?))
}
pub async fn claim_history(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<KnowledgeQuery>,
) -> Api<ClaimPage> {
    Ok(Json(
        a.store.claim_history(&id, q, &a.access(&h).await?).await?,
    ))
}
pub async fn people(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<KnowledgeQuery>,
) -> Api<PeoplePage> {
    Ok(Json(a.store.people(&id, q, &a.access(&h).await?).await?))
}
pub async fn person(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<PersonRecord> {
    Ok(Json(a.store.person(&id, &a.access(&h).await?).await?))
}
pub async fn create_person(
    State(a): State<App>,
    h: HeaderMap,
    Json(r): Json<PersonCreateRequest>,
) -> Api<PersonRecord> {
    a.admin(&h).await?;
    Ok(Json(a.store.create_person(r).await?))
}
pub async fn update_person(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(r): Json<PersonUpdateRequest>,
) -> Api<PersonRecord> {
    a.admin(&h).await?;
    Ok(Json(a.store.update_person(&id, r).await?))
}
pub async fn person_history(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<KnowledgeQuery>,
) -> Api<PersonHistoryPage> {
    Ok(Json(
        a.store.person_history(&id, q, &a.access(&h).await?).await?,
    ))
}
