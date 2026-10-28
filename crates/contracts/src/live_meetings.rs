use super::*;

pub const DEFAULT_MEETING_MICROPHONE: &str = "system-default";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingAudioDevice {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingAudioInputs {
    pub available: bool,
    pub system_audio_available: bool,
    pub microphone_permission: String,
    pub microphones: Vec<MeetingAudioDevice>,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveMeetingStart {
    #[serde(default)]
    pub transcription: Option<TranscriptionSelection>,
    #[serde(default)]
    pub project_id: String,
    pub series_id: Option<String>,
    #[serde(default)]
    pub title: String,
    pub microphone_id: Option<String>,
    pub system_audio: bool,
    pub whisper_command: String,
    pub model_path: String,
    pub chunk_seconds: u32,
    pub approve_recording: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveRecordingSettings {
    #[serde(default)]
    pub transcription: Option<TranscriptionSelection>,
    /// Resolve the packaged engine/model at use time so moving/updating the app
    /// does not leave saved settings pointing into an old bundle.
    #[serde(default)]
    pub use_bundled_transcription: bool,
    pub microphone_id: Option<String>,
    pub system_audio: bool,
    pub whisper_command: String,
    pub model_path: String,
    pub chunk_seconds: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveRecordingSettingsRequest {
    pub settings: LiveRecordingSettings,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveRecordingQuickStart {
    pub approve_recording: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveMeetingUpdate {
    pub expected_version: i64,
    pub title: String,
    #[serde(default)]
    pub project_id: String,
    pub series_id: Option<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveMeeting {
    #[serde(default)]
    pub transcription_assets: Option<TranscriptionAssets>,
    #[serde(default)]
    pub transcription: Option<TranscriptionSelection>,
    /// Auto detection is locked after the first nonempty local result.
    #[serde(default)]
    pub detected_language: Option<String>,
    #[serde(default)]
    pub use_bundled_transcription: bool,
    pub id: String,
    pub version: i64,
    pub project_id: String,
    pub series_id: Option<String>,
    pub title: String,
    pub microphone_id: Option<String>,
    pub system_audio: bool,
    pub whisper_command: String,
    pub model_path: String,
    pub chunk_seconds: u32,
    pub state: String,
    pub recorded_ms: u64,
    pub active_since: Option<String>,
    pub transcript: String,
    pub source_id: Option<String>,
    pub document_id: Option<String>,
    pub chunks_completed: u32,
    pub processing: bool,
    #[serde(default)]
    pub pending_chunks: u32,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveMeetingControl {
    pub expected_version: i64,
    pub action: String,
    pub approve_recording: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveMeetingNote {
    pub id: String,
    pub session_id: String,
    pub kind: String,
    pub text: String,
    pub offset_ms: u64,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveMeetingNoteRequest {
    pub kind: String,
    pub text: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LiveMeetingDetails {
    pub session: LiveMeeting,
    pub notes: Vec<LiveMeetingNote>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingSummaryRequest {
    pub connection: String,
    pub expected_revision: i64,
    pub approve_usage: bool,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingProcessingSettings {
    pub version: i64,
    pub automatic_summaries: bool,
    /// None follows the primary connection selected during onboarding.
    pub connection: Option<String>,
}
impl Default for MeetingProcessingSettings {
    fn default() -> Self {
        Self {
            version: 0,
            automatic_summaries: true,
            connection: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingProcessingSettingsRequest {
    pub settings: MeetingProcessingSettings,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingProcessingState {
    pub session_id: String,
    pub connection: String,
    pub generation: i64,
    pub status: String,
    pub run_id: Option<String>,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingProcessingRequest {
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingSummary {
    /// The summary and its runtime share an identifier.
    pub id: String,
    pub session_id: String,
    pub source_id: String,
    pub document_id: String,
    pub revision: i64,
    pub connection: String,
    pub status: String,
    pub sections: Vec<MeetingSummarySection>,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingSummarySection {
    pub kind: String,
    pub items: Vec<MeetingSummaryItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingSummaryItem {
    pub text: String,
    pub owner: Option<String>,
    pub due: Option<String>,
    pub evidence: Evidence,
    /// Set by the host only after validated action extraction.
    #[serde(default)]
    pub task_id: Option<String>,
}
