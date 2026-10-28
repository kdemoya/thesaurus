use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;

#[derive(Subcommand)]
pub enum ThreadCommands {
    Receipt {
        id: String,
        context: String,
    },
    List,
    Get {
        id: String,
    },
    Create {
        project: String,
        title: String,
        #[arg(long, required = true, value_delimiter = ',')]
        source: Vec<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Update {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        title: String,
        #[arg(long,value_parser=["active","closed"])]
        status: String,
        #[arg(long, required = true, value_delimiter = ',')]
        source: Vec<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Checkpoint {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long,default_value="checkpoint",value_parser=["checkpoint","task","observation","agent_run"])]
        kind: String,
        #[arg(long, default_value = "")]
        body: String,
        #[arg(long)]
        record: Option<String>,
        #[arg(long)]
        evidence_json: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Split {
        id: String,
        title: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long, required = true, value_delimiter = ',')]
        entry: Vec<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Merge {
        id: String,
        other_id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        other_expected_version: i64,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Context {
        id: String,
        #[arg(long, default_value_t = 8000)]
        budget: usize,
    },
    Resume {
        id: String,
        #[arg(long)]
        context: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        connection: String,
        #[arg(long)]
        approve_usage: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
}
#[derive(Subcommand)]
pub enum MeetingCommands {
    List,
    Get {
        id: String,
    },
    Create {
        project: String,
        name: String,
        #[arg(long, default_value = "")]
        cadence: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Link {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        source: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        held_on: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Occurrence {
        id: String,
        occurrence: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["active","removed"])]
        status: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Prepare {
        id: String,
    },
}
fn key(k: Option<String>) -> String {
    k.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
pub async fn thread(client: &Client, cmd: ThreadCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match cmd {
        ThreadCommands::Receipt { id, context } => (
            Method::GET,
            format!("/api/v1/threads/{id}/contexts/{context}"),
            None,
        ),
        ThreadCommands::List => (Method::GET, "/api/v1/threads".into(), None),
        ThreadCommands::Get { id } => (Method::GET, format!("/api/v1/threads/{id}"), None),
        ThreadCommands::Create {
            project,
            title,
            source,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/threads".into(),
            Some(
                json!({"project_id":project,"title":title,"source_ids":source,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        ThreadCommands::Update {
            id,
            expected_version,
            title,
            status,
            source,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/threads/{id}"),
            Some(
                json!({"expected_version":expected_version,"title":title,"status":status,"source_ids":source,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        ThreadCommands::Checkpoint {
            id,
            expected_version,
            kind,
            body,
            record,
            evidence_json,
            idempotency_key,
        } => {
            let evidence: Value = if let Some(json) = evidence_json {
                serde_json::from_str(&json)?
            } else {
                json!([])
            };
            (
                Method::POST,
                format!("/api/v1/threads/{id}/entries"),
                Some(
                    json!({"expected_version":expected_version,"kind":kind,"body":body,"record_id":record,"evidence":evidence,"idempotency_key":key(idempotency_key)}),
                ),
            )
        }
        ThreadCommands::Split {
            id,
            title,
            expected_version,
            entry,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/threads/{id}/split"),
            Some(
                json!({"expected_version":expected_version,"title":title,"entry_ids":entry,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        ThreadCommands::Merge {
            id,
            other_id,
            expected_version,
            other_expected_version,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/threads/{id}/merge"),
            Some(
                json!({"expected_version":expected_version,"other_id":other_id,"other_expected_version":other_expected_version,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        ThreadCommands::Context { id, budget } => (
            Method::POST,
            format!("/api/v1/threads/{id}/context"),
            Some(json!({"byte_budget":budget})),
        ),
        ThreadCommands::Resume {
            id,
            context,
            expected_version,
            connection,
            approve_usage,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/threads/{id}/resume"),
            Some(
                json!({"context_id":context,"expected_version":expected_version,"connection":connection,"approve_usage":approve_usage,"idempotency_key":key(idempotency_key)}),
            ),
        ),
    };
    client.call(method, &path, body).await
}
pub async fn meeting(client: &Client, cmd: MeetingCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match cmd {
        MeetingCommands::List => (Method::GET, "/api/v1/meetings".into(), None),
        MeetingCommands::Get { id } => (Method::GET, format!("/api/v1/meetings/{id}"), None),
        MeetingCommands::Create {
            project,
            name,
            cadence,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/meetings".into(),
            Some(
                json!({"project_id":project,"name":name,"cadence":cadence,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        MeetingCommands::Link {
            id,
            expected_version,
            source,
            revision,
            held_on,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/meetings/{id}/occurrences"),
            Some(
                json!({"expected_version":expected_version,"source_id":source,"revision":revision,"held_on":held_on,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        MeetingCommands::Occurrence {
            id,
            occurrence,
            expected_version,
            status,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/meetings/{id}/occurrences/{occurrence}"),
            Some(
                json!({"expected_version":expected_version,"status":status,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        MeetingCommands::Prepare { id } => (
            Method::GET,
            format!("/api/v1/meetings/{id}/preparation"),
            None,
        ),
    };
    client.call(method, &path, body).await
}
