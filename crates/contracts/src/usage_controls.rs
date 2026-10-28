use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UsagePolicy {
    pub connection: String,
    pub version: i64,
    pub daily_limit: i64,
    pub interactive_reserve: i64,
    pub background_paused: bool,
    pub fallback_connection: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsagePolicyRequest {
    pub expected_version: i64,
    pub daily_limit: i64,
    pub interactive_reserve: i64,
    pub background_paused: bool,
    pub fallback_connection: Option<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectionUsage {
    pub policy: UsagePolicy,
    pub configured_limit: i64,
    pub requests_reserved_today: i64,
    pub interactive_remaining: i64,
    pub background_remaining: i64,
    pub billing: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FallbackPreview {
    pub primary_run_id: String,
    pub primary_connection: String,
    pub fallback_connection: String,
    pub policy_version: i64,
    pub prompt: String,
    pub source_ids: Vec<String>,
    pub approximate_input_tokens: usize,
    pub signature: String,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FallbackRequest {
    pub signature: String,
    pub approve_usage: bool,
    pub idempotency_key: String,
}
