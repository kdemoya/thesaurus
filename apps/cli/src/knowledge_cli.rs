use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum KnowledgeCommands {
    Answer {
        project: String,
        question: String,
        #[arg(long)]
        valid_as_of: Option<String>,
        #[arg(long)]
        known_by: Option<String>,
        #[arg(long)]
        source: Vec<String>,
        #[arg(long)]
        compare_today: bool,
    },
    Claims {
        project: String,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        include_archived: bool,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Claim {
        id: String,
    },
    CreateClaim {
        #[arg(long)]
        json: String,
    },
    UpdateClaim {
        id: String,
        #[arg(long)]
        json: String,
    },
    ClaimHistory {
        id: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    People {
        project: String,
        #[arg(long)]
        query: Option<String>,
        #[arg(long)]
        include_archived: bool,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Person {
        id: String,
    },
    CreatePerson {
        #[arg(long)]
        json: String,
    },
    UpdatePerson {
        id: String,
        #[arg(long)]
        json: String,
    },
    PersonHistory {
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
fn body(s: &str) -> Result<Value> {
    let mut value: Value = serde_json::from_str(s)?;
    if !value.is_object() {
        anyhow::bail!("invalid: request must be a JSON object");
    }
    if value.get("idempotency_key").is_none() {
        value["idempotency_key"] = json!(uuid::Uuid::new_v4().to_string());
    }
    Ok(value)
}
pub async fn run(c: &Client, cmd: KnowledgeCommands) -> Result<Value> {
    use reqwest::Method;
    let (m, p, b) = match cmd {
        KnowledgeCommands::Answer {
            project,
            question,
            valid_as_of,
            known_by,
            source,
            compare_today,
        } => (
            Method::POST,
            if compare_today {
                "/api/v1/knowledge/compare"
            } else {
                "/api/v1/knowledge/answer"
            }
            .into(),
            Some(
                json!({"project_id":project,"question":question,"valid_as_of":valid_as_of,"known_by":known_by,"source_ids":if source.is_empty(){None}else{Some(source)}}),
            ),
        ),
        KnowledgeCommands::Claims {
            project,
            query,
            include_archived,
            offset,
        } => (
            Method::GET,
            format!(
                "/api/v1/projects/{}/claims?offset={offset}&include_archived={include_archived}&query={}",
                id(&project)?,
                url(&query.unwrap_or_default())
            ),
            None,
        ),
        KnowledgeCommands::Claim { id: i } => {
            (Method::GET, format!("/api/v1/claims/{}", id(&i)?), None)
        }
        KnowledgeCommands::CreateClaim { json } => {
            (Method::POST, "/api/v1/claims".into(), Some(body(&json)?))
        }
        KnowledgeCommands::UpdateClaim { id: i, json } => (
            Method::PATCH,
            format!("/api/v1/claims/{}", id(&i)?),
            Some(body(&json)?),
        ),
        KnowledgeCommands::ClaimHistory { id: i, offset } => (
            Method::GET,
            format!("/api/v1/claims/{}/history?offset={offset}", id(&i)?),
            None,
        ),
        KnowledgeCommands::People {
            project,
            query,
            include_archived,
            offset,
        } => (
            Method::GET,
            format!(
                "/api/v1/projects/{}/people?offset={offset}&include_archived={include_archived}&query={}",
                id(&project)?,
                url(&query.unwrap_or_default())
            ),
            None,
        ),
        KnowledgeCommands::Person { id: i } => {
            (Method::GET, format!("/api/v1/people/{}", id(&i)?), None)
        }
        KnowledgeCommands::CreatePerson { json } => {
            (Method::POST, "/api/v1/people".into(), Some(body(&json)?))
        }
        KnowledgeCommands::UpdatePerson { id: i, json } => (
            Method::PATCH,
            format!("/api/v1/people/{}", id(&i)?),
            Some(body(&json)?),
        ),
        KnowledgeCommands::PersonHistory { id: i, offset } => (
            Method::GET,
            format!("/api/v1/people/{}/history?offset={offset}", id(&i)?),
            None,
        ),
    };
    c.call(m, &p, b).await
}
fn url(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || [b'-', b'_', b'.', b'~'].contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
