use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum RecipeCommands {
    List {
        project: String,
    },
    Get {
        id: String,
        #[arg(long)]
        version: Option<i64>,
    },
    /// Save a complete recipe request from a JSON file; expected_version=0 creates it.
    Save {
        file: std::path::PathBuf,
        #[arg(long)]
        id: Option<String>,
    },
    Preview {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long, default_value = "{}")]
        inputs_json: String,
    },
    Run {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long, default_value = "{}")]
        inputs_json: String,
        #[arg(long)]
        signature: String,
        #[arg(long)]
        approve_usage: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Runs {
        id: String,
    },
}
pub async fn execute(client: &Client, cmd: RecipeCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match cmd {
        RecipeCommands::List { project } => (
            Method::GET,
            format!("/api/v1/projects/{project}/recipes"),
            None,
        ),
        RecipeCommands::Get { id, version } => (
            Method::GET,
            format!(
                "/api/v1/recipes/{id}{}",
                version.map(|v| format!("?version={v}")).unwrap_or_default()
            ),
            None,
        ),
        RecipeCommands::Save { file, id } => {
            let mut body: Value = serde_json::from_str(&std::fs::read_to_string(file)?)?;
            if body.get("idempotency_key").is_none() {
                body["idempotency_key"] = json!(uuid::Uuid::new_v4().to_string());
            }
            (
                if id.is_some() {
                    Method::PATCH
                } else {
                    Method::POST
                },
                id.map(|i| format!("/api/v1/recipes/{i}"))
                    .unwrap_or("/api/v1/recipes".into()),
                Some(body),
            )
        }
        RecipeCommands::Preview {
            id,
            expected_version,
            inputs_json,
        } => (
            Method::POST,
            format!("/api/v1/recipes/{id}/preview"),
            Some(
                json!({"expected_version":expected_version,"inputs":serde_json::from_str::<Value>(&inputs_json)?}),
            ),
        ),
        RecipeCommands::Run {
            id,
            expected_version,
            inputs_json,
            signature,
            approve_usage,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/recipes/{id}/run"),
            Some(
                json!({"expected_version":expected_version,"inputs":serde_json::from_str::<Value>(&inputs_json)?,"signature":signature,"approve_usage":approve_usage,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}),
            ),
        ),
        RecipeCommands::Runs { id } => (Method::GET, format!("/api/v1/recipes/{id}/runs"), None),
    };
    client.call(method, &path, body).await
}
