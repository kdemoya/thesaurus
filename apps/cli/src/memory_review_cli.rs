use anyhow::{Result, bail};
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;

#[derive(Subcommand)]
pub enum MemoryReviewCommands {
    List {
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Inspect {
        id: String,
    },
    Revision {
        id: String,
        version: i64,
    },
    Revise {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["correct","suppress","restore"])]
        action: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        restore_version: Option<i64>,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Conflicts,
    Conflict {
        id: String,
    },
    ConflictHistory {
        id: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    FlagConflict {
        #[arg(long,num_args=2..=8)]
        observations: Vec<String>,
        #[arg(long)]
        reason: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Resolve {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        decision: String,
        #[arg(long)]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
}
fn id(value: &str) -> Result<&str> {
    if value.is_empty()
        || value.len() > 100
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("invalid record ID");
    }
    Ok(value)
}
fn key(value: Option<String>) -> String {
    value.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
pub async fn run(client: &Client, cmd: MemoryReviewCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match cmd {
        MemoryReviewCommands::List { offset } => (
            Method::GET,
            format!("/api/v1/memory/inspect?offset={offset}"),
            None,
        ),
        MemoryReviewCommands::Inspect { id: record } => (
            Method::GET,
            format!("/api/v1/memory/inspect/{}", id(&record)?),
            None,
        ),
        MemoryReviewCommands::Revision {
            id: record,
            version,
        } => (
            Method::GET,
            format!(
                "/api/v1/memory/inspect/{}/revisions/{version}",
                id(&record)?
            ),
            None,
        ),
        MemoryReviewCommands::Revise {
            id: record,
            expected_version,
            action,
            body,
            restore_version,
            reason,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/memory/inspect/{}", id(&record)?),
            Some(
                json!({"expected_version":expected_version,"action":action,"body":body,"restore_version":restore_version,"reason":reason,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        MemoryReviewCommands::Conflicts => (Method::GET, "/api/v1/memory/conflicts".into(), None),
        MemoryReviewCommands::Conflict { id: record } => (
            Method::GET,
            format!("/api/v1/memory/conflicts/{}", id(&record)?),
            None,
        ),
        MemoryReviewCommands::ConflictHistory { id: record, offset } => (
            Method::GET,
            format!(
                "/api/v1/memory/conflicts/{}/history?offset={offset}",
                id(&record)?
            ),
            None,
        ),
        MemoryReviewCommands::FlagConflict {
            observations,
            reason,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/memory/conflicts".into(),
            Some(
                json!({"observation_ids":observations,"reason":reason,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        MemoryReviewCommands::Resolve {
            id: record,
            expected_version,
            decision,
            note,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/memory/conflicts/{}", id(&record)?),
            Some(
                json!({"expected_version":expected_version,"decision":decision,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
    };
    client.call(method, &path, body).await
}
