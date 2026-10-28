use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum UsageCommands {
    List,
    Set {
        connection: String,
        file: std::path::PathBuf,
    },
    FallbackPreview {
        primary: String,
    },
    FallbackRun {
        primary: String,
        #[arg(long)]
        signature: String,
        #[arg(long)]
        approve_usage: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
}
pub async fn execute(client: &Client, cmd: UsageCommands) -> Result<Value> {
    use reqwest::Method;
    let (m, p, b) = match cmd {
        UsageCommands::List => (Method::GET, "/api/v1/usage".into(), None),
        UsageCommands::Set { connection, file } => {
            let mut v: Value = serde_json::from_str(&std::fs::read_to_string(file)?)?;
            if v.get("idempotency_key").is_none() {
                v["idempotency_key"] = json!(uuid::Uuid::new_v4().to_string());
            }
            (Method::PUT, format!("/api/v1/usage/{connection}"), Some(v))
        }
        UsageCommands::FallbackPreview { primary } => (
            Method::GET,
            format!("/api/v1/runtime/runs/{primary}/fallback"),
            None,
        ),
        UsageCommands::FallbackRun {
            primary,
            signature,
            approve_usage,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/runtime/runs/{primary}/fallback"),
            Some(
                json!({"signature":signature,"approve_usage":approve_usage,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}),
            ),
        ),
    };
    client.call(m, &p, b).await
}
