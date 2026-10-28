//! Uses a host-reviewed model response. This executable never opens a model
//! connection or reads API credentials itself.
use thesaurus_plugin_sdk::*;
fn main() {
    if let Err(error) = serve_once(|v| {
        let mut response = PluginResponse {
            api_version: v.api_version,
            invocation_id: v.invocation_id.clone(),
            ..Default::default()
        };
        if v.api_version < 2 {
            return Err("Model requests require API 2".into());
        }
        let results: Vec<PluginModelResult> = serde_json::from_value(
            v.input
                .get("models")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
        )
        .map_err(|_| "Invalid host model results")?;
        if results.is_empty() {
            response.model_requests.push(PluginModelRequest{key:"work-review".into(),prompt:"Read the pinned context and suggest one concise next action. Cite source IDs/revisions/lines in the text; do not execute it.".into(),max_output_tokens:256,max_output_bytes:8000});
            if v.method == "contribution" {
                response.contribution=Some(PluginContributionOutput{text:"Model request proposed. Review the selected connection and exact prompt before running it.".into(),..Default::default()});
            }
        } else {
            let result = &results[0];
            if v.method == "contribution" {
                response.contribution = Some(PluginContributionOutput {
                    text: format!(
                        "Reviewed model suggestion ({}):\n{}",
                        result.connection, result.output
                    ),
                    ..Default::default()
                });
            } else if let Some(d) = v.input.get("document").filter(|d| !d.is_null()) {
                let d: Document =
                    serde_json::from_value(d.clone()).map_err(|_| "Invalid pinned transcript")?;
                if let Some(line) = d.body.lines().next() {
                    response.observations.push(PluginObservationProposal {
                        body: format!("Model-assisted candidate for review: {}", result.output),
                        source_ids: vec![d.source_id],
                        evidence: vec![Evidence {
                            document_id: d.id,
                            revision: d.revision,
                            line: 1,
                            quote: line.into(),
                        }],
                    });
                }
            }
        }
        Ok(response)
    }) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
