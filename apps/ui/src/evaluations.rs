use super::workflows::{Pending, action, chosen_project};
use super::*;
fn get(
    path: String,
    notice: RwSignal<String>,
    guard: RwSignal<bool>,
    after: impl FnOnce(Value) + 'static,
) {
    spawn_local(async move {
        let response = api(&path, "GET", None).await;
        if guard.try_get_untracked().is_none() {
            return;
        }
        match response {
            Ok(v) => after(v),
            Err(e) => notice.set(e),
        }
    });
}
fn pretty(v: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}
#[component]
pub(super) fn Evaluations(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let project = RwSignal::new(String::new());
    let suites = RwSignal::new(Vec::<EvalSuite>::new());
    let suite = RwSignal::new(None::<EvalSuite>);
    let name = RwSignal::new(String::new());
    let personal = RwSignal::new(false);
    let cases=RwSignal::new(r#"[{"id":"retrieval-1","name":"Find relevant project context","target":"retrieval","query":"project","expected_contains":["project"],"forbidden_contains":[],"required_evidence":[],"rubric":"Assess whether the returned context answers the query with source support.","replay_prompt":"Find the project context relevant to the query and cite the pinned evidence.","prompt_version":"context-v1","threshold":1.0}]"#.to_string());
    let baseline = RwSignal::new(String::new());
    let outputs = RwSignal::new("[]".to_string());
    let budget = RwSignal::new("0".to_string());
    let bytes = RwSignal::new("512000".to_string());
    let judge_requests = RwSignal::new("0".to_string());
    let judge_tokens = RwSignal::new("0".to_string());
    let history = RwSignal::new(Vec::<EvalRunSummary>::new());
    let run = RwSignal::new(None::<EvalRun>);
    let preview = RwSignal::new(String::new());
    let reviews = RwSignal::new(Vec::<EvalReview>::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let case_id = RwSignal::new(String::new());
    let score = RwSignal::new("1".to_string());
    let note = RwSignal::new(String::new());
    let judge_runtime = RwSignal::new(String::new());
    let connection = RwSignal::new(String::new());
    let judge_limit = RwSignal::new("20000".to_string());
    let approved = RwSignal::new(false);
    let judge_preview = RwSignal::new(None::<EvalJudgePreview>);
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        get(
            "/api/v1/evaluations/suites".into(),
            notice,
            busy,
            move |v| {
                if let Ok(s) = serde_json::from_value(v) {
                    suites.set(s);
                }
            },
        );
    });
    let load_history = move || {
        if let Some(p) = chosen_project(snap, project) {
            get(
                format!("/api/v1/projects/{}/evaluations", p.id),
                notice,
                busy,
                move |v| {
                    if let Ok(h) = serde_json::from_value(v) {
                        history.set(h);
                    }
                },
            );
        }
    };
    let execute = move |is_preview: bool| {
        let Some(s) = suite.get_untracked() else {
            notice.set("Save or select a suite first".into());
            return;
        };
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let out = match serde_json::from_str::<Vec<EvalOutput>>(&outputs.get_untracked()) {
            Ok(v) => v,
            Err(e) => {
                notice.set(format!("Invalid outputs JSON: {e}"));
                return;
            }
        };
        let cost = match budget.get_untracked().parse::<u64>() {
            Ok(v) => v,
            Err(_) => {
                notice.set("Invalid cost budget".into());
                return;
            }
        };
        let max_bytes = match bytes.get_untracked().parse::<usize>() {
            Ok(v) => v,
            Err(_) => {
                notice.set("Invalid output byte budget".into());
                return;
            }
        };
        let (max_judges, max_judge_tokens) = match (
            judge_requests.get_untracked().parse::<u32>(),
            judge_tokens.get_untracked().parse::<usize>(),
        ) {
            (Ok(a), Ok(b)) => (a, b),
            _ => {
                notice.set("Invalid saved judge budgets".into());
                return;
            }
        };
        let b = baseline.get_untracked();
        action(
            format!(
                "/api/v1/evaluations/suites/{}/{}",
                s.id,
                if is_preview { "preview" } else { "runs" }
            ),
            "POST",
            json!({"suite_version":s.version,"project_id":p.id,"baseline_id":if b.is_empty(){None}else{Some(b)},"outputs":out,"max_cases":50,"max_output_bytes":max_bytes,"max_cost_microunits":cost,"max_judge_requests":max_judges,"max_judge_input_tokens":max_judge_tokens}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if is_preview {
                    preview.set(pretty(&v));
                } else if let Ok(r) = serde_json::from_value::<EvalRun>(v) {
                    case_id.set(
                        r.results
                            .first()
                            .map(|c| c.case_id.clone())
                            .unwrap_or_default(),
                    );
                    run.set(Some(r));
                    reviews.set(vec![]);
                    preview.set(String::new());
                    load_history();
                }
            },
        );
    };
    view! {
        <div class="focus-toolbar"><label>"Project"</label><select prop:value=move||chosen_project(snap,project).map(|p|p.id).unwrap_or_default() on:change=move|ev|{project.set(event_target_value(&ev));history.set(vec![]);run.set(None);preview.set(String::new());baseline.set(String::new());}>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select><span class="muted">"Local evaluations · no model calls during deterministic runs"</span></div>
        <div class="focus-detail-columns">
            <section class="panel"><div class="section-heading"><h2>"Evaluation suites"</h2><span>"Versioned criteria"</span></div>
                <button class="button text" on:click=move|_|{suite.set(None);name.set(String::new());personal.set(false);}>"+ New suite"</button>
                {move||suites.get().into_iter().map(|s|{let label=format!("{} · v{} · {}",s.name,s.version,if s.project_id.is_some(){"project"}else{"personal"});view!{<button class="source-link" on:click=move|_|{name.set(s.name.clone());personal.set(s.project_id.is_none());cases.set(pretty(&s.cases));suite.set(Some(s.clone()));preview.set(String::new());}>{label}</button>}}).collect_view()}
                <form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let p=chosen_project(snap,project);let parsed=match serde_json::from_str::<Vec<EvalCase>>(&cases.get_untracked()){Ok(v)=>v,Err(e)=>{notice.set(format!("Invalid case JSON: {e}"));return;}};let old=suite.get_untracked();let scope=old.as_ref().map(|s|s.project_id.clone()).unwrap_or_else(||if personal.get_untracked(){None}else{p.map(|p|p.id)});if !personal.get_untracked() && scope.is_none(){notice.set("Choose a project first".into());return;}action(old.as_ref().map(|s|format!("/api/v1/evaluations/suites/{}",s.id)).unwrap_or("/api/v1/evaluations/suites".into()),if old.is_some(){"PATCH"}else{"POST"},json!({"expected_version":old.map(|s|s.version).unwrap_or(0),"name":name.get_untracked(),"project_id":scope,"cases":parsed}),snap,notice,busy,pending,move|v|{if let Ok(s)=serde_json::from_value(v){suite.set(Some(s));}});}>
                    <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Suite name"</label><input required maxlength="200" prop:value=move||name.get() on:input=move|ev|name.set(event_target_value(&ev))/><label><input type="checkbox" disabled=move||suite.get().is_some() prop:checked=move||personal.get() on:change=move|ev|personal.set(event_target_checked(&ev))/>" Personal reusable suite"</label>
                    <label>"Cases · JSON"</label><textarea rows="14" spellcheck="false" prop:value=move||cases.get() on:input=move|ev|cases.set(event_target_value(&ev))></textarea><p class="muted">"Targets: retrieval, memory, brief, priorities, supplied. Required text and forbidden text use exact case-sensitive matching. Evidence requires document_id, revision, line, and exact quote. Threshold: 0–1. Personal suites add no source access. Optional replay_prompt plus nonempty prompt_version enables separately approved ACP replays against saved snapshots."</p><button class="button primary" type="submit">"Save new version"</button></fieldset>
                </form>
            </section>
            <section class="panel"><div class="section-heading"><h2>"Run evaluation"</h2><span>{move||suite.get().map(|s|format!("{} · v{}",s.name,s.version)).unwrap_or("Choose a suite".into())}</span></div>
                <div class="workflow-form"><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Baseline run ID (optional)"</label><input prop:value=move||baseline.get() on:input=move|ev|baseline.set(event_target_value(&ev))/><label>"Maximum submitted cost (integer micro-units)"</label><input type="number" min="0" prop:value=move||budget.get() on:input=move|ev|budget.set(event_target_value(&ev))/><label>"Maximum output bytes (1–512000)"</label><input type="number" min="1" max="512000" prop:value=move||bytes.get() on:input=move|ev|bytes.set(event_target_value(&ev))/><label>"Maximum judge requests for this saved run (0 disables judges)"</label><input type="number" min="0" max="100" prop:value=move||judge_requests.get() on:input=move|ev|judge_requests.set(event_target_value(&ev))/><label>"Total estimated judge input token budget for this saved run"</label><input type="number" min="0" max="1000000" prop:value=move||judge_tokens.get() on:input=move|ev|judge_tokens.set(event_target_value(&ev))/><p class="muted">"Judge budgets are immutable after saving. Each separately approved request reserves one call and its input estimate atomically, including cancelled/failed attempts. These are request/estimate limits; ACP cannot enforce actual provider billing or output token limits."</p><label>"Supplied agent outputs · JSON array"</label><textarea rows="8" spellcheck="false" prop:value=move||outputs.get() on:input=move|ev|outputs.set(event_target_value(&ev))></textarea><p class="muted">"For supplied cases: case_id, text, evidence, agent, model, prompt_version, input_tokens, output_tokens, cost_microunits. Agent, model and prompt_version must be nonempty (use explicit unknown when needed). Provenance and usage are labeled user-reported. Local deterministic runs call no model."</p><div class="workflow-actions"><button class="button secondary" on:click=move|_|execute(true)>"Preview usage"</button><button class="button primary" on:click=move|_|execute(false)>"Run locally"</button></div></fieldset></div>
                <pre>{move||preview.get()}</pre><button class="button text" on:click=move|_|load_history()>"Load project run history"</button>
                {move||history.get().into_iter().map(|h|{let label=format!("{} · {}/{} pass · {} regressions · suite v{}",h.created_at,h.passed,h.cases,h.regressions,h.suite_version);view!{<button class="source-link" on:click=move|_|{get(format!("/api/v1/evaluations/runs/{}",h.id),notice,busy,move|v|{if let Ok(r)=serde_json::from_value::<EvalRun>(v){case_id.set(r.results.first().map(|c|c.case_id.clone()).unwrap_or_default());run.set(Some(r));reviews.set(vec![]);judge_preview.set(None);}});}>{label}</button>}}).collect_view()}
            </section>
        </div>
        {move||run.get().map(|r|view!{<section class="panel"><super::plugin_evaluation_results::PluginEvaluationResults receipts=r.plugin_checks.clone() notice=notice/><div class="section-heading"><h2>"Saved results"</h2><span>{format!("{} · {} · {} ms",r.id,r.runner_version,r.duration_ms)}</span></div><p class="muted">{format!("Pinned {} documents · {}",r.source_snapshots.len(),if r.results.iter().any(|c|c.output.usage_status.contains("unreported")||c.output.usage_status.is_empty()){ "Model token/cost usage unreported or legacy unknown; numeric totals are not comparable".to_string()}else{format!("{} input / {} output tokens · {} reported cost micro-units (supplied usage is self-reported)",r.input_tokens,r.output_tokens,r.cost_microunits)})}</p>
            {(!r.baseline_removed_cases.is_empty()).then(||view!{<p class="muted">{format!("Cases removed since baseline: {}",r.baseline_removed_cases.join(", "))}</p>})}{r.results.into_iter().map(|c|view!{<details class="focus-influence"><summary>{format!("{} · {:.2} / {:.2} · {}{}",c.name,c.score,c.threshold,if c.passed{"PASS"}else{"FAIL"},if c.regression{" · REGRESSION"}else{""})}</summary><p class="muted">{format!("Baseline {:?} · {} stale / {} invalid / {} duplicate citations",c.baseline_score,c.stale_evidence,c.invalid_evidence,c.duplicate_evidence)}</p>{c.comparison.into_iter().map(|x|view!{<p class="muted">{x}</p>}).collect_view()}{c.checks.into_iter().map(|x|view!{<p>{x}</p>}).collect_view()}<p class="muted">{format!("{} · agent {} {} · model {} · prompt {} / {} · runtime {:?} · {}",c.output.provenance_kind,c.output.agent,c.output.agent_version,c.output.model,c.output.prompt_version,c.output.prompt_hash,c.output.runtime_id,c.output.usage_status)}</p><pre style="white-space:pre-wrap;overflow-wrap:anywhere">{c.output.text}</pre></details>}).collect_view()}
        </section>})}
        <EvalReplayPanel run=run snap=snap notice=notice/>
        <Show when=move||run.get().is_some()><section class="panel"><div class="section-heading"><h2>"Human and model reviews"</h2><span>"Append-only · separate from deterministic score"</span></div><div class="workflow-form"><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Case"</label><select prop:value=move||case_id.get() on:change=move|ev|{case_id.set(event_target_value(&ev));judge_preview.set(None);approved.set(false);}>{move||run.get().map(|r|r.results.into_iter().map(|c|view!{<option value=c.case_id>{c.name}</option>}).collect_view())}</select><label>"Score 0–1"</label><input type="number" min="0" max="1" step="0.01" prop:value=move||score.get() on:input=move|ev|score.set(event_target_value(&ev))/><label>"Review note"</label><textarea rows="3" maxlength="4000" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))></textarea><label>"Completed judge runtime ID (optional)"</label><input prop:value=move||judge_runtime.get() on:input=move|ev|judge_runtime.set(event_target_value(&ev))/>
            <button class="button secondary" on:click=move|_|{let Some(r)=run.get_untracked() else{return;};let value=match score.get_untracked().parse::<f64>(){Ok(n)=>n,Err(_)=>{notice.set("Invalid review score".into());return;}};let runtime=judge_runtime.get_untracked();action(format!("/api/v1/evaluations/runs/{}/reviews",r.id),"POST",json!({"case_id":case_id.get_untracked(),"score":value,"note":note.get_untracked(),"judge_runtime_id":if runtime.is_empty(){None}else{Some(runtime)}}),snap,notice,busy,pending,move|_|notice.set("Review saved. Load reviews to inspect history.".into()));}>"Append review"</button>
            <button class="button text" on:click=move|_|{if let Some(r)=run.get_untracked(){get(format!("/api/v1/evaluations/runs/{}/reviews",r.id),notice,busy,move|v|{if let Ok(rows)=serde_json::from_value(v){reviews.set(rows);}});}}>"Load reviews"</button>
            <button class="button text" on:click=move|_|{if let Some(r)=run.get_untracked(){get(format!("/api/v1/evaluations/runs/{}/cases/{}/judge-preview",r.id,case_id.get_untracked()),notice,busy,move|v|{if let Ok(p)=serde_json::from_value(v){judge_preview.set(Some(p));approved.set(false);}});}}>"Prepare model judge"</button>
            {move||judge_preview.get().map(|p|view!{<p class="muted">{format!("≈{} input tokens · {} / {} requests reserved · {} input tokens remaining. {}",p.approximate_input_tokens,p.used_judge_requests,p.max_judge_requests,p.remaining_input_tokens,p.usage_note)}</p><details><summary>"Exact prompt"</summary><pre style="white-space:pre-wrap;overflow-wrap:anywhere">{p.prompt}</pre></details>})}
            <label>"ACP connection ID"</label><input prop:value=move||connection.get() on:input=move|ev|connection.set(event_target_value(&ev))/><label>"Maximum estimated input tokens"</label><input type="number" min="1" prop:value=move||judge_limit.get() on:input=move|ev|judge_limit.set(event_target_value(&ev))/><label><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>" Approve one request using this connection's existing account allowance"</label><button class="button primary" disabled=move||!approved.get() || judge_preview.get().is_none_or(|p|p.used_judge_requests>=p.max_judge_requests as i64 || p.approximate_input_tokens>p.remaining_input_tokens) on:click=move|_|{let Some(p)=judge_preview.get_untracked() else{return;};let limit=match judge_limit.get_untracked().parse::<usize>(){Ok(n)=>n,Err(_)=>{notice.set("Invalid judge estimate budget".into());return;}};action(format!("/api/v1/evaluations/runs/{}/cases/{}/judge",p.run_id,p.case_id),"POST",json!({"connection":connection.get_untracked(),"max_approximate_input_tokens":limit,"approve_usage":approved.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(r)=serde_json::from_value::<RuntimeRun>(v){judge_runtime.set(r.id.clone());notice.set(format!("Judge runtime {} started. Inspect it in Agents; append its review after completion.",r.id));approved.set(false);}});}>"Start selected judge"</button>
        </fieldset></div>{move||reviews.get().into_iter().map(|r|view!{<article class="focus-change"><strong>{format!("{} · {} · {:.2}",r.case_id,r.contributor,r.score)}</strong><p>{r.note}</p><small class="muted">{r.created_at}</small></article>}).collect_view()}</section></Show>
    }
}

