use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum ActionCommands {
    Previews,
    Receipts,
    Get {
        id: String,
    },
    Preview {
        #[arg(value_parser=["task_status","delete_activity_frame"])]
        provider: String,
        target: String,
        #[arg(long)]
        expected_version: Option<i64>,
        #[arg(long)]
        status: Option<String>,
    },
    Execute {
        preview: String,
        #[arg(long)]
        fingerprint: String,
        #[arg(long)]
        approve_effects: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Undo {
        receipt: String,
        #[arg(long)]
        expected_receipt_version: i64,
        #[arg(long)]
        approve_effects: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
}
pub async fn execute(c: &Client, cmd: ActionCommands) -> Result<Value> {
    use reqwest::Method;
    let (m, p, b) = match cmd {
        ActionCommands::Previews => (Method::GET, "/api/v1/actions/previews".into(), None),
        ActionCommands::Receipts => (Method::GET, "/api/v1/actions/receipts".into(), None),
        ActionCommands::Get { id } => (Method::GET, format!("/api/v1/actions/receipts/{id}"), None),
        ActionCommands::Preview {
            provider,
            target,
            expected_version,
            status,
        } => (
            Method::POST,
            "/api/v1/actions/previews".into(),
            Some(
                json!({"provider":provider,"target_id":target,"expected_version":expected_version,"status":status,"idempotency_key":uuid::Uuid::new_v4().to_string()}),
            ),
        ),
        ActionCommands::Execute {
            preview,
            fingerprint,
            approve_effects,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/actions/previews/{preview}/execute"),
            Some(
                json!({"fingerprint":fingerprint,"approve_effects":approve_effects,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}),
            ),
        ),
        ActionCommands::Undo {
            receipt,
            expected_receipt_version,
            approve_effects,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/actions/receipts/{receipt}/undo"),
            Some(
                json!({"expected_receipt_version":expected_receipt_version,"approve_effects":approve_effects,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}),
            ),
        ),
    };
    c.call(m, &p, b).await
}
