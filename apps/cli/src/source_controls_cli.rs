use anyhow::{Result, bail};
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum SourceControlCommands {
    Disclosure {
        source: String,
    },
    Health {
        source: String,
    },
    Policy {
        source: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        external_enabled: bool,
        #[arg(long)]
        deny_connection: Vec<String>,
        #[arg(long)]
        deny_grant: Vec<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Coverage {
        source: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        expected_interval_hours: Option<u32>,
        #[arg(long)]
        excluded: Vec<String>,
        #[arg(long, default_value = "[]")]
        gaps_json: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Retry {
        source: String,
        document: String,
    },
    Notifications,
    Notification {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["unread","read","dismissed"])]
        status: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Mutes,
    Mute {
        #[arg(long)]
        expected_version: i64,
        #[arg(long, default_value = "*")]
        kind: String,
        #[arg(long)]
        source: Option<String>,
        #[arg(long)]
        active: bool,
        #[arg(long)]
        until: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
}
fn id(id: &str) -> Result<&str> {
    if id.is_empty()
        || id.len() > 200
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("invalid record ID");
    }
    Ok(id)
}
fn key(key: Option<String>) -> String {
    key.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
pub async fn run(client: &Client, command: SourceControlCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match command {
        SourceControlCommands::Disclosure { source } => (
            Method::GET,
            format!("/api/v1/sources/{}/disclosure", id(&source)?),
            None,
        ),
        SourceControlCommands::Health { source } => (
            Method::GET,
            format!("/api/v1/sources/{}/health", id(&source)?),
            None,
        ),
        SourceControlCommands::Policy {
            source,
            expected_version,
            external_enabled,
            deny_connection,
            deny_grant,
            idempotency_key,
        } => (
            Method::PUT,
            format!("/api/v1/sources/{}/policy", id(&source)?),
            Some(
                json!({"expected_version":expected_version,"external_enabled":external_enabled,"denied_connections":deny_connection,"denied_grants":deny_grant,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        SourceControlCommands::Coverage {
            source,
            expected_version,
            expected_interval_hours,
            excluded,
            gaps_json,
            idempotency_key,
        } => (
            Method::PUT,
            format!("/api/v1/sources/{}/coverage", id(&source)?),
            Some(
                json!({"expected_version":expected_version,"expected_interval_hours":expected_interval_hours,"excluded_content":excluded,"capture_gaps":serde_json::from_str::<Value>(&gaps_json)?,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        SourceControlCommands::Retry { source, document } => (
            Method::POST,
            format!("/api/v1/sources/{}/retry", id(&source)?),
            Some(json!({"document_id":document})),
        ),
        SourceControlCommands::Notifications => (Method::GET, "/api/v1/notifications".into(), None),
        SourceControlCommands::Notification {
            id: record,
            expected_version,
            status,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/notifications/{}", id(&record)?),
            Some(
                json!({"expected_version":expected_version,"status":status,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        SourceControlCommands::Mutes => (Method::GET, "/api/v1/notification-mutes".into(), None),
        SourceControlCommands::Mute {
            expected_version,
            kind,
            source,
            active,
            until,
            idempotency_key,
        } => (
            Method::PUT,
            "/api/v1/notification-mutes".into(),
            Some(
                json!({"expected_version":expected_version,"kind":kind,"source_id":source,"active":active,"until":until,"idempotency_key":key(idempotency_key)}),
            ),
        ),
    };
    client.call(method, &path, body).await
}
