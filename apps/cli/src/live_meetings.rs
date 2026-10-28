use crate::server::{Api, App};
use axum::{
    Json,
    extract::{Path, State},
    http::HeaderMap,
};
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc, time::Duration};
use thesaurus_contracts::*;
use thesaurus_core::{capture::live, private_dir};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{Mutex, Semaphore, watch},
    task::JoinHandle,
};

#[cfg(test)]
mod processing_tests;

pub struct Recorder {
    cancel: watch::Sender<bool>,
    task: JoinHandle<()>,
}
pub struct LiveAudio {
    recorder: Mutex<Option<Recorder>>,
    transcriber: Mutex<Option<tokio::sync::oneshot::Receiver<()>>>,
    queue: Arc<Semaphore>,
}
impl LiveAudio {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Semaphore::new(4)),
            recorder: Mutex::new(None),
            transcriber: Mutex::new(None),
        }
    }
}
async fn stop_recorder(slot: &mut Option<Recorder>) {
    // Keep the handle in its ownership slot across await. A disconnected caller
    // must not detach a closing recorder and permit a second one to open.
    if let Some(recorder) = slot.as_mut() {
        let _ = recorder.cancel.send(true);
        let _ = (&mut recorder.task).await;
    }
    *slot = None;
}

fn launch(app: App, session: LiveMeeting) -> Recorder {
    let (cancel, receiver) = watch::channel(false);
    let task = tokio::spawn(async move {
        if let Err(e) = record(&app, &session, receiver).await {
            let _ = app
                .store
                .live_recording_error(&session.id, session.version, &e.to_string())
                .await;
        }
        let _ = app
            .store
            .finish_live_control(&session.id, session.version + 1)
            .await;
    });
    Recorder { cancel, task }
}
pub async fn shutdown(app: &App) {
    let mut slot = app.live_audio.recorder.lock().await;
    // Mark state before terminating the helper. A future process never auto-resumes.
    if let Ok(sessions) = app.store.live_meetings().await {
        for s in sessions
            .into_iter()
            .filter(|s| matches!(s.state.as_str(), "starting" | "recording"))
        {
            let _ = app
                .store
                .live_recording_error(
                    &s.id,
                    s.version,
                    "Service stopped; explicitly resume recording",
                )
                .await;
        }
    }
    stop_recorder(&mut slot).await;
}
pub async fn inputs(State(app): State<App>, h: HeaderMap) -> Api<MeetingAudioInputs> {
    app.admin(&h).await?;
    Ok(Json(serde_json::from_value(
        app.store
            .native_invoke("native.audio", "inputs", serde_json::json!({}))
            .await?,
    )?))
}
pub async fn list(State(app): State<App>, h: HeaderMap) -> Api<Vec<LiveMeeting>> {
    app.admin(&h).await?;
    Ok(Json(app.store.live_meetings().await?))
}
pub async fn settings(State(app): State<App>, h: HeaderMap) -> Api<Option<LiveRecordingSettings>> {
    app.admin(&h).await?;
    Ok(Json(app.store.live_recording_settings().await?))
}
pub async fn save_settings(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<LiveRecordingSettingsRequest>,
) -> Api<LiveRecordingSettings> {
    app.admin(&h).await?;
    let _slot = app.live_audio.recorder.lock().await;
    Ok(Json(app.store.save_live_recording_settings(req).await?))
}
pub async fn quick_start(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<LiveRecordingQuickStart>,
) -> Api<LiveMeeting> {
    app.admin(&h).await?;
    let mut slot = app.live_audio.recorder.lock().await;
    app.store.native_provider("native.audio").await?;
    drop(app.store.providers.acquire("native.audio")?);
    let (s, fresh) = app.store.quick_start_live_recording(req).await?;
    if fresh {
        stop_recorder(&mut slot).await;
        *slot = Some(launch(app.clone(), s.clone()));
    }
    Ok(Json(s))
}
pub async fn update(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<LiveMeetingUpdate>,
) -> Api<LiveMeeting> {
    app.admin(&h).await?;
    // Serialize metadata publication with controls and other metadata edits.
    let _slot = app.live_audio.recorder.lock().await;
    Ok(Json(app.store.update_live_meeting(&id, req).await?))
}
pub async fn details(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<LiveMeetingDetails> {
    app.admin(&h).await?;
    Ok(Json(app.store.live_meeting_details(&id).await?))
}
pub async fn start(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<LiveMeetingStart>,
) -> Api<LiveMeeting> {
    app.admin(&h).await?;
    // A single lock serializes recorder ownership across every client and session.
    let mut slot = app.live_audio.recorder.lock().await;
    app.store.native_provider("native.audio").await?;
    drop(app.store.providers.acquire("native.audio")?);
    let (s, fresh) = app.store.start_live_meeting(req).await?;
    if fresh {
        stop_recorder(&mut slot).await;
        *slot = Some(launch(app.clone(), s.clone()));
    }
    Ok(Json(s))
}
pub async fn control(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<LiveMeetingControl>,
) -> Api<LiveMeeting> {
    app.admin(&h).await?;
    let mut slot = app.live_audio.recorder.lock().await;
    // Controlling a historical session must not terminate another live recorder.
    if req.action == "resume" {
        app.store.native_provider("native.audio").await?;
        drop(app.store.providers.acquire("native.audio")?);
    }
    let (s, should_launch) = app.store.control_live_meeting(&id, req).await?;
    if should_launch {
        stop_recorder(&mut slot).await;
        *slot = Some(launch(app.clone(), s.clone()));
    } else if matches!(s.state.as_str(), "pausing" | "stopping") {
        stop_recorder(&mut slot).await;
    }
    let s = app.store.finish_live_control(&id, s.version).await?;
    Ok(Json(s))
}
pub async fn note(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<LiveMeetingNoteRequest>,
) -> Api<LiveMeetingNote> {
    app.admin(&h).await?;
    Ok(Json(app.store.add_live_note(&id, req).await?))
}
#[derive(Deserialize)]
struct BridgeEvent {
    event: String,
    file: Option<String>,
    track: Option<String>,
    offset_ms: Option<u64>,
    message: Option<String>,
}
async fn record(
    app: &App,
    s: &LiveMeeting,
    mut cancel: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let folder = app
        .data_dir
        .join("live-audio")
        .join(format!("{}-{}", s.id, s.version));
    private_dir(&folder)?;
    let result=async {
        let mut resource=app.store.native_open("native.audio","record",serde_json::json!({"session":s,"folder":folder})).await?;
        let child=&mut resource.child;
        // Holding this pipe makes the helper exit on an abrupt parent death too.
        let mut parent_lifetime=child.stdin.take();
        let mut lines=BufReader::new(child.stdout.take().ok_or_else(||anyhow::anyhow!("Audio bridge output unavailable"))?).lines();
        let mut poll=tokio::time::interval(Duration::from_millis(250));
        let mut ready=false;
        let mut stopping=false;
        let mut stop_deadline=None;
        let deadline=tokio::time::Instant::now()+Duration::from_secs(120);
        let loop_result: anyhow::Result<()>=async {
            loop {
                tokio::select! {
                    _=cancel.changed(), if !stopping=>{
                        stopping=true;stop_deadline=Some(tokio::time::Instant::now()+Duration::from_secs(5));
                        if let Some(pipe)=parent_lifetime.as_mut(){let _=pipe.write_all(b"stop\n").await;}
                    },
                    _=poll.tick()=>{
                        if stopping {if stop_deadline.is_some_and(|d|tokio::time::Instant::now()>d){anyhow::bail!("Audio inputs stopped forcibly; the final partial chunk may be missing");}continue;}
                        if *cancel.borrow(){continue;}
                        let current=app.store.live_meeting(&s.id).await?;
                        if current.version!=s.version || !matches!(current.state.as_str(),"starting"|"recording"){
                            stopping=true;stop_deadline=Some(tokio::time::Instant::now()+Duration::from_secs(5));
                            if let Some(pipe)=parent_lifetime.as_mut(){let _=pipe.write_all(b"stop\n").await;}
                            continue;
                        }
                        if live::elapsed_ms(&current)>=14_400_000 {anyhow::bail!("Four-hour recording limit reached; start a new meeting");}
                        if !ready && tokio::time::Instant::now()>deadline {anyhow::bail!("Audio input did not become ready; check permissions and explicitly resume");}
                    }
                    line=lines.next_line()=>{
                        let Some(line)=line? else {if stopping{break;}anyhow::bail!("Audio input ended; recording paused. Check the selected inputs before resuming");};
                        if line.len()>8192 {anyhow::bail!("Invalid audio bridge response");}
                        let e:BridgeEvent=serde_json::from_str(&line)?;
                        match e.event.as_str() {
                            "ready"=>{if stopping{continue;}if !app.store.live_recording_ready(&s.id,s.version).await?{break;}ready=true;}
                            "stopped"=>break,
                            "error"=>anyhow::bail!("{}",e.message.unwrap_or_else(||"Audio input failed".into())),
                            "chunk"=>{
                                let name=e.file.ok_or_else(||anyhow::anyhow!("Invalid audio chunk"))?;
                                let track=e.track.ok_or_else(||anyhow::anyhow!("Invalid audio track"))?;
                                if !["microphone","system"].contains(&track.as_str()) || name.contains(['/', '\\']) || !name.ends_with(".wav") || name.len()>100 {anyhow::bail!("Invalid audio chunk identity");}
                                let permit=app.live_audio.queue.clone().try_acquire_owned().map_err(|_|anyhow::anyhow!("Local transcription is falling behind; recording paused. Wait for queued chunks before resuming"))?;
                                let queue=app.data_dir.join("live-audio").join("queue");private_dir(&queue)?;
                                let chunk_id=uuid::Uuid::new_v4().to_string();let path:PathBuf=queue.join(format!("{chunk_id}.wav"));
                                tokio::fs::rename(folder.join(name),&path).await?;
                                let task_app=app.clone();let session_id=s.id.clone();let version=s.version;let offset=s.recorded_ms+e.offset_ms.unwrap_or(0);
                                if let Err(e)=app.store.live_processing(&s.id,true,None).await {let _=tokio::fs::remove_file(&path).await;return Err(e);}
                                let (done,next)=tokio::sync::oneshot::channel();
                                let previous=app.live_audio.transcriber.lock().await.replace(next);
                                tokio::spawn(async move {
                                    let _permit=permit;
                                    if let Some(previous)=previous {let _=previous.await;}

                                    let r=task_app.store.transcribe_live_chunk(&session_id,&chunk_id,&track,offset,&path,&task_app.data_dir).await;
                                    let _=tokio::fs::remove_file(&path).await;
                                    let error=r.err().map(|e|e.to_string());
                                    if let Some(e)=&error {let _=task_app.store.live_recording_error(&session_id,version,e).await;}
                                    let _=task_app.store.live_processing(&session_id,false,error).await;
                                    let _=done.send(());
                                });
                            }
                            _=>anyhow::bail!("Invalid audio bridge event"),
                        }
                    }
                }
            }
            Ok(())
        }.await;
        let _=child.kill().await;let _=child.wait().await;loop_result
    }.await;
    let _ = tokio::fs::remove_dir_all(&folder).await;
    result
}

/// Explicit processing never starts or stops a recorder.
pub async fn summaries(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<MeetingSummary>> {
    app.admin(&h).await?;
    Ok(Json(app.store.meeting_summaries(&id).await?))
}
pub async fn summarize(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<MeetingSummaryRequest>,
) -> Api<RuntimeRun> {
    let config = app.model_config().await?;
    app.admin(&h).await?;
    let c = config
        .connections
        .iter()
        .find(|c| c.id == req.connection && matches!(c.kind.as_str(), "acp" | "openai_compatible"))
        .ok_or_else(|| {
            anyhow::anyhow!("invalid: select a configured agent or model API connection")
        })?
        .clone();
    if c.kind == "acp"
        && !thesaurus_core::models::executable(c.command.as_deref().unwrap_or_default())
    {
        return Err(anyhow::anyhow!("invalid: configured agent executable is unavailable").into());
    }
    let external = thesaurus_core::models::external(&config, &c.id)?;
    let reserved = app
        .store
        .create_meeting_summary_runtime(&id, req, &c, external)
        .await?;
    Ok(Json(crate::runtime::launch(&app, c, reserved).await))
}

pub async fn processing_settings(
    State(app): State<App>,
    h: HeaderMap,
) -> Api<MeetingProcessingSettings> {
    app.admin(&h).await?;
    Ok(Json(app.store.meeting_processing_settings().await?))
}
pub async fn save_processing_settings(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<MeetingProcessingSettingsRequest>,
) -> Api<MeetingProcessingSettings> {
    app.admin(&h).await?;
    let config = app.model_config().await?;
    Ok(Json(
        app.store
            .save_meeting_processing_settings(req, &config)
            .await?,
    ))
}
pub async fn processing_state(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Option<MeetingProcessingState>> {
    app.admin(&h).await?;
    app.store.live_meeting(&id).await?;
    Ok(Json(app.store.meeting_processing_state(&id).await?))
}
pub async fn request_processing(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<MeetingProcessingRequest>,
) -> Api<MeetingProcessingState> {
    app.admin(&h).await?;
    Ok(Json(app.store.request_meeting_processing(&id, req).await?))
}

fn local_connection() -> thesaurus_core::config::Connection {
    thesaurus_core::config::Connection {
        id: "local".into(),
        kind: "local".into(),
        model: "Local extractive summary".into(),
        endpoint: None,
        command: None,
        args: vec![],
        token_env: None,
        credential_ref: None,
        daily_limit: 1000,
    }
}
pub(crate) fn start_processing_worker(
    app: &App,
    mut stop: watch::Receiver<bool>,
) -> JoinHandle<()> {
    let app = app.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            tokio::select! {
                _ = stop.changed() => break,
                _ = interval.tick() => if process_completed_meetings(&app).await.is_err() {
                    eprintln!("meeting automation cycle failed");
                }
            }
        }
    })
}
pub(crate) async fn process_completed_meetings(app: &App) -> anyhow::Result<()> {
    let config = app.model_config().await?;
    for job in app.store.pending_meeting_processing().await? {
        let session = app.store.live_meeting(&job.session_id).await?;
        if session.transcript.trim().is_empty() {
            app.store
                .block_meeting_processing(&session.id, "empty", None)
                .await?;
            continue;
        }
        let Some(id) = &session.document_id else {
            continue;
        };
        let doc = app.store.document(id, None, &Access::admin()).await?;
        // The last chunk publishes its document before releasing its pending count.
        if doc.body != session.transcript {
            continue;
        }
        let c = if job.connection == "local" {
            Some(local_connection())
        } else {
            config
                .connections
                .iter()
                .find(|c| c.id == job.connection)
                .cloned()
        };
        let result = async {
            let c = c.ok_or_else(|| {
                anyhow::anyhow!(
                    "The saved meeting model is unavailable. Choose a model in Settings and retry."
                )
            })?;
            let external = thesaurus_core::models::external(&config, &c.id)?;
            let reserved = app
                .store
                .create_managed_meeting_summary_runtime(
                    &session.id,
                    job.generation,
                    doc.revision,
                    &c,
                    external,
                )
                .await?;
            crate::runtime::launch(app, c, reserved).await;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        if let Err(e) = result {
            // Concurrent reservations and occupied runtime slots are queue waits,
            // not provider attempts. Terminal failures never resend a transcript.
            if !e.to_string().starts_with("conflict:") {
                app.store
                    .block_meeting_processing(&session.id, "failed", Some(&e.to_string()))
                    .await?;
            }
        }
    }
    Ok(())
}

pub async fn transcription_catalog(
    State(app): State<App>,
    h: HeaderMap,
) -> Api<TranscriptionCatalog> {
    app.admin(&h).await?;
    Ok(Json(app.store.transcription_catalog().await?))
}
pub async fn transcription_model(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(req): Json<TranscriptionModelAction>,
) -> Api<serde_json::Value> {
    app.admin(&h).await?;
    match req.action.as_str() {
        "download" => {
            if app.store.reserve_transcription_download(&id).await? {
                let store = app.store.clone();
                tokio::spawn(async move {
                    store.download_transcription_model(&id).await;
                });
            }
        }
        "cancel" => app.store.cancel_transcription_download(&id).await?,
        _ => return Err(anyhow::anyhow!("invalid: choose download or cancel").into()),
    }
    Ok(Json(serde_json::json!({"accepted":true})))
}
pub async fn transcription_credential(
    State(app): State<App>,
    h: HeaderMap,
    Json(req): Json<TranscriptionCredentialRequest>,
) -> Api<serde_json::Value> {
    app.admin(&h).await?;
    Ok(Json(
        serde_json::json!({"credential_ref":app.store.save_transcription_credential(req).await?}),
    ))
}
pub async fn revoke_transcription(State(app): State<App>, h: HeaderMap) -> Api<serde_json::Value> {
    app.admin(&h).await?;
    app.store.revoke_transcription_audio().await?;
    Ok(Json(serde_json::json!({"revoked":true})))
}
pub async fn transcription_receipts(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
) -> Api<Vec<TranscriptionReceipt>> {
    app.admin(&h).await?;
    Ok(Json(app.store.transcription_receipts(&id).await?))
}
pub async fn cleanup_transcription(
    State(app): State<App>,
    h: HeaderMap,
    Path((id, chunk)): Path<(String, String)>,
) -> Api<serde_json::Value> {
    app.admin(&h).await?;
    app.store
        .cleanup_transcription_delivery(&id, &chunk)
        .await?;
    Ok(Json(serde_json::json!({"cleaned_up":true})))
}
