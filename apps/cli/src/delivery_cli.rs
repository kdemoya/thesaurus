use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum CommitmentCommands {
    RepositoryLink {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        report: String,
        #[arg(long)]
        commit: String,
        #[arg(long)]
        path: Vec<String>,
        #[arg(long)]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    List {
        project: String,
        #[arg(long,default_value="open",value_parser=["open","proposed","completed","cancelled","all"])]
        status: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long, default_value_t = 25)]
        limit: u32,
    },
    Get {
        id: String,
    },
    History {
        id: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long, default_value_t = 25)]
        limit: u32,
    },
    Create {
        project: String,
        title: String,
        #[arg(long)]
        owner: String,
        #[arg(long)]
        promised_on: String,
        #[arg(long)]
        due: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        evidence_json: Option<String>,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Update {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["amend","link","propose_completion","confirm_completion","reject_completion","cancel","reopen"])]
        action: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        due: Option<String>,
        #[arg(long)]
        clear_due: bool,
        #[arg(long,value_parser=["discussion","implementation"])]
        kind: Option<String>,
        #[arg(long)]
        evidence_json: Option<String>,
        #[arg(long, value_delimiter = ',')]
        link_ids: Vec<String>,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
}
fn key(k: Option<String>) -> String {
    k.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
fn evidence(s: Option<String>) -> Result<Value> {
    Ok(match s {
        Some(s) => serde_json::from_str(&s)?,
        None => json!([]),
    })
}
fn segment(s: &str) -> Result<&str> {
    if s.is_empty()
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        anyhow::bail!("invalid record identifier");
    }
    Ok(s)
}
pub async fn run(client: &Client, cmd: CommitmentCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match cmd {
        CommitmentCommands::RepositoryLink {
            id,
            expected_version,
            report,
            commit,
            path,
            note,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/commitments/{}/repository-links", segment(&id)?),
            Some(
                json!({"expected_version":expected_version,"report_id":report,"commit_id":commit,"paths":path,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        CommitmentCommands::List {
            project,
            status,
            offset,
            limit,
        } => (
            Method::GET,
            format!(
                "/api/v1/projects/{}/commitments?status={status}&offset={offset}&limit={limit}",
                segment(&project)?
            ),
            None,
        ),
        CommitmentCommands::Get { id } => (
            Method::GET,
            format!("/api/v1/commitments/{}", segment(&id)?),
            None,
        ),
        CommitmentCommands::History { id, offset, limit } => (
            Method::GET,
            format!(
                "/api/v1/commitments/{}/history?offset={offset}&limit={limit}",
                segment(&id)?
            ),
            None,
        ),
        CommitmentCommands::Create {
            project,
            title,
            owner,
            promised_on,
            due,
            task,
            evidence_json,
            note,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/commitments".into(),
            Some(
                json!({"project_id":project,"title":title,"owner":owner,"promised_on":promised_on,"due":due,"task_id":task,"evidence":evidence(evidence_json)?,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        CommitmentCommands::Update {
            id,
            expected_version,
            action,
            title,
            owner,
            due,
            clear_due,
            kind,
            evidence_json,
            link_ids,
            note,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/commitments/{}", segment(&id)?),
            Some(
                json!({"expected_version":expected_version,"action":action,"title":title,"owner":owner,"due":due,"clear_due":clear_due,"kind":kind,"evidence":evidence(evidence_json)?,"link_ids":link_ids,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
    };
    client.call(method, &path, body).await
}
