use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum EvalCommands {
    /// List personal and visible project suites.
    Suites,
    Suite {
        id: String,
        #[arg(long)]
        version: Option<i64>,
    },
    /// Create/update a suite using a JSON file containing EvalSuiteRequest.
    Save {
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        file: std::path::PathBuf,
    },
    /// Preview or execute using a JSON file containing EvalRunRequest.
    Run {
        suite: String,
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        preview: bool,
    },
    History {
        project: String,
    },
    Get {
        id: String,
    },
    Reviews {
        id: String,
    },
    /// Append human review or import score from an explicitly approved ACP judge run.
    Review {
        id: String,
        case: String,
        #[arg(long, default_value_t = 0.0)]
        score: f64,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long)]
        judge_runtime: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// Preview/save a replay plan from a pinned saved run. JSON file: EvalReplayPlanRequest.
    ReplayPlan {
        id: String,
        #[arg(long)]
        file: std::path::PathBuf,
        #[arg(long)]
        preview: bool,
    },
    ReplayGet {
        id: String,
    },
    /// Start exactly one approved case in a saved replay plan.
    ReplayStart {
        id: String,
        case: String,
        #[arg(long)]
        prompt_hash: String,
        #[arg(long)]
        max_input_tokens: usize,
        #[arg(long)]
        approve_usage: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// After inspecting the completed runtime, import and score its receipt-bound output.
    ReplayFinalize {
        id: String,
        case: String,
        #[arg(long)]
        runtime: String,
        #[arg(long)]
        review_note: String,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    /// Print exact prompt, token estimate and source scope for the reviewed Runtime flow.
    JudgePreview {
        id: String,
        case: String,
    },
    Judge {
        id: String,
        case: String,
        #[arg(long)]
        connection: String,
        #[arg(long)]
        max_input_tokens: usize,
        #[arg(long)]
        approve_usage: bool,
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
fn request(file: std::path::PathBuf) -> Result<Value> {
    let text = std::fs::read_to_string(file)?;
    if text.len() > 600000 {
        anyhow::bail!("evaluation request exceeds 600KB");
    }
    let mut v: Value = serde_json::from_str(&text)?;
    if let Some(map) = v.as_object_mut() {
        map.entry("idempotency_key")
            .or_insert_with(|| json!(uuid::Uuid::new_v4().to_string()));
    }
    Ok(v)
}
pub async fn execute(client: &Client, cmd: EvalCommands) -> Result<Value> {
    use reqwest::Method;
    let (method, path, body) = match cmd {
        EvalCommands::ReplayPlan {
            id: s,
            file,
            preview,
        } => {
            id(&s)?;
            (
                Method::POST,
                format!(
                    "/api/v1/evaluations/runs/{s}/{}",
                    if preview {
                        "replay-preview"
                    } else {
                        "replay-plans"
                    }
                ),
                Some(request(file)?),
            )
        }
        EvalCommands::ReplayGet { id: s } => {
            id(&s)?;
            (
                Method::GET,
                format!("/api/v1/evaluations/replay-plans/{s}"),
                None,
            )
        }
        EvalCommands::ReplayStart {
            id: s,
            case,
            prompt_hash,
            max_input_tokens,
            approve_usage,
            idempotency_key,
        } => {
            id(&s)?;
            id(&case)?;
            (
                Method::POST,
                format!("/api/v1/evaluations/replay-plans/{s}/cases/{case}/start"),
                Some(
                    json!({"prompt_hash":prompt_hash,"max_approximate_input_tokens":max_input_tokens,"approve_usage":approve_usage,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}),
                ),
            )
        }
        EvalCommands::ReplayFinalize {
            id: s,
            case,
            runtime,
            review_note,
            idempotency_key,
        } => {
            id(&s)?;
            id(&case)?;
            id(&runtime)?;
            (
                Method::POST,
                format!("/api/v1/evaluations/replay-plans/{s}/cases/{case}/finalize"),
                Some(
                    json!({"runtime_id":runtime,"review_note":review_note,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}),
                ),
            )
        }
        EvalCommands::Suites => (Method::GET, "/api/v1/evaluations/suites".into(), None),
        EvalCommands::Suite { id: s, version } => {
            id(&s)?;
            (
                Method::GET,
                format!(
                    "/api/v1/evaluations/suites/{s}{}",
                    version.map(|v| format!("?version={v}")).unwrap_or_default()
                ),
                None,
            )
        }
        EvalCommands::Save { id: s, file } => {
            if let Some(s) = &s {
                id(s)?;
            }
            (
                if s.is_some() {
                    Method::PATCH
                } else {
                    Method::POST
                },
                s.map(|s| format!("/api/v1/evaluations/suites/{s}"))
                    .unwrap_or("/api/v1/evaluations/suites".into()),
                Some(request(file)?),
            )
        }
        EvalCommands::Run {
            suite,
            file,
            preview,
        } => {
            id(&suite)?;
            (
                Method::POST,
                format!(
                    "/api/v1/evaluations/suites/{suite}/{}",
                    if preview { "preview" } else { "runs" }
                ),
                Some(request(file)?),
            )
        }
        EvalCommands::History { project } => {
            id(&project)?;
            (
                Method::GET,
                format!("/api/v1/projects/{project}/evaluations"),
                None,
            )
        }
        EvalCommands::Get { id: s } => {
            id(&s)?;
            (Method::GET, format!("/api/v1/evaluations/runs/{s}"), None)
        }
        EvalCommands::Reviews { id: s } => {
            id(&s)?;
            (
                Method::GET,
                format!("/api/v1/evaluations/runs/{s}/reviews"),
                None,
            )
        }
        EvalCommands::Review {
            id: s,
            case,
            score,
            note,
            judge_runtime,
            idempotency_key,
        } => {
            id(&s)?;
            id(&case)?;
            (
                Method::POST,
                format!("/api/v1/evaluations/runs/{s}/reviews"),
                Some(
                    json!({"case_id":case,"score":score,"note":note,"judge_runtime_id":judge_runtime,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}),
                ),
            )
        }
        EvalCommands::Judge {
            id: s,
            case,
            connection,
            max_input_tokens,
            approve_usage,
            idempotency_key,
        } => {
            id(&s)?;
            id(&case)?;
            (
                Method::POST,
                format!("/api/v1/evaluations/runs/{s}/cases/{case}/judge"),
                Some(
                    json!({"connection":connection,"max_approximate_input_tokens":max_input_tokens,"approve_usage":approve_usage,"idempotency_key":idempotency_key.unwrap_or_else(||uuid::Uuid::new_v4().to_string())}),
                ),
            )
        }
        EvalCommands::JudgePreview { id: s, case } => {
            id(&s)?;
            id(&case)?;
            (
                Method::GET,
                format!("/api/v1/evaluations/runs/{s}/cases/{case}/judge-preview"),
                None,
            )
        }
    };
    client.call(method, &path, body).await
}
