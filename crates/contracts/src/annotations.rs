use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnchoredNote {
    pub id: String,
    pub source_id: String,
    pub evidence: Evidence,
    pub body: String,
    pub emphasis: bool,
    pub status: String,
    pub version: i64,
    pub contributor: String,
    pub created_at: String,
    pub updated_at: String,
    pub observation_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteHistory {
    pub id: i64,
    pub note_id: String,
    pub action: String,
    pub record: AnchoredNote,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteCreateRequest {
    pub evidence: Evidence,
    pub body: String,
    #[serde(default)]
    pub emphasis: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteUpdateRequest {
    pub expected_version: i64,
    pub body: String,
    pub emphasis: bool,
    pub status: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnnotationQuery {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
    pub revision: Option<i64>,
    #[serde(default)]
    pub include_archived: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotePage {
    pub document_id: String,
    pub items: Vec<AnchoredNote>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NoteHistoryPage {
    pub note_id: String,
    pub items: Vec<NoteHistory>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CorrectionRule {
    pub kind: String,
    pub from: String,
    pub to: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrectionSet {
    pub source_id: String,
    pub version: i64,
    pub rules: Vec<CorrectionRule>,
    pub contributor: String,
    pub updated_at: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionsRequest {
    pub expected_version: i64,
    pub rules: Vec<CorrectionRule>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrectionImpact {
    pub task: Task,
    pub corrected_title: String,
    pub corrected_owner: Option<String>,
    pub actionable: bool,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrectionPreview {
    pub source_id: String,
    pub signature: String,
    pub corrections_version: i64,
    pub items: Vec<CorrectionImpact>,
    pub actionable_count: usize,
    pub protected_count: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorrectionApplyRequest {
    pub signature: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrectionResult {
    pub source_id: String,
    pub applied: usize,
    pub protected: usize,
    pub receipt_id: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrectionAudit {
    pub id: i64,
    pub source_id: String,
    pub kind: String,
    pub contributor: String,
    pub created_at: String,
    pub detail: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrectionAuditPage {
    pub items: Vec<CorrectionAudit>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
