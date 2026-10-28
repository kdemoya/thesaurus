use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpstreamRequest {
    pub expected_version: i64,
    pub project_id: String,
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub endpoint: Option<String>,
    pub token_env: Option<String>,
    pub resource_uri: Option<String>,
    pub read_tool: Option<String>,
    #[serde(default)]
    pub tool_arguments: serde_json::Value,
    pub policy: String,
    pub allow_query_transmission: bool,
    pub allow_agent_live: bool,
    pub approve_upstream: bool,
    pub daily_limit: u32,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpstreamConnection {
    pub id: String,
    pub version: i64,
    pub source_id: String,
    pub project_id: String,
    pub name: String,
    pub transport: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub endpoint: Option<String>,
    pub token_env: Option<String>,
    pub resource_uri: Option<String>,
    pub read_tool: Option<String>,
    pub tool_arguments: serde_json::Value,
    pub policy: String,
    pub allow_query_transmission: bool,
    pub allow_agent_live: bool,
    pub daily_limit: u32,
    pub last_fetched_at: Option<String>,
    pub last_document_id: Option<String>,
    pub last_revision: Option<i64>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FederatedRequest {
    pub query: String,
    pub source_ids: Vec<String>,
    pub approve_upstream: bool,
    pub limit: u32,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrievalResult {
    pub hit: SearchHit,
    pub path: String,
    pub policy: String,
    pub fetched_at: Option<String>,
    pub stale: bool,
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetrievalOutcome {
    pub source_id: String,
    pub policy: String,
    pub path: String,
    pub error: Option<String>,
    pub checkpoint: Option<BriefSourceRevision>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FederatedResponse {
    pub query: String,
    pub results: Vec<RetrievalResult>,
    pub sources: Vec<RetrievalOutcome>,
    pub omissions: Vec<String>,
    pub watermark: i64,
}
