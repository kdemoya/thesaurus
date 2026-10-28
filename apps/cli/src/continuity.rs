use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde_json::{Value, json};
use thesaurus_contracts::*;

// Index routes expose metadata; detail routes enforce the complete source scope.
pub async fn threads(State(app): State<App>, headers: HeaderMap) -> Api<Vec<Value>> {
    let threads = app.store.work_threads(&app.access(&headers).await?).await?;
    Ok(Json(threads.into_iter().map(|t|json!({"id":t.id,"project_id":t.project_id,"title":t.title,"source_ids":t.source_ids,"status":t.status,"version":t.version,"entry_count":t.entries.len()+t.omitted_entries,"updated_at":t.updated_at})).collect()))
}
pub async fn thread(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<WorkThread> {
    Ok(Json(
        app.store
            .work_thread(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn create_thread(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<ThreadCreateRequest>,
) -> Api<WorkThread> {
    app.admin(&headers).await?;
    Ok(Json(app.store.create_thread(req).await?))
}
pub async fn update_thread(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<ThreadUpdateRequest>,
) -> Api<WorkThread> {
    app.admin(&headers).await?;
    Ok(Json(app.store.update_thread(&id, req).await?))
}
pub async fn append(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<ThreadEntryRequest>,
) -> Api<WorkThread> {
    app.admin(&headers).await?;
    Ok(Json(app.store.append_thread_entry(&id, req).await?))
}
pub async fn split(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<ThreadSplitRequest>,
) -> Api<WorkThread> {
    app.admin(&headers).await?;
    Ok(Json(app.store.split_thread(&id, req).await?))
}
pub async fn merge(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<ThreadMergeRequest>,
) -> Api<WorkThread> {
    app.admin(&headers).await?;
    Ok(Json(app.store.merge_threads(&id, req).await?))
}
pub async fn context(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<ThreadContextRequest>,
) -> Api<ThreadContext> {
    Ok(Json(
        app.store
            .thread_context(&id, &app.access(&headers).await?, req.byte_budget)
            .await?,
    ))
}
pub async fn meetings(State(app): State<App>, headers: HeaderMap) -> Api<Vec<Value>> {
    Ok(Json(app.store.meeting_series(&app.access(&headers).await?).await?.into_iter().map(|s|json!({"id":s.id,"project_id":s.project_id,"name":s.name,"cadence":s.cadence,"version":s.version,"occurrence_count":s.occurrences.iter().filter(|o|o.status=="active").count(),"updated_at":s.updated_at})).collect()))
}
pub async fn meeting(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<MeetingSeries> {
    Ok(Json(
        app.store.meeting(&id, &app.access(&headers).await?).await?,
    ))
}
pub async fn create_meeting(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<MeetingCreateRequest>,
) -> Api<MeetingSeries> {
    app.admin(&headers).await?;
    Ok(Json(app.store.create_meeting_series(req).await?))
}
pub async fn link(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<MeetingLinkRequest>,
) -> Api<MeetingSeries> {
    app.admin(&headers).await?;
    Ok(Json(app.store.link_meeting(&id, req).await?))
}
pub async fn update_occurrence(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, occurrence)): Path<(String, String)>,
    Json(req): Json<MeetingOccurrenceUpdate>,
) -> Api<MeetingSeries> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .update_meeting_occurrence(&id, &occurrence, req)
            .await?,
    ))
}
pub async fn prep(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<MeetingPreparation> {
    Ok(Json(
        app.store
            .meeting_preparation(&id, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn sources(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<ProjectSourcesRequest>,
) -> Api<ProjectBrief> {
    app.admin(&headers).await?;
    Ok(Json(app.store.expand_project_sources(&id, req).await?))
}

pub async fn delivery(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, context)): Path<(String, String)>,
) -> Api<Value> {
    app.admin(&headers).await?;
    Ok(Json(app.store.thread_delivery(&id, &context).await?))
}
