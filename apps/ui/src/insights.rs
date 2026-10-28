use crate::{Snap, api, new_id, refresh};
use leptos::{prelude::*, task::spawn_local};
use serde_json::{Value, json};
use thesaurus_contracts::*;
#[path = "insights_capacity.rs"]
mod capacity;
use capacity::Capacity;
fn today() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default()
        .chars()
        .take(10)
        .collect()
}
fn days_ago(days: f64) -> String {
    js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(
        js_sys::Date::now() - days * 86400000.,
    ))
    .to_iso_string()
    .as_string()
    .unwrap_or_default()
    .chars()
    .take(10)
    .collect()
}
fn submit(
    path: String,
    method: &'static str,
    body: Value,
    busy: RwSignal<bool>,
    snap: Snap,
    notice: RwSignal<String>,
    reload: RwSignal<u64>,
) {
    if busy.get_untracked() {
        return;
    }
    busy.set(true);
    spawn_local(async move {
        let response = api(&path, method, Some(body)).await;
        if busy.try_get_untracked().is_none() {
            return;
        }
        busy.set(false);
        match response {
            Ok(_) => {
                reload.update(|n| *n += 1);
                refresh(snap, notice);
            }
            Err(e) => notice.set(e),
        }
    });
}
fn scoped_tasks(snap: Snap, project: &str) -> Vec<Task> {
    snap.get()
        .map(|s| {
            let ids = s
                .briefs
                .iter()
                .find(|p| p.id == project)
                .map(|p| p.source_ids.clone())
                .unwrap_or_default();
            let docs = s
                .documents
                .into_iter()
                .filter(|d| ids.contains(&d.source_id))
                .map(|d| d.id)
                .collect::<Vec<_>>();
            s.tasks
                .into_iter()
                .filter(|t| docs.contains(&t.evidence.document_id))
                .collect()
        })
        .unwrap_or_default()
}
#[component]
pub fn Insights(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let project = RwSignal::new(String::new());
    let mode = RwSignal::new("opportunities".to_string());
    Effect::new(move |_| {
        if project.get().is_empty() {
            if let Some(p) = snap.get().and_then(|s| s.briefs.first().cloned()) {
                project.set(p.id);
            }
        }
    });
    view! {<div class="focus-toolbar"><label for="insight-project">"Project"</label><select id="insight-project" prop:value=move||project.get() on:change=move|ev|project.set(event_target_value(&ev))>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select><div class="focus-tabs">{["opportunities","outcomes","capacity"].into_iter().map(|tab|view!{<button class:active=move||mode.get()==tab on:click=move|_|mode.set(tab.into())>{tab}</button>}).collect_view()}</div></div><Show when=move||project.get().is_empty()><p class="muted">"Create a project in Briefs to review its evidence, outcomes and capacity."</p></Show><For each=move||{let id=project.get();if id.is_empty(){vec![]}else{vec![id]}} key=|id|id.clone() children=move|id|{
        let opportunities=id.clone();let outcomes=id.clone();
        view!{
            <Show when=move||mode.get()=="opportunities"><Opportunities project=opportunities.clone() snap=snap notice=notice/></Show>
            <Show when=move||mode.get()=="outcomes"><Outcomes project=outcomes.clone() snap=snap notice=notice/></Show>
            <Show when=move||mode.get()=="capacity"><Capacity project=id.clone() snap=snap notice=notice/></Show>
        }
    }/>}
}
#[component]
fn EvidenceLines(evidence: Vec<Evidence>, notice: RwSignal<String>) -> impl IntoView {
    view! {<details class="brief-evidence"><summary>{format!("{} source lines",evidence.len())}</summary>{evidence.into_iter().map(|e|{let target=e.clone();view!{<blockquote><button class="source-link" on:click=move|_|crate::navigate_evidence(target.clone(),notice)>{format!("{} r{} line {}",e.document_id,e.revision,e.line)}</button><p class="workflow-body">{e.quote}</p></blockquote>}}).collect_view()}</details>}
}
#[component]
fn Review(
    path: String,
    version: i64,
    kind: &'static str,
    snap: Snap,
    notice: RwSignal<String>,
    reload: RwSignal<u64>,
) -> impl IntoView {
    let note = RwSignal::new(String::new());
    let independence = RwSignal::new(false);
    let decision = RwSignal::new(
        if kind == "opportunity" {
            "accepted"
        } else {
            "reviewed"
        }
        .to_string(),
    );
    let busy = RwSignal::new(false);
    let path = StoredValue::new(path);
    view! {<form class="workflow-form" on:submit=move|ev|{ev.prevent_default();submit(path.get_value(),"PATCH",json!({"expected_version":version,"decision":decision.get_untracked(),"note":note.get_untracked(),"attest_independent":kind=="opportunity" && independence.get_untracked(),"idempotency_key":new_id()}),busy,snap,notice,reload);}>
    <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Review decision"<select prop:value=move||decision.get() on:change=move|ev|decision.set(event_target_value(&ev))>{if kind=="opportunity"{vec!["accepted","rejected","candidate"]}else{vec!["reviewed","rejected","draft"]}.into_iter().map(|state|view!{<option value=state>{state}</option>}).collect_view()}</select></label><label>"Reason and unresolved concerns"</label><textarea required maxlength="2000" rows="2" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))></textarea><Show when=move||kind=="opportunity"><label class="focus-check"><input type="checkbox" prop:checked=move||independence.get() on:change=move|ev|independence.set(event_target_checked(&ev))/><span>"I reviewed the original evidence and counterevidence, and attest that the supporting reports have independent origins rather than copies of one report. Required to accept."</span></label></Show><button class="button secondary" type="submit">"Save user review"</button></fieldset></form>}
}
#[component]
fn Opportunities(project: String, snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let project = StoredValue::new(project);
    let topic = RwSignal::new(String::new());
    let title = RwSignal::new(String::new());
    let hypothesis = RwSignal::new(String::new());
    let evidence = RwSignal::new(Vec::<Evidence>::new());
    let counter = RwSignal::new(Vec::<Evidence>::new());
    let affected = RwSignal::new(Vec::<String>::new());
    let explanation = RwSignal::new(String::new());
    let records = RwSignal::new(Vec::<Opportunity>::new());
    let reload = RwSignal::new(0u64);
    let busy = RwSignal::new(false);
    let doc = RwSignal::new(String::new());
    let line = RwSignal::new("1".to_string());
    Effect::new(move |_| {
        let _ = reload.get();
        let _ = snap.get().map(|s| s.watermark);
        let id = project.get_value();
        spawn_local(async move {
            match api(&format!("/api/v1/projects/{id}/opportunities"), "GET", None).await {
                Ok(value) => {
                    if records.try_get_untracked().is_some() {
                        match serde_json::from_value(value) {
                            Ok(value) => records.set(value),
                            Err(_) => notice.set("Invalid opportunities".into()),
                        }
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    });
    let discover = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let id = project.get_value();
        let request = json!({"topic":topic.get_untracked()});
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/projects/{id}/opportunities/discover"),
                "POST",
                Some(request),
            )
            .await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match result {
                Ok(value) => match serde_json::from_value::<OpportunityDiscovery>(value) {
                    Ok(result) => {
                        explanation.set(format!(
                            "{} {} documents considered; {} lines omitted.",
                            result.explanation, result.documents_considered, result.lines_omitted
                        ));
                        if let Some(candidate) = result.candidate {
                            title.set(candidate.title);
                            hypothesis.set(candidate.hypothesis);
                            evidence.set(candidate.evidence);
                            counter.set(candidate.counterevidence);
                            affected.set(candidate.affected_task_ids);
                        } else {
                            notice.set("No provisional grouping spans two distinct source origins without obvious copies. Add exact evidence manually or choose another topic.".into());
                        }
                    }
                    Err(_) => notice.set("Invalid discovery response".into()),
                },
                Err(e) => notice.set(e),
            }
        });
    };
    let add = move |is_counter: bool| {
        let candidate = snap.get_untracked().and_then(|s| {
            s.documents
                .into_iter()
                .find(|d| d.id == doc.get_untracked())
        });
        let Some(document) = candidate else {
            notice.set("Select a source document".into());
            return;
        };
        let Ok(number) = line.get_untracked().parse::<usize>() else {
            notice.set("Enter a positive line number".into());
            return;
        };
        let Some(quote) = number
            .checked_sub(1)
            .and_then(|n| document.body.lines().nth(n))
            .filter(|q| q.len() <= 2000 && !q.trim().is_empty())
        else {
            notice.set("Choose a nonempty exact source line up to 2000 bytes".into());
            return;
        };
        let cite = Evidence {
            document_id: document.id,
            revision: document.revision,
            line: number as i64,
            quote: quote.into(),
        };
        if is_counter {
            counter.update(|v| v.push(cite));
        } else {
            evidence.update(|v| v.push(cite));
        }
    };
    view! {<section class="panel"><div class="section-heading"><h2>"Recurring friction"</h2><span class="muted">"reviewable hypotheses"</span></div><form class="workflow-actions" on:submit=discover><label>"Topic"<input required maxlength="100" prop:value=move||topic.get() on:input=move|ev|topic.set(event_target_value(&ev))/></label><button class="button secondary" disabled=move||busy.get()>"Find provisional evidence groups"</button></form><p class="muted">{move||explanation.get()}</p><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();submit(format!("/api/v1/projects/{}/opportunities",project.get_value()),"POST",json!({"title":title.get_untracked(),"hypothesis":hypothesis.get_untracked(),"evidence":evidence.get_untracked(),"counterevidence":counter.get_untracked(),"affected_task_ids":affected.get_untracked(),"idempotency_key":new_id()}),busy,snap,notice,reload);}><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Opportunity title"</label><input required maxlength="300" prop:value=move||title.get() on:input=move|ev|title.set(event_target_value(&ev))/><label>"Hypothesis to review"</label><textarea required rows="3" maxlength="4000" prop:value=move||hypothesis.get() on:input=move|ev|hypothesis.set(event_target_value(&ev))></textarea><div class="workflow-attach"><select aria-label="Evidence source document" prop:value=move||doc.get() on:change=move|ev|doc.set(event_target_value(&ev))><option value="">"Select a project document"</option>{move||snap.get().map(|s|{let ids=s.briefs.iter().find(|p|p.id==project.get_value()).map(|p|p.source_ids.clone()).unwrap_or_default();s.documents.into_iter().filter(|d|ids.contains(&d.source_id)).map(|d|view!{<option value=d.id>{d.title}</option>}).collect_view()})}</select><input aria-label="Evidence line" type="number" min="1" prop:value=move||line.get() on:input=move|ev|line.set(event_target_value(&ev))/><button type="button" class="button text" on:click=move|_|add(false)>"Add evidence"</button><button type="button" class="button text" on:click=move|_|add(true)>"Add counterevidence"</button></div><h3 class="focus-section-title">"Supporting evidence · distinct origins to verify"</h3>{move||evidence.get().into_iter().enumerate().map(|(i,e)|view!{<div class="source-gap"><span>{e.quote}</span><button class="button text" type="button" on:click=move|_|evidence.update(|v|{if i<v.len(){v.remove(i);}})>"Remove"</button></div>}).collect_view()}<h3 class="focus-section-title">"Counterevidence · inspect what weakens the hypothesis"</h3>{move||counter.get().into_iter().enumerate().map(|(i,e)|view!{<div class="source-gap"><span>{e.quote}</span><button class="button text" type="button" on:click=move|_|counter.update(|v|{if i<v.len(){v.remove(i);}})>"Remove"</button></div>}).collect_view()}<details><summary>"Affected work"</summary>{move||scoped_tasks(snap,&project.get_value()).into_iter().map(|t|{let id=t.id.clone();let checked=id.clone();view!{<label class="focus-check"><input type="checkbox" prop:checked=move||affected.get().contains(&checked) on:change=move|ev|{let yes=event_target_checked(&ev);affected.update(|ids|{ids.retain(|x|x!=&id);if yes{ids.push(id.clone());}});}/><span>{t.title}</span></label>}}).collect_view()}</details><button class="button primary" type="submit">"Save candidate for review"</button></fieldset></form></section>
        {move||records.get().into_iter().map(|record|{let unverified_origins=record.status=="accepted" && record.independence_review.is_none();let path=format!("/api/v1/projects/{}/opportunities/{}",project.get_value(),record.id);view!{<section class="panel insight-record"><div class="section-heading"><h2>{record.title}</h2><span class="muted">{format!("{} · v{}",record.status,record.version)}</span></div><p>{record.hypothesis}</p><Show when=move||unverified_origins><p class="muted">"Independent origins unverified: this older acceptance has no recorded independence attestation. Review the original sources and accept again with the explicit attestation."</p></Show><EvidenceLines notice=notice evidence=record.evidence/><h3 class="focus-section-title">"Counterevidence"</h3><EvidenceLines notice=notice evidence=record.counterevidence/><p class="muted">{format!("Affected task IDs: {}",record.affected_task_ids.join(", "))}</p>{record.review_note.map(|note|view!{<p class="muted">{note}</p>})}{record.independence_review.map(|review|view!{<p class="muted">{format!("Independent origins attested by {} at {} · {} source origins · {}",review.contributor,review.reviewed_at,review.source_ids.len(),review.note)}</p>})}<Review path=path version=record.version kind="opportunity" snap=snap notice=notice reload=reload/></section>}}).collect_view()}
    }
}
#[component]
fn Outcomes(project: String, snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let project = StoredValue::new(project);
    let start = RwSignal::new(days_ago(6.));
    let end = RwSignal::new(today());
    let busy = RwSignal::new(false);
    let reload = RwSignal::new(0u64);
    let records = RwSignal::new(Vec::<WeeklyOutcomes>::new());
    Effect::new(move |_| {
        let _ = reload.get();
        let id = project.get_value();
        spawn_local(async move {
            match api(&format!("/api/v1/projects/{id}/outcomes"), "GET", None).await {
                Ok(value) => {
                    if records.try_get_untracked().is_some() {
                        match serde_json::from_value(value) {
                            Ok(value) => records.set(value),
                            Err(_) => notice.set("Invalid outcome drafts".into()),
                        }
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    });
    view! {<section class="panel"><div class="section-heading"><h2>"Outcomes and loose ends"</h2><span class="muted">"UTC dates"</span></div><p class="muted">"Draft a cited account using recorded task transitions, unresolved ledger promise history, accepted decision changes and source snapshots captured by the period boundary. Earlier history may be unknown for tasks imported before this feature."</p><form class="workflow-actions" on:submit=move|ev|{ev.prevent_default();submit(format!("/api/v1/projects/{}/outcomes",project.get_value()),"POST",json!({"start_date":start.get_untracked(),"end_date":end.get_untracked(),"idempotency_key":new_id()}),busy,snap,notice,reload);}>
    <label>"From"<input type="date" required prop:value=move||start.get() on:input=move|ev|start.set(event_target_value(&ev))/></label><label>"Through"<input type="date" required max=today() prop:value=move||end.get() on:input=move|ev|end.set(event_target_value(&ev))/></label><button class="button primary" disabled=move||busy.get()>"Save outcome draft"</button></form></section>{move||records.get().into_iter().map(|record|{let path=format!("/api/v1/projects/{}/outcomes/{}",project.get_value(),record.id);view!{<section class="panel insight-record"><div class="section-heading"><h2>{format!("{} → {}",record.start_date,record.end_date)}</h2><span class="muted">{format!("{} · v{}",record.status,record.version)}</span></div><small class="muted">{format!("Drafted {} · {} pinned source revisions",record.created_at,record.sources.len())}</small>{record.sections.into_iter().map(|section|{let empty=section.items.is_empty();view!{<h3 class="focus-section-title">{section.kind.replace('_'," ")}</h3><Show when=move||empty><p class="muted">"No supported items in this section."</p></Show>{section.items.into_iter().map(|item|view!{<div class="insight-outcome-line"><p>{item.text}</p><EvidenceLines notice=notice evidence=item.evidence/></div>}).collect_view()}}}).collect_view()}{record.notes.into_iter().map(|note|view!{<p class="muted">{note}</p>}).collect_view()}<details><summary>"Recorded task transitions"</summary>{record.transitions.into_iter().map(|t|view!{<p class="muted">{format!("{} · {} → {} · {}",t.recorded_at,t.previous_status.unwrap_or_else(||"first recorded".into()),t.task.status,t.task.title)}</p>}).collect_view()}</details><details><summary>"Ledger promises at period end"</summary>{record.commitment_snapshots.into_iter().map(|promise|view!{<div class="source-gap"><p>{format!("{} · {} · {} · v{} · observed {}",promise.commitment_id,promise.as_of.title,promise.as_of.status,promise.as_of.version,promise.as_of.created_at)}</p><EvidenceLines evidence=promise.promise_evidence notice=notice/>{promise.history.into_iter().map(|event|view!{<p class="muted">{format!("{} · {} · {} · {} · {}",event.created_at,event.contributor,event.action,event.title,event.note)}</p>}).collect_view()}</div>}).collect_view()}</details>{record.review_note.map(|note|view!{<p>{note}</p>})}<Review path=path version=record.version kind="outcomes" snap=snap notice=notice reload=reload/></section>}}).collect_view()}}
}
