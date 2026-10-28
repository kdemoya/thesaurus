use anyhow::{Context, Result, bail};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, patch, post},
};
use fs2::FileExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    collections::HashMap,
    convert::Infallible,
    fs::{self, OpenOptions},
    future::IntoFuture,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use thesaurus_contracts::*;
use thesaurus_core::{
    Config, Store, private_dir, private_write,
    store::{hash, token},
};
use tokio::sync::{Mutex, watch};

#[derive(Clone)]
pub struct App {
    pub(crate) live_audio: Arc<crate::live_meetings::LiveAudio>,
    pub store: Store,
    pub config: Config,
    pub token: String,
    pub port: u16,
    pairs: Arc<Mutex<HashMap<String, Instant>>>,
    sessions: Arc<Mutex<HashMap<String, (String, Instant)>>>,
    shutdown: watch::Sender<bool>,
    script_hashes: String,
    pub(crate) data_dir: PathBuf,
    pub(crate) runtime_controls:
        Arc<Mutex<HashMap<String, tokio::sync::mpsc::Sender<thesaurus_core::acp::Control>>>>,
}

pub struct ApiError(anyhow::Error);
impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}
pub(crate) type Api<T> = std::result::Result<Json<T>, ApiError>;
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let message = self.0.to_string();
        let (status, public) = if message.contains("unauthorized") {
            (
                StatusCode::UNAUTHORIZED,
                "Pair this client or reconnect your agent".to_string(),
            )
        } else if message.starts_with("forbidden") {
            (StatusCode::FORBIDDEN, message)
        } else if message.starts_with("conflict") {
            (StatusCode::CONFLICT, message)
        } else if message == "record not found" {
            (StatusCode::NOT_FOUND, message)
        } else if message.starts_with("invalid")
            || message.starts_with("unsupported")
            || message.starts_with("unknown")
            || message.starts_with("query")
            || message.starts_with("agent observations")
            || message.starts_with("grant must")
        {
            (StatusCode::BAD_REQUEST, message)
        } else {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "The operation could not be completed".into(),
            )
        };
        (status, Json(json!({"error":public}))).into_response()
    }
}

impl App {
    pub(crate) async fn model_config(&self) -> Result<Config> {
        self.store.model_config(&self.config).await
    }

    pub fn new(store: Store, config: Config, port: u16) -> Self {
        Self {
            live_audio: Arc::new(crate::live_meetings::LiveAudio::new()),
            store,
            config,
            token: token(),
            port,
            pairs: Default::default(),
            sessions: Default::default(),
            shutdown: watch::channel(false).0,
            script_hashes: String::new(),
            data_dir: PathBuf::new(),
            runtime_controls: Default::default(),
        }
    }
    pub(crate) async fn access(&self, headers: &HeaderMap) -> Result<Access> {
        if let Some(auth) = headers
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
        {
            if bool::from(auth.as_bytes().ct_eq(self.token.as_bytes())) {
                return Ok(Access::admin());
            }
            return self.store.authenticate(auth).await;
        }
        let cookie = headers
            .get(header::COOKIE)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| {
                s.split(';')
                    .find_map(|v| v.trim().strip_prefix("thesaurus_session="))
            });
        if let Some(cookie) = cookie {
            if self
                .sessions
                .lock()
                .await
                .get(&hash(cookie))
                .is_some_and(|(_, t)| t.elapsed() < Duration::from_secs(86400))
            {
                return Ok(Access::admin());
            }
        }
        bail!("unauthorized")
    }
    pub(crate) async fn admin(&self, headers: &HeaderMap) -> Result<()> {
        if self.access(headers).await?.source_ids.is_some() {
            bail!("forbidden: this operation requires the app or CLI")
        };
        Ok(())
    }
}

