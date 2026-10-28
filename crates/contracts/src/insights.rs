use crate::{BriefSection, BriefSourceRevision, Evidence, Task};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Opportunity {
    pub id: String,
    pub project_id: String,
    pub source_ids: Vec<String>,
    pub version: i64,
    pub title: String,
    pub hypothesis: String,
    pub evidence: Vec<Evidence>,
    pub counterevidence: Vec<Evidence>,
    pub affected_task_ids: Vec<String>,
    pub status: String,
    pub review_note: Option<String>,
    #[serde(default)]
    pub independence_review: Option<OpportunityIndependenceReview>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityIndependenceReview {
    pub contributor: String,
    pub reviewed_at: String,
    pub note: String,
    pub source_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpportunityCreate {
    pub title: String,
    pub hypothesis: String,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub counterevidence: Vec<Evidence>,
    #[serde(default)]
    pub affected_task_ids: Vec<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InsightReview {
    pub expected_version: i64,
    pub decision: String,
    pub note: String,
    #[serde(default)]
    pub attest_independent: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpportunityDiscovery {
    pub topic: String,
    pub candidate: Option<OpportunityCreate>,
    pub explanation: String,
    pub documents_considered: usize,
    pub lines_omitted: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskTransition {
    pub task: Task,
    pub previous_status: Option<String>,
    pub origin: String,
    pub recorded_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeeklyOutcomes {
    pub id: String,
    pub project_id: String,
    pub version: i64,
    pub start_date: String,
    pub end_date: String,
    pub sections: Vec<BriefSection>,
    pub transitions: Vec<TaskTransition>,
    #[serde(default)]
    pub commitment_snapshots: Vec<WeeklyCommitmentSnapshot>,
    pub sources: Vec<BriefSourceRevision>,
    pub notes: Vec<String>,
    pub status: String,
    pub review_note: Option<String>,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WeeklyCommitmentSnapshot {
    pub commitment_id: String,
    pub linked_task_id: Option<String>,
    pub promised_on: String,
    pub as_of: crate::CommitmentEvent,
    pub promise_evidence: Vec<Evidence>,
    pub history: Vec<crate::CommitmentEvent>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeeklyCreate {
    pub start_date: String,
    pub end_date: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskEffort {
    pub project_id: String,
    pub task_id: String,
    pub version: i64,
    pub minutes: u32,
    pub note: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEffortUpdate {
    pub expected_version: i64,
    pub minutes: u32,
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapacityWindow {
    pub starts_at: String,
    pub ends_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarCommitment {
    pub id: String,
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub selected: bool,
    pub origin: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapacityTradeoff {
    pub task_id: String,
    pub action: String,
    pub adjusted_minutes: Option<u32>,
    pub new_due: Option<String>,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapacitySave {
    pub id: Option<String>,
    pub expected_version: i64,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub utc_offset_minutes: i32,
    pub selected_task_ids: Vec<String>,
    pub working_windows: Vec<CapacityWindow>,
    pub calendar: Vec<CalendarCommitment>,
    pub tradeoffs: Vec<CapacityTradeoff>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapacityTask {
    pub task: Task,
    pub estimated_minutes: Option<u32>,
    pub planned_minutes: Option<u32>,
    pub available_before_deadline: Option<i64>,
    pub cumulative_due_minutes: Option<u64>,
    pub deadline_overload: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapacityPlan {
    pub id: String,
    pub project_id: String,
    pub source_ids: Vec<String>,
    pub version: i64,
    pub name: String,
    pub start_date: String,
    pub end_date: String,
    pub utc_offset_minutes: i32,
    pub selected_task_ids: Vec<String>,
    pub working_windows: Vec<CapacityWindow>,
    pub calendar: Vec<CalendarCommitment>,
    pub tradeoffs: Vec<CapacityTradeoff>,
    pub suggested_tradeoffs: Vec<CapacityTradeoff>,
    pub tasks: Vec<CapacityTask>,
    pub working_minutes: i64,
    pub calendar_minutes: i64,
    pub free_minutes: i64,
    pub planned_minutes: u64,
    pub overlapping_calendar_pairs: usize,
    pub notes: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarImport {
    pub ics: String,
    pub utc_offset_minutes: i32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarImportPreview {
    pub events: Vec<CalendarCommitment>,
    pub notes: Vec<String>,
}
