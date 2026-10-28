use super::*;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BundleOmissions {
    #[serde(default)]
    pub sources: usize,
    pub documents: usize,
    pub tasks: usize,
    pub observations: usize,
    pub citations: usize,
    pub notes: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextBuildRequest {
    pub query: String,
    pub character_budget: usize,
    pub token_budget: usize,
    #[serde(default)]
    pub source_ids: Option<Vec<String>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandoffReceipt {
    pub thread_id: String,
    pub context_id: String,
    pub context_revision: String,
    pub context_watermark: i64,
    pub run_id: String,
    pub runtime_status: String,
    pub session_id: Option<String>,
    pub state: String,
    pub version: i64,
    pub delivered_at: String,
    pub history: Vec<HandoffEvent>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandoffEvent {
    pub action: String,
    pub contributor: String,
    pub note: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandoffUpdateRequest {
    pub expected_version: i64,
    pub action: String,
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeObservation {
    pub run: RuntimeRun,
    pub cursor: String,
    pub changed: bool,
    pub watermark: i64,
    pub observed_at: String,
}
