use super::*;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OnboardingProgress {
    pub version: i64,
    pub goal: String,
    pub step: u8,
    pub primary_connection: String,
    pub status: String,
    pub memory_grant_id: Option<String>,
    pub updated_at: Option<String>,
}
impl Default for OnboardingProgress {
    fn default() -> Self {
        Self {
            version: 0,
            goal: "context".into(),
            step: 0,
            primary_connection: "local".into(),
            status: "new".into(),
            memory_grant_id: None,
            updated_at: None,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OnboardingSave {
    pub expected_version: i64,
    pub goal: String,
    pub step: u8,
    pub primary_connection: String,
    pub status: String,
    pub memory_grant_id: Option<String>,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetupReadiness {
    pub capability: String,
    pub state: String,
    pub detail: String,
    pub checked_at: String,
    pub repair_step: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectedAgent {
    pub name: String,
    pub kind: String,
    pub command: String,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetupConnection {
    pub connection: ConnectionInfo,
    pub destination: String,
    pub credential_state: String,
    pub check: Option<RuntimeRun>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OnboardingStatus {
    pub progress: OnboardingProgress,
    pub existing_workspace: bool,
    pub connections: Vec<SetupConnection>,
    pub detected_agents: Vec<DetectedAgent>,
    pub readiness: Vec<SetupReadiness>,
    pub plugin_setup: Vec<PluginSetupStatus>,
}
// Deliberately no Debug/Serialize: the transient API key never enters a receipt.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupConnectionRequest {
    pub id: String,
    pub kind: String,
    pub model: String,
    pub endpoint: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    pub credential_ref: Option<String>,
    pub api_key: Option<String>,
    pub daily_limit: i64,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupCheckRequest {
    pub connection: String,
    pub approve_usage: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetupMemoryRequest {
    pub name: String,
    pub source_ids: Vec<String>,
    pub local_agent: bool,
    pub approve_access: bool,
    pub idempotency_key: String,
}