pub fn router(mut app: App, web_dir: PathBuf) -> Router {
    if let Ok(index) = std::fs::read_to_string(web_dir.join("index.html")) {
        use base64::Engine;
        use sha2::{Digest, Sha256};
        for cap in regex::Regex::new(r"(?s)<script\b[^>]*>(.*?)</script>")
            .unwrap()
            .captures_iter(&index)
        {
            app.script_hashes.push_str(&format!(
                " 'sha256-{}'",
                base64::engine::general_purpose::STANDARD.encode(Sha256::digest(cap[1].as_bytes()))
            ));
        }
    }
    Router::new()
        .route(
            "/health",
            get(|| async { Json(json!({"status":"ready","protocol_version":PROTOCOL_VERSION})) }),
        )
        .route(
            "/api/v1/runtime/connections/{connection}/probe",
            post(crate::runtime::probe),
        )
        .route("/api/v1/runtime/runs", post(crate::runtime::start))
        .route("/api/v1/runtime/runs/{id}", get(crate::runtime::get))
        .route(
            "/api/v1/runtime/runs/{id}/cancel",
            post(crate::runtime::cancel),
        )
        .route(
            "/api/v1/runtime/runs/{id}/permissions/{permission}",
            post(crate::runtime::decide),
        )
        .route("/api/v1/bragbook", get(crate::bragbook::list))
        .route("/api/v1/bragbook/collect", post(crate::bragbook::collect))
        .route("/api/v1/bragbook/export", post(crate::bragbook::export))
        .route("/api/v1/bragbook/{id}", patch(crate::bragbook::review))
        .route("/api/v1/briefs", get(briefs).post(create_brief))
        .route("/api/v1/briefs/{id}", get(brief))
        .route(
            "/api/v1/briefs/{id}/generate",
            post(crate::runtime::generate_brief),
        )
        .route("/api/v1/briefs/{id}/review", post(review_brief))
        .route(
            "/api/v1/projects/{id}/sources",
            patch(crate::continuity::sources),
        )
        .route(
            "/api/v1/threads",
            get(crate::continuity::threads).post(crate::continuity::create_thread),
        )
        .route(
            "/api/v1/threads/{id}",
            get(crate::continuity::thread).patch(crate::continuity::update_thread),
        )
        .route(
            "/api/v1/threads/{id}/entries",
            post(crate::continuity::append),
        )
        .route("/api/v1/threads/{id}/split", post(crate::continuity::split))
        .route("/api/v1/threads/{id}/merge", post(crate::continuity::merge))
        .route(
            "/api/v1/threads/{id}/context",
            post(crate::continuity::context),
        )
        .route(
            "/api/v1/threads/{id}/contexts/{context}",
            get(crate::continuity::delivery),
        )
        .route(
            "/api/v1/threads/{id}/resume",
            post(crate::runtime::resume_thread),
        )
        .route(
            "/api/v1/meetings",
            get(crate::continuity::meetings).post(crate::continuity::create_meeting),
        )
        .route("/api/v1/meetings/{id}", get(crate::continuity::meeting))
        .route(
            "/api/v1/meetings/{id}/occurrences",
            post(crate::continuity::link),
        )
        .route(
            "/api/v1/meetings/{id}/occurrences/{occurrence}",
            patch(crate::continuity::update_occurrence),
        )
        .route(
            "/api/v1/meetings/{id}/preparation",
            get(crate::continuity::prep),
        )
        .route(
            "/api/v1/projects/{id}/priorities",
            get(crate::focus::priorities),
        )
        .route(
            "/api/v1/projects/{id}/priorities/tasks/{task}",
            patch(crate::focus::preference),
        )
        .route(
            "/api/v1/projects/{id}/priorities/reviews",
            get(crate::focus::reviews).post(crate::focus::review),
        )
        .route(
            "/api/v1/projects/{id}/priorities/reviews/{review}",
            get(crate::focus::reviewed),
        )
        .route("/api/v1/projects/{id}/waiting", get(crate::focus::waiting))
        .route("/api/v1/waiting", post(crate::focus::create_waiting))
        .route(
            "/api/v1/waiting/{id}",
            get(crate::focus::get_waiting).patch(crate::focus::update_waiting),
        )
        .route("/api/v1/pair", post(pair))
        .route("/api/v1/projects/{id}/recipes", get(crate::recipes::list))
        .route("/api/v1/recipes", post(crate::recipes::create))
        .route(
            "/api/v1/recipes/{id}",
            get(crate::recipes::get).patch(crate::recipes::save),
        )
        .route(
            "/api/v1/recipes/{id}/preview",
            post(crate::recipes::preview),
        )
        .route("/api/v1/recipes/{id}/run", post(crate::recipes::run))
        .route("/api/v1/recipes/{id}/runs", get(crate::recipes::runs))
        .route(
            "/api/v1/projects/{id}/commitments",
            get(crate::delivery::list),
        )
        .route("/api/v1/commitments", post(crate::delivery::create))
        .route(
            "/api/v1/commitments/{id}/repository-links",
            post(crate::delivery::repository_link),
        )
        .route(
            "/api/v1/commitments/{id}",
            get(crate::delivery::get).patch(crate::delivery::update),
        )
        .route(
            "/api/v1/commitments/{id}/history",
            get(crate::delivery::history),
        )
        .route(
            "/api/v1/evaluations/suites",
            get(crate::evaluations::suites).post(crate::evaluations::create),
        )
        .route(
            "/api/v1/evaluations/suites/{id}",
            get(crate::evaluations::suite).patch(crate::evaluations::update),
        )
        .route(
            "/api/v1/evaluations/suites/{id}/preview",
            post(crate::evaluations::preview),
        )
        .route(
            "/api/v1/evaluations/suites/{id}/runs",
            post(crate::evaluations::execute),
        )
        .route(
            "/api/v1/projects/{project}/evaluations",
            get(crate::evaluations::runs),
        )
        .route(
            "/api/v1/evaluations/runs/{id}",
            get(crate::evaluations::run),
        )
        .route(
            "/api/v1/evaluations/runs/{id}/reviews",
            get(crate::evaluations::reviews).post(crate::evaluations::review),
        )
        .route(
            "/api/v1/evaluations/runs/{id}/cases/{case}/judge-preview",
            get(crate::evaluations::judge),
        )
        .route(
            "/api/v1/evaluations/runs/{id}/cases/{case}/judge",
            post(crate::evaluations::start_judge),
        )
        .route(
            "/api/v1/evaluations/runs/{id}/replay-preview",
            post(crate::evaluations::replay_preview),
        )
        .route(
            "/api/v1/evaluations/runs/{id}/replay-plans",
            post(crate::evaluations::replay_create),
        )
        .route(
            "/api/v1/evaluations/replay-plans/{id}",
            get(crate::evaluations::replay_get),
        )
        .route(
            "/api/v1/evaluations/replay-plans/{id}/cases/{case}/start",
            post(crate::evaluations::replay_start),
        )
        .route(
            "/api/v1/evaluations/replay-plans/{id}/cases/{case}/finalize",
            post(crate::evaluations::replay_finalize),
        )
        .route("/api/v1/pairing", post(pairing))
        .route("/api/v1/session", get(session))
        .route("/api/v1/snapshot", get(snapshot))
        .route(
            "/api/v1/plugin-host/register",
            post(crate::plugins::register_host),
        )
        .route(
            "/api/v1/plugin-host/{id}/poll",
            post(crate::plugins::poll_host),
        )
        .route(
            "/api/v1/plugin-host/{id}/commands/{command}",
            post(crate::plugins::reply_host),
        )
        .route(
            "/api/v1/plugin-capabilities/{capability}/invoke",
            post(crate::plugins::invoke_native),
        )
        .route(
            "/api/v1/plugin-capabilities",
            get(crate::plugins::capabilities),
        )
        .route(
            "/api/v1/plugin-capabilities/{capability}",
            patch(crate::plugins::select_provider),
        )
        .route(
            "/api/v1/plugins",
            get(crate::plugins::list).post(crate::plugins::install),
        )
        .route(
            "/api/v1/plugins/{id}",
            get(crate::plugins::get)
                .patch(crate::plugins::update)
                .delete(crate::plugins::uninstall),
        )
        .route(
            "/api/v1/plugins/{id}/deliveries",
            get(crate::plugins::deliveries),
        )
        .route("/api/v1/plugins/{id}/scope", get(crate::plugins::scope))
        .route("/api/v1/plugins/{id}/setup", get(crate::plugins::setup))
        .route(
            "/api/v1/plugins/{id}/setup/{step}/check",
            post(crate::plugins::check_setup),
        )
        .route(
            "/api/v1/plugins/{id}/models/{proposal}",
            get(crate::plugin_models::get),
        )
        .route(
            "/api/v1/plugins/{id}/models/{proposal}/preview",
            post(crate::plugin_models::preview),
        )
        .route(
            "/api/v1/plugins/{id}/models/{proposal}/approve",
            post(crate::plugin_models::approve),
        )
        .route(
            "/api/v1/plugins/{id}/models/{proposal}/reject",
            post(crate::plugin_models::reject),
        )
        .route(
            "/api/v1/plugins/{id}/evaluation-inputs",
            get(crate::plugins::evaluation_inputs),
        )
        .route(
            "/api/v1/plugins/{id}/native-requests",
            get(crate::plugins::native_receipts),
        )
        .route(
            "/api/v1/plugins/{id}/runs",
            get(crate::plugins::contribution_runs).post(crate::plugins::request_contribution),
        )
        .route(
            "/api/v1/plugins/{id}/runs/{run}",
            get(crate::plugins::contribution_run),
        )
        .route(
            "/api/v1/plugins/{id}/runs/{run}/cancel",
            post(crate::plugins::cancel_contribution),
        )
        .route(
            "/api/v1/plugins/{id}/deliveries/{delivery}/retry",
            post(crate::plugins::retry),
        )
        .route(
            "/api/v1/plugins/{id}/deliveries/{delivery}/attempts",
            get(crate::plugins::attempts),
        )
        .route("/api/v1/sources", get(sources))
        .route("/api/v1/captures", post(import))
        .route("/api/v1/demo", post(demo))
        .route("/api/v1/documents/{id}", get(document))
        .route("/api/v1/memory/{id}", get(memory))
        .route("/api/v1/search", post(search))
        .route("/api/v1/memory/inspect", get(crate::memory_review::catalog))
        .route(
            "/api/v1/memory/inspect/{id}",
            get(crate::memory_review::inspect).patch(crate::memory_review::revise),
        )
        .route(
            "/api/v1/memory/inspect/{id}/revisions/{version}",
            get(crate::memory_review::revision),
        )
        .route(
            "/api/v1/memory/conflicts",
            get(crate::memory_review::conflicts).post(crate::memory_review::create_conflict),
        )
        .route(
            "/api/v1/memory/conflicts/{id}",
            get(crate::memory_review::get_conflict).patch(crate::memory_review::resolve_conflict),
        )
        .route(
            "/api/v1/memory/conflicts/{id}/history",
            get(crate::memory_review::conflict_history),
        )
        .route(
            "/api/v1/documents/{id}/notes",
            get(crate::annotations::notes),
        )
        .route("/api/v1/notes", post(crate::annotations::create_note))
        .route(
            "/api/v1/notes/{id}",
            get(crate::annotations::note).patch(crate::annotations::update_note),
        )
        .route(
            "/api/v1/notes/{id}/history",
            get(crate::annotations::note_history),
        )
        .route(
            "/api/v1/sources/{id}/corrections",
            get(crate::annotations::corrections).put(crate::annotations::set_corrections),
        )
        .route(
            "/api/v1/sources/{id}/corrections/preview",
            get(crate::annotations::preview),
        )
        .route(
            "/api/v1/sources/{id}/corrections/apply",
            post(crate::annotations::apply),
        )
        .route(
            "/api/v1/sources/{id}/corrections/history",
            get(crate::annotations::correction_history),
        )
        .route(
            "/api/v1/context/budgeted",
            post(crate::context_delivery::build),
        )
        .route(
            "/api/v1/threads/{thread}/contexts/{context}/receipt",
            get(crate::context_delivery::receipt).post(crate::context_delivery::receipt_update),
        )
        .route(
            "/api/v1/runtime/runs/{id}/observe",
            get(crate::context_delivery::observe),
        )
        .route("/api/v1/projects/{id}/cues", get(crate::discovery::cues))
        .route("/api/v1/projects/{id}/gaps", get(crate::discovery::gaps))
        .route("/api/v1/cues", post(crate::discovery::create_cue))
        .route("/api/v1/gaps", post(crate::discovery::create_gap))
        .route(
            "/api/v1/cues/{id}",
            get(crate::discovery::cue).patch(crate::discovery::update_cue),
        )
        .route(
            "/api/v1/gaps/{id}",
            get(crate::discovery::gap).patch(crate::discovery::update_gap),
        )
        .route(
            "/api/v1/projects/{id}/discovery/refresh",
            post(crate::discovery::refresh),
        )
        .route(
            "/api/v1/projects/{id}/discovery/mute",
            axum::routing::put(crate::discovery::mute),
        )
        .route(
            "/api/v1/discovery/{kind}/{id}/history",
            get(crate::discovery::history),
        )
        .route(
            "/api/v1/sources/{id}/disclosure",
            get(crate::source_controls::disclosure),
        )
        .route(
            "/api/v1/sources/{id}/policy",
            axum::routing::put(crate::source_controls::policy),
        )
        .route(
            "/api/v1/sources/{id}/health",
            get(crate::source_controls::health),
        )
        .route(
            "/api/v1/sources/{id}/coverage",
            axum::routing::put(crate::source_controls::coverage),
        )
        .route(
            "/api/v1/sources/{id}/retry",
            post(crate::source_controls::retry),
        )
        .route(
            "/api/v1/notifications",
            get(crate::source_controls::notifications),
        )
        .route(
            "/api/v1/notifications/{id}",
            patch(crate::source_controls::update_notification),
        )
        .route(
            "/api/v1/notification-mutes",
            get(crate::source_controls::mutes).put(crate::source_controls::set_mute),
        )
        .route(
            "/api/v1/repositories",
            get(crate::repository::list).post(crate::repository::create),
        )
        .route(
            "/api/v1/repositories/{id}",
            get(crate::repository::get).patch(crate::repository::update),
        )
        .route(
            "/api/v1/repositories/{id}/reports",
            get(crate::repository::reports).post(crate::repository::report),
        )
        .route(
            "/api/v1/repositories/{id}/symbols",
            get(crate::repository::symbol_history).post(crate::repository::symbols),
        )
        .route("/api/v1/usage", get(crate::usage_controls::list))
        .route(
            "/api/v1/usage/{id}",
            axum::routing::put(crate::usage_controls::update),
        )
        .route(
            "/api/v1/runtime/runs/{id}/fallback",
            get(crate::usage_controls::fallback_preview).post(crate::usage_controls::fallback_run),
        )
        .route("/api/v1/knowledge/answer", post(crate::knowledge::answer))
        .route("/api/v1/knowledge/compare", post(crate::knowledge::compare))
        .route(
            "/api/v1/projects/{id}/claims",
            get(crate::knowledge::claims),
        )
        .route(
            "/api/v1/projects/{id}/people",
            get(crate::knowledge::people),
        )
        .route("/api/v1/claims", post(crate::knowledge::create_claim))
        .route("/api/v1/people", post(crate::knowledge::create_person))
        .route(
            "/api/v1/claims/{id}",
            get(crate::knowledge::claim).patch(crate::knowledge::update_claim),
        )
        .route(
            "/api/v1/people/{id}",
            get(crate::knowledge::person).patch(crate::knowledge::update_person),
        )
        .route(
            "/api/v1/claims/{id}/history",
            get(crate::knowledge::claim_history),
        )
        .route(
            "/api/v1/people/{id}/history",
            get(crate::knowledge::person_history),
        )
        .route(
            "/api/v1/capture/audio",
            get(crate::capture::audio_list).post(crate::capture::transcribe),
        )
        .route("/api/v1/capture/audio/{id}", get(crate::capture::audio_get))
        .route(
            "/api/v1/capture/audio/{id}/reason",
            post(crate::capture::reason),
        )
        .route(
            "/api/v1/onboarding",
            get(crate::onboarding::status).put(crate::onboarding::save),
        )
        .route(
            "/api/v1/onboarding/connections",
            post(crate::onboarding::connection),
        )
        .route("/api/v1/onboarding/check", post(crate::onboarding::check))
        .route("/api/v1/onboarding/memory", post(crate::onboarding::memory))
        .route(
            "/api/v1/onboarding/memory/{id}",
            get(crate::onboarding::memory_config),
        )
        .route(
            "/api/v1/onboarding/example",
            post(crate::onboarding::example),
        )
        .route(
            "/api/v1/transcription",
            get(crate::live_meetings::transcription_catalog),
        )
        .route(
            "/api/v1/transcription/models/{id}",
            post(crate::live_meetings::transcription_model),
        )
        .route(
            "/api/v1/transcription/credential",
            post(crate::live_meetings::transcription_credential),
        )
        .route(
            "/api/v1/transcription/revoke",
            post(crate::live_meetings::revoke_transcription),
        )
        .route(
            "/api/v1/live-meetings/{id}/transcription-receipts",
            get(crate::live_meetings::transcription_receipts),
        )
        .route(
            "/api/v1/live-meetings/{id}/transcription-receipts/{chunk}/cleanup",
            post(crate::live_meetings::cleanup_transcription),
        )
        .route(
            "/api/v1/live-meetings/inputs",
            get(crate::live_meetings::inputs),
        )
        .route(
            "/api/v1/live-meetings",
            get(crate::live_meetings::list).post(crate::live_meetings::start),
        )
        .route(
            "/api/v1/live-meetings/settings",
            get(crate::live_meetings::settings).put(crate::live_meetings::save_settings),
        )
        .route(
            "/api/v1/live-meetings/quick-start",
            post(crate::live_meetings::quick_start),
        )
        .route(
            "/api/v1/live-meetings/{id}",
            get(crate::live_meetings::details).patch(crate::live_meetings::update),
        )
        .route(
            "/api/v1/live-meetings/{id}/control",
            post(crate::live_meetings::control),
        )
        .route(
            "/api/v1/live-meetings/{id}/summaries",
            get(crate::live_meetings::summaries).post(crate::live_meetings::summarize),
        )
        .route(
            "/api/v1/settings/meetings",
            get(crate::live_meetings::processing_settings)
                .put(crate::live_meetings::save_processing_settings),
        )
        .route(
            "/api/v1/live-meetings/{id}/processing",
            get(crate::live_meetings::processing_state)
                .post(crate::live_meetings::request_processing),
        )
        .route(
            "/api/v1/live-meetings/{id}/notes",
            post(crate::live_meetings::note),
        )
        .route("/api/v1/capture/windows", get(crate::capture::windows))
        .route(
            "/api/v1/capture/permission",
            post(crate::capture::permission),
        )
        .route(
            "/api/v1/capture/sessions",
            get(crate::capture::sessions).post(crate::capture::start),
        )
        .route(
            "/api/v1/capture/sessions/{id}/control",
            post(crate::capture::control),
        )
        .route(
            "/api/v1/capture/sessions/{id}/frames",
            get(crate::capture::frames),
        )
        .route(
            "/api/v1/capture/frames/{id}/image",
            get(crate::capture::image),
        )
        .route(
            "/api/v1/capture/frames/{id}",
            axum::routing::delete(crate::capture::delete_frame),
        )
        .route(
            "/api/v1/projects/{id}/opportunities",
            get(crate::insights::opportunities).post(crate::insights::create_opportunity),
        )
        .route(
            "/api/v1/projects/{id}/opportunities/discover",
            post(crate::insights::discover),
        )
        .route(
            "/api/v1/projects/{id}/opportunities/{record}",
            patch(crate::insights::review_opportunity),
        )
        .route(
            "/api/v1/projects/{id}/outcomes",
            get(crate::insights::weekly).post(crate::insights::create_weekly),
        )
        .route(
            "/api/v1/projects/{id}/outcomes/{record}",
            get(crate::insights::get_weekly).patch(crate::insights::review_weekly),
        )
        .route(
            "/api/v1/projects/{id}/efforts",
            get(crate::insights::efforts),
        )
        .route(
            "/api/v1/projects/{id}/efforts/{task}",
            axum::routing::put(crate::insights::effort),
        )
        .route(
            "/api/v1/projects/{id}/capacity",
            get(crate::insights::capacity).post(crate::insights::save_capacity),
        )
        .route(
            "/api/v1/calendar/import-preview",
            post(crate::insights::import_calendar),
        )
        .route(
            "/api/v1/projects/{id}/procedures",
            get(crate::procedures::suggestions),
        )
        .route(
            "/api/v1/projects/{id}/procedures/refresh",
            post(crate::procedures::refresh),
        )
        .route(
            "/api/v1/projects/{id}/procedure-reviews",
            get(crate::procedures::reviews),
        )
        .route("/api/v1/procedure-reviews", post(crate::procedures::review))
        .route(
            "/api/v1/procedures/{id}",
            get(crate::procedures::get).patch(crate::procedures::decide),
        )
        .route(
            "/api/v1/procedures/{id}/install",
            post(crate::procedures::install),
        )
        .route(
            "/api/v1/procedures/{id}/history",
            get(crate::procedures::history),
        )
        .route(
            "/api/v1/upstreams",
            get(crate::retrieval::list).post(crate::retrieval::create),
        )
        .route(
            "/api/v1/upstreams/{id}",
            get(crate::retrieval::get).patch(crate::retrieval::update),
        )
        .route(
            "/api/v1/upstreams/{id}/refresh",
            post(crate::retrieval::refresh),
        )
        .route("/api/v1/retrieval", post(crate::retrieval::retrieve))
        .route(
            "/api/v1/actions/previews",
            get(crate::actions::previews).post(crate::actions::preview),
        )
        .route(
            "/api/v1/actions/previews/{id}/execute",
            post(crate::actions::execute),
        )
        .route(
            "/api/v1/actions/previews/{id}",
            get(crate::actions::get_preview),
        )
        .route(
            "/api/v1/actions/previews/{id}/receipt",
            get(crate::actions::preview_receipt),
        )
        .route("/api/v1/actions/receipts", get(crate::actions::receipts))
        .route(
            "/api/v1/actions/receipts/{id}",
            get(crate::actions::receipt),
        )
        .route(
            "/api/v1/actions/receipts/{id}/undo",
            post(crate::actions::undo),
        )
        .route("/api/v1/context", post(context))
        .route("/api/v1/observations", post(observation))
        .route("/api/v1/exports", post(crate::exports::export))
        .route("/api/v1/tasks/{id}", patch(task))
        .route("/api/v1/documents/{id}/analyze", post(analyze))
        .route("/api/v1/grants", get(grants).post(grant))
        .route("/api/v1/grants/{id}/revoke", post(revoke))
        .route("/api/v1/events", get(events))
        .fallback_service(tower_http::services::ServeDir::new(web_dir))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(middleware::from_fn_with_state(app.clone(), security))
        .with_state(app)
}

