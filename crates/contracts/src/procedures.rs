use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedureRunReview {
    pub project_id: String,
    pub run_id: String,
    pub procedure: String,
    pub successful: bool,
    pub note: String,
    pub version: i64,
    pub contributor: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcedureReviewRequest {
    pub project_id: String,
    pub run_id: String,
    pub procedure: String,
    pub successful: bool,
    pub expected_version: i64,
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedureSupportingRun {
    pub run_id: String,
    pub connection: String,
    pub source_ids: Vec<String>,
    pub created_at: String,
    pub outcome_review: String,
    pub review_version: i64,
    pub output_excerpt: String,
    pub output_truncated: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedureSuggestion {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub template: String,
    pub source_ids: Vec<String>,
    pub connection: String,
    pub supporting_runs: Vec<ProcedureSupportingRun>,
    pub supporting_runs_omitted: usize,
    pub support_sufficient: bool,
    pub status: String,
    pub version: i64,
    pub recipe_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub explanation: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcedureDecisionRequest {
    pub expected_version: i64,
    pub action: String,
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcedureInstallRequest {
    pub expected_version: i64,
    pub name: String,
    pub template: String,
    pub inputs: Vec<String>,
    pub source_ids: Vec<String>,
    pub connection: String,
    pub output_format: String,
    pub character_budget: usize,
    pub max_runs_per_day: i64,
    pub note: String,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedureInstallation {
    pub suggestion: ProcedureSuggestion,
    pub recipe: WorkRecipe,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcedureQuery {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
    #[serde(default)]
    pub include_dismissed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedurePage {
    pub project_id: String,
    pub items: Vec<ProcedureSuggestion>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedureReviewPage {
    pub items: Vec<ProcedureRunReview>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedureEvent {
    pub sequence: i64,
    pub action: String,
    pub contributor: String,
    pub created_at: String,
    pub note: String,
    pub snapshot: serde_json::Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedureHistoryPage {
    pub items: Vec<ProcedureEvent>,
    pub total: usize,
    pub offset: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcedureRefresh {
    pub project_id: String,
    pub created: usize,
    pub updated: usize,
}
