mod actions;
mod actions_cli;
mod annotations;
mod annotations_cli;
mod bragbook;
mod bragbook_cli;
mod capture;
mod capture_cli;
mod context_delivery;
mod context_delivery_cli;
mod continuity;
mod delivery;
mod delivery_cli;
mod discovery;
mod discovery_cli;
mod evaluations;
mod evaluations_cli;
mod exports;
mod focus;
mod focus_cli;
mod insights;
mod insights_cli;
mod knowledge;
mod knowledge_cli;
mod live_meetings;
mod memory_review;
mod memory_review_cli;
mod onboarding;
mod plugin_models;
mod plugins;
mod plugins_cli;
mod procedures;
mod procedures_cli;
mod recipes;
mod recipes_cli;
mod repository;
mod repository_cli;
mod retrieval;
mod retrieval_cli;
mod runtime;
mod server;
mod source_controls;
mod source_controls_cli;
mod usage_controls;
mod usage_controls_cli;
mod workflow_cli;
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};
use std::path::PathBuf;
use thesaurus_client::Client;
use thesaurus_contracts::*;
use thesaurus_core::{Config, data_dir, private_dir, private_write};

#[derive(Parser)]
#[command(
    name = "thesaurus",
    version,
    about = "Your work, in context. A local Rust assistant and shared agent memory."
)]
struct Cli {
    #[arg(long, global = true, env = "THESAURUS_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Bragbook {
        #[command(subcommand)]
        command: bragbook_cli::BragbookCommands,
    },
    Plugin {
        #[command(subcommand)]
        command: plugins_cli::PluginCommands,
    },
    Procedure {
        #[command(subcommand)]
        command: procedures_cli::ProcedureCommands,
    },
    Retrieve {
        #[command(subcommand)]
        command: retrieval_cli::RetrievalCommands,
    },
    Action {
        #[command(subcommand)]
        command: actions_cli::ActionCommands,
    },
    Capture {
        #[command(subcommand)]
        command: capture_cli::CaptureCommands,
    },
    Insights {
        #[command(subcommand)]
        command: insights_cli::InsightCommands,
    },
    Knowledge {
        #[command(subcommand)]
        command: knowledge_cli::KnowledgeCommands,
    },
    Usage {
        #[command(subcommand)]
        command: usage_controls_cli::UsageCommands,
    },
    Repo {
        #[command(subcommand)]
        command: repository_cli::RepositoryCommands,
    },
    Discovery {
        #[command(subcommand)]
        command: discovery_cli::DiscoveryCommands,
    },
    SourceControls {
        #[command(subcommand)]
        command: source_controls_cli::SourceControlCommands,
    },
    /// Budget context, record handoff acknowledgement and observe retained runs.
    Delivery {
        #[command(subcommand)]
        command: context_delivery_cli::DeliveryCommands,
    },
    Annotation {
        #[command(subcommand)]
        command: annotations_cli::AnnotationCommands,
    },
    MemoryReview {
        #[command(subcommand)]
        command: memory_review_cli::MemoryReviewCommands,
    },
    /// Export selected evidence, history and context to a portable file.
    Export {
        #[arg(long, required = true, value_delimiter = ',')]
        source: Vec<String>,
        #[arg(long,default_value="markdown",value_parser=["markdown","json"])]
        format: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Define personal/project evaluation suites and compare pinned runs.
    Eval {
        #[command(subcommand)]
        command: evaluations_cli::EvalCommands,
    },
    /// Inspect commitments, preserve promise changes and review delivery evidence.
    Commitment {
        #[command(subcommand)]
        command: delivery_cli::CommitmentCommands,
    },
    /// Save, preview and run reusable project analyses.
    Recipe {
        #[command(subcommand)]
        command: recipes_cli::RecipeCommands,
    },
    /// Create private local application data and an example connection config.
    Init,
    /// Run the persistent service. Both clients can connect independently.
    Serve {
        #[arg(long, default_value_t = 4620)]
        port: u16,
        #[arg(long, env = "THESAURUS_WEB_DIR")]
        web_dir: Option<PathBuf>,
    },
    /// Pair and open the browser. The pairing code expires in two minutes.
    Open {
        #[arg(long)]
        print: bool,
    },
    /// Inspect tool and service availability without reading credentials.
    Doctor,
    /// Negotiate ACP capabilities without making a model request.
    RuntimeProbe {
        connection: String,
    },
    /// Start a durable ACP run with source-scoped, read-only memory.
    RuntimeRun {
        connection: String,
        #[arg(long)]
        prompt: String,
        #[arg(long, required = true, value_delimiter = ',')]
        source: Vec<String>,
        #[arg(long)]
        approve_usage: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    RuntimeGet {
        id: String,
    },
    RuntimeCancel {
        id: String,
    },
    RuntimePermission {
        id: String,
        permission: String,
        option: String,
    },
    /// Create, generate and review living project briefs.
    Brief {
        #[command(subcommand)]
        command: BriefCommands,
    },
    /// Save work checkpoints, split/merge threads and resume through another agent.
    Thread {
        #[command(subcommand)]
        command: workflow_cli::ThreadCommands,
    },
    /// Link recurring meeting occurrences and prepare locally for the next one.
    Meeting {
        #[command(subcommand)]
        command: workflow_cli::MeetingCommands,
    },
    /// Review explainable project priorities and set importance preferences.
    Priority {
        #[command(subcommand)]
        command: focus_cli::PriorityCommands,
    },
    /// Track evidence-backed requests awaiting a response.
    Waiting {
        #[command(subcommand)]
        command: focus_cli::WaitingCommands,
    },
    /// Import a Markdown or transcript file. Reimporting creates a revision only if changed.
    Import {
        file: PathBuf,
        #[arg(long, default_value = "markdown")]
        kind: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        allow_external: bool,
    },
    /// Load a clearly labeled synthetic workspace.
    Demo,
    /// Search source evidence and print JSON.
    Search {
        query: String,
        #[arg(long)]
        source: Option<String>,
    },
    /// Read a document or observation by ID.
    Get {
        id: String,
        #[arg(long)]
        revision: Option<i64>,
    },
    /// Assemble a bounded, cited context bundle without model usage.
    Context {
        query: String,
        #[arg(long, default_value_t = 8000)]
        budget: usize,
    },
    /// Display current source and task state as JSON.
    Status,
    /// Manage separate, revocable access for existing agents.
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    /// Run an MCP stdio proxy using a scoped agent grant.
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
}
#[derive(Subcommand)]
enum BriefCommands {
    Sources {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long, required = true, value_delimiter = ',')]
        source: Vec<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    List,
    Get {
        id: String,
    },
    Create {
        name: String,
        #[arg(long, required = true, value_delimiter = ',')]
        source: Vec<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Generate {
        id: String,
        #[arg(long)]
        connection: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(long)]
        approve_usage: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    Review {
        id: String,
        #[arg(long)]
        revision: i64,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["accept","reject"])]
        decision: String,
    },
}
#[derive(Subcommand)]
enum AgentCommands {
    Add {
        name: String,
        #[arg(long, required = true, value_delimiter = ',')]
        source: Vec<String>,
        #[arg(long)]
        write: bool,
        #[arg(long)]
        local_agent: bool,
    },
    List,
    Revoke {
        id: String,
    },
}
#[derive(Subcommand)]
enum McpCommands {
    Serve {
        #[arg(long)]
        grant_file: PathBuf,
    },
}

