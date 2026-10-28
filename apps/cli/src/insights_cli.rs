use anyhow::{Result, bail};
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::PathBuf;
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum InsightCommands {
    Discover {
        project: String,
        topic: String,
    },
    Opportunities {
        project: String,
    },
    CreateOpportunity {
        project: String,
        #[arg(long)]
        file: PathBuf,
    },
    ReviewOpportunity {
        project: String,
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["accepted","rejected","candidate"])]
        decision: String,
        #[arg(long)]
        attest_independent: bool,
        #[arg(long)]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Weekly {
        project: String,
    },
    CreateWeekly {
        project: String,
        #[arg(long)]
        start: String,
        #[arg(long)]
        end: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    GetWeekly {
        project: String,
        id: String,
    },
    ReviewWeekly {
        project: String,
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["reviewed","rejected","draft"])]
        decision: String,
        #[arg(long)]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Efforts {
        project: String,
    },
    Effort {
        project: String,
        task: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        minutes: u32,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Capacity {
        project: String,
    },
    SaveCapacity {
        project: String,
        #[arg(long)]
        file: PathBuf,
    },
    ImportCalendar {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value_t = 0)]
        utc_offset_minutes: i32,
    },
}
fn key(value: Option<String>) -> String {
    value.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
fn id(value: &str) -> Result<&str> {
    if value.is_empty()
        || value.len() > 200
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("invalid record ID");
    }
    Ok(value)
}
fn file(path: PathBuf) -> Result<String> {
    let meta = std::fs::metadata(&path)?;
    if !meta.is_file() || meta.len() > 1000000 {
        bail!("invalid: input must be a regular file up to 1MB");
    }
    Ok(std::fs::read_to_string(path)?)
}
fn request(path: PathBuf) -> Result<Value> {
    let mut value: Value = serde_json::from_str(&file(path)?)?;
    if value.get("idempotency_key").is_none() {
        value
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("invalid request object"))?
            .insert("idempotency_key".into(), json!(key(None)));
    }
    Ok(value)
}
pub async fn run(client: &Client, cmd: InsightCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match cmd {
        InsightCommands::Discover { project, topic } => (
            Method::POST,
            format!("/api/v1/projects/{}/opportunities/discover", id(&project)?),
            Some(json!({"topic":topic})),
        ),
        InsightCommands::Opportunities { project } => (
            Method::GET,
            format!("/api/v1/projects/{}/opportunities", id(&project)?),
            None,
        ),
        InsightCommands::CreateOpportunity { project, file } => (
            Method::POST,
            format!("/api/v1/projects/{}/opportunities", id(&project)?),
            Some(request(file)?),
        ),
        InsightCommands::ReviewOpportunity {
            project,
            id: record,
            expected_version,
            decision,
            attest_independent,
            note,
            idempotency_key,
        } => (
            Method::PATCH,
            format!(
                "/api/v1/projects/{}/opportunities/{}",
                id(&project)?,
                id(&record)?
            ),
            Some(
                json!({"expected_version":expected_version,"decision":decision,"note":note,"attest_independent":attest_independent,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        InsightCommands::Weekly { project } => (
            Method::GET,
            format!("/api/v1/projects/{}/outcomes", id(&project)?),
            None,
        ),
        InsightCommands::CreateWeekly {
            project,
            start,
            end,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/projects/{}/outcomes", id(&project)?),
            Some(json!({"start_date":start,"end_date":end,"idempotency_key":key(idempotency_key)})),
        ),
        InsightCommands::GetWeekly {
            project,
            id: record,
        } => (
            Method::GET,
            format!(
                "/api/v1/projects/{}/outcomes/{}",
                id(&project)?,
                id(&record)?
            ),
            None,
        ),
        InsightCommands::ReviewWeekly {
            project,
            id: record,
            expected_version,
            decision,
            note,
            idempotency_key,
        } => (
            Method::PATCH,
            format!(
                "/api/v1/projects/{}/outcomes/{}",
                id(&project)?,
                id(&record)?
            ),
            Some(
                json!({"expected_version":expected_version,"decision":decision,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        InsightCommands::Efforts { project } => (
            Method::GET,
            format!("/api/v1/projects/{}/efforts", id(&project)?),
            None,
        ),
        InsightCommands::Effort {
            project,
            task,
            expected_version,
            minutes,
            note,
            idempotency_key,
        } => (
            Method::PUT,
            format!("/api/v1/projects/{}/efforts/{}", id(&project)?, id(&task)?),
            Some(
                json!({"expected_version":expected_version,"minutes":minutes,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        InsightCommands::Capacity { project } => (
            Method::GET,
            format!("/api/v1/projects/{}/capacity", id(&project)?),
            None,
        ),
        InsightCommands::SaveCapacity { project, file } => (
            Method::POST,
            format!("/api/v1/projects/{}/capacity", id(&project)?),
            Some(request(file)?),
        ),
        InsightCommands::ImportCalendar {
            file: input,
            utc_offset_minutes,
        } => (
            Method::POST,
            "/api/v1/calendar/import-preview".into(),
            Some(json!({"ics":file(input)?,"utc_offset_minutes":utc_offset_minutes})),
        ),
    };
    client.call(method, &path, body).await
}
