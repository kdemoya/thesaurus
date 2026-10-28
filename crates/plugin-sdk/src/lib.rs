//! Minimal Rust SDK for the versioned subprocess protocol. A handler reads one
//! bounded invocation and writes one response; application logs belong on stderr.
use std::io::{self, BufRead, Read, Write};
pub use thesaurus_contracts::*;

pub fn serve_once(
    handler: impl FnOnce(PluginInvocation) -> Result<PluginResponse, String>,
) -> Result<(), String> {
    let mut line = String::new();
    io::stdin()
        .lock()
        .take(512_001)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if line.len() > 512_000 || !line.ends_with('\n') {
        return Err("Invocation exceeds protocol bounds".into());
    }
    let invocation: PluginInvocation = serde_json::from_str(&line).map_err(|e| e.to_string())?;
    if !(PLUGIN_API_MIN_VERSION..=PLUGIN_API_VERSION).contains(&invocation.api_version) {
        return Err("Unsupported host API".into());
    }
    let id = invocation.invocation_id.clone();
    let api = invocation.api_version;
    let response = handler(invocation)?;
    if response.api_version != api || response.invocation_id != id {
        return Err("Response does not match invocation".into());
    }
    if api < 2
        && (response.contribution.is_some()
            || !response.snapshots.is_empty()
            || !response.actions.is_empty()
            || !response.checks.is_empty()
            || !response.model_requests.is_empty()
            || !response.native_commands.is_empty()
            || response.readiness.is_some())
    {
        return Err("Contribution output requires API 2".into());
    }
    let bytes = serde_json::to_vec(&response).map_err(|e| e.to_string())?;
    if bytes.len() > 128_000 {
        return Err("Response exceeds protocol bounds".into());
    }
    let mut out = io::stdout().lock();
    out.write_all(&bytes)
        .and_then(|_| out.write_all(b"\n"))
        .map_err(|e| e.to_string())
}
