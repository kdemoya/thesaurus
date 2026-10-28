use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum PriorityCommands {
    List {
        project: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long, default_value_t = 100)]
        limit: u32,
    },
    Preference {
        project: String,
        task: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long,value_parser=["low","normal","high","critical"])]
        importance: String,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Review {
        project: String,
        #[arg(long)]
        signature: String,
        #[arg(long)]
        expected_review_id: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    History {
        project: String,
    },
    GetReview {
        project: String,
        id: String,
    },
}
#[derive(Subcommand)]
pub enum WaitingCommands {
    List {
        project: String,
        #[arg(long,default_value="waiting",value_parser=["waiting","resolved","cancelled","all"])]
        status: String,
        #[arg(long)]
        due_only: bool,
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long, default_value_t = 25)]
        limit: u32,
    },
    Get {
        id: String,
    },
    Create {
        project: String,
        title: String,
        #[arg(long)]
        person: String,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        requested_on: String,
        #[arg(long)]
        follow_up_on: String,
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
        #[arg(value_parser=["edit","snooze","follow_up","resolve","cancel","reopen"])]
        action: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        person: Option<String>,
        #[arg(long)]
        follow_up_on: Option<String>,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        evidence_json: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
}
fn key(value: Option<String>) -> String {
    value.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
pub async fn priorities(client: &Client, cmd: PriorityCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match cmd {
        PriorityCommands::List {
            project,
            offset,
            limit,
        } => (
            Method::GET,
            format!("/api/v1/projects/{project}/priorities?offset={offset}&limit={limit}"),
            None,
        ),
        PriorityCommands::Preference {
            project,
            task,
            expected_version,
            importance,
            note,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/projects/{project}/priorities/tasks/{task}"),
            Some(
                json!({"expected_version":expected_version,"importance":importance,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        PriorityCommands::Review {
            project,
            signature,
            expected_review_id,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/projects/{project}/priorities/reviews"),
            Some(
                json!({"signature":signature,"expected_review_id":expected_review_id,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        PriorityCommands::History { project } => (
            Method::GET,
            format!("/api/v1/projects/{project}/priorities/reviews"),
            None,
        ),
        PriorityCommands::GetReview { project, id } => (
            Method::GET,
            format!("/api/v1/projects/{project}/priorities/reviews/{id}"),
            None,
        ),
    };
    client.call(method, &path, body).await
}
pub async fn waiting(client: &Client, cmd: WaitingCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match cmd {
        WaitingCommands::List {
            project,
            status,
            due_only,
            offset,
            limit,
        } => (
            Method::GET,
            format!(
                "/api/v1/projects/{project}/waiting?status={status}&due_only={due_only}&offset={offset}&limit={limit}"
            ),
            None,
        ),
        WaitingCommands::Get { id } => (Method::GET, format!("/api/v1/waiting/{id}"), None),
        WaitingCommands::Create {
            project,
            title,
            person,
            task,
            requested_on,
            follow_up_on,
            evidence_json,
            note,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/waiting".into(),
            Some(
                json!({"project_id":project,"title":title,"person":person,"task_id":task,"requested_on":requested_on,"follow_up_on":follow_up_on,"evidence":match evidence_json{Some(e)=>serde_json::from_str::<Value>(&e)?,None=>json!([])},"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        WaitingCommands::Update {
            id,
            expected_version,
            action,
            title,
            person,
            follow_up_on,
            note,
            evidence_json,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/waiting/{id}"),
            Some(
                json!({"expected_version":expected_version,"action":action,"title":title,"person":person,"follow_up_on":follow_up_on,"note":note,"evidence":match evidence_json{Some(e)=>serde_json::from_str::<Value>(&e)?,None=>json!([])},"idempotency_key":key(idempotency_key)}),
            ),
        ),
    };
    client.call(method, &path, body).await
}