fn print_json(value: impl serde::Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let dir = cli.data_dir.unwrap_or_else(data_dir);
    match cli.command {
        Commands::Init => {
            private_dir(&dir)?;
            let config = dir.join("config.toml");
            if !config.exists() {
                private_write(&config, include_bytes!("../../../examples/config.toml"))?;
            }
            println!("Initialized {}. Next: thesaurus serve", dir.display());
            Ok(())
        }
        Commands::Serve { port, web_dir } => {
            let web = web_dir
                .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../ui/dist"));
            server::serve(dir, port, web).await
        }
        Commands::Doctor => {
            let config = Config::load(&dir)?;
            let daemon = match Client::connect(&dir, None) {
                Ok(c) => c
                    .call::<Value>(reqwest::Method::GET, "/health", None)
                    .await
                    .is_ok(),
                Err(_) => false,
            };
            print_json(
                json!({"protocol_version":PROTOCOL_VERSION,"data_directory":dir,"daemon_ready":daemon,"configured_connections":thesaurus_core::models::connections(&config),"installed_agents":{"codex":thesaurus_core::models::on_path("codex"),"claude":thesaurus_core::models::on_path("claude")},"note":"Executable presence does not verify account access or model entitlement. No model requests were made."}),
            )
        }
        Commands::RuntimeProbe { connection } => {
            let config = Config::load(&dir)?;
            let c = config
                .connections
                .iter()
                .find(|c| c.id == connection && c.kind == "acp")
                .context("unknown ACP connection")?;
            let work = dir.join("runtime-work/probe");
            private_dir(&work)?;
            print_json(thesaurus_core::acp::probe(c, &work).await?)
        }
        Commands::Mcp {
            command: McpCommands::Serve { grant_file },
        } => {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::symlink_metadata(&grant_file)?;
            if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
                bail!("grant file must be a private regular file (0600)")
            }
            let grant: Value = serde_json::from_slice(&std::fs::read(grant_file)?)?;
            let token = grant["token"]
                .as_str()
                .context("invalid grant file")?
                .to_string();
            thesaurus_mcp::serve(Client::connect(&dir, Some(token))?).await
        }
        command => {
            let client = Client::connect(&dir, None)?;
            let value: Value = match command {
                Commands::Commitment{command}=>delivery_cli::run(&client,command).await?,
                Commands::Eval{command}=>evaluations_cli::execute(&client,command).await?,
                Commands::Export{source,format,output}=>{let v:Value=client.call(reqwest::Method::POST,"/api/v1/exports",Some(json!({"source_ids":source,"format":format}))).await?;if let Some(path)=output{let body=v["body"].as_str().context("invalid export response")?;private_write(&path,body.as_bytes())?;json!({"written":path,"watermark":v["watermark"],"bytes":body.len()})}else{v}},
                Commands::MemoryReview{command}=>memory_review_cli::run(&client,command).await?,
                Commands::Annotation{command}=>annotations_cli::run(&client,command).await?,
                Commands::Delivery{command}=>context_delivery_cli::execute(&client,command).await?,
                Commands::Discovery{command}=>discovery_cli::run(&client,command).await?,
                Commands::SourceControls{command}=>source_controls_cli::run(&client,command).await?,
                Commands::Repo{command}=>repository_cli::execute(&client,command).await?,
                Commands::Usage{command}=>usage_controls_cli::execute(&client,command).await?,
                Commands::Knowledge{command}=>knowledge_cli::run(&client,command).await?,
                Commands::Capture{command}=>capture_cli::execute(&client,command).await?,
                Commands::Plugin{command}=>plugins_cli::execute(&client,command).await?,
                Commands::Bragbook{command}=>bragbook_cli::run(&client,command).await?,
                Commands::Insights{command}=>insights_cli::run(&client,command).await?,
                Commands::Procedure{command}=>procedures_cli::run(&client,command).await?,
                Commands::Retrieve{command}=>retrieval_cli::execute(&client,command).await?,
                Commands::Action{command}=>actions_cli::execute(&client,command).await?,
                Commands::Recipe{command}=>recipes_cli::execute(&client,command).await?,
                Commands::Priority{command}=>focus_cli::priorities(&client,command).await?,
                Commands::Waiting{command}=>focus_cli::waiting(&client,command).await?,
                Commands::Thread{command}=>workflow_cli::thread(&client,command).await?,
                Commands::Meeting{command}=>workflow_cli::meeting(&client,command).await?,
                Commands::Brief { command } => match command {
                    BriefCommands::Sources{id,expected_version,source,idempotency_key}=>client.call(reqwest::Method::PATCH,&format!("/api/v1/projects/{id}/sources"),Some(json!({"expected_version":expected_version,"source_ids":source,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}))).await?,
                    BriefCommands::List=>client.call(reqwest::Method::GET,"/api/v1/briefs",None).await?,
                    BriefCommands::Get{id}=>client.call(reqwest::Method::GET,&format!("/api/v1/briefs/{id}"),None).await?,
                    BriefCommands::Create{name,source,idempotency_key}=>client.call(reqwest::Method::POST,"/api/v1/briefs",Some(json!({"name":name,"source_ids":source,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}))).await?,
                    BriefCommands::Generate{id,connection,expected_version,approve_usage,idempotency_key}=>client.call(reqwest::Method::POST,&format!("/api/v1/briefs/{id}/generate"),Some(json!({"connection":connection,"expected_version":expected_version,"approve_usage":approve_usage,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}))).await?,
                    BriefCommands::Review{id,revision,expected_version,decision}=>client.call(reqwest::Method::POST,&format!("/api/v1/briefs/{id}/review"),Some(json!({"revision":revision,"expected_version":expected_version,"decision":decision}))).await?,
                },
                Commands::RuntimeRun{connection,prompt,source,approve_usage,idempotency_key}=>client.call(reqwest::Method::POST,"/api/v1/runtime/runs",Some(json!({"connection":connection,"prompt":prompt,"source_ids":source,"approve_usage":approve_usage,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}))).await?,
                Commands::RuntimeGet{id}=>client.call(reqwest::Method::GET,&format!("/api/v1/runtime/runs/{id}"),None).await?,
                Commands::RuntimeCancel{id}=>client.call(reqwest::Method::POST,&format!("/api/v1/runtime/runs/{id}/cancel"),Some(json!({}))).await?,
                Commands::RuntimePermission{id,permission,option}=>client.call(reqwest::Method::POST,&format!("/api/v1/runtime/runs/{id}/permissions/{permission}"),Some(json!({"option_id":option}))).await?,
                Commands::Open { print } => {
                    let result: Value = client
                        .call(reqwest::Method::POST, "/api/v1/pairing", Some(json!({})))
                        .await?;
                    let url = result["url"].as_str().context("missing pairing URL")?;
                    if print {
                        println!("{url}")
                    } else {
                        client.call::<Value>(reqwest::Method::POST,"/api/v1/plugin-capabilities/native.browser/invoke",Some(json!({"command":"open","input":{"url":url},"idempotency_key":format!("browser-open:{}",uuid::Uuid::new_v4())}))).await?;
                    }
                    return Ok(());
                }
                Commands::Demo => {
                    client
                        .call(reqwest::Method::POST, "/api/v1/demo", Some(json!({})))
                        .await?
                }
                Commands::Import {
                    file,
                    kind,
                    title,
                    allow_external,
                } => {
                    let meta = std::fs::symlink_metadata(&file)?;
                    if !meta.is_file() || meta.len() > 200_000 {
                        bail!("select a regular UTF-8 file of at most 200 KB")
                    }
                    let path = file.canonicalize()?;
                    let name = title.unwrap_or_else(|| {
                        path.file_stem()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into()
                    });
                    let req = ImportRequest {
                        source_name: name.clone(),
                        kind,
                        title: name,
                        uri: format!("file://{}", path.display()),
                        body: std::fs::read_to_string(path)?,
                        allow_external,
                    };
                    client
                        .call(
                            reqwest::Method::POST,
                            "/api/v1/captures",
                            Some(serde_json::to_value(req)?),
                        )
                        .await?
                }
                Commands::Search { query, source } => {
                    client
                        .call(
                            reqwest::Method::POST,
                            "/api/v1/search",
                            Some(json!({"query":query,"source_id":source,"limit":20})),
                        )
                        .await?
                }
                Commands::Context { query, budget } => {
                    client
                        .call(
                            reqwest::Method::POST,
                            "/api/v1/context",
                            Some(json!({"query":query,"character_budget":budget})),
                        )
                        .await?
                }
                Commands::Get { id, revision } => {
                    if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                        bail!("invalid ID")
                    };
                    client
                        .call(
                            reqwest::Method::GET,
                            &format!(
                                "/api/v1/memory/{id}{}",
                                revision.map_or(String::new(), |r| format!("?revision={r}"))
                            ),
                            None,
                        )
                        .await?
                }
                Commands::Status => {
                    client
                        .call(reqwest::Method::GET, "/api/v1/snapshot", None)
                        .await?
                }
                Commands::Agent {
                    command: AgentCommands::List,
                } => {
                    client
                        .call(reqwest::Method::GET, "/api/v1/grants", None)
                        .await?
                }
                Commands::Agent {
                    command: AgentCommands::Revoke { id },
                } => {
                    client
                        .call(
                            reqwest::Method::POST,
                            &format!("/api/v1/grants/{id}/revoke"),
                            Some(json!({})),
                        )
                        .await?
                }
                Commands::Agent {
                    command:
                        AgentCommands::Add {
                            name,
                            source,
                            write,
                            local_agent,
                        },
                } => {
                    let result:Value=client.call(reqwest::Method::POST,"/api/v1/grants",Some(json!({"name":name,"source_ids":source,"allow_write":write,"allow_external":!local_agent}))).await?;
                    let grant_dir = dir.join("grants");
                    private_dir(&grant_dir)?;
                    let path = grant_dir.join(format!(
                        "{}.json",
                        result["grant"]["id"].as_str().context("missing grant ID")?
                    ));
                    private_write(&path, &serde_json::to_vec(&result)?)?;
                    json!({"grant":result["grant"],"grant_file":path,"mcpServers":{"thesaurus":{"command":std::env::current_exe()?,"args":["--data-dir",dir,"mcp","serve","--grant-file",path]}}})
                }
                _ => unreachable!(),
            };
            print_json(value)
        }
    }
}
