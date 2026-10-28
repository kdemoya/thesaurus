use super::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EvidenceReference {
    pub document_id: String,
    pub revision: i64,
    pub line: i64,
    pub quote: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorityTask {
    pub id: String,
    pub title: String,
    pub owner: Option<String>,
    pub due: Option<String>,
    pub version: i64,
    pub evidence: EvidenceReference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorityPreference {
    pub project_id: String,
    pub task_id: String,
    pub importance: String,
    pub note: String,
    pub version: i64,
    pub updated_at: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorityPreferenceRequest {
    pub expected_version: i64,
    pub importance: String,
    #[serde(default)]
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrioritySignal {
    pub code: String,
    pub label: String,
    pub points: i64,
    pub state: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrioritySavedItem {
    pub task: PriorityTask,
    pub rank: i64,
    pub score: i64,
    pub preference: PriorityPreference,
    pub signals: Vec<PrioritySignal>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrioritySignalChange {
    pub code: String,
    pub before: String,
    pub after: String,
    pub points_delta: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorityInfluence {
    pub task_id: String,
    pub title: String,
    pub detail: String,
    pub evidence: EvidenceReference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorityItem {
    pub current: PrioritySavedItem,
    pub previous_rank: Option<i64>,
    pub previous_score: Option<i64>,
    pub changes: Vec<PrioritySignalChange>,
    pub influences: Vec<PriorityInfluence>,
    pub influences_omitted: usize,
    pub new_since_review: bool,
    pub stale_evidence: bool,
    pub waiting_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorityDeparture {
    pub task: PriorityTask,
    pub previous_rank: i64,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorityReview {
    pub id: String,
    pub project_id: String,
    pub as_of_date: String,
    pub created_at: String,
    pub algorithm_version: u32,
    pub watermark: i64,
    pub sources: Vec<BriefSourceRevision>,
    pub items: Vec<PrioritySavedItem>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorityReviewSummary {
    pub id: String,
    pub as_of_date: String,
    pub created_at: String,
    pub task_count: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PriorityBoard {
    pub project_id: String,
    pub as_of_date: String,
    pub created_at: String,
    pub algorithm_version: u32,
    pub watermark: i64,
    pub signature: String,
    pub baseline: Option<PriorityReviewSummary>,
    pub sources: Vec<BriefSourceRevision>,
    pub items: Vec<PriorityItem>,
    pub departed: Vec<PriorityDeparture>,
    pub total_items: usize,
    pub offset: usize,
    pub limit: usize,
    pub items_omitted: usize,
    pub departed_omitted: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorityReviewRequest {
    pub signature: String,
    pub expected_review_id: Option<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaitingFor {
    pub id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub title: String,
    pub person: String,
    pub source_ids: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub requested_on: String,
    pub follow_up_on: String,
    pub follow_up_interval_days: i64,
    pub status: String,
    pub version: i64,
    pub last_followed_up_at: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub follow_up_due: bool,
    pub stale_evidence: bool,
    pub linked_task_status: Option<String>,
    pub suggested_follow_up: Option<String>,
    pub history: Vec<WaitingEvent>,
    pub history_count: usize,
    pub history_omitted: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaitingEvent {
    pub action: String,
    pub note: String,
    pub evidence: Vec<Evidence>,
    pub person: String,
    pub title: String,
    pub status: String,
    pub follow_up_on: String,
    pub contributor: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitingCreateRequest {
    pub project_id: String,
    pub task_id: Option<String>,
    pub title: String,
    pub person: String,
    pub requested_on: String,
    pub follow_up_on: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitingUpdateRequest {
    pub expected_version: i64,
    pub action: String,
    pub title: Option<String>,
    pub person: Option<String>,
    pub follow_up_on: Option<String>,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaitingQuery {
    pub status: Option<String>,
    #[serde(default)]
    pub due_only: bool,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaitingList {
    pub project_id: String,
    pub as_of_date: String,
    pub watermark: i64,
    pub items: Vec<WaitingFor>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorityQuery {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
