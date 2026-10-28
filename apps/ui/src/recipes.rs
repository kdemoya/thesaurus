use super::workflows::{Pending, action, chosen_project};
use super::*;

#[component]
pub(super) fn Recipes(
    snap: Snap,
    notice: RwSignal<String>,
    view: RwSignal<String>,
) -> impl IntoView {
    let project = RwSignal::new(String::new());
    let records = RwSignal::new(Vec::<WorkRecipe>::new());
    let chosen = RwSignal::new(String::new());
    let tick = RwSignal::new(0u64);
    let epoch = RwSignal::new(0u64);
    Effect::new(move |_| {
        let id = chosen_project(snap, project)
            .map(|p| p.id)
            .unwrap_or_default();
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        epoch.update(|n| *n += 1);
        let e = epoch.get_untracked();
        if id.is_empty() {
            records.set(vec![]);
            return;
        }
        spawn_local(async move {
            match api(&format!("/api/v1/projects/{id}/recipes"), "GET", None).await {
                Ok(v) => {
                    if epoch.try_get_untracked() == Some(e) {
                        match serde_json::from_value::<Vec<WorkRecipe>>(v) {
                            Ok(r) => {
                                if serde_json::to_value(&r).ok()
                                    != serde_json::to_value(records.get_untracked()).ok()
                                {
                                    records.set(r);
                                }
                            }
                            Err(_) => notice.set("Invalid recipes response".into()),
                        }
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    });
    // Keep the editor mounted while unrelated live workspace events arrive.
    let editor_key = Memo::new(move |_| {
        let p = chosen_project(snap, project).map(|p| (p.id, p.name, p.source_ids));
        let id = chosen.get();
        let r = records.get().into_iter().find(|r| r.id == id);
        serde_json::to_string(&(p, id, r)).unwrap_or_default()
    });
    view! {
        <div class="focus-toolbar"><label>"Project"<select prop:value=move||chosen_project(snap,project).map(|p|p.id).unwrap_or_default() on:change=move|ev|{project.set(event_target_value(&ev));chosen.set(String::new());}>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select></label><button class="button secondary" on:click=move|_|chosen.set(String::new())>"New recipe"</button><button class="button text" on:click=move|_|tick.update(|n|*n+=1)>"Refresh"</button></div>
        <div class="brief-layout"><aside class="brief-sidebar"><section class="panel"><div class="section-heading"><h2>"Saved analyses"</h2></div>{move||records.get().into_iter().map(|r|{let id=r.id.clone();let active=r.id;view!{<button class="brief-project" class:active=move||chosen.get()==active on:click=move|_|chosen.set(id.clone())><span>{r.name}</span><small>{format!("v{}{}",r.version,if r.archived{" · archived"}else{""})}</small></button>}}).collect_view()}</section></aside>
        <div class="brief-main">{move||{let _=editor_key.get();untrack(move||chosen_project(snap,project)).map(|p|{let current=records.get_untracked().into_iter().find(|r|r.id==chosen.get_untracked());view!{<RecipeEditor recipe=current project=p snap=snap notice=notice chosen=chosen tick=tick view=view/>}})}}</div></div>
        <Show when=move||chosen_project(snap,project).is_none()><p class="muted">"Create a project in Briefs before saving an analysis."</p></Show>
    }
}
#[component]
fn RecipeEditor(
    recipe: Option<WorkRecipe>,
    project: ProjectBrief,
    snap: Snap,
    notice: RwSignal<String>,
    chosen: RwSignal<String>,
    tick: RwSignal<u64>,
    view: RwSignal<String>,
) -> impl IntoView {
    let name = RwSignal::new(recipe.as_ref().map(|r| r.name.clone()).unwrap_or_default());
    let template=RwSignal::new(recipe.as_ref().map(|r|r.template.clone()).unwrap_or("Summarize {{topic}} using the selected evidence. Include actions and unresolved questions.".into()));
    let inputs = RwSignal::new(
        recipe
            .as_ref()
            .map(|r| r.inputs.join(", "))
            .unwrap_or("topic".into()),
    );
    let connection = RwSignal::new(
        recipe
            .as_ref()
            .map(|r| r.connection.clone())
            .unwrap_or_else(|| {
                snap.get_untracked()
                    .and_then(|s| {
                        s.connections
                            .into_iter()
                            .find(|c| c.kind == "acp")
                            .map(|c| c.id)
                    })
                    .unwrap_or_default()
            }),
    );
    let format = RwSignal::new(
        recipe
            .as_ref()
            .map(|r| r.output_format.clone())
            .unwrap_or("markdown".into()),
    );
    let budget = RwSignal::new(recipe.as_ref().map(|r| r.character_budget).unwrap_or(8000));
    let max_runs = RwSignal::new(recipe.as_ref().map(|r| r.max_runs_per_day).unwrap_or(5));
    let archived = RwSignal::new(recipe.as_ref().is_some_and(|r| r.archived));
    let sources = RwSignal::new(
        recipe
            .as_ref()
            .map(|r| r.source_ids.clone())
            .unwrap_or(project.source_ids.clone()),
    );
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let preview = RwSignal::new(None::<RecipePreview>);
    let approved = RwSignal::new(false);
    let values = RwSignal::new(std::collections::BTreeMap::<String, String>::new());
    let runs = RwSignal::new(Vec::<RecipeRun>::new());
    let version = recipe.as_ref().map_or(0, |r| r.version);
    let id = recipe.as_ref().map(|r| r.id.clone());
    let load_id = id.clone();
    let save_id = id.clone();
    let preview_id = id.clone();
    let run_id = id.clone();
    if let Some(id) = load_id {
        spawn_local(async move {
            if let Ok(v) = api(&format!("/api/v1/recipes/{id}/runs"), "GET", None).await {
                if let Ok(r) = serde_json::from_value(v) {
                    if runs.try_get_untracked().is_some() {
                        runs.set(r)
                    }
                }
            }
        });
    }
    let project_id = project.id.clone();
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        action(
            save_id
                .as_ref()
                .map(|id| format!("/api/v1/recipes/{id}"))
                .unwrap_or("/api/v1/recipes".into()),
            if save_id.is_some() { "PATCH" } else { "POST" },
            json!({"project_id":project_id,"expected_version":version,"name":name.get_untracked(),"template":template.get_untracked(),"inputs":inputs.get_untracked().split(',').map(str::trim).filter(|s|!s.is_empty()).collect::<Vec<_>>(),"source_ids":sources.get_untracked(),"connection":connection.get_untracked(),"output_format":format.get_untracked(),"character_budget":budget.get_untracked(),"max_runs_per_day":max_runs.get_untracked(),"archived":archived.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                chosen.set(v["id"].as_str().unwrap_or_default().into());
                tick.update(|n| *n += 1);
            },
        );
    };
    let preview_action = move |_| {
        if busy.get_untracked() {
            return;
        }
        let Some(id) = preview_id.clone() else {
            return;
        };
        busy.set(true);
        approved.set(false);
        preview.set(None);
        let input = values.get_untracked();
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/recipes/{id}/preview"),
                "POST",
                Some(json!({"expected_version":version,"inputs":input})),
            )
            .await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match result {
                Ok(v) => match serde_json::from_value(v) {
                    Ok(p) => preview.set(Some(p)),
                    Err(_) => notice.set("Invalid recipe preview".into()),
                },
                Err(e) => notice.set(e),
            }
        });
    };
    view! {
        <section class="panel"><div class="section-heading"><h2>{if id.is_some(){"Edit recipe"}else{"New recipe"}}</h2><small>{format!("v{version} · {}",project.name)}</small></div><form on:submit=save><fieldset class="focus-form-fields" disabled=move||busy.get()>
            <label>"Name"<input required maxlength="200" prop:value=move||name.get() on:input=move|ev|name.set(event_target_value(&ev))/></label>
            <label>"Analysis template"<textarea required rows="5" maxlength="6000" prop:value=move||template.get() on:input=move|ev|template.set(event_target_value(&ev))></textarea></label><small class="muted">"Use {{input_name}} for editable inputs. Save edits before previewing a run."</small>
            <label>"Input names, comma separated"<input prop:value=move||inputs.get() on:input=move|ev|inputs.set(event_target_value(&ev))/></label>
            <label>"Agent connection"<select prop:value=move||connection.get() on:change=move|ev|connection.set(event_target_value(&ev))>{move||snap.get().map(|s|s.connections.into_iter().filter(|c|c.kind=="acp").map(|c|view!{<option value=c.id.clone()>{c.id.clone()}</option>}).collect_view())}</select></label>
            <label>"Output format"<select prop:value=move||format.get() on:change=move|ev|format.set(event_target_value(&ev))><option value="markdown">"Markdown"</option><option value="json">"JSON"</option></select></label>
            <label>"Prompt character budget"<input type="number" min="1000" max="14000" prop:value=move||budget.get() on:input=move|ev|budget.set(event_target_value(&ev).parse().unwrap_or(8000))/></label>
            <label>"Daily run limit"<input type="number" min="1" max="100" prop:value=move||max_runs.get() on:input=move|ev|max_runs.set(event_target_value(&ev).parse().unwrap_or(5))/></label>
            <fieldset class="runtime-sources"><legend>"Source scope"</legend>{project.source_ids.into_iter().map(|source|{let check=source.clone();let label=snap.get_untracked().and_then(|s|s.sources.into_iter().find(|s|s.id==source).map(|s|s.name)).unwrap_or(source.clone());view!{<label><input type="checkbox" prop:checked=move||sources.get().contains(&check) on:change=move|ev|{let checked=event_target_checked(&ev);sources.update(|s|{s.retain(|id|id!=&source);if checked{s.push(source.clone());}});}/>{label}</label>}}).collect_view()}</fieldset>
            <label><input type="checkbox" prop:checked=move||archived.get() on:change=move|ev|archived.set(event_target_checked(&ev))/>"Archived"</label><button type="submit" class="button secondary">"Save recipe revision"</button>
        </fieldset></form></section>
        {recipe.map(|r|view!{<section class="panel"><div class="section-heading"><h2>"Run saved revision"</h2></div><fieldset class="focus-form-fields" disabled=move||busy.get()>{r.inputs.into_iter().map(|key|{let read=key.clone();let write=key.clone();view!{<label>{key}<input maxlength="2000" prop:value=move||values.get().get(&read).cloned().unwrap_or_default() on:input=move|ev|{values.update(|v|{v.insert(write.clone(),event_target_value(&ev));});preview.set(None);approved.set(false);}/></label>}}).collect_view()}<button class="button secondary" disabled=r.archived on:click=preview_action>"Preview context and usage"</button></fieldset>
            {move||preview.get().map(|p|{let request_inputs=values.get_untracked();let id=run_id.clone().unwrap_or_default();view!{<p>{format!("{} · {} sources · ~{} tokens · {}/{} runs today",p.connection,p.sources.len(),p.approximate_tokens,p.runs_today,p.max_runs_per_day)}</p><p class="muted">{format!("{} documents / {} characters omitted. Token count is an estimate; provider billing applies.",p.omitted_documents,p.omitted_characters)}</p><p class="muted">{format!("{} live-only source(s) excluded from indexed context. Selected source IDs (local): {}. Only included sources enter the agent grant.",p.omitted_live_source_ids.len(),p.selected_source_ids.join(", "))}</p><details><summary>"Exact prompt"</summary><pre>{p.prompt}</pre></details><label><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>"Approve one use of this agent connection and selected source disclosure"</label><button class="button primary" disabled=move||busy.get()||!approved.get()||!p.external_sources_allowed||(p.runs_today>=p.max_runs_per_day) on:click=move|_|{action(format!("/api/v1/recipes/{id}/run"),"POST",json!({"expected_version":version,"inputs":request_inputs,"signature":p.signature,"approve_usage":approved.get_untracked()}),snap,notice,busy,pending,move|_|{approved.set(false);preview.set(None);view.set("activity".into());});}>"Run analysis"</button>}})}
        </section>})}
        <section class="panel"><div class="section-heading"><h2>"Run receipts"</h2></div>{move||runs.get().into_iter().map(|r|{let status=snap.get().and_then(|s|s.runtime_runs.into_iter().find(|run|run.id==r.run_id));view!{<details><summary>{format!("{} · recipe v{} · {}",r.created_at,r.recipe_version,status.as_ref().map(|s|s.status.as_str()).unwrap_or("saved"))}</summary><p>{format!("Run {} · {}",r.run_id,r.preview.connection)}</p><pre>{status.map(|s|s.output).unwrap_or_default()}</pre><details><summary>"Pinned context"</summary><pre>{r.preview.prompt}</pre></details></details>}}).collect_view()}</section>
    }
}
