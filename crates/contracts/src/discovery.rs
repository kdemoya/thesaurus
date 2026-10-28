use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuestionCue {
    pub id: String,
    pub project_id: String,
    pub kind: String,
    pub text: String,
    pub status: String,
    pub source_ids: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub original_evidence: Vec<Evidence>,
    pub version: i64,
    pub contributor: String,
    pub first_seen: String,
    pub updated_at: String,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeGap {
    pub id: String,
    pub project_id: String,
    pub question: String,
    pub missing_source: String,
    pub required_source_ids: Vec<String>,
    pub source_ids: Vec<String>,
    pub status: String,
    pub answer: Option<String>,
    pub answer_evidence: Vec<Evidence>,
    pub version: i64,
    pub contributor: String,
    pub created_at: String,
    pub updated_at: String,
    pub sources: Vec<BriefSourceRevision>,
    pub suggestions: Vec<GapSuggestion>,
    pub suggestions_omitted: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GapSuggestion {
    pub id: String,
    pub evidence: Evidence,
    pub source_id: String,
    pub matching_terms: Vec<String>,
    pub status: String,
    pub discovered_at: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryQuery {
    pub status: Option<String>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CuePage {
    pub project_id: String,
    pub muted: bool,
    pub items: Vec<QuestionCue>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub detection: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GapPage {
    pub project_id: String,
    pub items: Vec<KnowledgeGap>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueCreateRequest {
    pub project_id: String,
    pub kind: String,
    pub text: String,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CueUpdateRequest {
    pub expected_version: i64,
    pub status: String,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GapCreateRequest {
    pub project_id: String,
    pub question: String,
    pub missing_source: String,
    pub required_source_ids: Vec<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GapUpdateRequest {
    pub expected_version: i64,
    pub action: String,
    pub question: Option<String>,
    pub missing_source: Option<String>,
    pub required_source_ids: Option<Vec<String>>,
    pub answer: Option<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    pub suggestion_id: Option<String>,
    #[serde(default)]
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryMuteRequest {
    pub muted: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoverySettings {
    pub project_id: String,
    pub muted: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryRefresh {
    pub project_id: String,
    pub revisions_checked: usize,
    pub cues_created: usize,
    pub cues_updated: usize,
    pub suggestions_created: usize,
    pub cue_lines_omitted: usize,
    pub suggestions_omitted: usize,
    pub detection: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryHistoryItem {
    pub sequence: i64,
    pub kind: String,
    pub id: String,
    pub action: String,
    pub contributor: String,
    pub created_at: String,
    pub note: String,
    pub snapshot: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryHistoryPage {
    pub items: Vec<DiscoveryHistoryItem>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
