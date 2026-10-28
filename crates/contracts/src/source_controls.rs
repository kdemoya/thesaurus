use crate::EvidenceReference;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourcePolicy {
    pub source_id: String,
    pub version: i64,
    pub external_enabled: bool,
    pub denied_connections: Vec<String>,
    pub denied_grants: Vec<String>,
    pub updated_at: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourcePolicyUpdate {
    pub expected_version: i64,
    pub external_enabled: bool,
    pub denied_connections: Vec<String>,
    pub denied_grants: Vec<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DestinationDisclosure {
    pub destination_id: String,
    pub kind: String,
    pub name: String,
    pub external: bool,
    pub allowed: bool,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceDisclosurePreview {
    pub policy: SourcePolicy,
    pub source_name: String,
    pub source_uri: String,
    pub document_ids: Vec<String>,
    pub destinations: Vec<DestinationDisclosure>,
    pub scope_note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureGap {
    pub started_at: String,
    pub ended_at: Option<String>,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceCoverage {
    pub source_id: String,
    pub version: i64,
    pub expected_interval_hours: Option<u32>,
    pub excluded_content: Vec<String>,
    pub capture_gaps: Vec<CaptureGap>,
    pub updated_at: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceCoverageUpdate {
    pub expected_version: i64,
    pub expected_interval_hours: Option<u32>,
    pub excluded_content: Vec<String>,
    pub capture_gaps: Vec<CaptureGap>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceHealth {
    pub coverage: SourceCoverage,
    pub source_name: String,
    pub last_ingested_at: String,
    pub last_synced_at: Option<String>,
    pub upstream_id: Option<String>,
    pub upstream_error: Option<String>,
    pub retrieval_policy: Option<String>,
    pub document_ids: Vec<String>,
    pub current_documents_processed: i64,
    pub failed_current_jobs: i64,
    pub pending_current_jobs: i64,
    pub stale: bool,
    pub ingestion_mode: String,
    pub effect_on_answers: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkNotification {
    pub id: String,
    pub version: i64,
    pub kind: String,
    pub title: String,
    pub why_now: String,
    pub source_ids: Vec<String>,
    pub record_id: String,
    pub evidence: Vec<EvidenceReference>,
    pub status: String,
    pub triggered_at: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationUpdate {
    pub expected_version: i64,
    pub status: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotificationMute {
    pub id: String,
    pub version: i64,
    pub kind: String,
    pub source_id: Option<String>,
    pub active: bool,
    pub until: Option<String>,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationMuteUpdate {
    pub expected_version: i64,
    pub kind: String,
    pub source_id: Option<String>,
    pub active: bool,
    pub until: Option<String>,
    pub idempotency_key: String,
}
