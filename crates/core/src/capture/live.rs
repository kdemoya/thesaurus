//! Durable live meeting state; the daemon is the sole owner of the recorder.
use super::*;
use chrono::{DateTime, Utc};

pub fn elapsed_ms(s: &LiveMeeting) -> u64 {
    s.recorded_ms
        + s.active_since
            .as_ref()
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .map(|t| {
                (Utc::now() - t.with_timezone(&Utc))
                    .num_milliseconds()
                    .max(0) as u64
            })
            .unwrap_or(0)
}
async fn load(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, id: &str) -> Result<LiveMeeting> {
    let text: String = sqlx::query_scalar("SELECT record FROM live_meetings WHERE id=?")
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .context("record not found")?;
    Ok(serde_json::from_str(&text)?)
}
async fn save(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, s: &LiveMeeting) -> Result<()> {
    sqlx::query("UPDATE live_meetings SET state=?,version=?,record=? WHERE id=?")
        .bind(&s.state)
        .bind(s.version)
        .bind(serde_json::to_string(s)?)
        .bind(&s.id)
        .execute(&mut **tx)
        .await?;
    event(tx, "live-meeting", &s.id).await
}
pub(crate) async fn validate_settings(settings: &LiveRecordingSettings) -> Result<()> {
    if settings.microphone_id.is_none() && !settings.system_audio {
        bail!("invalid: select a microphone or system audio");
    }
    if let Some(id) = &settings.microphone_id {
        validate_text(id, 1000, "microphone")?;
    }
    if !(10..=60).contains(&settings.chunk_seconds) {
        bail!("invalid: chunk duration must be 10–60 seconds");
    }
    if settings
        .transcription
        .as_ref()
        .is_some_and(|s| s.provider.is_remote())
    {
        return Ok(());
    }
    for name in [&settings.whisper_command, &settings.model_path] {
        if !Path::new(name).is_absolute()
            || name.len() > 4096
            || name.contains('\0')
            || !tokio::fs::metadata(name)
                .await
                .map(|m| m.is_file())
                .unwrap_or(false)
        {
            if settings.use_bundled_transcription {
                bail!(
                    "invalid: bundled transcription files are missing. Install the complete Thesaurus app; development builds need scripts/build-transcription.sh. Custom paths are available in advanced audio settings"
                );
            }
            bail!("invalid: choose existing absolute whisper executable/model paths");
        }
    }
    Ok(())
}
impl Store {
    pub async fn live_recording_settings(&self) -> Result<Option<LiveRecordingSettings>> {
        let saved: Option<String> =
            sqlx::query_scalar("SELECT record FROM live_recording_settings WHERE id=1")
                .fetch_optional(&self.pool)
                .await?;
        if let Some(saved) = saved {
            let mut settings = serde_json::from_str(&saved)?;
            super::recording_defaults::resolve_bundled(&mut settings)?;
            return Ok(Some(settings));
        }
        // Existing installations already have an audio setup in their last session.
        let previous =
            self.live_meetings()
                .await?
                .into_iter()
                .next()
                .map(|s| LiveRecordingSettings {
                    transcription: s.transcription,
                    use_bundled_transcription: s.use_bundled_transcription,
                    microphone_id: s.microphone_id,
                    system_audio: s.system_audio,
                    whisper_command: s.whisper_command,
                    model_path: s.model_path,
                    chunk_seconds: s.chunk_seconds,
                });
        match previous {
            Some(mut settings) => {
                super::recording_defaults::resolve_bundled(&mut settings)?;
                Ok(Some(settings))
            }
            None => Ok(Some(self.default_recording_settings().await?)),
        }
    }
    pub async fn save_live_recording_settings(
        &self,
        mut req: LiveRecordingSettingsRequest,
    ) -> Result<LiveRecordingSettings> {
        self.resolve_transcription_settings(&mut req.settings)
            .await?;
        if let Some(s) = &req.settings.transcription {
            if s.provider.is_remote() {
                self.resolve_credential(&s.credential_ref).await?;
            }
        }
        validate_settings(&req.settings).await?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "live-recording-settings",
            "local",
            &req.idempotency_key,
            &req,
        )
        .await?;
        if old.is_none() {
            self.save_audio_grant(&mut tx, &req.settings).await?;
            sqlx::query("INSERT INTO live_recording_settings(id,record) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record")
                .bind(serde_json::to_string(&req.settings)?).execute(&mut *tx).await?;
            remember(&mut tx, &req.idempotency_key, &digest, "local").await?;
            event(&mut tx, "live-recording-settings", "local").await?;
        }
        tx.commit().await?;
        self.live_recording_settings()
            .await?
            .context("Recording settings unavailable")
    }
    pub async fn quick_start_live_recording(
        &self,
        req: LiveRecordingQuickStart,
    ) -> Result<(LiveMeeting, bool)> {
        if !req.approve_recording {
            bail!("forbidden: explicitly start recording");
        }
        let settings = self
            .live_recording_settings()
            .await?
            .context("Recording defaults unavailable")?;
        if let Some(s) = &settings.transcription {
            if s.provider.is_remote() {
                self.require_audio_grant(s).await?;
            }
        }
        let bundled = settings.use_bundled_transcription;
        self.start_live_meeting_configured(
            LiveMeetingStart {
                transcription: settings.transcription,
                project_id: String::new(),
                series_id: None,
                title: String::new(),
                microphone_id: settings.microphone_id,
                system_audio: settings.system_audio,
                whisper_command: settings.whisper_command,
                model_path: settings.model_path,
                chunk_seconds: settings.chunk_seconds,
                approve_recording: true,
                idempotency_key: req.idempotency_key,
            },
            bundled,
            false,
        )
        .await
    }
    pub async fn update_live_meeting(
        &self,
        id: &str,
        req: LiveMeetingUpdate,
    ) -> Result<LiveMeeting> {
        validate_text(&req.title, 200, "recording title")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "live-meeting-details",
            id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        let mut s = load(&mut tx, id).await?;
        if old.is_none() {
            if s.version != req.expected_version {
                bail!("conflict: recording changed; refresh before saving its details");
            }
            // Recorder versions fence the helper. Editing a live version would
            // terminate capture, and pending chunks could still publish old metadata.
            if s.state != "stopped" || s.pending_chunks > 0 || s.processing {
                bail!(
                    "conflict: stop recording and wait for transcription to finish before saving details"
                );
            }
            if !s.project_id.is_empty() && s.project_id != req.project_id {
                bail!("invalid: this recording is already assigned to a project");
            }
            if !req.project_id.is_empty() {
                let mut scope = project_sources(&mut tx, &req.project_id).await?;
                permit(&mut tx, &scope, &Access::admin()).await?;
                if s.source_id.is_none() && !s.transcript.is_empty() && scope.len() >= 50 {
                    bail!("invalid: the selected project already has 50 sources");
                }
                if let Some(source) = &s.source_id {
                    if !scope.contains(source) {
                        scope.push(source.clone());
                        crate::continuity::normalize_sources(&mut scope)?;
                        permit(&mut tx, &scope, &Access::admin()).await?;
                        sqlx::query("UPDATE project_briefs SET source_ids=?,version=version+1,updated_at=? WHERE id=?")
                            .bind(serde_json::to_string(&scope)?).bind(now()).bind(&req.project_id)
                            .execute(&mut *tx).await?;
                        event(&mut tx, "brief", &req.project_id).await?;
                    }
                }
            }
            if let Some(series) = &req.series_id {
                let project: Option<String> =
                    sqlx::query_scalar("SELECT project_id FROM meeting_series WHERE id=?")
                        .bind(series)
                        .fetch_optional(&mut *tx)
                        .await?;
                if req.project_id.is_empty() || project.as_deref() != Some(&req.project_id) {
                    bail!("invalid: meeting series must belong to the selected project");
                }
            }
            s.title = req.title;
            s.project_id = req.project_id;
            s.series_id = req.series_id;
            s.version += 1;
            s.updated_at = now();
            save(&mut tx, &s).await?;
            remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        }
        tx.commit().await?;
        // Re-import through the normal revision/FTS/invalidation path. A retry
        // also repairs publication if this step was interrupted after commit.
        if !s.transcript.is_empty() {
            self.publish_live_transcript(&s).await?;
        }
        self.live_meeting(id).await
    }
    pub async fn live_meetings(&self) -> Result<Vec<LiveMeeting>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT record FROM live_meetings ORDER BY created_at DESC LIMIT 50",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(|r| Ok(serde_json::from_str(r)?)).collect()
    }
    pub async fn live_meeting(&self, id: &str) -> Result<LiveMeeting> {
        let mut tx = self.pool.begin().await?;
        load(&mut tx, id).await
    }
    pub async fn live_meeting_details(&self, id: &str) -> Result<LiveMeetingDetails> {
        let session = self.live_meeting(id).await?;
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT record FROM live_meeting_notes WHERE session_id=? ORDER BY created_at,id",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        Ok(LiveMeetingDetails {
            session,
            notes: rows
                .iter()
                .map(|r| serde_json::from_str(r))
                .collect::<std::result::Result<_, _>>()?,
        })
    }
    pub async fn start_live_meeting(&self, req: LiveMeetingStart) -> Result<(LiveMeeting, bool)> {
        self.start_live_meeting_configured(req, false, true).await
    }
    async fn start_live_meeting_configured(
        &self,
        req: LiveMeetingStart,
        use_bundled_transcription: bool,
        authorize_route: bool,
    ) -> Result<(LiveMeeting, bool)> {
        if !req.approve_recording {
            bail!("forbidden: explicitly approve these audio inputs before recording");
        }
        let original_request = req.clone();
        let mut req = req;
        if req.title.trim().is_empty() {
            req.title = format!("Recording · {}", Utc::now().format("%Y-%m-%d %H:%M UTC"));
        }
        validate_text(&req.title, 200, "recording title")?;
        let mut settings = LiveRecordingSettings {
            transcription: req.transcription.clone(),
            use_bundled_transcription,
            microphone_id: req.microphone_id.clone(),
            system_audio: req.system_audio,
            whisper_command: req.whisper_command.clone(),
            model_path: req.model_path.clone(),
            chunk_seconds: req.chunk_seconds,
        };
        self.resolve_transcription_settings(&mut settings).await?;
        validate_settings(&settings).await?;
        if !settings
            .transcription
            .as_ref()
            .is_some_and(|s| s.provider.is_remote())
        {
            self.native_provider("native.transcription").await?;
        }
        let assets = self.transcription_assets(&settings).await?;
        let use_bundled_transcription = settings.use_bundled_transcription;
        req.whisper_command = settings.whisper_command.clone();
        req.model_path = settings.model_path.clone();
        req.transcription = settings.transcription.clone();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "live-meeting-start",
            "local",
            &req.idempotency_key,
            &original_request,
        )
        .await?;
        if let Some(id) = old {
            let r = load(&mut tx, &id).await?;
            tx.commit().await?;
            return Ok((r, false));
        }
        if !req.project_id.is_empty() {
            let scope = project_sources(&mut tx, &req.project_id).await?;
            permit(&mut tx, &scope, &Access::admin()).await?;
        }
        if let Some(id) = &req.series_id {
            let project: Option<String> =
                sqlx::query_scalar("SELECT project_id FROM meeting_series WHERE id=?")
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?;
            if project.as_deref() != Some(&req.project_id) {
                bail!("invalid: meeting series must belong to the selected project");
            }
        }
        let busy: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM live_meetings WHERE state IN ('starting','recording','pausing','stopping'))",
        )
        .fetch_one(&mut *tx)
        .await?;
        if busy {
            bail!("conflict: pause or stop the current meeting before starting another");
        }
        if authorize_route {
            self.save_audio_grant(&mut tx, &settings).await?;
        }
        let s = LiveMeeting {
            transcription_assets: assets,
            transcription: settings.transcription.clone(),
            detected_language: None,
            use_bundled_transcription,
            id: uuid::Uuid::new_v4().to_string(),
            version: 1,
            project_id: req.project_id,
            series_id: req.series_id,
            title: req.title,
            microphone_id: req.microphone_id,
            system_audio: req.system_audio,
            whisper_command: req.whisper_command,
            model_path: req.model_path,
            chunk_seconds: req.chunk_seconds,
            state: "starting".into(),
            recorded_ms: 0,
            active_since: None,
            transcript: String::new(),
            source_id: None,
            document_id: None,
            chunks_completed: 0,
            processing: false,
            pending_chunks: 0,
            error: None,
            created_at: now(),
            updated_at: now(),
        };
        sqlx::query(
            "INSERT INTO live_meetings(id,version,state,record,created_at) VALUES(?,?,?,?,?)",
        )
        .bind(&s.id)
        .bind(s.version)
        .bind(&s.state)
        .bind(serde_json::to_string(&s)?)
        .bind(&s.created_at)
        .execute(&mut *tx)
        .await?;
        super::processing::queue_new_meeting(&mut tx, &s.id).await?;
        sqlx::query("INSERT INTO live_recording_settings(id,record) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET record=excluded.record")
            .bind(serde_json::to_string(&settings)?).execute(&mut *tx).await?;
        remember(&mut tx, &req.idempotency_key, &digest, &s.id).await?;
        event(&mut tx, "live-meeting", &s.id).await?;
        tx.commit().await?;
        Ok((s, true))
    }
    pub async fn control_live_meeting(
        &self,
        id: &str,
        req: LiveMeetingControl,
    ) -> Result<(LiveMeeting, bool)> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (digest, old) = operation(
            &mut tx,
            "live-meeting-control",
            id,
            &req.idempotency_key,
            &req,
        )
        .await?;
        let mut s = load(&mut tx, id).await?;
        if old.is_some() {
            tx.commit().await?;
            return Ok((s, false));
        }
        if s.version != req.expected_version {
            bail!("conflict: meeting changed; refresh before controlling it");
        }
        match req.action.as_str() {
            "pause" if matches!(s.state.as_str(), "starting" | "recording") => {
                s.recorded_ms = elapsed_ms(&s);
                s.active_since = None;
                s.state = "pausing".into();
            }
            "stop" if s.state != "stopped" => {
                s.recorded_ms = elapsed_ms(&s);
                s.active_since = None;
                s.state = if matches!(s.state.as_str(), "starting" | "recording") {
                    "stopping"
                } else {
                    "stopped"
                }
                .into();
            }
            "resume" if s.state == "paused" => {
                if !req.approve_recording {
                    bail!("forbidden: explicitly resume the selected audio inputs");
                }
                let busy:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM live_meetings WHERE state IN ('starting','recording','pausing','stopping'))").fetch_one(&mut *tx).await?;
                if busy {
                    bail!("conflict: another meeting is already recording");
                }
                if !s.project_id.is_empty() {
                    let scope = project_sources(&mut tx, &s.project_id).await?;
                    permit(&mut tx, &scope, &Access::admin()).await?;
                }
                if s.recorded_ms >= 14_400_000 {
                    bail!("invalid: four-hour meeting limit reached; start a new meeting");
                }
                if let Some(selection) = &s.transcription {
                    if selection.provider.is_remote() {
                        self.require_audio_grant(selection).await?;
                    } else {
                        if !Path::new(&s.model_path).is_file() {
                            bail!("invalid: restore this session's pinned model before resuming");
                        }
                        if selection.use_vad {
                            let path = s
                                .transcription_assets
                                .as_ref()
                                .and_then(|a| a.vad_path.as_deref())
                                .context(
                                    "invalid: this session has no pinned speech detection model",
                                )?;
                            if !Path::new(path).is_file() {
                                bail!(
                                    "invalid: restore this session's pinned speech detection model before resuming"
                                );
                            }
                        }
                    }
                }
                s.state = "starting".into();
                s.error = None;
            }
            _ => bail!("invalid: this control is unavailable in the current meeting state"),
        }
        s.version += 1;
        s.updated_at = now();
        save(&mut tx, &s).await?;
        remember(&mut tx, &req.idempotency_key, &digest, id).await?;
        tx.commit().await?;
        let launch = s.state == "starting";
        Ok((s, launch))
    }
    pub async fn finish_live_control(&self, id: &str, version: i64) -> Result<LiveMeeting> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut s = load(&mut tx, id).await?;
        if s.version == version {
            s.state = match s.state.as_str() {
                "pausing" => "paused",
                "stopping" => "stopped",
                other => other,
            }
            .into();
            s.updated_at = now();
            save(&mut tx, &s).await?;
        }
        tx.commit().await?;
        Ok(s)
    }
    pub async fn live_recording_ready(&self, id: &str, version: i64) -> Result<bool> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut s = load(&mut tx, id).await?;
        if s.version != version || s.state != "starting" {
            return Ok(false);
        }
        s.state = "recording".into();
        s.active_since = Some(now());
        s.updated_at = now();
        save(&mut tx, &s).await?;
        tx.commit().await?;
        Ok(true)
    }
    pub async fn live_recording_error(&self, id: &str, version: i64, error: &str) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut s = load(&mut tx, id).await?;
        if s.version == version + 1
            && matches!(
                s.state.as_str(),
                "paused" | "stopped" | "pausing" | "stopping"
            )
        {
            s.error = Some(error.chars().take(1500).collect());
            s.updated_at = now();
            save(&mut tx, &s).await?;
            tx.commit().await?;
            return Ok(());
        }
        if s.version != version || !matches!(s.state.as_str(), "starting" | "recording") {
            return Ok(());
        }
        s.recorded_ms = elapsed_ms(&s);
        s.active_since = None;
        s.state = "paused".into();
        s.version += 1;
        s.error = Some(error.chars().take(1500).collect());
        s.updated_at = now();
        save(&mut tx, &s).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn live_processing(
        &self,
        id: &str,
        processing: bool,
        error: Option<String>,
    ) -> Result<()> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut s = load(&mut tx, id).await?;
        s.pending_chunks = if processing {
            s.pending_chunks.saturating_add(1)
        } else {
            s.pending_chunks.saturating_sub(1)
        };
        s.processing = s.pending_chunks > 0;
        if error.is_some() {
            s.error = error;
        }
        s.updated_at = now();
        save(&mut tx, &s).await?;
        tx.commit().await?;
        Ok(())
    }
    pub async fn add_live_note(
        &self,
        id: &str,
        req: LiveMeetingNoteRequest,
    ) -> Result<LiveMeetingNote> {
        if !["note", "bookmark"].contains(&req.kind.as_str()) {
            bail!("invalid: choose note or bookmark");
        }
        validate_text(&req.text, 4000, "meeting note")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let s = load(&mut tx, id).await?;
        let (digest, old) =
            operation(&mut tx, "live-meeting-note", id, &req.idempotency_key, &req).await?;
        if let Some(old) = old {
            let text: String =
                sqlx::query_scalar("SELECT record FROM live_meeting_notes WHERE id=?")
                    .bind(old)
                    .fetch_one(&mut *tx)
                    .await?;
            return Ok(serde_json::from_str(&text)?);
        }
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM live_meeting_notes WHERE session_id=?")
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
        if count >= 500 {
            bail!("invalid: meeting note limit reached");
        }
        let note = LiveMeetingNote {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: id.into(),
            kind: req.kind,
            text: req.text,
            offset_ms: elapsed_ms(&s),
            created_at: now(),
        };
        sqlx::query("INSERT INTO live_meeting_notes VALUES(?,?,?,?)")
            .bind(&note.id)
            .bind(id)
            .bind(serde_json::to_string(&note)?)
            .bind(&note.created_at)
            .execute(&mut *tx)
            .await?;
        remember(&mut tx, &req.idempotency_key, &digest, &note.id).await?;
        event(&mut tx, "live-meeting", id).await?;
        tx.commit().await?;
        Ok(note)
    }
    async fn publish_live_transcript(&self, s: &LiveMeeting) -> Result<()> {
        let uri = format!("capture://live-meeting/{}", s.id);
        // A post-recording rename preserves any sharing choice made afterward.
        // Live chunks still force the transcript to remain local-only.
        let allow_external = if s.state == "stopped" && s.pending_chunks == 0 && !s.processing {
            sqlx::query_scalar::<_, bool>("SELECT allow_external FROM sources WHERE uri=?")
                .bind(&uri)
                .fetch_optional(&self.pool)
                .await?
                .unwrap_or(false)
        } else {
            false
        };
        let doc = self
            .import_into_project(
                ImportRequest {
                    source_name: s.title.clone(),
                    kind: "transcript".into(),
                    uri,
                    title: s.title.clone(),
                    body: s.transcript.clone(),
                    allow_external,
                },
                Some(&s.project_id),
            )
            .await?
            .document;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let mut latest = load(&mut tx, &s.id).await?;
        latest.source_id = Some(doc.source_id);
        latest.document_id = Some(doc.id);
        latest.updated_at = now();
        save(&mut tx, &latest).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Called by the daemon's single transcription worker. Audio is never a memory source.
    pub async fn transcribe_live_chunk(
        &self,
        id: &str,
        chunk_id: &str,
        track: &str,
        offset_ms: u64,
        path: &Path,
        dir: &Path,
    ) -> Result<()> {
        let s = self.live_meeting(id).await?;
        let folder = dir.join("live-audio").join("transcribe");
        private_dir(&folder)?;
        let output = folder.join(uuid::Uuid::new_v4().to_string());
        let result = async {
            if tokio::fs::metadata(path).await?.len() > 4_000_000 {
                bail!("invalid: live audio chunk exceeds bound");
            }
            let seen:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM live_meeting_chunks WHERE id=?)").bind(chunk_id).fetch_one(&self.pool).await?;
            if seen {return Ok(());}
            let (text, provenance, detected) = self.transcribe_selected(&s,chunk_id,track,offset_ms,path,&output).await?;
            let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
            let mut current = load(&mut tx, id).await?;
            let seen: bool =
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM live_meeting_chunks WHERE id=?)")
                    .bind(chunk_id)
                    .fetch_one(&mut *tx)
                    .await?;
            if seen {
                return Ok(());
            }
            let line = if text.trim().is_empty() {String::new()}else{format!(
                "[{:02}:{:02} · {}] {}\n",
                offset_ms / 60000,
                (offset_ms / 1000) % 60,
                track,
                text.split_whitespace().collect::<Vec<_>>().join(" ")
            )};
            if current.transcript.len() + line.len() > 190_000 {
                bail!("invalid: transcript limit reached; stop and start a new meeting");
            }
            if !line.is_empty() && current.transcription.as_ref().is_some_and(|t|t.language=="auto") && current.detected_language.is_none() {current.detected_language=detected;}
            current.transcript.push_str(&line);
            current.chunks_completed += 1;
            current.updated_at = now();
            sqlx::query("INSERT INTO live_meeting_chunks VALUES(?,?,?)")
                .bind(chunk_id)
                .bind(id)
                .bind(
                    serde_json::json!({"track":track,"offset_ms":offset_ms,"text":text,"provenance":provenance})
                        .to_string(),
                )
                .execute(&mut *tx)
                .await?;
            save(&mut tx, &current).await?;
            tx.commit().await?;
            if !line.is_empty() {self.publish_live_transcript(&current).await?;}
            Ok::<_, anyhow::Error>(())
        }
        .await;
        let _ = tokio::fs::remove_file(path).await;
        let _ = tokio::fs::remove_file(output.with_extension("txt")).await;
        let _ = tokio::fs::remove_file(output.with_extension("json")).await;
        result
    }
}
