use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolService {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub language_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryRequest {
    pub expected_version: i64,
    pub project_id: String,
    pub name: String,
    pub path: String,
    #[serde(default = "initial_head")]
    pub initial_ref: String,
    #[serde(default)]
    pub include: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub allow_external: bool,
    pub symbol_service: Option<SymbolService>,
    pub approve_service: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepositoryConnection {
    pub id: String,
    pub version: i64,
    pub project_id: String,
    pub name: String,
    pub path: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub allow_external: bool,
    pub symbol_service: Option<SymbolService>,
    pub checkpoint: String,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryReportRequest {
    pub expected_version: i64,
    pub to_ref: String,
    pub advance_checkpoint: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepositoryCommit {
    pub id: String,
    pub subject: String,
    pub files: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepositoryFile {
    pub status: String,
    pub path: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepositoryTaskMatch {
    pub task_id: String,
    pub title: String,
    pub evidence: Evidence,
    pub matched_paths: Vec<String>,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepositoryReport {
    pub id: String,
    pub repository_id: String,
    pub project_id: String,
    pub source_ids: Vec<String>,
    pub from_commit: String,
    pub to_commit: String,
    pub commits: Vec<RepositoryCommit>,
    pub files: Vec<RepositoryFile>,
    pub task_matches: Vec<RepositoryTaskMatch>,
    pub omissions: Vec<String>,
    pub created_at: String,
    pub checkpoint_advanced: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SymbolQueryRequest {
    pub expected_version: i64,
    pub file: String,
    pub line: u32,
    pub utf16_character: u32,
    pub method: String,
    pub approve_service: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolLocation {
    pub file: String,
    pub line: u32,
    pub utf16_character: u32,
    pub end_line: u32,
    pub end_utf16_character: u32,
    pub excerpt: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SymbolQueryResult {
    pub id: String,
    pub repository_id: String,
    pub project_id: String,
    pub source_ids: Vec<String>,
    pub commit: String,
    pub file: String,
    pub file_hash: String,
    pub line: u32,
    pub utf16_character: u32,
    pub method: String,
    pub locations: Vec<SymbolLocation>,
    pub omitted_locations: usize,
    pub service: SymbolService,
    pub created_at: String,
    pub notes: Vec<String>,
}

fn initial_head() -> String {
    "HEAD".into()
}
