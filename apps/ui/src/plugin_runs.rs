use super::workflows::{Pending, action};
use super::*;

fn executable(kind: &PluginContributionKind) -> bool {
    matches!(
        kind,
        PluginContributionKind::Command
            | PluginContributionKind::Processor
            | PluginContributionKind::Panel
            | PluginContributionKind::Connector
            | PluginContributionKind::Action
            | PluginContributionKind::Evaluation
    )
}
fn refresh_runs(
    id: String,
    rows: RwSignal<Vec<PluginContributionRun>>,
    loading: RwSignal<bool>,
    error: RwSignal<String>,
    scope: RwSignal<Vec<String>>,
    evaluations: RwSignal<Vec<PluginEvaluationInputSummary>>,
) {
    if loading.get_untracked() {
        return;
    }
    loading.set(true);
    spawn_local(async move {
        let result = api(&format!("/api/v1/plugins/{id}/runs"), "GET", None)
            .await
            .and_then(|v| {
                serde_json::from_value(v).map_err(|_| "Invalid contribution history".into())
            });
        let current_scope = api(&format!("/api/v1/plugins/{id}/scope"), "GET", None)
            .await
            .and_then(|v| {
                serde_json::from_value::<Vec<String>>(v).map_err(|_| "Invalid plugin scope".into())
            });
        let inputs = api(
            &format!("/api/v1/plugins/{id}/evaluation-inputs"),
            "GET",
            None,
        )
        .await
        .and_then(|v| {
            serde_json::from_value::<Vec<PluginEvaluationInputSummary>>(v)
                .map_err(|_| "Invalid evaluation inputs".into())
        });
        if rows.try_get_untracked().is_none() {
            return;
        }
        loading.set(false);
        if let Ok(inputs) = inputs {
            evaluations.set(inputs);
        }
        if let Ok(ids) = current_scope {
            scope.set(ids);
        }
        match result {
            Ok(v) => {
                rows.set(v);
                error.set(String::new());
            }
            Err(e) => error.set(e),
        }
    });
}
#[component]
pub(super) fn PluginRuns(
    plugin: PluginRecord,
    snap: Snap,
    notice: RwSignal<String>,
) -> impl IntoView {
    let choices = plugin
        .manifest
        .contributions
        .iter()
        .filter(|c| executable(&c.kind))
        .cloned()
        .collect::<Vec<_>>();
    let contribution = RwSignal::new(choices.first().map(|c| c.id.clone()).unwrap_or_default());
    let sources = RwSignal::new(Vec::<String>::new());
    let scope = RwSignal::new(plugin.grant.source_ids.clone());
    let destination = RwSignal::new(String::new());
    let evaluation_targets = RwSignal::new(Vec::<PluginEvaluationInputSummary>::new());
    let evaluation_id = RwSignal::new(String::new());
    let eval_cases = RwSignal::new(Vec::<String>::new());
    let types = StoredValue::new(
        choices
            .iter()
            .map(|c| (c.id.clone(), c.kind.clone()))
            .collect::<Vec<_>>(),
    );
    let connector = move || {
        types.get_value().iter().any(|(id, kind)| {
            id == &contribution.get() && *kind == PluginContributionKind::Connector
        })
    };
    let is_evaluation = move || {
        types.get_value().iter().any(|(id, kind)| {
            id == &contribution.get() && *kind == PluginContributionKind::Evaluation
        })
    };
    let instructions = RwSignal::new(String::new());
    let model_connections = StoredValue::new(plugin.grant.connection_ids.clone());
    let rows = RwSignal::new(Vec::<PluginContributionRun>::new());
    let busy = RwSignal::new(false);
    let loading = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let pending = RwSignal::new(None::<Pending>);
    let tick = RwSignal::new(0u64);
    let load_id = plugin.id.clone();
    Effect::new(move |_| {
        let _ = tick.get();
        refresh_runs(
            load_id.clone(),
            rows,
            loading,
            error,
            scope,
            evaluation_targets,
        );
    });
    let poll_id = plugin.id.clone();
    let timer = gloo_timers::callback::Interval::new(1000, move || {
        if rows.get_untracked().iter().any(|r| {
            matches!(
                r.status.as_str(),
                "queued" | "running" | "cancelling" | "awaiting_model"
            )
        }) {
            refresh_runs(
                poll_id.clone(),
                rows,
                loading,
                error,
                scope,
                evaluation_targets,
            );
        }
    });
    let _timer = StoredValue::new_local(timer);
    let submit_id = plugin.id.clone();
    let refresh_id = plugin.id.clone();
    let external = plugin.grant.external;
    let granted_projects = StoredValue::new(plugin.grant.project_ids.clone());
    let enabled = plugin.enabled && plugin.manifest.api_max >= 2 && !choices.is_empty();
    view! {<details class="workflow-manage"><summary>"Contributions and run history"</summary>
        <p class="muted">"Choose a contribution and its input sources. Runs pin the selected document revisions. Connectors import owned sources; actions prepare previews for explicit review. Failed or interrupted runs require a new request."</p>
        <form class="workflow-form" on:submit=move|ev|{
            ev.prevent_default();
            let destination=if connector()&&!destination.get_untracked().is_empty(){Some(destination.get_untracked())}else{None};
            action(format!("/api/v1/plugins/{submit_id}/runs"),"POST",json!({"expected_version":plugin.version,"contribution_id":contribution.get_untracked(),"source_ids":if is_evaluation(){vec![]}else{sources.get_untracked()},"project_ids":[],"destination_project_id":destination,"evaluation":if is_evaluation(){Some(PluginEvaluationTarget{run_id:evaluation_id.get_untracked(),case_ids:eval_cases.get_untracked()})}else{None},"input":{"instructions":instructions.get_untracked()}}),snap,notice,busy,pending,move|_|tick.update(|v|*v+=1));
        }><fieldset disabled=move||busy.get()||!enabled>
            <label>"Contribution"<select required prop:value=move||contribution.get() on:change=move|ev|contribution.set(event_target_value(&ev))><option value="">"Choose contribution"</option>{choices.into_iter().map(|c|view!{<option value=c.id>{format!("{} · {:?}",c.title,c.kind)}</option>}).collect_view()}</select></label>
            <label>"Instructions (optional)"<textarea maxlength="16000" prop:value=move||instructions.get() on:input=move|ev|instructions.set(event_target_value(&ev))></textarea></label>
            <Show when=connector><label>"Import into project"<select aria-label="Import into project" prop:value=move||destination.get() on:change=move|ev|destination.set(event_target_value(&ev))><option value="">"Workspace only"</option>{move||snap.get().map(|s|s.briefs.into_iter().filter(|p|granted_projects.get_value().contains(&p.id)).map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select><small class="muted">"Imported sources start local-only. Choosing a destination does not send its documents to the connector."</small></label></Show>
            <Show when=is_evaluation><div><label>"Saved evaluation"<select aria-label="Saved evaluation" required prop:value=move||evaluation_id.get() on:change=move|ev|{evaluation_id.set(event_target_value(&ev));eval_cases.set(vec![]);}><option value="">"Choose a saved run"</option>{move||evaluation_targets.get().into_iter().map(|r|view!{<option value=r.run_id>{format!("{} v{} · {} · {} sources",r.suite_name,r.suite_version,r.created_at,r.source_count)}</option>}).collect_view()}</select></label><p class="muted">"The selected run's complete original source scope and selected outputs are supplied to this plugin. All sources must still be granted. This invokes the local check executable; it does not start a model."</p><fieldset class="runtime-sources"><legend>"Cases (all if none selected)"</legend>{move||evaluation_targets.get().into_iter().find(|r|r.run_id==evaluation_id.get()).map(|r|r.cases.into_iter().map(|c|{let read=c.id.clone();let id=c.id;view!{<label><input type="checkbox" prop:checked=move||eval_cases.get().contains(&read) on:change=move|ev|eval_cases.update(|cases|{cases.retain(|c|c!=&id);if event_target_checked(&ev){cases.push(id.clone());}})/>{c.name}</label>}}).collect_view())}</fieldset></div></Show>
            <Show when=move||!is_evaluation()><fieldset class="runtime-sources"><legend>"Input sources"</legend>{move||snap.get().map(|s|{
                let ids=scope.get();
                s.sources.into_iter().filter(|s|ids.contains(&s.id)&&(!external||s.allow_external)).map(|s|{let read=s.id.clone();let id=s.id;view!{<label><input type="checkbox" prop:checked=move||sources.get().contains(&read) on:change=move|ev|sources.update(|items|{items.retain(|v|v!=&id);if event_target_checked(&ev){items.push(id.clone());}})/>{s.name}</label>}}).collect_view()
            })}</fieldset></Show>
            <button class="button secondary" type="submit">"Run contribution"</button>
        </fieldset></form>
        <button class="source-link" disabled=move||loading.get() on:click=move|_|refresh_runs(refresh_id.clone(),rows,loading,error,scope,evaluation_targets)>"Refresh runs"</button>
        <Show when=move||!error.get().is_empty()><p role="alert">{move||error.get()}</p></Show>
        <For each=move||rows.get() key=|r|(r.id.clone(),r.status.clone(),r.finished_at.clone()) children=move|run|view!{<ContributionRun run=run connections=model_connections.get_value() snap=snap notice=notice tick=tick/>}/>
    </details>}
}
#[component]
pub(super) fn Citations(evidence: Vec<Evidence>, notice: RwSignal<String>) -> impl IntoView {
    view! {<div>{evidence.into_iter().map(|e|{let target=e.clone();view!{<blockquote><button class="source-link" on:click=move|_|crate::navigate_evidence(target.clone(),notice)>{format!("{} r{} line {}",e.document_id,e.revision,e.line)}</button><p class="workflow-body">{e.quote}</p></blockquote>}}).collect_view()}</div>}
}
#[component]
fn ContributionRun(
    run: PluginContributionRun,
    connections: Vec<String>,
    snap: Snap,
    notice: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let active = matches!(run.status.as_str(), "queued" | "running" | "awaiting_model");
    let path = format!("/api/v1/plugins/{}/runs/{}/cancel", run.plugin_id, run.id);
    view! {<article class="plugin-delivery"><h3>{format!("{} · {}",run.contribution.title,run.status)}</h3><small class="muted">{format!("{} · package {} · configuration v{}",run.created_at,run.package_version,run.configuration_version)}</small>
        {run.model_proposal_ids.iter().cloned().map(|id|view!{<super::plugin_model_review::PluginModelReview plugin=run.plugin_id.clone() id=id connections=connections.clone() snap=snap notice=notice/>}).collect_view()}
        {run.error.map(|e|view!{<p role="status">{e}</p>})}
        {run.output.map(|output|view!{<p class="workflow-body">{output.text}</p><Citations evidence=output.evidence notice=notice/>{output.panel.map(|panel|view!{<section aria-label=panel.title.clone()><h3>{panel.title.clone()}</h3>{panel.sections.into_iter().map(|s|view!{<section><h4>{s.heading}</h4><p class="workflow-body">{s.text}</p><Citations evidence=s.evidence notice=notice/></section>}).collect_view()}</section>})}})}
        {run.imported_documents.into_iter().map(|d|{let target=Evidence{document_id:d.document_id.clone(),revision:d.revision,line:1,quote:String::new()};view!{<p><button class="source-link" on:click=move|_|crate::navigate_evidence(target.clone(),notice)>{format!("{} r{} · {}",d.document_id,d.revision,if d.changed{"imported"}else{"unchanged"})}</button></p>}}).collect_view()}
        {run.action_preview_ids.into_iter().map(|id|view!{<super::plugin_action_review::PluginActionReview id=id snap=snap notice=notice/>}).collect_view()}
        <super::plugin_evaluation_results::PluginEvaluationResults receipts=run.evaluation_receipt.into_iter().collect() notice=notice/>
        {active.then(||view!{<button class="source-link" disabled=move||busy.get() on:click=move|_|action(path.clone(),"POST",json!({}),snap,notice,busy,pending,move|_|tick.update(|v|*v+=1))>"Cancel run"</button>})}
        <details><summary>"Run provenance"</summary><pre>{serde_json::to_string_pretty(&json!({"id":run.id,"api_version":run.api_version,"documents":run.documents,"tasks":run.tasks,"evaluation":run.evaluation,"evaluation_signature":run.evaluation_signature,"destination_project_id":run.destination_project_id,"source_ids":run.source_ids,"grant":run.grant,"configuration_hash":run.configuration_hash,"executable_definition_hash":run.executable_definition_hash})).unwrap_or_default()}</pre></details>
    </article>}
}
