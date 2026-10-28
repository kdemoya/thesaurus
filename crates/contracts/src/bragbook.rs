use super::*;

/// A local accomplishment candidate. Acceptance attests personal contribution;
/// source evidence alone never establishes ownership or measured impact.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BragbookEntry {
    pub id: String,
    pub version: i64,
    pub status: String,
    pub title: String,
    pub contribution: String,
    pub impact: String,
    pub occurred_on: Option<String>,
    pub source_ids: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub origin_kind: String,
    pub origin_id: String,
    pub origin_version: i64,
    pub origin_note: String,
    pub collected_at: String,
    pub updated_at: String,
    pub reviewed_at: Option<String>,
    pub stale_evidence: bool,
    pub support_changed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BragbookReview {
    pub expected_version: i64,
    pub status: String,
    pub title: String,
    pub contribution: String,
    pub impact: String,
    pub occurred_on: Option<String>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BragbookQuery {
    pub status: Option<String>,
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BragbookList {
    pub items: Vec<BragbookEntry>,
    pub total: usize,
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BragbookExportRequest {
    pub entry_ids: Vec<String>,
    pub format: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BragbookCollection {
    pub created: usize,
    pub documents_considered: usize,
    pub skipped_lines: usize,
}
