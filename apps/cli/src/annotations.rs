use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use thesaurus_contracts::*;
pub async fn notes(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<AnnotationQuery>,
) -> Api<NotePage> {
    Ok(Json(
        a.store.anchored_notes(&id, q, &a.access(&h).await?).await?,
    ))
}
pub async fn note(State(a): State<App>, h: HeaderMap, Path(id): Path<String>) -> Api<AnchoredNote> {
    Ok(Json(
        a.store.anchored_note(&id, &a.access(&h).await?).await?,
    ))
}
pub async fn note_history(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<AnnotationQuery>,
) -> Api<NoteHistoryPage> {
    Ok(Json(
        a.store
            .anchored_note_history(&id, q, &a.access(&h).await?)
            .await?,
    ))
}
pub async fn create_note(
    State(a): State<App>,
    h: HeaderMap,
    Json(req): Json<NoteCreateRequest>,
) -> Api<AnchoredNote> {
    a.admin(&h).await?;
    Ok(Json(a.store.create_anchored_note(req).await?))
}
pub async fn update_note(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<NoteUpdateRequest>,
) -> Api<AnchoredNote> {
    a.admin(&h).await?;
    Ok(Json(a.store.update_anchored_note(&id, req).await?))
}
pub async fn corrections(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<CorrectionSet> {
    Ok(Json(
        a.store.correction_set(&id, &a.access(&h).await?).await?,
    ))
}
pub async fn set_corrections(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<CorrectionsRequest>,
) -> Api<CorrectionSet> {
    a.admin(&h).await?;
    Ok(Json(a.store.set_corrections(&id, req).await?))
}
pub async fn preview(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<CorrectionPreview> {
    a.admin(&h).await?;
    Ok(Json(
        a.store.correction_preview(&id, &Access::admin()).await?,
    ))
}
pub async fn apply(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<CorrectionApplyRequest>,
) -> Api<CorrectionResult> {
    a.admin(&h).await?;
    Ok(Json(a.store.apply_corrections(&id, req).await?))
}
pub async fn correction_history(
    State(a): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<AnnotationQuery>,
) -> Api<CorrectionAuditPage> {
    a.admin(&h).await?;
    Ok(Json(
        a.store.correction_history(&id, q, &Access::admin()).await?,
    ))
}
