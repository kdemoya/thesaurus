use super::*;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptionProvider {
    #[default]
    Bundled,
    LocalNormal,
    LocalLarge,
    Custom,
    Deepgram,
    Soniox,
    Company,
}
impl TranscriptionProvider {
    pub fn is_remote(self) -> bool {
        matches!(self, Self::Deepgram | Self::Soniox | Self::Company)
    }
}
fn auto_language() -> String {
    "auto".into()
}
fn yes() -> bool {
    true
}
fn default_minutes() -> u32 {
    120
}

/// A host-owned route. Secrets never appear in settings, sessions or receipts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionSelection {
    #[serde(default)]
    pub provider: TranscriptionProvider,
    #[serde(default = "auto_language")]
    pub language: String,
    #[serde(default = "yes")]
    pub use_vad: bool,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub credential_ref: String,
    #[serde(default)]
    pub approve_audio_upload: bool,
    /// Aggregate attempted audio across all remote routes, including both tracks.
    #[serde(default = "default_minutes")]
    pub daily_audio_minutes: u32,
}
impl Default for TranscriptionSelection {
    fn default() -> Self {
        Self {
            provider: TranscriptionProvider::Bundled,
            language: auto_language(),
            use_vad: true,
            endpoint: String::new(),
            model: String::new(),
            credential_ref: String::new(),
            approve_audio_upload: false,
            daily_audio_minutes: default_minutes(),
        }
    }
}

/// Artifact identity captured by the host when a session starts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptionAssets {
    pub model_sha256: Option<String>,
    pub vad_path: Option<String>,
    pub vad_sha256: Option<String>,
    pub engine_version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptionModel {
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    pub state: String,
    pub downloaded_bytes: u64,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptionCatalog {
    pub models: Vec<TranscriptionModel>,
    pub engine_available: bool,
    pub hardware: String,
    pub remote_audio_seconds_today: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionModelAction {
    pub action: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranscriptionCredentialRequest {
    #[serde(default)]
    pub endpoint: String,
    pub provider: TranscriptionProvider,
    pub secret: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptionReceipt {
    pub chunk_id: String,
    pub track: String,
    pub offset_ms: u64,
    pub audio_seconds: u64,
    pub provider: TranscriptionProvider,
    pub model: String,
    pub destination: String,
    pub status: String,
    pub provider_request_id: Option<String>,
    pub provider_file_id: Option<String>,
    pub cleanup_pending: bool,
    pub error: Option<String>,
    pub created_at: String,
}