async fn security(State(app): State<App>, req: Request, next: Next) -> Response {
    if *app.shutdown.borrow() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error":"conflict: service is stopping; reconnect after restart"})),
        )
            .into_response();
    }
    let allowed_host = |host: &str| {
        host == format!("127.0.0.1:{}", app.port)
            || host == format!("localhost:{}", app.port)
            || host == "localhost"
    };
    if !req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(allowed_host)
    {
        return StatusCode::MISDIRECTED_REQUEST.into_response();
    }
    if let Some(origin) = req
        .headers()
        .get(header::ORIGIN)
        .and_then(|h| h.to_str().ok())
    {
        if origin != format!("http://127.0.0.1:{}", app.port)
            && origin != format!("http://localhost:{}", app.port)
        {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    if req.method() != axum::http::Method::GET
        && req.uri().path() != "/api/v1/pair"
        && !req.headers().contains_key(header::AUTHORIZATION)
    {
        let secret = req
            .headers()
            .get(header::COOKIE)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| {
                s.split(';')
                    .find_map(|v| v.trim().strip_prefix("thesaurus_session="))
            });
        let valid = if let Some(secret) = secret {
            app.sessions
                .lock()
                .await
                .get(&hash(secret))
                .is_some_and(|(csrf, t)| {
                    t.elapsed() < Duration::from_secs(86400)
                        && req
                            .headers()
                            .get("x-thesaurus-csrf")
                            .and_then(|h| h.to_str().ok())
                            == Some(csrf.as_str())
                })
        } else {
            false
        };
        if !valid {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let mut response = next.run(req).await;
    let h = response.headers_mut();
    h.insert(header::CACHE_CONTROL, "no-store".parse().unwrap());
    h.insert("x-content-type-options", "nosniff".parse().unwrap());
    h.insert("referrer-policy", "no-referrer".parse().unwrap());
    h.insert("content-security-policy",format!("default-src 'self'; script-src 'self' 'wasm-unsafe-eval'{}; style-src 'self' 'unsafe-inline'; connect-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",app.script_hashes).parse().unwrap());
    response
}

async fn pairing(State(app): State<App>, headers: HeaderMap) -> Api<Value> {
    app.admin(&headers).await?;
    let code = token();
    let mut pairs = app.pairs.lock().await;
    pairs.retain(|_, t| t.elapsed() < Duration::from_secs(120));
    pairs.insert(hash(&code), Instant::now());
    Ok(Json(
        json!({"url":format!("http://127.0.0.1:{}/#pair={code}",app.port)}),
    ))
}
#[derive(Deserialize)]
struct Pair {
    code: String,
}
async fn pair(
    State(app): State<App>,
    Json(req): Json<Pair>,
) -> std::result::Result<Response, ApiError> {
    let valid = app
        .pairs
        .lock()
        .await
        .remove(&hash(&req.code))
        .is_some_and(|t| t.elapsed() < Duration::from_secs(120));
    if !valid {
        return Err(anyhow::anyhow!("unauthorized").into());
    }
    let secret = token();
    let csrf = token();
    let mut sessions = app.sessions.lock().await;
    sessions.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(86400));
    sessions.insert(hash(&secret), (csrf.clone(), Instant::now()));
    Ok((
        [(
            header::SET_COOKIE,
            format!("thesaurus_session={secret}; HttpOnly; SameSite=Strict; Path=/; Max-Age=86400"),
        )],
        Json(json!({"csrf":csrf})),
    )
        .into_response())
}
async fn session(State(app): State<App>, headers: HeaderMap) -> Api<Value> {
    app.admin(&headers).await?;
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| {
            s.split(';')
                .find_map(|v| v.trim().strip_prefix("thesaurus_session="))
        });
    let csrf = if let Some(cookie) = cookie {
        app.sessions
            .lock()
            .await
            .get(&hash(cookie))
            .map(|(s, _)| s.clone())
    } else {
        None
    };
    Ok(Json(
        json!({"csrf":csrf,"protocol_version":PROTOCOL_VERSION}),
    ))
}
async fn snapshot(State(app): State<App>, headers: HeaderMap) -> Api<Snapshot> {
    let config = app.model_config().await?;
    app.admin(&headers).await?;
    let mut snap = app.store.snapshot(&Access::admin()).await?;
    snap.connections = thesaurus_core::models::connections(&config);
    let primary = app.store.onboarding_progress().await?.primary_connection;
    snap.connections.sort_by_key(|c| c.id != primary);
    Ok(Json(snap))
}
async fn sources(State(app): State<App>, headers: HeaderMap) -> Api<Vec<Source>> {
    Ok(Json(app.store.sources(&app.access(&headers).await?).await?))
}
async fn import(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<ImportRequest>,
) -> Api<ImportResult> {
    app.admin(&headers).await?;
    Ok(Json(app.store.import(req).await?))
}
#[derive(Deserialize, Default)]
struct Revision {
    revision: Option<i64>,
}
async fn document(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<Revision>,
) -> Api<Document> {
    Ok(Json(
        app.store
            .document(&id, q.revision, &app.access(&headers).await?)
            .await?,
    ))
}
async fn memory(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<Revision>,
) -> Api<Value> {
    let access = app.access(&headers).await?;
    if id.starts_with("doc-") {
        return Ok(Json(serde_json::to_value(
            app.store.document(&id, q.revision, &access).await?,
        )?));
    };
    let inspection = app.store.inspect_memory(&id, &access).await?;
    if inspection.current.status == "suppressed" {
        return Err(anyhow::anyhow!("record not found").into());
    }
    let mut observation = inspection.original;
    if inspection.current.version > 0 {
        observation.body = inspection.current.body;
        observation.contributor = format!(
            "{} (originally {})",
            inspection.current.contributor, observation.contributor
        );
    }
    Ok(Json(serde_json::to_value(observation)?))
}
async fn search(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<SearchRequest>,
) -> Api<Vec<SearchHit>> {
    Ok(Json(
        app.store.search(&req, &app.access(&headers).await?).await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContextRequest {
    query: String,
    character_budget: Option<usize>,
}
async fn context(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<ContextRequest>,
) -> Api<ContextBundle> {
    Ok(Json(
        app.store
            .context(
                &req.query,
                req.character_budget.unwrap_or(8000),
                &app.access(&headers).await?,
            )
            .await?,
    ))
}
async fn observation(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<ObservationRequest>,
) -> Api<Observation> {
    Ok(Json(
        app.store
            .append_observation(req, &app.access(&headers).await?)
            .await?,
    ))
}
async fn task(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<TaskUpdate>,
) -> Api<Task> {
    app.admin(&headers).await?;
    Ok(Json(app.store.update_task(&id, req).await?))
}
async fn analyze(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<AnalysisRequest>,
) -> Api<Job> {
    let config = app.model_config().await?;
    app.admin(&headers).await?;
    let mut limit = None;
    if config
        .connections
        .iter()
        .any(|c| c.id == req.connection && c.kind == "acp")
    {
        bail_api("invalid: use an ACP runtime run instead of document extraction")?;
    }
    if req.connection != "local" {
        if !req.approve_usage {
            bail_api("forbidden: approve this model usage request")?;
        }
        let connection = config
            .connections
            .iter()
            .find(|c| c.id == req.connection)
            .context("unknown connection")?;
        let doc = app.store.document(&id, None, &Access::admin()).await?;
        app.store
            .check_source_destination(
                &doc.source_id,
                &req.connection,
                thesaurus_core::models::external(&config, &req.connection)?,
            )
            .await?;
        let source = app
            .store
            .sources(&Access::admin())
            .await?
            .into_iter()
            .find(|s| s.id == doc.source_id)
            .context("record not found")?;
        if thesaurus_core::models::external(&config, &req.connection)? && !source.allow_external {
            bail_api(
                "forbidden: source is local only; update its disclosure permission before model use",
            )?;
        }
        limit = Some(connection.daily_limit);
    }
    Ok(Json(
        app.store
            .queue_analysis(&id, &req.connection, limit)
            .await?,
    ))
}
fn bail_api(message: &str) -> Result<()> {
    bail!("{message}")
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantRequest {
    name: String,
    source_ids: Vec<String>,
    #[serde(default)]
    allow_write: bool,
    #[serde(default)]
    allow_external: bool,
}
async fn grant(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<GrantRequest>,
) -> Api<Value> {
    app.admin(&headers).await?;
    let (grant, secret) = app
        .store
        .create_grant(
            req.name,
            req.source_ids,
            req.allow_write,
            req.allow_external,
        )
        .await?;
    Ok(Json(json!({"grant":grant,"token":secret})))
}
async fn grants(State(app): State<App>, headers: HeaderMap) -> Api<Vec<GrantInfo>> {
    app.admin(&headers).await?;
    Ok(Json(app.store.grants().await?))
}
async fn revoke(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Api<Value> {
    app.admin(&headers).await?;
    app.store.revoke_grant(&id).await?;
    Ok(Json(json!({"revoked":true})))
}

async fn events(
    State(app): State<App>,
    headers: HeaderMap,
) -> std::result::Result<
    Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>>>,
    ApiError,
> {
    app.admin(&headers).await?;
    let mut since = headers
        .get("last-event-id")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let mut shutdown = app.shutdown.subscribe();
    let stream = async_stream::stream! {loop {if *shutdown.borrow(){break}match app.store.events(since).await {Ok(events)=>for event in events {since=event.sequence;yield Ok(Event::default().id(since.to_string()).event("change").data(serde_json::to_string(&event).unwrap_or_default()));},Err(_)=>break}tokio::select!{_=shutdown.changed()=>break,_=tokio::time::sleep(Duration::from_millis(500))=>{}}}};
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10))))
}

async fn demo(State(app): State<App>, headers: HeaderMap) -> Api<Value> {
    app.admin(&headers).await?;
    for (name, kind, title, body) in [
        (
            "Product sync",
            "transcript",
            "A calmer way to launch",
            include_str!("../../../fixtures/product-sync.md"),
        ),
        (
            "Platform notes",
            "knowledge",
            "Launch readiness guide",
            include_str!("../../../fixtures/launch-guide.md"),
        ),
        (
            "Project notes",
            "markdown",
            "This week's focus",
            include_str!("../../../fixtures/focus.md"),
        ),
    ] {
        app.store
            .import(ImportRequest {
                source_name: name.into(),
                kind: kind.into(),
                uri: format!("sample://{}", kind),
                title: title.into(),
                body: body.into(),
                allow_external: false,
            })
            .await?;
    }
    Ok(Json(json!({"imported":3})))
}

fn workspace_dir(dir: &std::path::Path) -> Result<PathBuf> {
    private_dir(dir)?;
    // Native providers and child processes receive absolute resource paths even
    // when the CLI was started with a relative --data-dir.
    Ok(fs::canonicalize(dir).context("unable to resolve the workspace directory")?)
}
pub async fn serve(dir: PathBuf, port: u16, web_dir: PathBuf) -> Result<()> {
    let dir = workspace_dir(&dir)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(dir.join("daemon.lock"))?;
    lock.try_lock_exclusive()
        .context("another daemon already owns this data directory")?;
    let socket = dir.join("control.sock");
    if socket.exists() {
        fs::remove_file(&socket)?;
    }
    let store = Store::open(&dir.join("thesaurus.sqlite3")).await?;
    fs::set_permissions(
        dir.join("thesaurus.sqlite3"),
        fs::Permissions::from_mode(0o600),
    )?;
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let actual_port = listener.local_addr()?.port();
    let control = tokio::net::UnixListener::bind(&socket)?;
    fs::set_permissions(&socket, fs::Permissions::from_mode(0o600))?;
    let config = Config::load(&dir)?;
    let mut app = App::new(store.clone(), config.clone(), actual_port);
    app.data_dir = dir.clone();
    crate::capture::maintenance(app.clone());
    let plugin_tasks = crate::plugins::start_workers(&app);
    {
        let mut tx = store.pool.begin().await?;
        thesaurus_core::plugins::emit_tx(&mut tx, "service.ready", "service", "local", 1, &[], &[])
            .await?;
        tx.commit().await?;
    }
    let instance = Instance {
        protocol_version: PROTOCOL_VERSION,
        pid: std::process::id(),
        api_url: format!("http://127.0.0.1:{actual_port}"),
        socket: socket.to_string_lossy().into_owned(),
        token: app.token.clone(),
    };
    private_write(
        &dir.join("instance.json"),
        &serde_json::to_vec_pretty(&instance)?,
    )?;
    let (stop, receiver) = watch::channel(false);
    let meeting_worker = crate::live_meetings::start_processing_worker(&app, receiver.clone());
    let mut worker_stop = receiver.clone();
    let notification_store = store.clone();
    let mut notification_stop = receiver.clone();
    let notification_worker = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = notification_stop.changed() => break,
                _ = interval.tick() => {
                    if notification_store.reconcile_notifications().await.is_err() {
                        eprintln!("notification reconciliation failed");
                    }
                }
            }
        }
    });
    let work = dir.join("agent-work");
    private_dir(&work)?;
    let worker = tokio::spawn(async move {
        loop {
            tokio::select! {_=worker_stop.changed()=>break,result=store.process_next(&config,&work)=>{if result.is_err(){eprintln!("worker cycle failed")}}}
            if store.recover_meeting_summaries().await.is_err() {
                eprintln!("meeting summary reconciliation failed");
            }
            // Reconcile durable receipts locally if finalization was interrupted. Never rerun a model.
            if store.recover_briefs().await.is_err() {
                eprintln!("brief reconciliation failed");
            }
            tokio::select! {_=worker_stop.changed()=>break,_=tokio::time::sleep(Duration::from_millis(300))=>{}}
        }
    });
    let shutdown = app.shutdown.clone();
    let runtime_app = app.clone();
    let api = router(app, web_dir);
    let mut control_stop = receiver.clone();
    let control_task = tokio::spawn(
        axum::serve(control, api.clone())
            .with_graceful_shutdown(async move {
                let _ = control_stop.changed().await;
            })
            .into_future(),
    );
    eprintln!(
        "Thesaurus is ready at {}. Run thesaurus open to pair a browser.",
        instance.api_url
    );
    let mut http_stop = shutdown.subscribe();
    let mut http_task = tokio::spawn(
        axum::serve(listener, api)
            .with_graceful_shutdown(async move {
                let _ = http_stop.changed().await;
            })
            .into_future(),
    );
    let http_result = tokio::select! {
        _=tokio::signal::ctrl_c()=>None,
        result=&mut http_task=>Some(result),
    };
    // Stop accepting new work and wake SSE before waiting on any HTTP body.
    // A slow browser downloading assets must not keep a recording or daemon
    // ownership alive after the user stops the service.
    shutdown.send_replace(true);
    let stopping = tokio::time::timeout(Duration::from_millis(500), async {
        let mut tx = runtime_app.store.pool.begin_with("BEGIN IMMEDIATE").await?;
        thesaurus_core::plugins::emit_tx(
            &mut tx,
            "service.stopping",
            "service",
            "local",
            1,
            &[],
            &[],
        )
        .await?;
        tx.commit().await?;
        runtime_app.store.queue_plugin_deliveries().await?;
        Ok::<_, anyhow::Error>(())
    })
    .await;
    if !matches!(stopping, Ok(Ok(()))) {
        eprintln!("Plugin shutdown event could not be queued before the shutdown deadline");
    }
    meeting_worker.abort();
    let _ = meeting_worker.await;
    crate::live_meetings::shutdown(&runtime_app).await;
    crate::runtime::shutdown(&runtime_app).await;
    for task in plugin_tasks {
        task.abort();
        let _ = task.await;
    }
    let _ = stop.send(true);
    worker.abort();
    notification_worker.abort();
    let _ = worker.await;
    if http_result.is_none() {
        drain_server(&mut http_task).await;
    }
    let mut control_task = control_task;
    drain_server(&mut control_task).await;
    let _ = fs::remove_file(dir.join("instance.json"));
    let _ = fs::remove_file(socket);
    drop(lock);
    if let Some(result) = http_result {
        result??;
    }
    Ok(())
}

