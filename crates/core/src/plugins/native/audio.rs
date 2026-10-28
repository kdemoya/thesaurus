use crate::capture::command;
use anyhow::{Context, Result, bail};
use std::path::PathBuf;
use thesaurus_contracts::*;
use tokio::process::Command;
pub fn recorder_path() -> Result<PathBuf> {
    if !cfg!(target_os = "macos") {
        bail!("unsupported: live meeting audio currently requires macOS");
    }
    let path = std::env::current_exe()?
        .parent()
        .context("invalid service path")?
        .join("thesaurus-audio");
    if !path.is_file() {
        bail!(
            "unsupported: build the native audio bridge with scripts/build-audio.sh, then restart the service"
        );
    }
    Ok(path)
}
pub async fn inputs() -> Result<MeetingAudioInputs> {
    let path = match recorder_path() {
        Ok(path) => path,
        Err(e) => {
            return Ok(MeetingAudioInputs {
                available: false,
                system_audio_available: false,
                microphone_permission: "unknown".into(),
                microphones: vec![],
                detail: e.to_string(),
            });
        }
    };
    let bytes = command(Command::new(path).arg("inputs"), 15, 100_000).await?;
    Ok(serde_json::from_slice(&bytes)?)
}
