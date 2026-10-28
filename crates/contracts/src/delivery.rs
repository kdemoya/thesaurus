use super::*;

/// A user reviewed promise and the immutable sources behind it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Commitment {
    pub id: String,
    pub project_id: String,
    pub task_id: Option<String>,
    pub title: String,
    pub owner: String,
    pub promised_on: String,
    pub due: Option<String>,
    pub status: String,
    pub version: i64,
    pub source_ids: Vec<String>,
    pub promise_evidence: Vec<Evidence>,
    pub links: Vec<DeliveryLink>,
    pub completion_proposal: Option<CompletionProposal>,
    pub created_at: String,
    pub updated_at: String,
    pub stale_evidence: bool,
    pub linked_task_status: Option<String>,
    pub history_count: usize,
    pub history: Vec<CommitmentEvent>,
    pub history_omitted: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliveryLink {
    pub id: String,
    /// discussion, implementation, or promise amendment.
    pub kind: String,
    pub note: String,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub repository: Option<DeliveryRepositoryEvidence>,
    pub contributor: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeliveryRepositoryEvidence {
    pub repository_id: String,
    pub report_id: String,
    pub report_hash: String,
    pub report_created_at: String,
    pub from_commit: String,
    pub to_commit: String,
    pub commit_id: String,
    pub commit_subject: String,
    pub paths: Vec<String>,
    pub paths_omitted: usize,
    pub report_omissions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeliveryRepositoryRequest {
    pub expected_version: i64,
    pub report_id: String,
    pub commit_id: String,
    #[serde(default)]
    pub paths: Vec<String>,
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompletionProposal {
    pub link_ids: Vec<String>,
    pub rationale: String,
    pub contributor: String,
    pub proposed_at: String,
    /// Version whose proposal must be explicitly reviewed before completion.
    pub proposed_version: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitmentEvent {
    pub sequence: i64,
    pub action: String,
    pub note: String,
    pub contributor: String,
    pub created_at: String,
    pub version: i64,
    pub title: String,
    pub owner: String,
    pub due: Option<String>,
    pub status: String,
    pub evidence: Vec<Evidence>,
    pub link_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitmentCreateRequest {
    pub project_id: String,
    pub task_id: Option<String>,
    pub title: String,
    pub owner: String,
    pub promised_on: String,
    pub due: Option<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitmentUpdateRequest {
    pub expected_version: i64,
    /// amend, link, propose_completion, confirm_completion, reject_completion, cancel, reopen.
    pub action: String,
    pub title: Option<String>,
    pub owner: Option<String>,
    /// amend can set or explicitly clear the deadline with clear_due.
    pub due: Option<String>,
    #[serde(default)]
    pub clear_due: bool,
    pub kind: Option<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub link_ids: Vec<String>,
    #[serde(default)]
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitmentQuery {
    pub status: Option<String>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitmentList {
    pub project_id: String,
    pub items: Vec<Commitment>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitmentHistory {
    pub commitment_id: String,
    pub items: Vec<CommitmentEvent>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
