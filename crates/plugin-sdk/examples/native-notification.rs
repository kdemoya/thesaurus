//! An explicitly granted plugin requests a local notification through the host
//! broker. It never requests OS permission or receives a native handle/token.
use thesaurus_plugin_sdk::*;
fn main() {
    if let Err(error) = serve_once(|v| {
        if v.api_version < 2 {
            return Err("Native requests require API 2".into());
        }
        let manual = v.method == "contribution";
        let id = v.input["contribution"]["id"].as_str().unwrap_or_default();
        let dismiss = manual && id == "dismiss";
        if manual && !matches!(id, "notify" | "dismiss") {
            return Err("Unknown native command contribution".into());
        }
        let document = if manual {
            v.input["documents"]
                .as_array()
                .and_then(|docs| docs.first())
                .cloned()
        } else {
            v.input.get("document").cloned()
        };
        let body = document
            .as_ref()
            .and_then(|d| d["body"].as_str())
            .unwrap_or("A synthetic plugin notification was requested.");
        let active = manual || body.contains("TODO") || body.contains("follow up");
        let mut response = PluginResponse {
            api_version: v.api_version,
            invocation_id: v.invocation_id,
            ..Default::default()
        };
        if active {
            response.native_commands.push(PluginNativeProposal { key: "follow-up".into(), capability: "native.notifications".into(), command: if dismiss {"dismiss"} else {"show"}.into(),
                input: if dismiss { serde_json::json!({}) } else { serde_json::json!({"title":"Transcript follow-up","body":body.chars().take(400).collect::<String>()}) } });
        }
        if manual {
            response.contribution = Some(PluginContributionOutput { text: "Native request proposed. Inspect Native requests for the provider result; acceptance is not OS completion.".into(), ..Default::default() });
        }
        Ok(response)
    }) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
