use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum RepositoryCommands {
    List,
    Get {
        id: String,
    },
    /// Register/update selected local path, project, prefix selection and optional approved LSP service.
    Save {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        file: std::path::PathBuf,
    },
    Report {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long, default_value = "HEAD")]
        to_ref: String,
        #[arg(long)]
        advance_checkpoint: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Reports {
        id: String,
    },
    Symbols {
        id: String,
        file: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        line: u32,
        #[arg(long)]
        character: u32,
        #[arg(long,value_parser=["definition","references"],default_value="definition")]
        method: String,
        #[arg(long)]
        approve_service: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    SymbolHistory {
        id: String,
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
fn key(s: Option<String>) -> String {
    s.unwrap_or_else(|| uuid::Uuid::new_v4().to_string())
}
pub async fn execute(client: &Client, command: RepositoryCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match command {
        RepositoryCommands::List => (Method::GET, "/api/v1/repositories".into(), None),
        RepositoryCommands::Get { id: s } => {
            id(&s)?;
            (Method::GET, format!("/api/v1/repositories/{s}"), None)
        }
        RepositoryCommands::Save { id: s, file } => {
            if let Some(s) = &s {
                id(s)?;
            }
            if std::fs::metadata(&file)?.len() > 100000 {
                anyhow::bail!("invalid: repository request exceeds100KB");
            }
            let mut req: Value = serde_json::from_str(&std::fs::read_to_string(file)?)?;
            req.as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("invalid: expected JSON object"))?
                .entry("idempotency_key")
                .or_insert_with(|| json!(key(None)));
            (
                if s.is_some() {
                    Method::PATCH
                } else {
                    Method::POST
                },
                s.map(|s| format!("/api/v1/repositories/{s}"))
                    .unwrap_or("/api/v1/repositories".into()),
                Some(req),
            )
        }
        RepositoryCommands::Report {
            id: s,
            expected_version,
            to_ref,
            advance_checkpoint,
            idempotency_key,
        } => {
            id(&s)?;
            (
                Method::POST,
                format!("/api/v1/repositories/{s}/reports"),
                Some(
                    json!({"expected_version":expected_version,"to_ref":to_ref,"advance_checkpoint":advance_checkpoint,"idempotency_key":key(idempotency_key)}),
                ),
            )
        }
        RepositoryCommands::Reports { id: s } => {
            id(&s)?;
            (
                Method::GET,
                format!("/api/v1/repositories/{s}/reports"),
                None,
            )
        }
        RepositoryCommands::Symbols {
            id: s,
            file,
            expected_version,
            line,
            character,
            method,
            approve_service,
            idempotency_key,
        } => {
            id(&s)?;
            (
                Method::POST,
                format!("/api/v1/repositories/{s}/symbols"),
                Some(
                    json!({"expected_version":expected_version,"file":file,"line":line,"utf16_character":character,"method":method,"approve_service":approve_service,"idempotency_key":key(idempotency_key)}),
                ),
            )
        }
        RepositoryCommands::SymbolHistory { id: s } => {
            id(&s)?;
            (
                Method::GET,
                format!("/api/v1/repositories/{s}/symbols"),
                None,
            )
        }
    };
    client.call(method, &path, body).await
}
