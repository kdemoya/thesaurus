use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkThread {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub source_ids: Vec<String>,
    pub status: String,
    pub merged_into: Option<String>,
    pub version: i64,
    pub entries: Vec<ThreadEntry>,
    #[serde(default)]
    pub omitted_entries: usize,
    #[serde(default)]
    pub omitted_history: usize,
    pub history: Vec<ThreadHistory>,
    pub runs: Vec<ThreadRun>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadEntry {
    pub id: String,
    pub kind: String,
    pub body: String,
    pub record_id: Option<String>,
    pub evidence: Vec<Evidence>,
    pub contributor: String,
    pub truncated: bool,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadHistory {
    pub action: String,
    pub related_id: Option<String>,
    pub entry_ids: Vec<String>,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadRun {
    pub context_id: String,
    pub run_id: String,
    pub connection: String,
    pub status: String,
    pub session_id: Option<String>,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadCreateRequest {
    pub project_id: String,
    pub title: String,
    pub source_ids: Vec<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadUpdateRequest {
    pub expected_version: i64,
    pub title: String,
    pub status: String,
    pub source_ids: Vec<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadEntryRequest {
    pub expected_version: i64,
    pub kind: String,
    #[serde(default)]
    pub body: String,
    pub record_id: Option<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadSplitRequest {
    pub expected_version: i64,
    pub title: String,
    pub entry_ids: Vec<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadMergeRequest {
    pub expected_version: i64,
    pub other_id: String,
    pub other_expected_version: i64,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadContextRequest {
    pub byte_budget: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScopedTask {
    pub task: Task,
    pub stale_evidence: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContextOmissions {
    pub entries: usize,
    pub tasks: usize,
    pub observations: usize,
    pub brief_items: usize,
    pub source_lines: usize,
    pub notes: Vec<String>,
    #[serde(default)]
    pub waiting_for: usize,
    #[serde(default)]
    pub priority_preferences: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewedBrief {
    pub revision: i64,
    pub stale: bool,
    pub sections: Vec<BriefSection>,
    #[serde(default)]
    pub omitted_source_ids: Vec<String>,
    #[serde(default)]
    pub omissions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThreadContext {
    pub id: String,
    pub thread_id: String,
    pub thread_version: i64,
    pub title: String,
    pub project_id: String,
    pub sources: Vec<BriefSourceRevision>,
    #[serde(default)]
    pub selected_source_ids: Vec<String>,
    #[serde(default)]
    pub retrieval_policies: Vec<DerivedRetrievalPolicy>,
    #[serde(default)]
    pub omitted_source_ids: Vec<String>,
    pub entries: Vec<ThreadEntry>,
    pub tasks: Vec<ScopedTask>,
    pub observations: Vec<Observation>,
    #[serde(default)]
    pub waiting_for: Vec<WaitingFor>,
    #[serde(default)]
    pub priority_preferences: Vec<PriorityPreference>,
    pub brief: Option<ReviewedBrief>,
    pub source_excerpts: Vec<Evidence>,
    pub omissions: ContextOmissions,
    pub byte_budget: usize,
    pub watermark: i64,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThreadResumeRequest {
    pub context_id: String,
    pub expected_version: i64,
    pub connection: String,
    pub approve_usage: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectSourcesRequest {
    pub expected_version: i64,
    pub source_ids: Vec<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingSeries {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub cadence: String,
    pub version: i64,
    pub occurrences: Vec<MeetingOccurrence>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStamp {
    pub id: String,
    pub version: i64,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingOccurrence {
    pub id: String,
    pub source_id: String,
    pub document_id: String,
    pub revision: i64,
    pub title: String,
    pub held_on: String,
    pub status: String,
    pub sources: Vec<BriefSourceRevision>,
    pub tasks: Vec<TaskStamp>,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingCreateRequest {
    pub project_id: String,
    pub name: String,
    #[serde(default)]
    pub cadence: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingLinkRequest {
    pub expected_version: i64,
    pub source_id: String,
    pub revision: i64,
    pub held_on: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeetingOccurrenceUpdate {
    pub expected_version: i64,
    pub status: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingCommitment {
    pub task: Task,
    pub stale_evidence: bool,
    pub meeting_id: Option<String>,
    pub change: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceChange {
    pub source_id: String,
    pub document_id: String,
    pub title: String,
    pub from_revision: Option<i64>,
    pub to_revision: Option<i64>,
    pub excerpts: Vec<Evidence>,
    pub excerpts_omitted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeetingPreparation {
    pub series_id: String,
    pub project_id: String,
    pub name: String,
    pub baseline: Option<MeetingOccurrence>,
    pub baseline_transcript_changed: bool,
    pub commitments: Vec<MeetingCommitment>,
    #[serde(default)]
    pub waiting_for: Vec<WaitingFor>,
    pub task_changes: Vec<MeetingCommitment>,
    pub source_changes: Vec<SourceChange>,
    pub brief: Option<ReviewedBrief>,
    pub sources: Vec<BriefSourceRevision>,
    pub omissions: Vec<String>,
    pub watermark: i64,
    pub created_at: String,
}
