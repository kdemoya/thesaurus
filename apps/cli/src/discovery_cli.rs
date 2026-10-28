use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum DiscoveryCommands {
    Cues {
        project: String,
        #[arg(long,default_value="active",value_parser=["active","unreviewed","needs_answer","resolved","dismissed","all"])]
        status: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Cue {
        id: String,
    },
    CreateCue {
        project: String,
        text: String,
        #[arg(long,default_value="question",value_parser=["question","conflict"])]
        kind: String,
        #[arg(long)]
        evidence_json: String,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    ReviewCue {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["needs_answer","resolved","dismissed","unreviewed"])]
        status: String,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        evidence_json: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Gaps {
        project: String,
        #[arg(long,default_value="active",value_parser=["active","open","needs_review","answered","archived","all"])]
        status: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Gap {
        id: String,
    },
    CreateGap {
        project: String,
        question: String,
        #[arg(long)]
        missing_source: String,
        #[arg(long, required = true)]
        source: Vec<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    UpdateGap {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["edit","answer","archive","reopen","accept_suggestion","dismiss_suggestion"])]
        action: String,
        #[arg(long)]
        question: Option<String>,
        #[arg(long)]
        missing_source: Option<String>,
        #[arg(long)]
        source: Vec<String>,
        #[arg(long)]
        answer: Option<String>,
        #[arg(long)]
        evidence_json: Option<String>,
        #[arg(long)]
        suggestion: Option<String>,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Refresh {
        project: String,
    },
    Mute {
        project: String,
        #[arg(long)]
        muted: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    History {
        #[arg(value_parser=["cue","gap"])]
        kind: String,
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
fn evidence(s: Option<String>) -> Result<Value> {
    Ok(match s {
        Some(s) => serde_json::from_str(&s)?,
        None => json!([]),
    })
}
pub async fn run(c: &Client, cmd: DiscoveryCommands) -> Result<Value> {
    use reqwest::Method;
    let (m, p, b) = match cmd {
        DiscoveryCommands::Cues {
            project,
            status,
            offset,
        } => (
            Method::GET,
            format!(
                "/api/v1/projects/{}/cues?status={status}&offset={offset}",
                id(&project)?
            ),
            None,
        ),
        DiscoveryCommands::Cue { id: i } => {
            (Method::GET, format!("/api/v1/cues/{}", id(&i)?), None)
        }
        DiscoveryCommands::CreateCue {
            project,
            text,
            kind,
            evidence_json,
            note,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/cues".into(),
            Some(
                json!({"project_id":project,"text":text,"kind":kind,"evidence":serde_json::from_str::<Value>(&evidence_json)?,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        DiscoveryCommands::ReviewCue {
            id: i,
            expected_version,
            status,
            note,
            evidence_json,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/cues/{}", id(&i)?),
            Some(
                json!({"expected_version":expected_version,"status":status,"note":note,"evidence":evidence(evidence_json)?,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        DiscoveryCommands::Gaps {
            project,
            status,
            offset,
        } => (
            Method::GET,
            format!(
                "/api/v1/projects/{}/gaps?status={status}&offset={offset}",
                id(&project)?
            ),
            None,
        ),
        DiscoveryCommands::Gap { id: i } => {
            (Method::GET, format!("/api/v1/gaps/{}", id(&i)?), None)
        }
        DiscoveryCommands::CreateGap {
            project,
            question,
            missing_source,
            source,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/gaps".into(),
            Some(
                json!({"project_id":project,"question":question,"missing_source":missing_source,"required_source_ids":source,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        DiscoveryCommands::UpdateGap {
            id: i,
            expected_version,
            action,
            question,
            missing_source,
            source,
            answer,
            evidence_json,
            suggestion,
            note,
            idempotency_key,
        } => (
            Method::PATCH,
            format!("/api/v1/gaps/{}", id(&i)?),
            Some(
                json!({"expected_version":expected_version,"action":action,"question":question,"missing_source":missing_source,"required_source_ids":if source.is_empty(){None}else{Some(source)},"answer":answer,"evidence":evidence(evidence_json)?,"suggestion_id":suggestion,"note":note,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        DiscoveryCommands::Refresh { project } => (
            Method::POST,
            format!("/api/v1/projects/{}/discovery/refresh", id(&project)?),
            Some(json!({})),
        ),
        DiscoveryCommands::Mute {
            project,
            muted,
            idempotency_key,
        } => (
            Method::PUT,
            format!("/api/v1/projects/{}/discovery/mute", id(&project)?),
            Some(json!({"muted":muted,"idempotency_key":key(idempotency_key)})),
        ),
        DiscoveryCommands::History {
            kind,
            id: i,
            offset,
        } => (
            Method::GET,
            format!(
                "/api/v1/discovery/{kind}/{}/history?offset={offset}",
                id(&i)?
            ),
            None,
        ),
    };
    c.call(m, &p, b).await
}