async fn drain_server<T: Send + 'static>(task: &mut tokio::task::JoinHandle<T>) {
    if tokio::time::timeout(Duration::from_secs(2), &mut *task)
        .await
        .is_err()
    {
        task.abort();
        let _ = task.await;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn relative_workspace_paths_become_absolute_without_accepting_symlinks() {
        let cwd = std::env::current_dir().unwrap();
        let dir = tempfile::tempdir_in(&cwd).unwrap();
        let relative = dir.path().strip_prefix(&cwd).unwrap();
        assert!(!relative.is_absolute());
        let resolved = super::workspace_dir(relative).unwrap();
        assert!(resolved.is_absolute());
        assert_eq!(resolved, std::fs::canonicalize(dir.path()).unwrap());
        #[cfg(unix)]
        {
            let link = dir.path().join("symlink");
            std::os::unix::fs::symlink(dir.path(), &link).unwrap();
            assert!(super::workspace_dir(&link).is_err());
        }
    }
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use tower::ServiceExt;

    async fn app() -> (tempfile::TempDir, App, Router) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("test.sqlite")).await.unwrap();
        let app = App::new(store, Config::default(), 4620);
        let router = router(app.clone(), dir.path().into());
        (dir, app, router)
    }
    fn request(method: &str, path: &str, token: Option<&str>, body: Value) -> Request<Body> {
        let mut b = Request::builder()
            .method(method)
            .uri(path)
            .header("Host", "localhost")
            .header("Content-Type", "application/json");
        if let Some(token) = token {
            b = b.header("Authorization", format!("Bearer {token}"));
        }
        b.body(Body::from(body.to_string())).unwrap()
    }
    async fn value(response: Response) -> Value {
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
    }
    #[tokio::test]
    async fn bragbook_requires_authentication_and_uses_the_review_transport() {
        let (_dir, app, router) = app().await;
        let response = router
            .clone()
            .oneshot(request("GET", "/api/v1/bragbook", None, json!({})))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = router
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/bragbook",
                Some(&app.token),
                json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(value(response).await["total"], 0);
        let response = router
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/bragbook/collect",
                Some(&app.token),
                json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let response = router
            .oneshot(request(
                "POST",
                "/api/v1/bragbook/export",
                Some(&app.token),
                json!({"entry_ids":[],"format":"markdown"}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
    #[tokio::test]
    async fn shutdown_rejects_new_work_without_mutating_the_workspace() {
        let (_dir, app, router) = app().await;
        app.shutdown.send_replace(true);
        let response=router.oneshot(request("POST","/api/v1/captures",Some(&app.token),json!({"source_name":"Synthetic shutdown source","kind":"transcript","uri":"fixture://shutdown","title":"Synthetic shutdown source","body":"This must not be imported","allow_external":false}))).await.unwrap();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            app.store
                .sources(&Access::admin())
                .await
                .unwrap()
                .is_empty()
        );
    }
    #[tokio::test]
    async fn shutdown_bounds_stalled_http_drain() {
        let stopped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        struct Guard(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Guard {
            fn drop(&mut self) {
                self.0.store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let done = stopped.clone();
        let mut task = tokio::spawn(async move {
            let _guard = Guard(done);
            std::future::pending::<()>().await;
        });
        drain_server(&mut task).await;
        assert!(task.is_finished());
        assert!(stopped.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[tokio::test]
    async fn private_reads_require_auth_and_cross_origin_requests_are_rejected() {
        let (_dir, app, router) = app().await;
        assert_eq!(
            router
                .clone()
                .oneshot(request("GET", "/api/v1/snapshot", None, Value::Null))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
        let mut req = request("GET", "/api/v1/snapshot", Some(&app.token), Value::Null);
        req.headers_mut()
            .insert("Origin", "https://untrusted.example".parse().unwrap());
        assert_eq!(
            router.clone().oneshot(req).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        let response = router
            .oneshot(request(
                "GET",
                "/api/v1/snapshot",
                Some(&app.token),
                Value::Null,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            !response
                .headers()
                .contains_key("Access-Control-Allow-Origin")
        );
    }

    #[tokio::test]
    async fn browser_pairing_is_single_use_and_cookie_mutations_need_csrf() {
        let (_dir, app, router) = app().await;
        let response = router
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/pairing",
                Some(&app.token),
                json!({}),
            ))
            .await
            .unwrap();
        let url = value(response).await["url"].as_str().unwrap().to_string();
        let code = url.split("#pair=").nth(1).unwrap();
        let paired = router
            .clone()
            .oneshot(request("POST", "/api/v1/pair", None, json!({"code":code})))
            .await
            .unwrap();
        assert_eq!(paired.status(), StatusCode::OK);
        let cookie = paired.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let csrf = value(paired).await["csrf"].as_str().unwrap().to_string();
        let again = router
            .clone()
            .oneshot(request("POST", "/api/v1/pair", None, json!({"code":code})))
            .await
            .unwrap();
        assert_eq!(again.status(), StatusCode::UNAUTHORIZED);
        let mut no_csrf_bragbook = request("POST", "/api/v1/bragbook/collect", None, json!({}));
        no_csrf_bragbook
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        assert_eq!(
            router
                .clone()
                .oneshot(no_csrf_bragbook)
                .await
                .unwrap()
                .status(),
            StatusCode::FORBIDDEN
        );
        let mut no_csrf = request("POST", "/api/v1/demo", None, json!({}));
        no_csrf
            .headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        assert_eq!(
            router.clone().oneshot(no_csrf).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        let mut ok = request("POST", "/api/v1/demo", None, json!({}));
        ok.headers_mut()
            .insert(header::COOKIE, cookie.parse().unwrap());
        ok.headers_mut()
            .insert("x-thesaurus-csrf", csrf.parse().unwrap());
        assert_eq!(router.oneshot(ok).await.unwrap().status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn model_requests_need_both_explicit_usage_and_source_permission() {
        let (_dir, mut app, _) = app().await;
        app.config
            .connections
            .push(thesaurus_core::config::Connection {
                id: "agent".into(),
                kind: "claude_cli".into(),
                model: String::new(),
                command: None,
                args: vec![],
                endpoint: None,
                token_env: None,
                credential_ref: None,
                daily_limit: 1,
            });
        let doc = app
            .store
            .import(ImportRequest {
                source_name: "Private".into(),
                kind: "transcript".into(),
                uri: "test://private".into(),
                title: "Private".into(),
                body: "Action: Check rollout".into(),
                allow_external: false,
            })
            .await
            .unwrap()
            .document;
        let router = router(app.clone(), PathBuf::new());
        for approved in [false, true] {
            let result = router
                .clone()
                .oneshot(request(
                    "POST",
                    &format!("/api/v1/documents/{}/analyze", doc.id),
                    Some(&app.token),
                    json!({"connection":"agent","approve_usage":approved}),
                ))
                .await
                .unwrap();
            assert_eq!(result.status(), StatusCode::FORBIDDEN);
        }
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE connection='agent'")
            .fetch_one(&app.store.pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
}

async fn briefs(State(app): State<App>, headers: HeaderMap) -> Api<Vec<ProjectBrief>> {
    Ok(Json(app.store.briefs(&app.access(&headers).await?).await?))
}
async fn brief(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Api<ProjectBrief> {
    Ok(Json(
        app.store.brief(&id, &app.access(&headers).await?).await?,
    ))
}
async fn create_brief(
    State(app): State<App>,
    headers: HeaderMap,
    Json(req): Json<BriefCreateRequest>,
) -> Api<ProjectBrief> {
    app.admin(&headers).await?;
    Ok(Json(app.store.create_brief(req).await?))
}
async fn review_brief(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<BriefReviewRequest>,
) -> Api<ProjectBrief> {
    app.admin(&headers).await?;
    Ok(Json(app.store.review_brief(&id, req).await?))
}
