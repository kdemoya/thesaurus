//! Packaged offline transcription assets; no download or capture on startup.
use super::*;

pub(super) fn bundled_paths() -> Result<(String, String)> {
    let folder = std::env::current_exe()?
        .parent()
        .context("invalid service executable path")?
        .join("transcription");
    Ok((
        folder
            .join("whisper-cli")
            .to_str()
            .context("invalid bundled transcription path")?
            .into(),
        folder
            .join("ggml-base.bin")
            .to_str()
            .context("invalid bundled model path")?
            .into(),
    ))
}
pub(super) fn resolve_bundled(settings: &mut LiveRecordingSettings) -> Result<()> {
    if settings.use_bundled_transcription {
        (settings.whisper_command, settings.model_path) = bundled_paths()?;
    }
    Ok(())
}

impl Store {
    pub(super) async fn default_recording_settings(&self) -> Result<LiveRecordingSettings> {
        // Discovery is passive. OS permission prompts remain owned by the
        // selected audio provider and occur only after an explicit Start.
        let system_audio = self
            .native_invoke("native.audio", "inputs", serde_json::json!({}))
            .await
            .ok()
            .and_then(|v| serde_json::from_value::<MeetingAudioInputs>(v).ok())
            .is_some_and(|i| i.available && i.system_audio_available);
        let mut settings = LiveRecordingSettings {
            transcription: None,
            use_bundled_transcription: true,
            microphone_id: Some(DEFAULT_MEETING_MICROPHONE.into()),
            system_audio,
            whisper_command: String::new(),
            model_path: String::new(),
            chunk_seconds: 20,
        };
        resolve_bundled(&mut settings)?;
        Ok(settings)
    }
}
