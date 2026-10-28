//! Build: cargo build -p thesaurus-plugin-sdk --example transcript-review
//! Receives a policy-filtered transcript revision and proposes one attributed,
//! evidence-backed observation. It never invokes a model or accepts a task.
use serde_json::json;
use thesaurus_plugin_sdk::*;

fn main() {
    if let Err(error) = serve_once(|invocation| {
        let mut response = PluginResponse {
            api_version: invocation.api_version,
            invocation_id: invocation.invocation_id.clone(),
            result: json!({"reviewed":false}),
            observations: vec![],
            contribution: None,
            snapshots: vec![],
            actions: vec![],
            checks: vec![],
            model_requests: vec![],
            readiness: None,
            native_commands: vec![],
        };
        if invocation.method == "contribution" {
            let id = invocation.input["contribution"]["id"]
                .as_str()
                .unwrap_or_default();
            if !["review-followups", "summarize-followups", "followups-panel"].contains(&id) {
                return Err("Unknown contribution".into());
            }
            let documents: Vec<Document> =
                serde_json::from_value(invocation.input["documents"].clone())
                    .map_err(|e| e.to_string())?;
            let mut sections = vec![];
            let mut evidence = vec![];
            for document in documents.into_iter().take(10) {
                if let Some((index, line)) = document
                    .body
                    .lines()
                    .enumerate()
                    .find(|(_, line)| line.contains("TODO") || line.contains("follow up"))
                {
                    let citation = Evidence {
                        document_id: document.id,
                        revision: document.revision,
                        line: index as i64 + 1,
                        quote: line.into(),
                    };
                    let text = format!(
                        "Follow-up candidate for review: {}",
                        line.chars().take(1000).collect::<String>()
                    );
                    if id == "review-followups" {
                        response.observations.push(PluginObservationProposal {
                            body: text.clone(),
                            source_ids: vec![document.source_id],
                            evidence: vec![citation.clone()],
                        });
                    }
                    sections.push(PluginPanelSection {
                        heading: document.title.chars().take(120).collect(),
                        text,
                        evidence: vec![citation.clone()],
                    });
                    evidence.push(citation);
                }
            }
            response.result = serde_json::Value::Null;
            response.contribution = Some(PluginContributionOutput {
                text: format!(
                    "Found {} follow-up candidates in the pinned input revisions.{}",
                    sections.len(),
                    if id == "review-followups" && !sections.is_empty() {
                        " Attributed observations were proposed for review."
                    } else {
                        " No memory changes were requested."
                    }
                ),
                evidence,
                panel: (id == "followups-panel").then_some(PluginPanel {
                    title: "Transcript follow-ups".into(),
                    sections,
                }),
            });
            return Ok(response);
        }
        if invocation.method != "event" {
            return Ok(response);
        }
        let Some(document) = invocation.input.get("document") else {
            return Ok(response);
        };
        let document: Document =
            serde_json::from_value(document.clone()).map_err(|e| e.to_string())?;
        if let Some((index, line)) = document
            .body
            .lines()
            .enumerate()
            .find(|(_, line)| line.contains("TODO") || line.contains("follow up"))
        {
            response.observations.push(PluginObservationProposal {
                body: format!(
                    "Follow-up candidate for review: {}",
                    line.chars().take(1000).collect::<String>()
                ),
                source_ids: vec![document.source_id],
                evidence: vec![Evidence {
                    document_id: document.id,
                    revision: document.revision,
                    line: index as i64 + 1,
                    quote: line.into(),
                }],
            });
            response.result = json!({"reviewed":true});
        }
        Ok(response)
    }) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
