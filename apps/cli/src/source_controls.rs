use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use thesaurus_contracts::*;
pub async fn disclosure(
    State(app): State<App>,
    headers: HeaderMap,
    Path(source): Path<String>,
) -> Api<SourceDisclosurePreview> {
    let config = app.model_config().await?;
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .source_disclosure_preview(&source, &config, &Access::admin())
            .await?,
    ))
}
pub async fn policy(
    State(app): State<App>,
    headers: HeaderMap,
    Path(source): Path<String>,
    Json(req): Json<SourcePolicyUpdate>,
) -> Api<SourcePolicy> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .update_source_policy(&source, req, &Access::admin())
            .await?,
    ))
}
pub async fn health(
    State(app): State<App>,
    headers: HeaderMap,
    Path(source): Path<String>,
) -> Api<SourceHealth> {
    Ok(Json(
        app.store
            .source_health(&source, &app.access(&headers).await?)
            .await?,
    ))
}
pub async fn coverage(
    State(app): State<App>,
    headers: HeaderMap,
    Path(source): Path<String>,
    Json(req): Json<SourceCoverageUpdate>,
) -> Api<SourceCoverage> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .update_source_coverage(&source, req, &Access::admin())
            .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retry {
    pub document_id: String,
}
pub async fn retry(
    State(app): State<App>,
    headers: HeaderMap,
    Path(source): Path<String>,
    Json(req): Json<Retry>,
) -> Api<Job> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .retry_source_capture(&source, &req.document_id, &Access::admin())
            .await?,
    ))
}
pub async fn notifications(
    State(app): State<App>,
    headers: HeaderMap,
) -> Api<Vec<WorkNotification>> {
    app.admin(&headers).await?;
    app.store.reconcile_notifications().await?;
    Ok(Json(app.store.notifications(&Access::admin()).await?))
}
pub async fn update_notification(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<NotificationUpdate>,
) -> Api<WorkNotification> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .update_notification(&id, req, &Access::admin())
            .await?,
    ))
}
pub async fn mutes(State(app): State<App>, headers: HeaderMap) -> Api<Vec<NotificationMute>> {
    app.admin(&headers).await?;
    Ok(Json(app.store.notification_mutes(&Access::admin()).await?))
}
pub async fn set_mute(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<NotificationMuteUpdate>,
) -> Api<NotificationMute> {
    app.admin(&headers).await?;
    Ok(Json(
        app.store
            .set_notification_mute(req, &Access::admin())
            .await?,
    ))
}
