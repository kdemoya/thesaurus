use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DatedClaim {
    pub id: String,
    pub project_id: String,
    pub subject: String,
    pub predicate: String,
    pub value: String,
    pub certainty: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub observed_at: String,
    pub version: i64,
    pub status: String,
    pub source_ids: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub contributor: String,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimCreateRequest {
    pub project_id: String,
    pub subject: String,
    pub predicate: String,
    pub value: String,
    pub certainty: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimUpdateRequest {
    pub expected_version: i64,
    pub action: String,
    pub value: Option<String>,
    pub certainty: Option<String>,
    pub valid_from: Option<String>,
    pub valid_until: Option<String>,
    #[serde(default)]
    pub clear_valid_until: bool,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeAnswerRequest {
    pub project_id: String,
    pub question: String,
    pub valid_as_of: Option<String>,
    pub known_by: Option<String>,
    pub source_ids: Option<Vec<String>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnswerPassage {
    pub source_id: String,
    pub source_title: String,
    pub evidence: Evidence,
    pub observed_at: String,
    pub validity: String,
    pub match_terms: Vec<String>,
    pub retrieval_path: String,
    pub retrieval_policy: String,
    pub fetched_at: Option<String>,
    pub stale: bool,
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnswerContradiction {
    pub subject: String,
    pub predicate: String,
    pub values: Vec<String>,
    pub claim_ids: Vec<String>,
    pub explanation: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeAnswer {
    pub project_id: String,
    pub question: String,
    pub valid_as_of: String,
    pub known_by: String,
    pub generated_at: String,
    pub answer: String,
    pub claims: Vec<DatedClaim>,
    pub passages: Vec<AnswerPassage>,
    pub contradictions: Vec<AnswerContradiction>,
    pub missing_evidence: Vec<String>,
    pub source_ids: Vec<String>,
    pub claims_omitted: usize,
    pub passages_omitted: usize,
    pub method: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeFacetChange {
    pub subject: String,
    pub predicate: String,
    pub before: Vec<String>,
    pub today: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeComparison {
    pub requested: KnowledgeAnswer,
    pub today: KnowledgeAnswer,
    pub changes: Vec<KnowledgeFacetChange>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonAlias {
    pub label: String,
    pub certainty: String,
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonRole {
    pub role: String,
    pub scope: String,
    pub valid_from: String,
    pub valid_until: Option<String>,
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthoredWork {
    pub title: String,
    pub topic: String,
    pub relationship: String,
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonRecord {
    pub id: String,
    pub project_id: String,
    pub display_name: String,
    pub identity_note: String,
    pub evidence: Vec<Evidence>,
    pub aliases: Vec<PersonAlias>,
    pub roles: Vec<PersonRole>,
    pub works: Vec<AuthoredWork>,
    pub source_ids: Vec<String>,
    pub version: i64,
    pub status: String,
    pub contributor: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonCreateRequest {
    pub project_id: String,
    pub display_name: String,
    pub identity_note: String,
    pub evidence: Vec<Evidence>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersonUpdateRequest {
    pub expected_version: i64,
    pub action: String,
    pub display_name: Option<String>,
    pub identity_note: Option<String>,
    pub alias: Option<PersonAlias>,
    pub role: Option<PersonRole>,
    pub work: Option<AuthoredWork>,
    pub record_index: Option<usize>,
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeQuery {
    pub query: Option<String>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
    #[serde(default)]
    pub include_archived: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonMatch {
    pub person: PersonRecord,
    pub match_basis: Vec<String>,
    pub uncertain_identity: bool,
    pub score: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeoplePage {
    pub project_id: String,
    pub query: String,
    pub items: Vec<PersonMatch>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
    pub caution: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaimPage {
    pub project_id: String,
    pub items: Vec<DatedClaim>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonHistory {
    pub sequence: i64,
    pub action: String,
    pub note: String,
    pub contributor: String,
    pub created_at: String,
    pub record: PersonRecord,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersonHistoryPage {
    pub items: Vec<PersonHistory>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
