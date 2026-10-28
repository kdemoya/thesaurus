//! A small real connector and reviewed action contribution. The host imports
//! snapshots and creates previews; this executable never accepts tasks itself.
use std::{io::Read, path::Path};
use thesaurus_plugin_sdk::*;

fn read_notes(configuration: &serde_json::Value) -> Result<String, String> {
    let path = configuration["path"]
        .as_str()
        .ok_or("Configure an absolute notes file path")?;
    if !Path::new(path).is_absolute() {
        return Err("Choose an absolute notes file path".into());
    }
    let file =
        std::fs::File::open(path).map_err(|_| "Configured notes file could not be opened")?;
    if !file
        .metadata()
        .map_err(|_| "Notes file metadata unavailable")?
        .is_file()
    {
        return Err("Choose a regular notes file".into());
    }
    let mut body = String::new();
    file.take(64_001)
        .read_to_string(&mut body)
        .map_err(|_| "Notes must contain UTF-8 text")?;
    if body.trim().is_empty() || body.len() > 64_000 {
        return Err("Choose a nonempty notes file up to 64 KB".into());
    }
    Ok(body)
}
fn main() {
    if let Err(error) = serve_once(|invocation| {
        if invocation.api_version < 2
            || !matches!(invocation.method.as_str(), "contribution" | "readiness")
        {
            return Err("Contribution API 2 required".into());
        }
        let mut response = PluginResponse {
            api_version: invocation.api_version,
            invocation_id: invocation.invocation_id.clone(),
            ..Default::default()
        };
        if invocation.method == "readiness" {
            if invocation.input["step"]["id"] != "notes-file" {
                return Err("Unknown setup step".into());
            }
            let (state,detail)=match read_notes(&invocation.configuration) {
                Ok(_)=>(PluginSetupState::Ready,"Configured notes file is readable, nonempty UTF-8 within 64 KB. No source was imported".to_owned()),
                Err(reason)=>(PluginSetupState::NeedsSetup,reason),
            };
            response.readiness = Some(PluginReadinessResult {
                step_id: "notes-file".into(),
                state,
                detail,
            });
            return Ok(response);
        }
        let text = match invocation.input["contribution"]["id"].as_str() {
            Some("sync-file") => {
                let body = read_notes(&invocation.configuration)?;
                response.snapshots.push(PluginSourceSnapshot {
                    key: "primary".into(),
                    source_name: "Local notes file".into(),
                    kind: "markdown".into(),
                    title: invocation.configuration["title"]
                        .as_str()
                        .unwrap_or("Imported notes")
                        .into(),
                    body,
                });
                "Read the configured notes file. Its snapshot was submitted to the host for canonical import.".to_owned()
            }
            Some("review-tasks") => {
                let tasks: Vec<Task> = serde_json::from_value(invocation.input["tasks"].clone())
                    .map_err(|_| "Host task input unavailable")?;
                for task in tasks
                    .into_iter()
                    .filter(|t| t.status == "candidate")
                    .take(10)
                {
                    response.actions.push(PluginActionProposal {
                        provider: "task_status".into(),
                        target_id: task.id,
                        expected_version: task.version,
                        status: "accepted".into(),
                        rationale:
                            "An explicit action marker in the selected notes is ready for review."
                                .into(),
                        evidence: vec![task.evidence],
                    });
                }
                format!(
                    "Prepared {} task status previews. Each requires explicit review and approval.",
                    response.actions.len()
                )
            }
            Some("check-evidence") => {
                let input: PluginEvaluationInput =
                    serde_json::from_value(invocation.input["evaluation"].clone())
                        .map_err(|_| "Saved evaluation input unavailable")?;
                let docs: Vec<Document> =
                    serde_json::from_value(invocation.input["documents"].clone())
                        .map_err(|_| "Pinned evaluation sources unavailable")?;
                let minimum = invocation.configuration["minimum-evidence"]
                    .as_u64()
                    .unwrap_or(1);
                if !(1..=20).contains(&minimum) {
                    return Err("Minimum evidence must be between one and twenty".into());
                }
                for case in input.cases {
                    let mut seen = std::collections::HashSet::new();
                    let mut valid = vec![];
                    let mut invalid = 0;
                    for e in &case.result.output.evidence {
                        if e.line > 0
                            && docs.iter().any(|d| {
                                d.id == e.document_id
                                    && d.revision == e.revision
                                    && d.body.lines().nth(e.line as usize - 1)
                                        == Some(e.quote.as_str())
                            })
                        {
                            if seen.insert((
                                e.document_id.clone(),
                                e.revision,
                                e.line,
                                e.quote.clone(),
                            )) {
                                valid.push(e.clone());
                            }
                        } else {
                            invalid += 1;
                        }
                    }
                    let enough = valid.len() >= minimum as usize && invalid == 0;
                    response.checks.push(PluginEvaluationCheck{case_id:case.definition.id,check_id:"minimum-exact-evidence".into(),title:"Evidence coverage".into(),score:if enough{1.0}else{0.0},detail:format!("{} unique exact citations; {} invalid; minimum {}. This local check requires evidence even when the base case does not.",valid.len(),invalid,minimum),evidence:valid.into_iter().take(10).collect()});
                }
                format!(
                    "Returned {} local evidence coverage checks on the saved evaluation inputs.",
                    response.checks.len()
                )
            }
            _ => return Err("Unknown contribution".into()),
        };
        response.contribution = Some(PluginContributionOutput {
            text,
            ..Default::default()
        });
        Ok(response)
    }) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