#[component]
fn EvalReplayPanel(
    run: RwSignal<Option<EvalRun>>,
    snap: Snap,
    notice: RwSignal<String>,
) -> impl IntoView {
    let version = RwSignal::new(String::new());
    let case = RwSignal::new(String::new());
    let connection = RwSignal::new(String::new());
    let requests = RwSignal::new("1".to_string());
    let tokens = RwSignal::new("8000".to_string());
    let bytes = RwSignal::new("64000".to_string());
    let plan = RwSignal::new(None::<EvalReplayPlan>);
    let plan_id = RwSignal::new(String::new());
    let runtime_id = RwSignal::new(String::new());
    let runtime = RwSignal::new(None::<RuntimeRun>);
    let approved = RwSignal::new(false);
    let reviewed = RwSignal::new(false);
    let note = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let epoch = RwSignal::new(0u64);
    let parent = Memo::new(move |_| run.get().map(|r| r.id));
    Effect::new(move |_| {
        let _ = parent.get();
        if let Some(r) = run.get_untracked() {
            version.set(r.suite.version.to_string());
            case.set(
                r.suite
                    .cases
                    .first()
                    .map(|c| c.id.clone())
                    .unwrap_or_default(),
            );
        }
        plan.set(None);
        runtime.set(None);
        approved.set(false);
        reviewed.set(false);
        epoch.update(|n| *n += 1);
    });
    let prepare = move |preview: bool| {
        let Some(parent) = run.get_untracked() else {
            return;
        };
        let (version, request_count, input, output) = match (
            version.get_untracked().parse::<i64>(),
            requests.get_untracked().parse::<u32>(),
            tokens.get_untracked().parse::<usize>(),
            bytes.get_untracked().parse::<usize>(),
        ) {
            (Ok(a), Ok(b), Ok(c), Ok(d)) => (a, b, c, d),
            _ => {
                notice.set("Invalid replay version or budgets".into());
                return;
            }
        };
        let original = parent.id.clone();
        action(
            format!(
                "/api/v1/evaluations/runs/{}/{}",
                parent.id,
                if preview {
                    "replay-preview"
                } else {
                    "replay-plans"
                }
            ),
            "POST",
            json!({"suite_version":version,"case_ids":[case.get_untracked()],"connection":connection.get_untracked(),"max_requests":request_count,"max_input_tokens":input,"max_output_bytes":output}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if run
                    .try_get_untracked()
                    .flatten()
                    .is_none_or(|r| r.id != original)
                {
                    return;
                }
                if let Ok(p) = serde_json::from_value::<EvalReplayPlan>(v) {
                    plan_id.set(p.id.clone());
                    plan.set(Some(p));
                    runtime.set(None);
                    runtime_id.set(String::new());
                    approved.set(false);
                    reviewed.set(false);
                }
            },
        );
    };
    view! {<Show when=move||run.get().is_some()><section class="panel"><div class="section-heading"><h2>"Replay with an agent"</h2><span>"Pinned evidence · separately approved requests"</span></div><p class="muted">"Save replay_prompt and prompt_version on each case. Choose a saved suite version to compare a changed prompt, and an ACP connection to compare a different agent/model. The parent run's exact source snapshot is reused; replay offers no memory MCP or tools. Preview/save calls no model. ACP software runs with its existing local privileges."</p>
    <div class="workflow-form"><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Saved suite version"</label><input type="number" min="1" prop:value=move||version.get() on:input=move|ev|version.set(event_target_value(&ev))/><label>"Case ID in that version"</label><input prop:value=move||case.get() on:input=move|ev|case.set(event_target_value(&ev))/><label>"ACP connection / configured model"</label><select prop:value=move||connection.get() on:change=move|ev|connection.set(event_target_value(&ev))><option value="">"Choose connection"</option>{move||snap.get().map(|s|s.connections.into_iter().filter(|c|c.kind=="acp").map(|c|view!{<option value=c.id.clone()>{format!("{} · {}",c.id,if c.model.is_empty(){"agent default / unreported"}else{c.model.as_str()})}</option>}).collect_view())}</select><label>"Immutable maximum requests"</label><input type="number" min="1" max="100" prop:value=move||requests.get() on:input=move|ev|requests.set(event_target_value(&ev))/><label>"Immutable total estimated input tokens"</label><input type="number" min="1" max="1000000" prop:value=move||tokens.get() on:input=move|ev|tokens.set(event_target_value(&ev))/><label>"Maximum imported answer bytes"</label><input type="number" min="1" max="64000" prop:value=move||bytes.get() on:input=move|ev|bytes.set(event_target_value(&ev))/><div class="workflow-actions"><button class="button secondary" on:click=move|_|prepare(true)>"Preview exact replay"</button><button class="button secondary" on:click=move|_|prepare(false)>"Save replay plan"</button></div>
    <label>"Saved replay plan ID (restore after reconnect)"</label><input prop:value=move||plan_id.get() on:input=move|ev|plan_id.set(event_target_value(&ev))/><button class="button text" on:click=move|_|{let id=plan_id.get_untracked();if id.is_empty(){return;}epoch.update(|n|*n+=1);let ticket=epoch.get_untracked();spawn_local(async move{let response=api(&format!("/api/v1/evaluations/replay-plans/{id}"),"GET",None).await;if epoch.try_get_untracked()!=Some(ticket)||plan_id.try_get_untracked().as_deref()!=Some(id.as_str()){return;}match response{Ok(v)=>if let Ok(p)=serde_json::from_value(v){plan.set(Some(p));approved.set(false);runtime.set(None);},Err(e)=>notice.set(e)}});}>"Load saved plan"</button>
    </fieldset></div>
    {move||plan.get().map(|p|{let case_id=p.cases.first().map(|c|c.case_id.clone()).unwrap_or_default();let start_plan=p.clone();let start_case=case_id.clone();let finalize_plan=p.id.clone();let finalize_case=case_id.clone();view!{<p class="muted">{format!("Plan {} · parent {} · suite v{} · connection {} · configured model {} · config hash {} · {} / {} reserved · {} estimated input tokens remaining",if p.id.is_empty(){"preview (unsaved)"}else{&p.id},p.parent_run_id,p.suite.version,p.connection,p.configured_model,p.connection_config_hash,p.used_requests,p.max_requests,p.remaining_input_tokens)}</p><p class="muted">{p.usage_note}</p><details><summary>"Pinned source revisions and complete exact prompts"</summary><pre style="white-space:pre-wrap;overflow-wrap:anywhere">{pretty(&json!({"source_ids":p.source_ids,"source_snapshots":p.source_snapshots,"cases":p.cases}))}</pre></details>
    <label><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>" Approve this saved plan's exact selected-case prompt and existing account usage"</label><button class="button primary" disabled={move||busy.get()||!approved.get()||start_plan.id.is_empty()||start_plan.used_requests>=start_plan.max_requests as i64} on:click=move|_|{let Some(p)=plan.get_untracked()else{return;};let Some(c)=p.cases.iter().find(|c|c.case_id==start_case)else{return;};let expected=p.id.clone();action(format!("/api/v1/evaluations/replay-plans/{}/cases/{}/start",p.id,c.case_id),"POST",json!({"prompt_hash":c.prompt_hash,"max_approximate_input_tokens":c.approximate_input_tokens,"approve_usage":approved.get_untracked()}),snap,notice,busy,pending,move|v|{if plan.try_get_untracked().flatten().is_none_or(|p|p.id!=expected){return;}if let Ok(r)=serde_json::from_value::<RuntimeRun>(v){runtime_id.set(r.id.clone());runtime.set(Some(r));approved.set(false);reviewed.set(false);notice.set("Replay started. Inspect completion here or use Agents to cancel/respond to permissions.".into());}});}>"Start one selected case"</button>
    <div class="workflow-form"><label>"Runtime ID"</label><input prop:value=move||runtime_id.get() on:input=move|ev|{runtime_id.set(event_target_value(&ev));runtime.set(None);reviewed.set(false);}/><button class="button text" on:click=move|_|{let id=runtime_id.get_untracked();epoch.update(|n|*n+=1);let ticket=epoch.get_untracked();spawn_local(async move{let response=api(&format!("/api/v1/runtime/runs/{id}"),"GET",None).await;if epoch.try_get_untracked()!=Some(ticket)||runtime_id.try_get_untracked().as_deref()!=Some(id.as_str()){return;}match response{Ok(v)=>if let Ok(r)=serde_json::from_value(v){runtime.set(Some(r));reviewed.set(false);},Err(e)=>notice.set(e)}});}>"Inspect runtime and output"</button>{move||runtime.get().map(|r|view!{<pre style="white-space:pre-wrap;overflow-wrap:anywhere">{pretty(&r)}</pre>})}<label>"Output review note"</label><textarea rows="3" maxlength="4000" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))></textarea><label><input type="checkbox" prop:checked=move||reviewed.get() on:change=move|ev|reviewed.set(event_target_checked(&ev))/>" I inspected this completed output and approve importing it for deterministic scoring"</label><button class="button secondary" disabled=move||busy.get()||!reviewed.get()||runtime.get().is_none_or(|r|r.status!="completed") on:click=move|_|{action(format!("/api/v1/evaluations/replay-plans/{finalize_plan}/cases/{finalize_case}/finalize"),"POST",json!({"runtime_id":runtime_id.get_untracked(),"review_note":note.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(r)=serde_json::from_value::<EvalRun>(v){notice.set(format!("Replay scored against pinned snapshot; baseline {:?}",r.baseline_id));run.set(Some(r));}});}>"Import reviewed output and compare"</button></div>}})}
    </section></Show>}
}
