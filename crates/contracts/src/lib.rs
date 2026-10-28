mod onboarding;
pub use onboarding::*;
mod live_meetings;
pub use live_meetings::*;
mod transcription;
pub use transcription::*;
mod actions;
pub use actions::*;
mod retrieval;
pub use retrieval::*;
mod procedures;
pub use procedures::*;
mod capture;
pub use capture::*;
mod insights;
pub use insights::*;
mod knowledge;
pub use knowledge::*;
mod usage_controls;
pub use usage_controls::*;
mod repository;
pub use repository::*;
mod source_controls;
pub use source_controls::*;
mod discovery;
pub use discovery::*;
mod context_delivery;
pub use context_delivery::*;
mod annotations;
pub use annotations::*;
mod memory_review;
pub use memory_review::*;
mod evaluations;
pub use evaluations::*;
mod focus;
pub use focus::*;
mod recipes;
pub use recipes::*;
mod delivery;
pub use delivery::*;
mod continuity;
pub use continuity::*;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub uri: String,
    pub allow_external: bool,
    pub updated_at: String,
    pub document_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Document {
    pub id: String,
    pub source_id: String,
    pub title: String,
    pub body: String,
    pub revision: i64,
    pub content_hash: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Evidence {
    pub document_id: String,
    pub revision: i64,
    pub line: i64,
    pub quote: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub owner: Option<String>,
    pub due: Option<String>,
    pub status: String,
    pub evidence: Evidence,
    pub method: String,
    pub version: i64,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    pub id: String,
    pub body: String,
    pub contributor: String,
    pub source_ids: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Job {
    pub id: String,
    pub document_id: String,
    pub revision: i64,
    pub connection: String,
    pub status: String,
    pub attempt: i64,
    pub summary: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangeEvent {
    pub sequence: i64,
    pub kind: String,
    pub record_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub id: String,
    pub kind: String,
    pub model: String,
    pub billing: String,
    pub available: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub sources: Vec<Source>,
    pub documents: Vec<Document>,
    pub tasks: Vec<Task>,
    pub observations: Vec<Observation>,
    pub jobs: Vec<Job>,
    pub connections: Vec<ConnectionInfo>,
    #[serde(default)]
    pub runtime_runs: Vec<RuntimeRun>,
    #[serde(default)]
    pub briefs: Vec<ProjectBrief>,
    #[serde(default)]
    pub threads: Vec<WorkThread>,
    #[serde(default)]
    pub meeting_series: Vec<MeetingSeries>,
    pub watermark: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectBrief {
    pub id: String,
    pub name: String,
    pub source_ids: Vec<String>,
    pub version: i64,
    pub accepted_revision: Option<i64>,
    pub revisions: Vec<BriefRevision>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BriefRevision {
    pub revision: i64,
    pub run_id: Option<String>,
    pub connection: String,
    pub status: String,
    pub base_revision: Option<i64>,
    pub sections: Vec<BriefSection>,
    pub sources: Vec<BriefSourceRevision>,
    #[serde(default)]
    pub selected_source_ids: Vec<String>,
    #[serde(default)]
    pub retrieval_policies: Vec<DerivedRetrievalPolicy>,
    #[serde(default)]
    pub omitted_source_ids: Vec<String>,
    #[serde(default)]
    pub omissions: Vec<String>,
    pub watermark: i64,
    pub stale: bool,
    pub changed_sources: Vec<String>,
    pub changes: Vec<BriefSectionChange>,
    pub created_at: String,
    pub reviewed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DerivedRetrievalPolicy {
    pub source_id: String,
    pub policy: String,
    pub upstream_version: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BriefSourceRevision {
    pub source_id: String,
    pub document_id: String,
    pub title: String,
    pub revision: i64,
    pub content_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BriefSection {
    pub kind: String,
    pub items: Vec<BriefItem>,
    pub stale: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BriefItem {
    pub text: String,
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BriefSectionChange {
    pub kind: String,
    pub added: Vec<BriefItem>,
    pub removed: Vec<BriefItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefCreateRequest {
    pub name: String,
    pub source_ids: Vec<String>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefGenerateRequest {
    pub connection: String,
    pub expected_version: i64,
    pub approve_usage: bool,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefReviewRequest {
    pub revision: i64,
    pub expected_version: i64,
    pub decision: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchHit {
    pub document_id: String,
    pub source_id: String,
    pub title: String,
    pub excerpt: String,
    pub revision: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    #[serde(default)]
    pub query: String,
    pub source_id: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextBundle {
    pub query: String,
    pub documents: Vec<SearchHit>,
    pub tasks: Vec<Task>,
    pub observations: Vec<Observation>,
    pub watermark: i64,
    pub character_budget: usize,
    pub truncated: bool,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub sources: Vec<BriefSourceRevision>,
    #[serde(default)]
    pub citations: Vec<Evidence>,
    #[serde(default)]
    pub token_budget: usize,
    #[serde(default)]
    pub estimated_tokens: usize,
    #[serde(default)]
    pub omissions: BundleOmissions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    pub source_name: String,
    pub kind: String,
    pub uri: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub allow_external: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub document: Document,
    pub changed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskUpdate {
    pub status: String,
    pub expected_version: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationRequest {
    pub body: String,
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instance {
    pub protocol_version: u32,
    pub pid: u32,
    pub api_url: String,
    pub socket: String,
    pub token: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnalysisRequest {
    pub connection: String,
    #[serde(default)]
    pub approve_usage: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrantInfo {
    pub id: String,
    pub name: String,
    pub source_ids: Vec<String>,
    pub allow_write: bool,
    pub allow_external: bool,
    pub revoked: bool,
}

#[derive(Clone, Debug)]
pub struct Access {
    pub contributor: String,
    pub source_ids: Option<Vec<String>>,
    pub allow_write: bool,
    pub external: bool,
}

impl Access {
    pub fn admin() -> Self {
        Self {
            contributor: "You".into(),
            source_ids: None,
            allow_write: true,
            external: false,
        }
    }
    pub fn permits(&self, source_id: &str, allow_external: bool) -> bool {
        (!self.external || allow_external)
            && self
                .source_ids
                .as_ref()
                .is_none_or(|ids| ids.iter().any(|id| id == source_id))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRequest {
    pub connection: String,
    pub prompt: String,
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub approve_usage: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PermissionOption {
    pub id: String,
    pub label: String,
    pub kind: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimePermission {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub input: String,
    pub options: Vec<PermissionOption>,
    pub decision: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeRun {
    #[serde(default)]
    pub provenance: Option<RuntimeProvenance>,
    pub id: String,
    pub connection: String,
    pub prompt: String,
    pub source_ids: Vec<String>,
    pub status: String,
    pub session_id: Option<String>,
    pub output: String,
    pub progress: String,
    pub stop_reason: Option<String>,
    pub error: Option<String>,
    pub permissions: Vec<RuntimePermission>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeProbe {
    pub protocol_version: u32,
    pub agent_name: String,
    pub agent_version: String,
    pub auth_methods: Vec<String>,
    pub load_session: bool,
    pub close_session: bool,
    pub mcp_stdio: bool,
    pub cancellation: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuntimeProvenance {
    pub agent_name: String,
    pub agent_version: String,
    pub model: String,
    pub model_source: String,
    pub actual_prompt_hash: String,
}
mod plugins;
pub use plugins::*;
mod bragbook;
pub use bragbook::*;
