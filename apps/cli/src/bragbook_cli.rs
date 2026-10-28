use anyhow::{Result, bail};
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::PathBuf;
use thesaurus_client::Client;

#[derive(Subcommand)]
pub enum BragbookCommands {
    List {
        #[arg(long,default_value="candidate",value_parser=["candidate","accepted","dismissed","all"])]
        status: String,
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    Collect,
    /// Review or edit an entry using a BragbookReview JSON file.
    Review {
        id: String,
        #[arg(long)]
        file: PathBuf,
    },
    Export {
        #[arg(long, required = true, value_delimiter = ',')]
        entry: Vec<String>,
        #[arg(long,default_value="markdown",value_parser=["markdown","json"])]
        format: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}
pub async fn run(client: &Client, command: BragbookCommands) -> Result<Value> {
    let value: Value = match command {
        BragbookCommands::List { status, offset } => {
            client
                .call(
                    reqwest::Method::GET,
                    &format!("/api/v1/bragbook?status={status}&offset={offset}"),
                    None,
                )
                .await?
        }
        BragbookCommands::Collect => {
            client
                .call(
                    reqwest::Method::POST,
                    "/api/v1/bragbook/collect",
                    Some(json!({})),
                )
                .await?
        }
        BragbookCommands::Review { id, file } => {
            if uuid::Uuid::parse_str(&id).is_err() {
                bail!("invalid entry ID");
            }
            let meta = std::fs::metadata(&file)?;
            if !meta.is_file() || meta.len() > 16000 {
                bail!("review input must be a regular file up to 16 KB");
            }
            let bytes = std::fs::read(file)?;
            if bytes.len() > 16000 {
                bail!("review file exceeds 16 KB");
            }
            let req: thesaurus_contracts::BragbookReview = serde_json::from_slice(&bytes)?;
            client
                .call(
                    reqwest::Method::PATCH,
                    &format!("/api/v1/bragbook/{id}"),
                    Some(serde_json::to_value(req)?),
                )
                .await?
        }
        BragbookCommands::Export {
            entry,
            format,
            output,
        } => {
            let value: Value = client
                .call(
                    reqwest::Method::POST,
                    "/api/v1/bragbook/export",
                    Some(json!({"entry_ids":entry,"format":format})),
                )
                .await?;
            if let Some(path) = output {
                thesaurus_core::private_write(
                    &path,
                    value["body"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("invalid export response"))?
                        .as_bytes(),
                )?;
                json!({"output":path,"entry_count":value["entry_count"]})
            } else {
                value
            }
        }
    };
    Ok(value)
}
