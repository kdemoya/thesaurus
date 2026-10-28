use anyhow::Result;
use clap::Subcommand;
use serde_json::{Value, json};
use thesaurus_client::Client;
#[derive(Subcommand)]
pub enum CaptureCommands {
    Audio,
    AudioGet {
        id: String,
    },
    Transcribe {
        #[arg(long)]
        file: std::path::PathBuf,
    },
    Reason {
        id: String,
        #[arg(long)]
        connection: String,
        #[arg(long)]
        approve_usage: bool,
    },
    Windows,
    Sessions,
    Start {
        #[arg(long)]
        file: std::path::PathBuf,
    },
    Control {
        id: String,
        #[arg(long)]
        expected_version: i64,
        #[arg(value_parser=["pause","resume","stop"])]
        action: String,
        #[arg(long)]
        approve_capture: bool,
    },
    Frames {
        id: String,
        #[arg(long, default_value = "")]
        query: String,
    },
    Image {
        id: String,
    },
    DeleteFrame {
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
fn request(file: std::path::PathBuf) -> Result<Value> {
    if std::fs::metadata(&file)?.len() > 100000 {
        anyhow::bail!("invalid: request exceeds100KB");
    }
    let mut r: Value = serde_json::from_str(&std::fs::read_to_string(file)?)?;
    r.as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("invalid JSON request"))?
        .entry("idempotency_key")
        .or_insert_with(|| json!(uuid::Uuid::new_v4().to_string()));
    Ok(r)
}
pub async fn execute(client: &Client, cmd: CaptureCommands) -> Result<Value> {
    use reqwest::Method;
    let (m, p, b) = match cmd {
        CaptureCommands::Audio => (Method::GET, "/api/v1/capture/audio".into(), None),
        CaptureCommands::AudioGet { id: s } => {
            id(&s)?;
            (Method::GET, format!("/api/v1/capture/audio/{s}"), None)
        }
        CaptureCommands::Transcribe { file } => (
            Method::POST,
            "/api/v1/capture/audio".into(),
            Some(request(file)?),
        ),
        CaptureCommands::Reason {
            id: s,
            connection,
            approve_usage,
        } => {
            id(&s)?;
            (
                Method::POST,
                format!("/api/v1/capture/audio/{s}/reason"),
                Some(
                    json!({"connection":connection,"approve_usage":approve_usage,"idempotency_key":uuid::Uuid::new_v4().to_string()}),
                ),
            )
        }
        CaptureCommands::Windows => (Method::GET, "/api/v1/capture/windows".into(), None),
        CaptureCommands::Sessions => (Method::GET, "/api/v1/capture/sessions".into(), None),
        CaptureCommands::Start { file } => (
            Method::POST,
            "/api/v1/capture/sessions".into(),
            Some(request(file)?),
        ),
        CaptureCommands::Control {
            id: s,
            expected_version,
            action,
            approve_capture,
        } => {
            id(&s)?;
            (
                Method::POST,
                format!("/api/v1/capture/sessions/{s}/control"),
                Some(
                    json!({"expected_version":expected_version,"action":action,"approve_capture":approve_capture,"idempotency_key":uuid::Uuid::new_v4().to_string()}),
                ),
            )
        }
        CaptureCommands::Frames { id: s, query } => {
            id(&s)?;
            let mut u = reqwest::Url::parse(&format!(
                "http://localhost/api/v1/capture/sessions/{s}/frames"
            ))?;
            u.query_pairs_mut().append_pair("q", &query);
            (
                Method::GET,
                format!("{}?{}", u.path(), u.query().unwrap_or_default()),
                None,
            )
        }
        CaptureCommands::Image { id: s } => {
            id(&s)?;
            (
                Method::GET,
                format!("/api/v1/capture/frames/{s}/image"),
                None,
            )
        }
        CaptureCommands::DeleteFrame { id: s } => {
            id(&s)?;
            (Method::DELETE, format!("/api/v1/capture/frames/{s}"), None)
        }
    };
    client.call(m, &p, b).await
}
