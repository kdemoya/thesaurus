use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionPreview {
    pub id: String,
    pub provider: String,
    pub target_id: String,
    pub before: Value,
    pub proposed: Value,
    pub fingerprint: String,
    pub supports_undo: bool,
    pub explanation: String,
    pub created_at: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<PluginActionOrigin>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginActionOrigin {
    pub plugin_id: String,
    pub installed_at: String,
    pub configuration_version: i64,
    pub run_id: String,
    pub contribution_id: String,
    pub rationale: String,
    pub evidence: Vec<crate::Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionPreviewRequest {
    pub provider: String,
    pub target_id: String,
    pub expected_version: Option<i64>,
    pub status: Option<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionExecuteRequest {
    pub fingerprint: String,
    pub approve_effects: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionUndoRequest {
    pub expected_receipt_version: i64,
    pub approve_effects: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionReceipt {
    pub id: String,
    pub preview_id: String,
    pub provider: String,
    pub target_id: String,
    pub version: i64,
    pub before: Value,
    pub after: Value,
    pub supports_undo: bool,
    pub status: String,
    pub executed_at: String,
    pub undone_at: Option<String>,
    pub undo_effect: Option<Value>,
    pub contributor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<PluginActionOrigin>,
}
