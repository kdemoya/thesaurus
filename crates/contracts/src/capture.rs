use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTranscribeRequest {
    pub project_id: String,
    pub title: String,
    pub audio_path: String,
    pub whisper_command: String,
    pub model_path: String,
    pub reasoning_connection: Option<String>,
    pub allow_external: bool,
    pub approve_local_processing: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AudioCapture {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub audio_path: String,
    pub whisper_command: String,
    pub model_path: String,
    pub transcription_destination: String,
    pub reasoning_connection: Option<String>,
    pub reasoning_destination: String,
    pub status: String,
    pub transcript: String,
    pub source_id: Option<String>,
    pub document_id: Option<String>,
    pub reasoning_runs: Vec<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioReasonRequest {
    pub connection: String,
    pub approve_usage: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaptureWindow {
    pub window_id: u32,
    pub process_id: i64,
    pub app: String,
    pub title: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityStartRequest {
    pub project_id: String,
    pub window: CaptureWindow,
    pub interval_seconds: u32,
    pub retention_hours: u32,
    pub max_frames: u32,
    pub use_local_ocr: bool,
    pub approve_capture: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivitySession {
    pub id: String,
    pub version: i64,
    pub project_id: String,
    pub window: CaptureWindow,
    pub state: String,
    pub interval_seconds: u32,
    pub retention_hours: u32,
    pub max_frames: u32,
    pub use_local_ocr: bool,
    pub retained_frames: i64,
    pub retained_bytes: i64,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityControlRequest {
    pub expected_version: i64,
    pub action: String,
    pub approve_capture: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityFrame {
    pub id: String,
    pub session_id: String,
    pub project_id: String,
    pub app: String,
    pub window_title: String,
    pub text: String,
    pub png_bytes: usize,
    pub created_at: String,
}
