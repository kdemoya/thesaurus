use crate::Observation;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryRevision {
    pub version: i64,
    pub body: String,
    pub status: String,
    pub action: String,
    pub reason: String,
    pub contributor: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryInspection {
    pub original: Observation,
    pub current: MemoryRevision,
    pub history: Vec<MemoryRevision>,
    pub history_count: i64,
    pub stale_evidence: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryReviseRequest {
    pub expected_version: i64,
    /// correct, suppress, or restore. Restore creates a new revision from an old one.
    pub action: String,
    pub body: Option<String>,
    pub restore_version: Option<i64>,
    pub reason: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryClaim {
    pub observation_id: String,
    pub version: i64,
    pub body: String,
    pub contributed_at: String,
    pub contributor: String,
    pub source_ids: Vec<String>,
    pub evidence: Vec<crate::Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryConflict {
    pub id: String,
    pub version: i64,
    pub claims: Vec<MemoryClaim>,
    pub source_ids: Vec<String>,
    pub reason: String,
    pub status: String,
    pub decision: Option<String>,
    pub resolution_note: Option<String>,
    pub resolved_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryConflictCreate {
    pub observation_ids: Vec<String>,
    pub reason: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryConflictResolve {
    pub expected_version: i64,
    /// keep_both, keep:<observation ID>, suppress_all, or reopen.
    pub decision: String,
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryConflictHistory {
    pub records: Vec<MemoryConflict>,
    pub total: i64,
    pub offset: u32,
}
