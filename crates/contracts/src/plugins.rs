//! Versioned contracts shared by service, desktop and subprocess plugin hosts.
use super::*;
use serde_json::Value;

pub const PLUGIN_API_MIN_VERSION: u32 = 1;
pub const PLUGIN_API_VERSION: u32 = 2;
pub const PLUGIN_EVENT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginHost {
    Service,
    Desktop,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PluginPermissions {
    #[serde(default)]
    pub operations: Vec<String>,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub project_ids: Vec<String>,
    #[serde(default)]
    pub connection_ids: Vec<String>,
    #[serde(default)]
    pub external: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PluginCapability {
    pub id: String,
    pub commands: Vec<String>,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginFieldKind {
    String,
    Boolean,
    Integer,
    Choice,
    CredentialReference,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PluginConfigField {
    pub key: String,
    pub label: String,
    pub kind: PluginFieldKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub choices: Vec<String>,
    #[serde(default)]
    pub default: Option<Value>,
    #[serde(default)]
    pub description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginContributionKind {
    Connector,
    Processor,
    Command,
    Action,
    Evaluation,
    Panel,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PluginContribution {
    pub id: String,
    pub kind: PluginContributionKind,
    pub title: String,
    pub description: String,
}

/// Declarative host-rendered setup; never arbitrary plugin HTML or scripts.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PluginSetupStep {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub applies_to: Vec<String>,
    #[serde(default)]
    pub prerequisites: Vec<PluginPrerequisite>,
    /// Explicit API 2 `readiness` invocation, or native `setup_check` command.
    #[serde(default)]
    pub check: bool,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub repair: Option<PluginSetupRepair>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginPrerequisite {
    Configuration { key: String },
    Operation { operation: String },
    Dependency { plugin_id: String },
    Capability { capability: String },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PluginSetupRepair {
    Configuration { key: String },
    Grants,
    Provider { capability: String },
    Feature { feature: PluginSetupFeature },
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginSetupFeature {
    Audio,
    Capture,
    Notifications,
    Models,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginSetupState {
    Ready,
    NeedsSetup,
    Checking,
    Unavailable,
    Unverified,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginReadinessResult {
    pub step_id: String,
    pub state: PluginSetupState,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginSetupStatus {
    pub plugin_id: String,
    pub configuration_version: i64,
    pub step: PluginSetupStep,
    pub state: PluginSetupState,
    pub detail: String,
    pub checked_at: Option<String>,
    pub check_run_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSetupCheckRequest {
    pub expected_version: i64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub api_min: u32,
    pub api_max: u32,
    pub host: PluginHost,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub hooks: Vec<String>,
    #[serde(default)]
    pub configuration: Vec<PluginConfigField>,
    #[serde(default)]
    pub contributions: Vec<PluginContribution>,
    #[serde(default)]
    pub requests: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub setup: Vec<PluginSetupStep>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PluginExecutable {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginInstallRequest {
    pub manifest: PluginManifest,
    pub executable: PluginExecutable,
    pub grant: PluginPermissions,
    #[serde(default = "plugin_empty_object")]
    pub configuration: Value,
    pub idempotency_key: String,
}
pub fn plugin_empty_object() -> Value {
    serde_json::json!({})
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginRecord {
    pub id: String,
    pub version: i64,
    pub manifest: PluginManifest,
    pub executable: Option<PluginExecutable>,
    pub builtin: bool,
    pub enabled: bool,
    pub grant: PluginPermissions,
    pub configuration: Value,
    pub installed_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginUpdateRequest {
    pub expected_version: i64,
    pub enabled: bool,
    pub grant: PluginPermissions,
    pub configuration: Value,
    #[serde(default)]
    pub manifest: Option<PluginManifest>,
    #[serde(default)]
    pub executable: Option<PluginExecutable>,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginUninstallRequest {
    pub expected_version: i64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEvent {
    pub id: String,
    pub sequence: i64,
    pub schema_version: u32,
    pub kind: String,
    pub record_kind: String,
    pub record_id: String,
    pub record_revision: i64,
    pub source_ids: Vec<String>,
    pub project_ids: Vec<String>,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub origin_plugin: Option<String>,
    pub depth: u32,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginDelivery {
    pub id: String,
    pub plugin_id: String,
    pub event_id: String,
    pub event_sequence: i64,
    pub status: String,
    pub attempts: u32,
    pub next_attempt_at: String,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginDeliveryAttempt {
    pub id: String,
    pub delivery_id: String,
    pub plugin_id: String,
    pub configuration_version: i64,
    pub package_version: String,
    pub api_version: u32,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub error: Option<String>,
    pub configuration_hash: Option<String>,
    pub executable_definition_hash: Option<String>,
    pub grant: Option<PluginPermissions>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginInvocation {
    pub api_version: u32,
    pub invocation_id: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub method: String,
    pub configuration: Value,
    pub event: Option<PluginEvent>,
    /// Only policy-filtered, bounded host data. No admin token or credential.
    pub input: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginObservationProposal {
    pub body: String,
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginResponse {
    pub api_version: u32,
    pub invocation_id: String,
    #[serde(default)]
    pub result: Value,
    #[serde(default)]
    pub observations: Vec<PluginObservationProposal>,
    /// API 2 contribution output. Text is always rendered as text, never HTML.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contribution: Option<PluginContributionOutput>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub snapshots: Vec<PluginSourceSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<PluginActionProposal>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<PluginEvaluationCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_requests: Vec<PluginModelRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub readiness: Option<PluginReadinessResult>,
    /// API 2 native requests require exact per-command grants. They enter the
    /// host broker; accepting a request does not imply its OS effect completed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub native_commands: Vec<PluginNativeProposal>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginNativeProposal {
    pub key: String,
    pub capability: String,
    pub command: String,
    #[serde(default)]
    pub input: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginNativeReceipt {
    pub id: String,
    pub plugin_id: String,
    pub origin_kind: String,
    pub origin_id: String,
    pub configuration_version: i64,
    pub package_version: String,
    pub grant: PluginPermissions,
    pub source_ids: Vec<String>,
    pub project_ids: Vec<String>,
    pub capability: String,
    pub command: String,
    pub status: String,
    pub created_at: String,
    pub result: Option<Value>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginModelRequest {
    pub key: String,
    pub prompt: String,
    /// Requested from API providers; ACP has no generic enforceable token cap.
    pub max_output_tokens: u32,
    pub max_output_bytes: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginModelProposal {
    pub id: String,
    pub plugin_id: String,
    pub plugin_run_id: String,
    pub request: PluginModelRequest,
    pub status: String,
    pub created_at: String,
    pub runtime_id: Option<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub approved_signature: Option<String>,
    #[serde(default)]
    pub approved_external: Option<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginModelPreview {
    pub proposal: PluginModelProposal,
    pub connection: String,
    pub connection_kind: String,
    pub model: String,
    pub external: bool,
    pub prompt: String,
    pub source_ids: Vec<String>,
    pub approximate_input_tokens: usize,
    pub signature: String,
    pub note: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginModelApproveRequest {
    pub connection: String,
    pub signature: String,
    pub approve_usage: bool,
    pub approve_result_delivery: bool,
    pub max_approximate_input_tokens: usize,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginModelResult {
    pub proposal_id: String,
    pub key: String,
    pub runtime_id: String,
    pub connection: String,
    pub output: String,
    pub provenance: Option<RuntimeProvenance>,
    pub usage_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginEvaluationTarget {
    pub run_id: String,
    /// Empty selects all cases from the saved run.
    #[serde(default)]
    pub case_ids: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEvaluationInput {
    pub run_id: String,
    pub signature: String,
    pub suite_id: String,
    pub suite_version: i64,
    pub project_id: String,
    pub cases: Vec<PluginEvaluationCaseInput>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEvaluationCaseInput {
    pub definition: EvalCase,
    pub result: EvalCaseResult,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEvaluationInputSummary {
    pub run_id: String,
    pub suite_name: String,
    pub suite_version: i64,
    pub project_id: String,
    pub created_at: String,
    pub source_count: usize,
    pub cases: Vec<PluginEvaluationCaseSummary>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEvaluationCaseSummary {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginEvaluationCheck {
    pub case_id: String,
    pub check_id: String,
    pub title: String,
    pub score: f64,
    pub detail: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEvaluationCheckResult {
    pub check: PluginEvaluationCheck,
    pub threshold: f64,
    pub passed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginEvaluationReceipt {
    pub id: String,
    pub plugin_run_id: String,
    pub plugin_id: String,
    pub installed_at: String,
    pub configuration_version: i64,
    pub package_version: String,
    pub contribution_id: String,
    pub evaluation_run_id: String,
    pub evaluation_signature: String,
    pub configuration_hash: String,
    pub executable_definition_hash: String,
    pub created_at: String,
    pub results: Vec<PluginEvaluationCheckResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginSourceSnapshot {
    /// Stable identity within this plugin installation; the host owns its URI.
    pub key: String,
    pub source_name: String,
    pub kind: String,
    pub title: String,
    pub body: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginActionProposal {
    pub provider: String,
    pub target_id: String,
    pub expected_version: i64,
    pub status: String,
    pub rationale: String,
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginTaskRef {
    pub task_id: String,
    pub version: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginImportedDocument {
    pub source_id: String,
    pub document_id: String,
    pub revision: i64,
    pub changed: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginContributionOutput {
    pub text: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<PluginPanel>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPanel {
    pub title: String,
    pub sections: Vec<PluginPanelSection>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginPanelSection {
    pub heading: String,
    pub text: String,
    #[serde(default)]
    pub evidence: Vec<Evidence>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginContributionRequest {
    pub expected_version: i64,
    pub contribution_id: String,
    #[serde(default)]
    pub source_ids: Vec<String>,
    #[serde(default)]
    pub project_ids: Vec<String>,
    /// Connector output destination, separate from its input scope.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_project_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation: Option<PluginEvaluationTarget>,
    #[serde(default = "plugin_empty_object")]
    pub input: Value,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginDocumentRef {
    pub document_id: String,
    pub revision: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginContributionRun {
    pub id: String,
    pub plugin_id: String,
    pub installed_at: String,
    pub configuration_version: i64,
    pub package_version: String,
    pub api_version: u32,
    pub contribution: PluginContribution,
    #[serde(default = "plugin_contribution_method")]
    pub invocation_method: String,
    #[serde(default)]
    pub trigger_event: Option<PluginEvent>,
    pub source_ids: Vec<String>,
    pub project_ids: Vec<String>,
    pub documents: Vec<PluginDocumentRef>,
    #[serde(default)]
    pub tasks: Vec<PluginTaskRef>,
    #[serde(default)]
    pub destination_project_id: Option<String>,
    #[serde(default)]
    pub imported_documents: Vec<PluginImportedDocument>,
    #[serde(default)]
    pub action_preview_ids: Vec<String>,
    #[serde(default)]
    pub evaluation: Option<PluginEvaluationTarget>,
    #[serde(default)]
    pub evaluation_signature: Option<String>,
    #[serde(default)]
    pub evaluation_receipt: Option<PluginEvaluationReceipt>,
    #[serde(default)]
    pub model_proposal_ids: Vec<String>,
    #[serde(default)]
    pub model_results: Vec<PluginModelResult>,
    #[serde(default)]
    pub readiness: Option<PluginReadinessResult>,
    pub grant: PluginPermissions,
    pub configuration_hash: String,
    pub executable_definition_hash: String,
    pub input: Value,
    pub event_id: String,
    pub status: String,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub output: Option<PluginContributionOutput>,
    pub error: Option<String>,
}
pub fn plugin_contribution_method() -> String {
    "contribution".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginCapabilityStatus {
    pub capability: String,
    pub provider_id: String,
    pub host: PluginHost,
    pub available: bool,
    pub detail: String,
    pub selection_version: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginProviderSelectionRequest {
    pub expected_version: i64,
    pub provider_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginHostRegistration {
    pub host_id: String,
    pub manifests: Vec<PluginManifest>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginHostSession {
    pub host_id: String,
    pub lease_token: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginHostHeartbeat {
    pub providers: Vec<PluginProviderHealth>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginProviderHealth {
    pub provider_id: String,
    pub version: i64,
    pub available: bool,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginHostPoll {
    pub plugins: Vec<PluginRecord>,
    pub selections: Vec<PluginCapabilityStatus>,
    pub commands: Vec<PluginNativeCommand>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginNativeCommand {
    pub id: String,
    pub host_id: String,
    pub provider_id: String,
    pub provider_version: i64,
    pub capability: String,
    pub command: String,
    pub input: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginNativeRequest {
    pub command: String,
    #[serde(default = "plugin_empty_object")]
    pub input: Value,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginNativeReply {
    pub result: Value,
    pub error: Option<String>,
}
