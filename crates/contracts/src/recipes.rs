use crate::BriefSourceRevision;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkRecipe {
    pub id: String,
    pub project_id: String,
    pub version: i64,
    pub name: String,
    pub template: String,
    pub inputs: Vec<String>,
    pub source_ids: Vec<String>,
    pub connection: String,
    pub output_format: String,
    pub character_budget: usize,
    pub max_runs_per_day: i64,
    pub archived: bool,
    pub created_at: String,
    pub updated_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeSaveRequest {
    pub project_id: String,
    pub expected_version: i64,
    pub name: String,
    pub template: String,
    pub inputs: Vec<String>,
    pub source_ids: Vec<String>,
    pub connection: String,
    pub output_format: String,
    pub character_budget: usize,
    pub max_runs_per_day: i64,
    #[serde(default)]
    pub archived: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipePreviewRequest {
    pub expected_version: i64,
    pub inputs: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipePreview {
    #[serde(default)]
    pub selected_source_ids: Vec<String>,
    #[serde(default)]
    pub included_source_ids: Vec<String>,
    #[serde(default)]
    pub omitted_live_source_ids: Vec<String>,
    #[serde(default)]
    pub retrieval_policies: Vec<crate::DerivedRetrievalPolicy>,
    pub recipe_id: String,
    pub recipe_version: i64,
    pub connection: String,
    pub prompt: String,
    pub sources: Vec<BriefSourceRevision>,
    pub watermark: i64,
    pub omitted_documents: usize,
    pub omitted_characters: usize,
    pub approximate_tokens: usize,
    pub signature: String,
    pub runs_today: i64,
    pub max_runs_per_day: i64,
    pub external_sources_allowed: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeRunRequest {
    pub expected_version: i64,
    pub inputs: BTreeMap<String, String>,
    pub signature: String,
    pub approve_usage: bool,
    pub idempotency_key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecipeRun {
    pub run_id: String,
    pub recipe_id: String,
    pub recipe_version: i64,
    pub inputs: BTreeMap<String, String>,
    pub preview: RecipePreview,
    pub created_at: String,
}
