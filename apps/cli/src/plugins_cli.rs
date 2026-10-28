use anyhow::{Context, Result, bail};
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::PathBuf;
use thesaurus_client::Client;
use thesaurus_contracts::*;

#[derive(Subcommand)]
pub enum PluginCommands {
    List,
    Get {
        id: String,
    },
    Install {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        program: PathBuf,
        #[arg(long = "arg", allow_hyphen_values = true)]
        args: Vec<String>,
        #[arg(long)]
        configuration: Option<PathBuf>,
        #[arg(long = "source")]
        sources: Vec<String>,
        #[arg(long = "project")]
        projects: Vec<String>,
        #[arg(long = "operation")]
        operations: Vec<String>,
        #[arg(long)]
        external: bool,
        #[arg(long = "connection")]
        connections: Vec<String>,
    },
    Enable {
        id: String,
    },
    Disable {
        id: String,
    },
    Configure {
        id: String,
        #[arg(long)]
        file: PathBuf,
    },
    Upgrade {
        id: String,
        #[arg(long)]
        manifest: Option<PathBuf>,
        #[arg(long)]
        program: Option<PathBuf>,
        #[arg(long = "arg", allow_hyphen_values = true)]
        args: Vec<String>,
        #[arg(long)]
        configuration: Option<PathBuf>,
    },
    Uninstall {
        id: String,
    },
    Deliveries {
        id: String,
    },
    NativeRequests {
        id: String,
    },
    Attempts {
        id: String,
        delivery: String,
    },
    Native {
        capability: String,
        command: String,
        #[arg(long)]
        input: Option<PathBuf>,
    },
    Retry {
        id: String,
        delivery: String,
    },
    Capabilities,
    /// Inspect declared prerequisites and recent checks without running code.
    Setup {
        id: String,
    },
    /// Explicitly queue a bounded read-only plugin setup check.
    CheckSetup {
        id: String,
        step: String,
    },
    /// Run a declared API 2 contribution with explicit input scope.
    Run {
        id: String,
        contribution: String,
        #[arg(long = "source")]
        sources: Vec<String>,
        #[arg(long = "project")]
        projects: Vec<String>,
        #[arg(long)]
        destination_project: Option<String>,
        #[arg(long,conflicts_with_all=["sources","projects","destination_project"])]
        eval_run: Option<String>,
        #[arg(long = "eval-case", requires = "eval_run")]
        eval_cases: Vec<String>,
        #[arg(long)]
        input: Option<PathBuf>,
    },
    Runs {
        id: String,
    },
    ModelGet {
        id: String,
        proposal: String,
    },
    ModelPreview {
        id: String,
        proposal: String,
        connection: String,
    },
    ModelReject {
        id: String,
        proposal: String,
    },
    /// Approve an exact reviewed prompt; output is returned to the plugin.
    ModelApprove {
        id: String,
        proposal: String,
        connection: String,
        #[arg(long)]
        signature: String,
        #[arg(long)]
        max_input_tokens: usize,
        #[arg(long)]
        approve_usage: bool,
        #[arg(long)]
        approve_result_delivery: bool,
        #[arg(long)]
        idempotency_key: Option<String>,
    },
    RunStatus {
        id: String,
        run: String,
    },
    CancelRun {
        id: String,
        run: String,
    },
    Select {
        capability: String,
        provider: String,
    },
}
fn read_json(file: PathBuf) -> Result<Value> {
    if std::fs::metadata(&file)?.len() > 256_000 {
        bail!("invalid: plugin file exceeds 256 KB");
    }
    Ok(serde_json::from_slice(&std::fs::read(file)?)?)
}
fn id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 120
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        bail!("invalid plugin identifier");
    }
    Ok(())
}
pub async fn execute(client: &Client, command: PluginCommands) -> Result<Value> {
    use reqwest::Method;
    let key = uuid::Uuid::new_v4().to_string();
    let (method, path, body) = match command {
        PluginCommands::Setup { id: s } => {
            id(&s)?;
            (Method::GET, format!("/api/v1/plugins/{s}/setup"), None)
        }
        PluginCommands::CheckSetup { id: s, step } => {
            id(&s)?;
            id(&step)?;
            let p: PluginRecord = serde_json::from_value(
                client
                    .call(Method::GET, &format!("/api/v1/plugins/{s}"), None)
                    .await?,
            )?;
            (
                Method::POST,
                format!("/api/v1/plugins/{s}/setup/{step}/check"),
                Some(json!({"expected_version":p.version,"idempotency_key":key})),
            )
        }
        PluginCommands::ModelGet { id: s, proposal } => {
            id(&s)?;
            id(&proposal)?;
            (
                Method::GET,
                format!("/api/v1/plugins/{s}/models/{proposal}"),
                None,
            )
        }
        PluginCommands::ModelPreview {
            id: s,
            proposal,
            connection,
        } => {
            id(&s)?;
            id(&proposal)?;
            (
                Method::POST,
                format!("/api/v1/plugins/{s}/models/{proposal}/preview"),
                Some(json!({"connection":connection})),
            )
        }
        PluginCommands::ModelReject { id: s, proposal } => {
            id(&s)?;
            id(&proposal)?;
            (
                Method::POST,
                format!("/api/v1/plugins/{s}/models/{proposal}/reject"),
                Some(json!({})),
            )
        }
        PluginCommands::ModelApprove {
            id: s,
            proposal,
            connection,
            signature,
            max_input_tokens,
            approve_usage,
            approve_result_delivery,
            idempotency_key,
        } => {
            id(&s)?;
            id(&proposal)?;
            (
                Method::POST,
                format!("/api/v1/plugins/{s}/models/{proposal}/approve"),
                Some(
                    json!({"connection":connection,"signature":signature,"max_approximate_input_tokens":max_input_tokens,"approve_usage":approve_usage,"approve_result_delivery":approve_result_delivery,"idempotency_key":idempotency_key.unwrap_or_else(||key.clone())}),
                ),
            )
        }
        PluginCommands::Run {
            id: s,
            contribution,
            sources,
            projects,
            destination_project,
            eval_run,
            eval_cases,
            input,
        } => {
            id(&s)?;
            id(&contribution)?;
            let current: PluginRecord = client
                .call(Method::GET, &format!("/api/v1/plugins/{s}"), None)
                .await?;
            (
                Method::POST,
                format!("/api/v1/plugins/{s}/runs"),
                Some(
                    json!({"expected_version":current.version,"contribution_id":contribution,"source_ids":sources,"project_ids":projects,"destination_project_id":destination_project,"evaluation":eval_run.map(|run_id|PluginEvaluationTarget{run_id,case_ids:eval_cases}),"input":input.map(read_json).transpose()?.unwrap_or(json!({})),"idempotency_key":key}),
                ),
            )
        }
        PluginCommands::Runs { id: s } => {
            id(&s)?;
            (Method::GET, format!("/api/v1/plugins/{s}/runs"), None)
        }
        PluginCommands::RunStatus { id: s, run } => {
            id(&s)?;
            id(&run)?;
            (Method::GET, format!("/api/v1/plugins/{s}/runs/{run}"), None)
        }
        PluginCommands::CancelRun { id: s, run } => {
            id(&s)?;
            id(&run)?;
            (
                Method::POST,
                format!("/api/v1/plugins/{s}/runs/{run}/cancel"),
                Some(json!({})),
            )
        }
        PluginCommands::List => (Method::GET, "/api/v1/plugins".into(), None),
        PluginCommands::Capabilities => (Method::GET, "/api/v1/plugin-capabilities".into(), None),
        PluginCommands::Attempts { id: s, delivery } => {
            id(&s)?;
            id(&delivery)?;
            (
                Method::GET,
                format!("/api/v1/plugins/{s}/deliveries/{delivery}/attempts"),
                None,
            )
        }
        PluginCommands::Native {
            capability,
            command,
            input,
        } => {
            id(&capability)?;
            id(&command)?;
            (
                Method::POST,
                format!("/api/v1/plugin-capabilities/{capability}/invoke"),
                Some(
                    json!({"command":command,"input":input.map(read_json).transpose()?.unwrap_or(json!({})),"idempotency_key":key}),
                ),
            )
        }
        PluginCommands::Select {
            capability,
            provider,
        } => {
            id(&capability)?;
            id(&provider)?;
            let current: Vec<PluginCapabilityStatus> = client
                .call(Method::GET, "/api/v1/plugin-capabilities", None)
                .await?;
            let selected = current
                .into_iter()
                .find(|c| c.capability == capability)
                .context("unknown capability")?;
            (
                Method::PATCH,
                format!("/api/v1/plugin-capabilities/{capability}"),
                Some(
                    json!({"expected_version":selected.selection_version,"provider_id":provider,"idempotency_key":key}),
                ),
            )
        }
        PluginCommands::Get { id: s } => {
            id(&s)?;
            (Method::GET, format!("/api/v1/plugins/{s}"), None)
        }
        PluginCommands::Deliveries { id: s } => {
            id(&s)?;
            (Method::GET, format!("/api/v1/plugins/{s}/deliveries"), None)
        }
        PluginCommands::NativeRequests { id: s } => {
            id(&s)?;
            (
                Method::GET,
                format!("/api/v1/plugins/{s}/native-requests"),
                None,
            )
        }
        PluginCommands::Retry { id: s, delivery } => {
            id(&s)?;
            id(&delivery)?;
            (
                Method::POST,
                format!("/api/v1/plugins/{s}/deliveries/{delivery}/retry"),
                Some(json!({})),
            )
        }
        PluginCommands::Install {
            manifest,
            program,
            args,
            configuration,
            sources,
            projects,
            operations,
            external,
            connections,
        } => {
            let manifest: PluginManifest = serde_json::from_value(read_json(manifest)?)?;
            let executable = PluginExecutable {
                program: std::fs::canonicalize(program)?
                    .to_string_lossy()
                    .into_owned(),
                args,
            };
            let configuration = configuration
                .map(read_json)
                .transpose()?
                .unwrap_or_else(|| json!({}));
            (
                Method::POST,
                "/api/v1/plugins".into(),
                Some(serde_json::to_value(PluginInstallRequest {
                    manifest,
                    executable,
                    configuration,
                    grant: PluginPermissions {
                        source_ids: sources,
                        project_ids: projects,
                        operations,
                        external,
                        connection_ids: connections,
                    },
                    idempotency_key: key,
                })?),
            )
        }
        command => {
            let (s, enabled, configuration, manifest, executable, uninstall) = match command {
                PluginCommands::Enable { id: s } => (s, Some(true), None, None, None, false),
                PluginCommands::Disable { id: s } => (s, Some(false), None, None, None, false),
                PluginCommands::Configure { id: s, file } => {
                    (s, None, Some(read_json(file)?), None, None, false)
                }
                PluginCommands::Upgrade {
                    id: s,
                    manifest,
                    program,
                    args,
                    configuration,
                } => {
                    let executable = program
                        .map(|p| {
                            Ok::<_, anyhow::Error>(PluginExecutable {
                                program: std::fs::canonicalize(p)?.to_string_lossy().into_owned(),
                                args,
                            })
                        })
                        .transpose()?;
                    (
                        s,
                        Some(false),
                        configuration.map(read_json).transpose()?,
                        manifest
                            .map(read_json)
                            .transpose()?
                            .map(serde_json::from_value::<PluginManifest>)
                            .transpose()?,
                        executable,
                        false,
                    )
                }
                PluginCommands::Uninstall { id: s } => (s, None, None, None, None, true),
                _ => unreachable!(),
            };
            id(&s)?;
            let p: PluginRecord = client
                .call(Method::GET, &format!("/api/v1/plugins/{s}"), None)
                .await
                .context("read plugin before updating")?;
            if uninstall {
                (
                    Method::DELETE,
                    format!("/api/v1/plugins/{s}"),
                    Some(json!({"expected_version":p.version,"idempotency_key":key})),
                )
            } else {
                (
                    Method::PATCH,
                    format!("/api/v1/plugins/{s}"),
                    Some(serde_json::to_value(PluginUpdateRequest {
                        expected_version: p.version,
                        enabled: enabled.unwrap_or(p.enabled),
                        configuration: configuration.unwrap_or(p.configuration),
                        grant: p.grant,
                        manifest,
                        executable,
                        idempotency_key: key,
                    })?),
                )
            }
        }
    };
    client.call(method, &path, body).await
}
