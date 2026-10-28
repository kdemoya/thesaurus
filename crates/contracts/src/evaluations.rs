use super::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    pub id: String,
    pub name: String,
    /// retrieval, memory, brief, priorities, or supplied.
    pub target: String,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub expected_contains: Vec<String>,
    #[serde(default)]
    pub forbidden_contains: Vec<String>,
    #[serde(default)]
    pub required_evidence: Vec<Evidence>,
    #[serde(default)]
    pub rubric: String,
    #[serde(default)]
    pub replay_prompt: String,
    #[serde(default)]
    pub prompt_version: String,
    pub threshold: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalSuiteRequest {
    pub expected_version: i64,
    pub name: String,
    pub project_id: Option<String>,
    pub cases: Vec<EvalCase>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalSuite {
    #[serde(default)]
    pub source_ids: Vec<String>,
    pub id: String,
    pub version: i64,
    pub name: String,
    pub project_id: Option<String>,
    pub cases: Vec<EvalCase>,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct EvalOutput {
    pub case_id: String,
    #[serde(default)]
    pub runtime_id: Option<String>,
    #[serde(default)]
    pub agent_version: String,
    #[serde(default)]
    pub prompt_hash: String,
    #[serde(default)]
    pub provenance_kind: String,
    #[serde(default)]
    pub usage_status: String,
    pub text: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default)]
    pub agent: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub prompt_version: String,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cost_microunits: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalRunRequest {
    #[serde(default)]
    pub max_judge_requests: u32,
    #[serde(default)]
    pub max_judge_input_tokens: usize,
    pub suite_version: i64,
    pub project_id: String,
    pub baseline_id: Option<String>,
    #[serde(default)]
    pub outputs: Vec<EvalOutput>,
    pub max_cases: usize,
    pub max_output_bytes: usize,
    pub max_cost_microunits: u64,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalPreview {
    pub max_judge_requests: u32,
    pub max_judge_input_tokens: usize,
    pub suite_id: String,
    pub suite_version: i64,
    pub project_id: String,
    pub cases: usize,
    pub source_ids: Vec<String>,
    pub source_bytes: usize,
    pub submitted_cost_microunits: u64,
    pub model_calls: usize,
    pub signature: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalCaseResult {
    pub case_id: String,
    pub name: String,
    pub target: String,
    pub threshold: f64,
    pub score: f64,
    pub passed: bool,
    pub checks: Vec<String>,
    pub output: EvalOutput,
    pub stale_evidence: usize,
    pub duplicate_evidence: usize,
    pub invalid_evidence: usize,
    pub baseline_score: Option<f64>,
    pub regression: bool,
    pub comparison: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalRun {
    /// Immutable addon receipts loaded separately from the original run record.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub plugin_checks: Vec<PluginEvaluationReceipt>,
    #[serde(default)]
    pub max_judge_requests: u32,
    #[serde(default)]
    pub max_judge_input_tokens: usize,
    pub id: String,
    pub suite: EvalSuite,
    pub project_id: String,
    pub source_ids: Vec<String>,
    pub source_snapshots: Vec<Document>,
    pub created_at: String,
    pub runner_version: String,
    pub baseline_id: Option<String>,
    pub baseline_removed_cases: Vec<String>,
    pub results: Vec<EvalCaseResult>,
    pub cost_microunits: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub duration_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalRunSummary {
    pub id: String,
    pub suite_id: String,
    pub suite_version: i64,
    pub project_id: String,
    pub created_at: String,
    pub cases: usize,
    pub passed: usize,
    pub regressions: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalReviewRequest {
    pub case_id: String,
    pub score: f64,
    pub note: String,
    pub judge_runtime_id: Option<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalReview {
    pub id: String,
    pub run_id: String,
    pub case_id: String,
    pub score: f64,
    pub note: String,
    pub contributor: String,
    pub created_at: String,
    pub judge_runtime_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalJudgePreview {
    pub max_judge_requests: u32,
    pub used_judge_requests: i64,
    pub remaining_input_tokens: usize,
    pub run_id: String,
    pub case_id: String,
    pub prompt: String,
    pub prompt_hash: String,
    pub source_ids: Vec<String>,
    pub approximate_input_tokens: usize,
    pub usage_note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalJudgeRequest {
    pub connection: String,
    pub max_approximate_input_tokens: usize,
    pub approve_usage: bool,
    pub idempotency_key: String,
}

/// A saved replay plan never starts a provider request. Each case is separately approved.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalReplayPlanRequest {
    pub suite_version: i64,
    pub case_ids: Vec<String>,
    pub connection: String,
    pub max_requests: u32,
    pub max_input_tokens: usize,
    pub max_output_bytes: usize,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalReplayCase {
    pub case_id: String,
    pub prompt_version: String,
    pub prompt: String,
    pub prompt_hash: String,
    pub approximate_input_tokens: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvalReplayPlan {
    pub id: String,
    pub parent_run_id: String,
    pub suite: EvalSuite,
    pub project_id: String,
    pub source_ids: Vec<String>,
    pub source_snapshots: Vec<Document>,
    pub connection: String,
    pub connection_config_hash: String,
    pub configured_model: String,
    pub cases: Vec<EvalReplayCase>,
    pub max_requests: u32,
    pub max_input_tokens: usize,
    pub max_output_bytes: usize,
    pub used_requests: i64,
    pub remaining_input_tokens: usize,
    pub created_at: String,
    pub usage_note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalReplayStartRequest {
    pub prompt_hash: String,
    pub max_approximate_input_tokens: usize,
    pub approve_usage: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalReplayFinalizeRequest {
    pub runtime_id: String,
    pub review_note: String,
    pub idempotency_key: String,
}
