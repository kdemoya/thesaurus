use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;

pub async fn priorities(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
    Query(query): Query<PriorityQuery>,
) -> Api<PriorityBoard> {
    let access = app.access(&headers).await?;
    let agent = access.source_ids.is_some();
    let limit = query.limit.unwrap_or(if agent { 25 } else { 500 }) as usize;
    let offset = query.offset.unwrap_or(0) as usize;
    if limit == 0 || limit > if agent { 50 } else { 500 } || offset > 500 {
        return Err(anyhow::anyhow!("invalid priority page limits").into());
    }
    let mut board = app.store.priorities(&project, &access).await?;
    board.items = board.items.into_iter().skip(offset).take(limit).collect();
    board.offset = offset;
    board.limit = limit;
    board.items_omitted = board.total_items.saturating_sub(board.items.len());
    let departures = if agent { 50 } else { 500 };
    board.departed_omitted = board.departed.len().saturating_sub(departures);
    board.departed.truncate(departures);
    if agent {
        while serde_json::to_vec(&board)
            .map_err(anyhow::Error::from)?
            .len()
            > 300000
        {
            if board.departed.pop().is_some() {
                board.departed_omitted += 1;
            } else if board.items.pop().is_some() {
                board.items_omitted += 1;
            } else {
                return Err(anyhow::anyhow!(
                    "invalid: priority metadata exceeds shared response limit"
                )
                .into());
            }
        }
    }
    Ok(Json(board))
}
pub async fn preference(
    State(app): State<App>,
    headers: HeaderMap,
    Path((project, task)): Path<(String, String)>,
    Json(req): Json<PriorityPreferenceRequest>,
) -> Api<PriorityPreference> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .set_priority_preference(&project, &task, req)
            .await?,
    ))
}
pub async fn review(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
    Json(req): Json<PriorityReviewRequest>,
) -> Api<PriorityReview> {
    app.admin(&headers).await?;
    Ok(Json(app.store.review_priorities(&project, req).await?))
}
pub async fn reviews(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
) -> Api<Vec<PriorityReviewSummary>> {
    app.admin(&headers).await?;
    Ok(Json(app.store.priority_reviews(&project).await?))
}
pub async fn reviewed(
    State(app): State<App>,
    headers: HeaderMap,
    Path((project, id)): Path<(String, String)>,
) -> Api<PriorityReview> {
    app.admin(&headers).await?;
    Ok(Json(app.store.priority_review(&project, &id).await?))
}
pub async fn waiting(
    State(app): State<App>,
    headers: HeaderMap,
    Path(project): Path<String>,
    Query(query): Query<WaitingQuery>,
) -> Api<WaitingList> {
    let access = app.access(&headers).await?;
    let mut list = app.store.waiting_list(&project, query, &access).await?;
    if access.source_ids.is_some() {
        while serde_json::to_vec(&list)
            .map_err(anyhow::Error::from)?
            .len()
            > 300000
        {
            if list.items.pop().is_none() {
                return Err(anyhow::anyhow!(
                    "invalid: waiting metadata exceeds shared response limit"
                )
                .into());
            }
        }
    }
    Ok(Json(list))
}
pub async fn get_waiting(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<WaitingFor> {
    Ok(Json(
        app.store
            .waiting_for(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn create_waiting(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<WaitingCreateRequest>,
) -> Api<WaitingFor> {
    app.admin(&headers).await?;
    Ok(Json(app.store.create_waiting(req).await?))
}
pub async fn update_waiting(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<WaitingUpdateRequest>,
) -> Api<WaitingFor> {
    app.admin(&headers).await?;
    Ok(Json(app.store.update_waiting(&id, req).await?))
}
