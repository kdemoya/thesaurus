use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum RetrievalCommands {
    List,
    Get {
        id: String,
    },
    Save {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        file: std::path::PathBuf,
    },
    Query {
        query: String,
        #[arg(long, required = true)]
        source: Vec<String>,
        #[arg(long)]
        approve_upstream: bool,
        #[arg(long, default_value_t = 20)]
        limit: u32,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Refresh {
        id: String,
        query: String,
        #[arg(long)]
        approve_upstream: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
}
fn id(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 100
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        anyhow::bail!("invalid id");
    }
    Ok(())
}
fn key(k: Option<String>) -> String {
    k.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
pub async fn execute(client: &Client, cmd: RetrievalCommands) -> Result<Value> {
    use reqwest::Method;
    let (m, p, b) = match cmd {
        RetrievalCommands::List => (Method::GET, "/api/v1/upstreams".into(), None),
        RetrievalCommands::Get { id: s } => {
            id(&s)?;
            (Method::GET, format!("/api/v1/upstreams/{s}"), None)
        }
        RetrievalCommands::Save { id: s, file } => {
            if let Some(s) = &s {
                id(s)?;
            }
            if std::fs::metadata(&file)?.len() > 100000 {
                anyhow::bail!("invalid: upstream request exceeds100KB");
            }
            let mut req: Value = serde_json::from_str(&std::fs::read_to_string(file)?)?;
            req.as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("invalid JSON object"))?
                .entry("idempotency_key")
                .or_insert_with(|| json!(key(None)));
            (
                if s.is_some() {
                    Method::PATCH
                } else {
                    Method::POST
                },
                s.map(|s| format!("/api/v1/upstreams/{s}"))
                    .unwrap_or("/api/v1/upstreams".into()),
                Some(req),
            )
        }
        RetrievalCommands::Query {
            query,
            source,
            approve_upstream,
            limit,
            idempotency_key,
        } => (
            Method::POST,
            "/api/v1/retrieval".into(),
            Some(
                json!({"query":query,"source_ids":source,"approve_upstream":approve_upstream,"limit":limit,"idempotency_key":key(idempotency_key)}),
            ),
        ),
        RetrievalCommands::Refresh {
            id: s,
            query,
            approve_upstream,
            idempotency_key,
        } => {
            id(&s)?;
            (
                Method::POST,
                format!("/api/v1/upstreams/{s}/refresh"),
                Some(
                    json!({"query":query,"approve_upstream":approve_upstream,"idempotency_key":key(idempotency_key)}),
                ),
            )
        }
    };
    client.call(m, &p, b).await
}
