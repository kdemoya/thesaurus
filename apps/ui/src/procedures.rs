use super::workflows::{Pending, action, chosen_project};
use super::*;
use std::{cell::Cell, rc::Rc};
fn input_names(text: &str) -> String {
    let mut names = vec![];
    for part in text.split("{{").skip(1) {
        if let Some((key, _)) = part.split_once("}}") {
            if !key.is_empty()
                && key.chars().enumerate().all(|(i, c)| {
                    if i == 0 {
                        c.is_ascii_alphabetic()
                    } else {
                        c.is_ascii_alphanumeric() || c == '_'
                    }
                })
            {
                names.push(key.to_string());
            }
        }
    }
    names.sort();
    names.dedup();
    names.join(", ")
}
#[component]
pub(super) fn Procedures(
    snap: Snap,
    notice: RwSignal<String>,
    project: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let list = RwSignal::new(None::<ProcedurePage>);
    let reviews = RwSignal::new(None::<ProcedureReviewPage>);
    let current = RwSignal::new(String::new());
    let offset = RwSignal::new(0usize);
    let review_offset = RwSignal::new(0usize);
    let dismissed = RwSignal::new(false);
    let active = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let run = RwSignal::new(String::new());
    let label = RwSignal::new(String::new());
    let success = RwSignal::new(false);
    let outcome_note = RwSignal::new(String::new());
    let review_version = RwSignal::new(0i64);
    let run_detail = RwSignal::new(None::<RuntimeRun>);
    let run_detail_request = RwSignal::new(String::new());
    let name = RwSignal::new(None::<String>);
    let template = RwSignal::new(None::<String>);
    let inputs = RwSignal::new(None::<String>);
    let connection = RwSignal::new(None::<String>);
    let scope = RwSignal::new(None::<Vec<String>>);
    let format = RwSignal::new("markdown".to_string());
    let budget = RwSignal::new("6000".to_string());
    let maxruns = RwSignal::new("1".to_string());
    let review_note = RwSignal::new(String::new());
    let expected = RwSignal::new(None::<i64>);
    let editing = RwSignal::new(String::new());
    let acknowledged = RwSignal::new(false);
    let history = RwSignal::new(None::<ProcedureHistoryPage>);
    let history_offset = RwSignal::new(0usize);
    let history_id = RwSignal::new(String::new());
    let reset = move || {
        name.set(None);
        template.set(None);
        inputs.set(None);
        connection.set(None);
        scope.set(None);
        review_note.set(String::new());
        expected.set(None);
        acknowledged.set(false);
        pending.set(None);
    };
    let serial = Rc::new(Cell::new(0u64));
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let c = alive.clone();
    on_cleanup(move || c.store(false, std::sync::atomic::Ordering::Relaxed));
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        let start = offset.get();
        let rs = review_offset.get();
        let all = dismissed.get();
        let hid = history_id.get();
        let ho = history_offset.get();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let changed = active.get_untracked() != p.id;
        if changed {
            active.set(p.id.clone());
            list.set(None);
            reviews.set(None);
            current.set(String::new());
            offset.set(0);
            review_offset.set(0);
            run.set(String::new());
            label.set(String::new());
            success.set(false);
            outcome_note.set(String::new());
            review_version.set(0);
            run_detail.set(None);
            run_detail_request.set(String::new());
            history_id.set(String::new());
            history.set(None);
            reset();
        }
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        spawn_local(async move {
            if changed {
                let _ = api(
                    &format!("/api/v1/projects/{}/procedures/refresh", p.id),
                    "POST",
                    Some(json!({})),
                )
                .await;
            }
            let result = api(
                &format!(
                    "/api/v1/projects/{}/procedures?offset={}&limit=10&include_dismissed={all}",
                    p.id,
                    if changed { 0 } else { start }
                ),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<ProcedurePage>(v)
                    .map_err(|_| "Could not read procedure suggestions".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(v) => list.set(Some(v)),
                Err(e) => notice.set(e),
            }
            let result = api(
                &format!(
                    "/api/v1/projects/{}/procedure-reviews?offset={}&limit=25",
                    p.id,
                    if changed { 0 } else { rs }
                ),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<ProcedureReviewPage>(v)
                    .map_err(|_| "Could not read outcome reviews".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(v) => reviews.set(Some(v)),
                Err(e) => notice.set(e),
            }
            if !hid.is_empty() {
                let result = api(
                    &format!("/api/v1/procedures/{hid}/history?offset={ho}&limit=5"),
                    "GET",
                    None,
                )
                .await
                .and_then(|v| {
                    serde_json::from_value::<ProcedureHistoryPage>(v)
                        .map_err(|_| "Could not read suggestion history".into())
                });
                if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                    return;
                }
                match result {
                    Ok(v) => history.set(Some(v)),
                    Err(e) => notice.set(e),
                }
            }
        });
    });
    let chosen = move || {
        list.get().and_then(|p| {
            p.items
                .iter()
                .find(|s| s.id == current.get())
                .or_else(|| p.items.first())
                .cloned()
        })
    };
    Effect::new(move |_| {
        if let Some(s) = chosen() {
            if editing.get_untracked() != s.id {
                editing.set(s.id);
                reset();
            }
        }
    });
    let capture = move || {
        if expected.get_untracked().is_none() {
            expected.set(chosen().map(|s| s.version));
        }
    };
    let select_run = move |id: String| {
        run.set(id.clone());
        success.set(false);
        outcome_note.set(String::new());
        review_version.set(0);
        if let Some(r) = reviews
            .get_untracked()
            .and_then(|p| p.items.into_iter().find(|r| r.run_id == id))
        {
            label.set(r.procedure);
            success.set(r.successful);
            outcome_note.set(r.note);
            review_version.set(r.version);
        }
    };
    let review = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let pid = p.id.clone();
        action(
            "/api/v1/procedure-reviews".into(),
            "POST",
            json!({"project_id":p.id,"run_id":run.get_untracked(),"procedure":label.get_untracked(),"successful":success.get_untracked(),"expected_version":review_version.get_untracked(),"note":outcome_note.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if !chosen_project(snap, project).is_some_and(|p| p.id == pid) {
                    return;
                }
                review_version.set(v["version"].as_i64().unwrap_or(0));
                tick.update(|n| *n += 1);
            },
        );
    };
    let install = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if !acknowledged.get_untracked() {
            notice.set("Review the proposed recipe and supporting runs first".into());
            return;
        }
        let Some(s) = chosen() else {
            return;
        };
        let b = match budget.get_untracked().parse::<usize>() {
            Ok(n) => n,
            Err(_) => {
                notice.set("Choose a valid character budget".into());
                return;
            }
        };
        let max = match maxruns.get_untracked().parse::<i64>() {
            Ok(n) => n,
            Err(_) => {
                notice.set("Choose a valid daily run limit".into());
                return;
            }
        };
        let body_template = template.get_untracked().unwrap_or(s.template);
        let names = inputs
            .get_untracked()
            .unwrap_or_else(|| input_names(&body_template))
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect::<Vec<_>>();
        action(
            format!("/api/v1/procedures/{}/install", s.id),
            "POST",
            json!({"expected_version":expected.get_untracked().unwrap_or(s.version),"name":name.get_untracked().unwrap_or(s.name),"template":body_template,"inputs":names,"source_ids":scope.get_untracked().unwrap_or(s.source_ids),"connection":connection.get_untracked().unwrap_or(s.connection),"output_format":format.get_untracked(),"character_budget":b,"max_runs_per_day":max,"note":review_note.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                reset();
                notice.set(format!(
                    "Installed reviewed recipe {}. Open Recipes to preview and run it.",
                    v["recipe"]["id"].as_str().unwrap_or_default()
                ));
                tick.update(|n| *n += 1);
            },
        );
    };
    view! {<div class="brief-layout waiting-layout"><aside class="brief-sidebar"><section class="panel brief-create"><div class="section-heading"><h2>"Review a work outcome"</h2></div><p class="muted">"Agent completion is not proof of successful work. Review the output, then group successful runs under a shared procedure name."</p><form on:submit=review><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Completed run"<select required prop:value=move||run.get() on:change=move|ev|select_run(event_target_value(&ev))><option value="">"Choose run…"</option>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.runtime_runs.into_iter().filter(|r|r.status=="completed"&&!r.source_ids.is_empty()&&r.source_ids.iter().all(|id|p.source_ids.contains(id))).map(|r|view!{<option value=r.id.clone()>{format!("{} · {}",r.id,r.connection)}</option>}).collect_view())}</select></label>{move||snap.get().and_then(|s|s.runtime_runs.into_iter().find(|r|r.id==run.get())).map(|r|view!{<details><summary>"Inspect selected run output"</summary><pre>{r.output}</pre><details><summary>"Original prompt"</summary><pre>{r.prompt}</pre></details></details>})}<label>"Reusable procedure name"<input required maxlength="200" prop:value=move||label.get() on:input=move|ev|label.set(event_target_value(&ev)) placeholder="Weekly rollout review"/></label><label><input type="checkbox" prop:checked=move||success.get() on:change=move|ev|success.set(event_target_checked(&ev))/>"I reviewed this work as successful"</label><label>"Outcome and why it worked (or failed)"<textarea required maxlength="2000" prop:value=move||outcome_note.get() on:input=move|ev|outcome_note.set(event_target_value(&ev))></textarea></label><button type="submit" class="button primary">"Record outcome review"</button></fieldset></form></section></aside>
    <div class="brief-main"><section class="panel"><div class="section-heading"><h2>"Suggested reusable procedures"</h2><span>{move||list.get().map(|p|p.total.to_string()).unwrap_or_default()}</span></div><p class="muted">"At least two distinct successful outcome reviews are required. Suggestions create no files, install no agent skill, and execute nothing until you review a concrete saved recipe."</p><label><input type="checkbox" prop:checked=move||dismissed.get() on:change=move|ev|{dismissed.set(event_target_checked(&ev));offset.set(0);}/>"Include dismissed"</label>{move||list.get().map(|p|p.items.into_iter().map(|s|{let id=s.id;view!{<div class="memory-line"><button class="task-title" on:click=move|_|current.set(id.clone())>{s.name}</button><small>{format!("{} · {} reviewed runs{}",s.status,s.supporting_runs.len()+s.supporting_runs_omitted,if s.support_sufficient{""}else{" · support no longer sufficient"})}</small></div>}}).collect_view())}<Show when=move||list.get().is_some_and(|p|p.total==0)><p class="muted">"No repeated successful procedure yet. Review useful completed runs to build evidence."</p></Show><div class="workflow-actions"><button class="button text" disabled=move||offset.get()==0 on:click=move|_|offset.update(|n|*n=n.saturating_sub(10))>"← Previous"</button><button class="button text" disabled=move||list.get().is_none_or(|p|p.next_offset.is_none()) on:click=move|_|{if let Some(n)=list.get_untracked().and_then(|p|p.next_offset){offset.set(n);}}>"Next →"</button></div></section>
    {move||chosen().map(|s|{let decision=s.clone();let hist=s.id.clone();let can_install=s.status=="proposed"&&s.support_sufficient;let is_installed=s.status=="installed";view!{<section class="panel"><div class="section-heading"><h2>{s.name}</h2><span>{format!("{} · v{}",s.status,s.version)}</span></div><p>{s.explanation}</p><p class="muted">{format!("Suggested route: {} · {} support runs omitted",s.connection,s.supporting_runs_omitted)}</p>{s.recipe_id.map(|id|view!{<p>{format!("Installed recipe: {id} · manage it in Recipes")}</p>})}<h3>"Supporting successful runs"</h3>{s.supporting_runs.into_iter().map(|r|{let id=r.run_id.clone();view!{<details><summary>{format!("{} · {} · {}",r.run_id,r.connection,r.created_at)}</summary><p>{r.outcome_review}</p><pre>{r.output_excerpt}</pre><small>{if r.output_truncated{"Excerpt truncated; inspect full run before relying on it"}else{"Full output shown"}}</small><button class="button text" on:click=move|_|{let id=id.clone();let project_id=active.get_untracked();run_detail_request.set(id.clone());spawn_local(async move{let response=api(&format!("/api/v1/runtime/runs/{id}"),"GET",None).await.and_then(|v|serde_json::from_value::<RuntimeRun>(v).map_err(|_|"Could not read supporting run".into()));if run_detail_request.try_get_untracked().as_deref()!=Some(id.as_str())||active.try_get_untracked().as_deref()!=Some(project_id.as_str()){return;}match response{Ok(r)=>run_detail.set(Some(r)),Err(e)=>notice.set(e)}});}>"Inspect full supporting run"</button></details>}}).collect_view()}
    <label>"Review or dismissal note"<textarea maxlength="2000" disabled=move||busy.get() prop:value=move||review_note.get() on:input=move|ev|{capture();review_note.set(event_target_value(&ev));}></textarea></label><div class="workflow-actions"><button class="button text" disabled=move||busy.get()||is_installed on:click=move|_|{action(format!("/api/v1/procedures/{}",decision.id),"PATCH",json!({"expected_version":decision.version,"action":if decision.status=="dismissed"{"reopen"}else{"dismiss"},"note":review_note.get_untracked()}),snap,notice,busy,pending,move|_|{reset();tick.update(|n|*n+=1);});}>{if s.status=="dismissed"{"Reopen suggestion"}else{"Dismiss suggestion"}}</button><button class="button text" on:click=move|_|{history_id.set(hist.clone());history_offset.set(0);}>"Suggestion history"</button></div>
    <form on:submit=install><fieldset class="focus-form-fields" disabled=move||busy.get()||!can_install><label>"Reviewed recipe name"<input required maxlength="200" prop:value=move||name.get().unwrap_or_else(||chosen().map(|s|s.name).unwrap_or_default()) on:input=move|ev|{capture();name.set(Some(event_target_value(&ev)));}/></label><label>"Editable instruction template"<textarea required rows="8" maxlength="6000" prop:value=move||template.get().unwrap_or_else(||chosen().map(|s|s.template).unwrap_or_default()) on:input=move|ev|{capture();template.set(Some(event_target_value(&ev)));}></textarea></label><label>"Declared inputs (comma separated, {{name}} placeholders)"<input prop:value=move||inputs.get().unwrap_or_else(||input_names(&template.get().unwrap_or_else(||chosen().map(|s|s.template).unwrap_or_default()))) on:input=move|ev|{capture();inputs.set(Some(event_target_value(&ev)));}/></label><fieldset class="runtime-sources"><legend>"Source scope (historical support sources required)"</legend>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.sources.into_iter().filter(|s|p.source_ids.contains(&s.id)).map(|s|{let id=s.id.clone();let check=s.id.clone();let required=s.id;view!{<label><input type="checkbox" disabled=move||chosen().is_some_and(|s|s.source_ids.contains(&required)) prop:checked=move||scope.get().unwrap_or_else(||chosen().map(|s|s.source_ids).unwrap_or_default()).contains(&check) on:change=move|ev|{capture();let yes=event_target_checked(&ev);let mut ids=scope.get_untracked().unwrap_or_else(||chosen().map(|s|s.source_ids).unwrap_or_default());ids.retain(|s|s!=&id);if yes{ids.push(id.clone());}scope.set(Some(ids));}/>{s.name}</label>}}).collect_view())}</fieldset><label>"Reasoning route"<select prop:value=move||connection.get().unwrap_or_else(||chosen().map(|s|s.connection).unwrap_or_default()) on:change=move|ev|{capture();connection.set(Some(event_target_value(&ev)));}>{move||snap.get().map(|s|s.connections.into_iter().filter(|c|c.kind=="acp").map(|c|view!{<option value=c.id.clone()>{c.id.clone()}</option>}).collect_view())}</select></label><label>"Output format"<select prop:value=move||format.get() on:change=move|ev|{capture();format.set(event_target_value(&ev));}><option value="markdown">"Markdown"</option><option value="json">"JSON"</option></select></label><label>"Character budget"<input type="number" min="1000" max="14000" required prop:value=move||budget.get() on:input=move|ev|{capture();budget.set(event_target_value(&ev));}/></label><label>"Maximum runs per day"<input type="number" min="1" max="100" required prop:value=move||maxruns.get() on:input=move|ev|{capture();maxruns.set(event_target_value(&ev));}/></label><label>"Review note / dismissal reason"<textarea required maxlength="2000" prop:value=move||review_note.get() on:input=move|ev|{capture();review_note.set(event_target_value(&ev));}></textarea></label><label><input type="checkbox" required prop:checked=move||acknowledged.get() on:change=move|ev|acknowledged.set(event_target_checked(&ev))/>"I reviewed the instructions, sources, model route, budgets and supporting runs"</label><button type="submit" class="button primary">"Install reviewed recipe"</button><button type="button" class="button text" on:click=move|_|reset()>"Discard draft / use current version"</button></fieldset></form><p class="muted">"Installation saves a recipe only. Opening Recipes lets you preview its prompt and separately approve model usage."</p></section>}})}
    <section class="panel"><div class="section-heading"><h2>"Outcome review ledger"</h2></div>{move||reviews.get().map(|p|p.items.into_iter().map(|r|{let edit=r.clone();view!{<article class="observation"><button class="task-title" on:click=move|_|{run.set(edit.run_id.clone());label.set(edit.procedure.clone());success.set(edit.successful);outcome_note.set(edit.note.clone());review_version.set(edit.version);}>{format!("{} · {}",r.procedure,r.run_id)}</button><small>{format!("{} · v{} · {}",if r.successful{"Reviewed successful"}else{"Not marked successful"},r.version,r.contributor)}</small><p>{r.note}</p></article>}}).collect_view())}<div class="workflow-actions"><button class="button text" disabled=move||review_offset.get()==0 on:click=move|_|review_offset.update(|n|*n=n.saturating_sub(25))>"← Previous"</button><button class="button text" disabled=move||reviews.get().is_none_or(|p|p.next_offset.is_none()) on:click=move|_|{if let Some(n)=reviews.get_untracked().and_then(|p|p.next_offset){review_offset.set(n);}}>"Next →"</button></div></section>
    {move||run_detail.get().map(|r|view!{<section class="panel"><div class="section-heading"><h2>{format!("Supporting run {}",r.id)}</h2><button class="button text" on:click=move|_|{run_detail_request.set(String::new());run_detail.set(None);}>"Close"</button></div><p class="muted">{format!("{} · {}",r.connection,r.status)}</p><h3>"Original prompt"</h3><pre>{r.prompt}</pre><h3>"Original output"</h3><pre>{r.output}</pre></section>})}{move||history.get().map(|h|view!{<section class="panel"><div class="section-heading"><h2>"Procedure suggestion history"</h2></div>{h.items.into_iter().map(|e|view!{<details><summary>{format!("{} · {} · {}",e.action,e.contributor,e.created_at)}</summary><p>{e.note}</p><pre>{serde_json::to_string_pretty(&e.snapshot).unwrap_or_default()}</pre></details>}).collect_view()}<div class="workflow-actions"><button class="button text" disabled=move||history_offset.get()==0 on:click=move|_|history_offset.update(|n|*n=n.saturating_sub(5))>"← Newer"</button><button class="button text" disabled=h.next_offset.is_none() on:click=move|_|{if let Some(n)=h.next_offset{history_offset.set(n);}}>"Older →"</button></div></section>})}
    </div></div>}
}
