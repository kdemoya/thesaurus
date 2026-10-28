use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::HeaderMap,
};
use base64::Engine;
use serde_json::{Value, json};
use thesaurus_contracts::*;
pub async fn audio_list(State(app): State<App>, h: HeaderMap) -> Api<Vec<AudioCapture>> {
    app.admin(&h).await?;
    Ok(Json(app.store.audio_captures().await?))
}
pub async fn audio_get(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<AudioCapture> {
    app.admin(&h).await?;
    Ok(Json(app.store.audio_capture(&id).await?))
}
pub async fn transcribe(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<AudioTranscribeRequest>,
) -> Api<AudioCapture> {
    let config = app.model_config().await?;
    app.admin(&h).await?;
    if let Some(c) = &req.reasoning_connection {
        if !config
            .connections
            .iter()
            .any(|x| &x.id == c && x.kind == "acp")
        {
            return Err(anyhow::anyhow!("unknown ACP reasoning connection").into());
        }
    }
    let allow = req.allow_external;
    let (r, fresh) = app.store.reserve_audio(req).await?;
    if fresh {
        let task = app.clone();
        let id = r.id.clone();
        tokio::spawn(async move {
            let _ = task.store.process_audio(&id, &task.data_dir, allow).await;
        });
    }
    Ok(Json(r))
}
pub async fn reason(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<AudioReasonRequest>,
) -> Api<RuntimeRun> {
    let config = app.model_config().await?;
    app.admin(&h).await?;
    let c = config
        .connections
        .iter()
        .find(|x| x.id == req.connection && x.kind == "acp")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("unknown ACP reasoning connection"))?;
    let request = app.store.audio_reasoning_request(&id, req).await?;
    let reserved = app.store.create_runtime(request, c.daily_limit).await?;
    app.store.link_audio_reasoning(&id, &reserved.0.id).await?;
    Ok(Json(crate::runtime::launch(&app, c, reserved).await))
}
pub async fn windows(State(app): State<App>, h: HeaderMap) -> Api<Vec<CaptureWindow>> {
    app.admin(&h).await?;
    Ok(Json(serde_json::from_value(
        app.store
            .native_invoke("native.screen_capture", "windows", json!({}))
            .await?,
    )?))
}
pub async fn sessions(State(app): State<App>, h: HeaderMap) -> Api<Vec<ActivitySession>> {
    app.admin(&h).await?;
    Ok(Json(app.store.activity_sessions().await?))
}
fn launch(app: App, session: ActivitySession) {
    tokio::spawn(async move {
        loop {
            match app
                .store
                .activity_tick(&session.id, session.version, &app.data_dir)
                .await
            {
                Ok(true) => {}
                Ok(false) => break,
                Err(e) => {
                    let _ = app
                        .store
                        .pause_activity_error(&session.id, session.version, &e.to_string())
                        .await;
                    break;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(
                session.interval_seconds as u64,
            ))
            .await;
        }
    });
}
pub async fn start(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<ActivityStartRequest>,
) -> Api<ActivitySession> {
    app.admin(&h).await?;
    let (s, fresh) = app.store.start_activity(req).await?;
    if fresh {
        launch(app, s.clone());
    }
    Ok(Json(s))
}
pub async fn control(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<ActivityControlRequest>,
) -> Api<ActivitySession> {
    app.admin(&h).await?;
    let (s, fresh) = app.store.control_activity(&id, req).await?;
    if fresh {
        launch(app, s.clone());
    }
    Ok(Json(s))
}
#[derive(serde::Deserialize)]
pub struct Search {
    #[serde(default)]
    pub q: String,
}
pub async fn frames(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<Search>,
) -> Api<Vec<ActivityFrame>> {
    app.admin(&h).await?;
    Ok(Json(app.store.activity_frames(&id, &q.q).await?))
}
pub async fn image(State(app): State<App>, h: HeaderMap, Path(id): Path<String>) -> Api<Value> {
    app.admin(&h).await?;
    Ok(Json(
        json!({"png_base64":base64::engine::general_purpose::STANDARD.encode(app.store.activity_image(&id).await?)}),
    ))
}
pub async fn delete_frame(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Value> {
    app.admin(&h).await?;
    app.store.delete_activity_frame(&id).await?;
    Ok(Json(json!({"deleted":true})))
}
/// Retention maintenance only; never activates capture or launches processors.
pub fn maintenance(app: App) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            if app.store.pool.is_closed() {
                break;
            }
            let _ = app.store.purge_activity().await;
        }
    });
}

pub async fn permission(State(app): State<App>, h: HeaderMap) -> Api<Value> {
    app.admin(&h).await?;
    Ok(Json(
        app.store
            .native_invoke("native.screen_capture", "permission", json!({}))
            .await?,
    ))
}
