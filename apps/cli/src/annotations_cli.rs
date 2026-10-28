use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum AnnotationCommands {
    Notes {
        document: String,
        #[arg(long)]
        revision: Option<i64>,
        #[arg(long)]
        include_archived: bool,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Get {
        id: String,
    },
    History {
        id: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Create {
        #[arg(long)]
        evidence_json: String,
        body: String,
        #[arg(long)]
        emphasis: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Update {
        id: String,
        body: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        emphasis: bool,
        #[arg(long,default_value="active",value_parser=["active","archived"])]
        status: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Corrections {
        source: String,
    },
    SetCorrections {
        source: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        rules_json: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Preview {
        source: String,
    },
    Apply {
        source: String,
        #[arg(long)]
        signature: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    CorrectionHistory {
        source: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
}
fn key(k: Option<String>) -> String {
    k.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
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
pub async fn run(c: &Client, cmd: AnnotationCommands) -> Result<Value> {
    use reqwest::Method;
    let (m, p, b) = match cmd {
        AnnotationCommands::Notes {
            document,
            revision,
            include_archived,
            offset,
        } => (
            Method::GET,
            format!(
                "/api/v1/documents/{}/notes?offset={offset}&include_archived={include_archived}{}",
                id(&document)?,
                revision
                    .map(|r| format!("&revision={r}"))
                    .unwrap_or_default()
            ),
            None,
        ),
        AnnotationCommands::Get { id: note } => {
            (Method::GET, format!("/api/v1/notes/{}", id(&note)?), None)
        }
        AnnotationCommands::History { id: note, offset } => (
            Method::GET,
            format!("/api/v1/notes/{}/history?offset={offset}", id(&note)?),
            None,
        ),
        AnnotationCommands::Create {
            evidence_json,
            body,
            emphasis,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/notes".into(),
            Some(
                json!({"evidence":serde_json::from_str::<Value>(&evidence_json)?,"body":body,"emphasis":emphasis,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        AnnotationCommands::Update {
            id: note,
            body,
            expected_version,
            emphasis,
            status,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/notes/{}", id(&note)?),
            Some(
                json!({"body":body,"expected_version":expected_version,"emphasis":emphasis,"status":status,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        AnnotationCommands::Corrections { source } => (
            Method::GET,
            format!("/api/v1/sources/{}/corrections", id(&source)?),
            None,
        ),
        AnnotationCommands::SetCorrections {
            source,
            expected_version,
            rules_json,
            idempotency_key,
        } => (
            Method::PUT,
            format!("/api/v1/sources/{}/corrections", id(&source)?),
            Some(
                json!({"expected_version":expected_version,"rules":serde_json::from_str::<Value>(&rules_json)?,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        AnnotationCommands::Preview { source } => (
            Method::GET,
            format!("/api/v1/sources/{}/corrections/preview", id(&source)?),
            None,
        ),
        AnnotationCommands::Apply {
            source,
            signature,
            idempotency_key,
        } => (
            Method::POST,
            format!("/api/v1/sources/{}/corrections/apply", id(&source)?),
            Some(json!({"signature":signature,"idempotency_key":key(idempotency_key)})),
        ),
        AnnotationCommands::CorrectionHistory { source, offset } => (
            Method::GET,
            format!(
                "/api/v1/sources/{}/corrections/history?offset={offset}",
                id(&source)?
            ),
            None,
        ),
    };
    c.call(m, &p, b).await
}
