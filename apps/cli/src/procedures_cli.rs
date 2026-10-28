use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum ProcedureCommands {
    List {
        project: String,
        #[arg(long)]
        include_dismissed: bool,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Get {
        id: String,
    },
    Refresh {
        project: String,
    },
    Reviews {
        project: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Review {
        project: String,
        run: String,
        procedure: String,
        #[arg(long)]
        successful: bool,
        #[arg(long, default_value_t = 0)]
        expected_version: i64,
        #[arg(long)]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Decide {
        id: String,
        #[arg(value_parser=["dismiss","reopen"])]
        action: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Install {
        id: String,
        #[arg(long)]
        json: String,
    },
    History {
        id: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
}
fn id(s: &str) -> Result<&str> {
    if s.is_empty()
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        anyhow::bail!("invalid record identifier");
    }
    Ok(s)
}
fn key(k: Option<String>) -> String {
    k.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
pub async fn run(c: &Client, cmd: ProcedureCommands) -> Result<Value> {
    use reqwest::Method;
    let (m, p, b) = match cmd {
        ProcedureCommands::List {
            project,
            include_dismissed,
            offset,
        } => (
            Method::GET,
            format!(
                "/api/v1/projects/{}/procedures?offset={offset}&include_dismissed={include_dismissed}",
                id(&project)?
            ),
            None,
        ),
        ProcedureCommands::Get { id: i } => {
            (Method::GET, format!("/api/v1/procedures/{}", id(&i)?), None)
        }
        ProcedureCommands::Refresh { project } => (
            Method::POST,
            format!("/api/v1/projects/{}/procedures/refresh", id(&project)?),
            Some(json!({})),
        ),
        ProcedureCommands::Reviews { project, offset } => (
            Method::GET,
            format!(
                "/api/v1/projects/{}/procedure-reviews?offset={offset}",
                id(&project)?
            ),
            None,
        ),
        ProcedureCommands::Review {
            project,
            run,
            procedure,
            successful,
            expected_version,
            note,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/procedure-reviews".into(),
            Some(
                json!({"project_id":project,"run_id":run,"procedure":procedure,"successful":successful,"expected_version":expected_version,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        ProcedureCommands::Decide {
            id: i,
            action,
            expected_version,
            note,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/procedures/{}", id(&i)?),
            Some(
                json!({"expected_version":expected_version,"action":action,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        ProcedureCommands::Install { id: i, json: s } => {
            let mut body: Value = serde_json::from_str(&s)?;
            if !body.is_object() {
                anyhow::bail!("invalid: installation request must be an object");
            }
            if body.get("idempotency_key").is_none() {
                body["idempotency_key"] = json!(key(None));
            }
            (
                Method::POST,
                format!("/api/v1/procedures/{}/install", id(&i)?),
                Some(body),
            )
        }
        ProcedureCommands::History { id: i, offset } => (
            Method::GET,
            format!("/api/v1/procedures/{}/history?offset={offset}", id(&i)?),
            None,
        ),
    };
    c.call(m, &p, b).await
}
