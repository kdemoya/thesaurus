use super::workflows::{Citations, Pending, action, chosen_project};
use super::*;
use std::{cell::Cell, rc::Rc};
fn picked(
    snap: Snap,
    project: RwSignal<String>,
    doc: &str,
    line: &str,
) -> Result<Vec<Evidence>, String> {
    if doc.is_empty() {
        return Ok(vec![]);
    }
    let p = chosen_project(snap, project).ok_or("Choose a project")?;
    let d = snap
        .get_untracked()
        .and_then(|s| {
            s.documents
                .into_iter()
                .find(|d| d.id == doc && p.source_ids.contains(&d.source_id))
        })
        .ok_or("Choose a project source")?;
    let n = line
        .parse::<i64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("Choose a positive source line")?;
    let q = d
        .body
        .lines()
        .nth(n as usize - 1)
        .ok_or("Source line does not exist")?;
    if q.trim().is_empty() || q.len() > 2000 {
        return Err("Choose a nonempty line up to 2,000 bytes".into());
    }
    Ok(vec![Evidence {
        document_id: d.id,
        revision: d.revision,
        line: n,
        quote: q.into(),
    }])
}
#[component]
fn Picker(
    snap: Snap,
    project: RwSignal<String>,
    doc: RwSignal<String>,
    line: RwSignal<String>,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    view! {<div class="waiting-evidence-picker"><label>"Evidence source"<select prop:value=move||doc.get() on:change=move|ev|{doc.set(event_target_value(&ev));line.set("1".into());}><option value="">"Choose source…"</option>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.documents.into_iter().filter(|d|p.source_ids.contains(&d.source_id)).map(|d|view!{<option value=d.id>{format!("{} · r{}",d.title,d.revision)}</option>}).collect_view())}</select></label><label>"Line"<input type="number" min="1" prop:value=move||line.get() on:input=move|ev|line.set(event_target_value(&ev))/></label>{move||match picked(snap,project,&doc.get(),&line.get()){Ok(e)=>view!{<Citations evidence=e selected=selected notice=notice/>}.into_any(),Err(e)=>view!{<small class="muted">{e}</small>}.into_any()}}</div>}
}
#[component]
pub(super) fn Knowledge(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    project: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let mode = RwSignal::new("answers".to_string());
    view! {<div class="focus-tabs"><button class:active=move||mode.get()=="answers" on:click=move|_|mode.set("answers".into())>"Evidence & history"</button><button class:active=move||mode.get()=="claims" on:click=move|_|mode.set("claims".into())>"Dated claims"</button><button class:active=move||mode.get()=="people" on:click=move|_|mode.set("people".into())>"People & expertise"</button></div><Show when=move||mode.get()=="answers"><Answers snap=snap project=project notice=notice selected=selected/></Show><Show when=move||mode.get()=="claims"><Claims snap=snap project=project notice=notice selected=selected tick=tick/></Show><Show when=move||mode.get()=="people"><People snap=snap project=project notice=notice selected=selected tick=tick/></Show>}
}
#[component]
fn EvidencePanel(
    data: KnowledgeAnswer,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    view! {<section class="panel"><div class="section-heading"><h2>"Answer evidence"</h2><span>"Local extractive view"</span></div><p class="muted">{format!("Valid on {} · known by {}",data.valid_as_of,data.known_by)}</p><pre>{data.answer}</pre><h3>"Registered dated claims"</h3>{data.claims.into_iter().map(|c|view!{<article class="observation"><strong>{format!("{} · {}: {}",c.subject,c.predicate,c.value)}</strong><small>{format!("{} · valid {} until {} (exclusive) · observed {} · {} v{}",c.certainty,c.valid_from,c.valid_until.unwrap_or_else(||"open ended".into()),c.observed_at,c.id,c.version)}</small><Citations evidence=c.evidence selected=selected notice=notice/></article>}).collect_view()}<h3>"Source passages"</h3>{data.passages.into_iter().map(|p|view!{<article class="observation"><strong>{p.source_title}</strong><small>{format!("{} · policy {} · observed {} · fetched {} · freshness unverified · {}{}",p.retrieval_path,p.retrieval_policy,p.observed_at,p.fetched_at.unwrap_or_else(||"not available".into()),p.validity,p.error.map(|e|format!(" · {e}")).unwrap_or_default())}</small><Citations evidence=vec![p.evidence] selected=selected notice=notice/></article>}).collect_view()}<h3>"Different values to review"</h3>{data.contradictions.into_iter().map(|c|view!{<article class="observation"><strong>{format!("{} · {}",c.subject,c.predicate)}</strong><p>{c.values.join(" / ")}</p><p class="muted">{c.explanation}</p><small>{c.claim_ids.join(", ")}</small></article>}).collect_view()}<h3>"Missing evidence & uncertainty"</h3><ul>{data.missing_evidence.into_iter().map(|s|view!{<li>{s}</li>}).collect_view()}</ul><p class="muted">{format!("{} claims / {} passages omitted. {}",data.claims_omitted,data.passages_omitted,data.method)}</p></section>}
}
#[component]
fn Answers(
    snap: Snap,
    project: RwSignal<String>,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    let question = RwSignal::new(String::new());
    let date = RwSignal::new(super::focus::utc_day());
    let known = RwSignal::new(String::new());
    let compare = RwSignal::new(false);
    let sources = RwSignal::new(Vec::<String>::new());
    let active = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let result = RwSignal::new(None::<KnowledgeAnswer>);
    let comparison = RwSignal::new(None::<KnowledgeComparison>);
    let serial = Rc::new(Cell::new(0u64));
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let c = alive.clone();
    on_cleanup(move || c.store(false, std::sync::atomic::Ordering::Relaxed));
    Effect::new(move |_| {
        if let Some(p) = chosen_project(snap, project) {
            if active.get_untracked() != p.id {
                active.set(p.id);
                sources.set(p.source_ids);
                result.set(None);
                comparison.set(None);
                question.set(String::new());
            }
        }
    });
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let mode = compare.get_untracked();
        let path = if mode {
            "/api/v1/knowledge/compare"
        } else {
            "/api/v1/knowledge/answer"
        };
        let body = json!({"project_id":p.id,"question":question.get_untracked(),"valid_as_of":date.get_untracked(),"known_by":if known.get_untracked().is_empty(){None}else{Some(known.get_untracked())},"source_ids":sources.get_untracked()});
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        spawn_local(async move {
            let response = api(path, "POST", Some(body)).await;
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            busy.set(false);
            if !chosen_project(snap, project).is_some_and(|current| current.id == p.id) {
                return;
            }
            match response {
                Ok(v) => {
                    if mode {
                        match serde_json::from_value::<KnowledgeComparison>(v) {
                            Ok(v) => {
                                comparison.set(Some(v));
                                result.set(None);
                            }
                            Err(_) => notice.set("Could not read knowledge comparison".into()),
                        }
                    } else {
                        match serde_json::from_value::<KnowledgeAnswer>(v) {
                            Ok(v) => {
                                result.set(Some(v));
                                comparison.set(None);
                            }
                            Err(_) => notice.set("Could not read answer evidence".into()),
                        }
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    };
    view! {<section class="panel"><div class="section-heading"><h2>"Ask with evidence"</h2><span>"No model usage"</span></div><p class="muted">"Choose when the claim was valid separately from when it was known. Source import timestamps do not establish historical validity."</p><form on:submit=submit><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Question"<input required maxlength="1000" prop:value=move||question.get() on:input=move|ev|question.set(event_target_value(&ev))/></label><label>"Valid as of (UTC date)"<input type="date" required prop:value=move||date.get() on:input=move|ev|date.set(event_target_value(&ev))/></label><label>"Known by (UTC timestamp, optional)"<input prop:value=move||known.get() on:input=move|ev|known.set(event_target_value(&ev)) placeholder="2026-10-17T14:00:00Z · blank means now"/></label><fieldset class="runtime-sources"><legend>"Permitted project sources"</legend>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.sources.into_iter().filter(|s|p.source_ids.contains(&s.id)).map(|s|{let id=s.id.clone();let check=s.id;view!{<label><input type="checkbox" prop:checked=move||sources.get().contains(&check) on:change=move|ev|{let yes=event_target_checked(&ev);sources.update(|ids|{ids.retain(|s|s!=&id);if yes{ids.push(id.clone());}});}/>{s.name}</label>}}).collect_view())}</fieldset><label><input type="checkbox" prop:checked=move||compare.get() on:change=move|ev|compare.set(event_target_checked(&ev))/>"Compare with today as currently known"</label><button type="submit" class="button primary">{move||if busy.get(){"Retrieving…"}else{"Retrieve answer evidence"}}</button></fieldset></form></section>
    {move||result.get().map(|data|view!{<EvidencePanel data=data selected=selected notice=notice/>})}{move||comparison.get().map(|c|view!{<section class="panel"><div class="section-heading"><h2>"Changes to today"</h2></div><p class="muted">"Comparison covers returned dated claims; omissions can hide further changes."</p>{c.changes.into_iter().map(|d|view!{<article class="observation"><strong>{format!("{} · {}",d.subject,d.predicate)}</strong><p>{format!("{} → {}",if d.before.is_empty(){"No returned claim".into()}else{d.before.join(" / ")},if d.today.is_empty(){"No returned claim".into()}else{d.today.join(" / ")})}</p></article>}).collect_view()}</section><h3>"Requested point in time"</h3><EvidencePanel data=c.requested selected=selected notice=notice/><h3>"Today, as currently known"</h3><EvidencePanel data=c.today selected=selected notice=notice/>})}}
}
#[component]
fn History(path: String, notice: RwSignal<String>) -> impl IntoView {
    let path = StoredValue::new(path);
    let offset = RwSignal::new(0usize);
    let rows = RwSignal::new(None::<Value>);
    let serial = Rc::new(Cell::new(0u64));
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let c = alive.clone();
    on_cleanup(move || c.store(false, std::sync::atomic::Ordering::Relaxed));
    Effect::new(move |_| {
        let start = offset.get();
        let path = path.get_value();
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        spawn_local(async move {
            let result = api(&format!("{path}?offset={start}&limit=5"), "GET", None).await;
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(v) => rows.set(Some(v)),
                Err(e) => notice.set(e),
            }
        });
    });
    view! {<details><summary>"Immutable history"</summary>{move||rows.get().map(|v|view!{<pre>{serde_json::to_string_pretty(&v["items"]).unwrap_or_default()}</pre>})}<div class="workflow-actions"><button class="button text" disabled=move||offset.get()==0 on:click=move|_|offset.update(|n|*n=n.saturating_sub(5))>"← Newer"</button><button class="button text" disabled=move||rows.get().is_none_or(|v|v["next_offset"].as_u64().is_none()) on:click=move|_|{if let Some(n)=rows.get_untracked().and_then(|v|v["next_offset"].as_u64()){offset.set(n as usize);}}>"Older →"</button></div></details>}
}
#[component]
fn Claims(
    snap: Snap,
    project: RwSignal<String>,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let list = RwSignal::new(None::<ClaimPage>);
    let current = RwSignal::new(String::new());
    let offset = RwSignal::new(0usize);
    let archived = RwSignal::new(false);
    let active = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let subject = RwSignal::new(String::new());
    let predicate = RwSignal::new(String::new());
    let value = RwSignal::new(String::new());
    let certainty = RwSignal::new("asserted".to_string());
    let from = RwSignal::new(super::focus::utc_day());
    let until = RwSignal::new(String::new());
    let doc = RwSignal::new(String::new());
    let line = RwSignal::new("1".to_string());
    let note = RwSignal::new(String::new());
    let edit_value = RwSignal::new(None::<String>);
    let edit_from = RwSignal::new(None::<String>);
    let edit_until = RwSignal::new(None::<String>);
    let edit_certainty = RwSignal::new(None::<String>);
    let edit_note = RwSignal::new(String::new());
    let edit_doc = RwSignal::new(String::new());
    let edit_line = RwSignal::new("1".to_string());
    let editing = RwSignal::new(String::new());
    let edit_version = RwSignal::new(None::<i64>);
    let reset = move || {
        edit_value.set(None);
        edit_from.set(None);
        edit_until.set(None);
        edit_certainty.set(None);
        edit_note.set(String::new());
        edit_doc.set(String::new());
        edit_version.set(None);
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
        let all = archived.get();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let changed = active.get_untracked() != p.id;
        if changed {
            active.set(p.id.clone());
            offset.set(0);
            current.set(String::new());
            list.set(None);
            subject.set(String::new());
            predicate.set(String::new());
            value.set(String::new());
            doc.set(String::new());
            note.set(String::new());
            reset();
        }
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        spawn_local(async move {
            let result = api(
                &format!(
                    "/api/v1/projects/{}/claims?offset={}&limit=25&include_archived={all}",
                    p.id,
                    if changed { 0 } else { start }
                ),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<ClaimPage>(v)
                    .map_err(|_| "Could not read dated claims".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(v) => list.set(Some(v)),
                Err(e) => notice.set(e),
            }
        });
    });
    let chosen = move || {
        list.get().and_then(|p| {
            p.items
                .iter()
                .find(|c| c.id == current.get())
                .or_else(|| p.items.first())
                .cloned()
        })
    };
    Effect::new(move |_| {
        if let Some(c) = chosen() {
            if editing.get_untracked() != c.id {
                editing.set(c.id);
                reset();
            }
        }
    });
    let capture_version = move || {
        if edit_version.get_untracked().is_none() {
            edit_version.set(chosen().map(|c| c.version));
        }
    };
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let ev = match picked(snap, project, &doc.get_untracked(), &line.get_untracked()) {
            Ok(e) => e,
            Err(e) => {
                notice.set(e);
                return;
            }
        };
        let pid = p.id.clone();
        action(
            "/api/v1/claims".into(),
            "POST",
            json!({"project_id":p.id,"subject":subject.get_untracked(),"predicate":predicate.get_untracked(),"value":value.get_untracked(),"certainty":certainty.get_untracked(),"valid_from":from.get_untracked(),"valid_until":if until.get_untracked().is_empty(){None}else{Some(until.get_untracked())},"evidence":ev,"note":note.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if !chosen_project(snap, project).is_some_and(|p| p.id == pid) {
                    return;
                }
                current.set(v["id"].as_str().unwrap_or_default().into());
                subject.set(String::new());
                predicate.set(String::new());
                value.set(String::new());
                doc.set(String::new());
                note.set(String::new());
                offset.set(0);
                tick.update(|n| *n += 1);
            },
        );
    };
    let amend = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(c) = chosen() else {
            return;
        };
        let e = match picked(
            snap,
            project,
            &edit_doc.get_untracked(),
            &edit_line.get_untracked(),
        ) {
            Ok(e) => e,
            Err(e) => {
                notice.set(e);
                return;
            }
        };
        let end = edit_until
            .get_untracked()
            .unwrap_or_else(|| c.valid_until.unwrap_or_default());
        action(
            format!("/api/v1/claims/{}", c.id),
            "PATCH",
            json!({"expected_version":edit_version.get_untracked().unwrap_or(c.version),"action":"amend","value":edit_value.get_untracked(),"certainty":edit_certainty.get_untracked(),"valid_from":edit_from.get_untracked(),"valid_until":if end.is_empty(){None}else{Some(end.clone())},"clear_valid_until":end.is_empty(),"evidence":e,"note":edit_note.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |_| {
                reset();
                tick.update(|n| *n += 1);
            },
        );
    };
    view! {<div class="brief-layout waiting-layout"><aside class="brief-sidebar"><section class="panel brief-create"><div class="section-heading"><h2>"Register a dated claim"</h2></div><p class="muted">"Record an explicit ownership or decision claim, with a validity interval and supporting evidence. Observation time is recorded by the service."</p><form on:submit=create><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Subject"<input required maxlength="300" prop:value=move||subject.get() on:input=move|ev|subject.set(event_target_value(&ev)) placeholder="Staging rollout"/></label><label>"Facet"<input required maxlength="200" prop:value=move||predicate.get() on:input=move|ev|predicate.set(event_target_value(&ev)) placeholder="owner"/></label><label>"Value"<textarea required maxlength="2000" prop:value=move||value.get() on:input=move|ev|value.set(event_target_value(&ev))></textarea></label><label>"Certainty"<select prop:value=move||certainty.get() on:change=move|ev|certainty.set(event_target_value(&ev))><option value="asserted">"Asserted by cited evidence"</option><option value="uncertain">"Uncertain"</option></select></label><label>"Valid from"<input type="date" required prop:value=move||from.get() on:input=move|ev|from.set(event_target_value(&ev))/></label><label>"Valid until (exclusive, optional)"<input type="date" prop:value=move||until.get() on:input=move|ev|until.set(event_target_value(&ev))/></label><Picker snap=snap project=project doc=doc line=line notice=notice selected=selected/><label>"Context"<textarea maxlength="2000" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))></textarea></label><button type="submit" class="button primary">"Register claim"</button></fieldset></form></section></aside>
    <div class="brief-main"><section class="panel"><div class="section-heading"><h2>"Dated claim ledger"</h2><span>{move||list.get().map(|l|l.total.to_string()).unwrap_or_default()}</span></div><label><input type="checkbox" prop:checked=move||archived.get() on:change=move|ev|{archived.set(event_target_checked(&ev));offset.set(0);}/>"Include retracted"</label>{move||list.get().map(|p|p.items.into_iter().map(|c|{let id=c.id;view!{<div class="memory-line"><button class="task-title" on:click=move|_|current.set(id.clone())>{format!("{} · {}: {}",c.subject,c.predicate,c.value)}</button><small>{format!("{} · {} · valid {}",c.status,c.certainty,c.valid_from)}</small></div>}}).collect_view())}<div class="workflow-actions"><button class="button text" disabled=move||offset.get()==0 on:click=move|_|offset.update(|n|*n=n.saturating_sub(25))>"← Previous"</button><button class="button text" disabled=move||list.get().is_none_or(|p|p.next_offset.is_none()) on:click=move|_|{if let Some(n)=list.get_untracked().and_then(|p|p.next_offset){offset.set(n);}}>"Next →"</button></div></section>
    {move||chosen().map(|c|{let record=c.clone();let retract=c.clone();let status=c.status.clone();let hist=format!("/api/v1/claims/{}/history",c.id);view!{<section class="panel"><div class="section-heading"><h2>{format!("{} · {}",c.subject,c.predicate)}</h2><span>{format!("{} · v{}",c.status,c.version)}</span></div><p>{c.value}</p><p class="muted">{format!("Valid {} until {} (exclusive) · observed {} · {}",c.valid_from,c.valid_until.unwrap_or_else(||"open ended".into()),c.observed_at,c.contributor)}</p><Citations evidence=c.evidence selected=selected notice=notice/><p>{c.note}</p><button class="button text" disabled=move||busy.get() on:click=move|_|{action(format!("/api/v1/claims/{}",retract.id),"PATCH",json!({"expected_version":retract.version,"action":if retract.status=="active"{"retract"}else{"restore"},"note":"Explicit user claim review"}),snap,notice,busy,pending,move|_|{reset();tick.update(|n|*n+=1);});}>{if status=="active"{"Retract claim"}else{"Restore claim"}}</button>
    <details><summary>"Amend with evidence"</summary><form on:submit=amend><fieldset class="focus-form-fields" disabled=move||busy.get()||chosen().is_none_or(|c|c.status!="active")><label>"Value"<textarea maxlength="2000" prop:value=move||edit_value.get().unwrap_or_else(||chosen().map(|c|c.value).unwrap_or_default()) on:input=move|ev|{capture_version();edit_value.set(Some(event_target_value(&ev)));}></textarea></label><label>"Certainty"<select prop:value=move||edit_certainty.get().unwrap_or_else(||chosen().map(|c|c.certainty).unwrap_or_default()) on:change=move|ev|{capture_version();edit_certainty.set(Some(event_target_value(&ev)));}><option value="asserted">"Asserted"</option><option value="uncertain">"Uncertain"</option></select></label><label>"Valid from"<input type="date" prop:value=move||edit_from.get().unwrap_or_else(||chosen().map(|c|c.valid_from).unwrap_or_default()) on:input=move|ev|{capture_version();edit_from.set(Some(event_target_value(&ev)));}/></label><label>"Valid until (blank clears)"<input type="date" prop:value=move||edit_until.get().unwrap_or_else(||chosen().and_then(|c|c.valid_until).unwrap_or_default()) on:input=move|ev|{capture_version();edit_until.set(Some(event_target_value(&ev)));}/></label><Picker snap=snap project=project doc=edit_doc line=edit_line notice=notice selected=selected/><label>"Reason"<textarea required maxlength="2000" prop:value=move||edit_note.get() on:input=move|ev|{capture_version();edit_note.set(event_target_value(&ev));}></textarea></label><button type="submit" class="button primary">"Append claim revision"</button><button type="button" class="button text" on:click=move|_|reset()>"Discard draft / use current version"</button></fieldset></form></details><History path=hist notice=notice/><small class="muted">{format!("Claim {}",record.id)}</small></section>}})}
    </div></div>}
}
#[component]
fn People(
    snap: Snap,
    project: RwSignal<String>,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let list = RwSignal::new(None::<PeoplePage>);
    let query = RwSignal::new(String::new());
    let offset = RwSignal::new(0usize);
    let current = RwSignal::new(String::new());
    let active = RwSignal::new(String::new());
    let archived = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let name = RwSignal::new(String::new());
    let identity = RwSignal::new(String::new());
    let doc = RwSignal::new(String::new());
    let line = RwSignal::new("1".to_string());
    let command = RwSignal::new("alias".to_string());
    let first = RwSignal::new(String::new());
    let second = RwSignal::new(String::new());
    let certainty = RwSignal::new("uncertain".to_string());
    let relationship = RwSignal::new("author".to_string());
    let from = RwSignal::new(super::focus::utc_day());
    let until = RwSignal::new(String::new());
    let edit_doc = RwSignal::new(String::new());
    let edit_line = RwSignal::new("1".to_string());
    let note = RwSignal::new(String::new());
    let editing = RwSignal::new(String::new());
    let version = RwSignal::new(None::<i64>);
    let reset = move || {
        first.set(String::new());
        second.set(String::new());
        edit_doc.set(String::new());
        note.set(String::new());
        version.set(None);
        pending.set(None);
    };
    let serial = Rc::new(Cell::new(0u64));
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let c = alive.clone();
    on_cleanup(move || c.store(false, std::sync::atomic::Ordering::Relaxed));
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        let q = query.get();
        let start = offset.get();
        let all = archived.get();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let changed = active.get_untracked() != p.id;
        if changed {
            active.set(p.id.clone());
            offset.set(0);
            current.set(String::new());
            query.set(String::new());
            list.set(None);
            name.set(String::new());
            identity.set(String::new());
            doc.set(String::new());
            reset();
        }
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        spawn_local(async move {
            let q = js_sys::encode_uri_component(if changed { "" } else { &q })
                .as_string()
                .unwrap_or_default();
            let result=api(&format!("/api/v1/projects/{}/people?query={q}&offset={}&limit=10&include_archived={all}",p.id,if changed{0}else{start}),"GET",None).await.and_then(|v|serde_json::from_value::<PeoplePage>(v).map_err(|_|"Could not read people lookup".into()));
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(v) => list.set(Some(v)),
                Err(e) => notice.set(e),
            }
        });
    });
    let chosen = move || {
        list.get().and_then(|p| {
            p.items
                .iter()
                .find(|m| m.person.id == current.get())
                .or_else(|| p.items.first())
                .map(|m| m.person.clone())
        })
    };
    Effect::new(move |_| {
        if let Some(p) = chosen() {
            if editing.get_untracked() != p.id {
                editing.set(p.id);
                reset();
            }
        }
    });
    let capture = move || {
        if version.get_untracked().is_none() {
            version.set(chosen().map(|p| p.version));
        }
    };
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let e = match picked(snap, project, &doc.get_untracked(), &line.get_untracked()) {
            Ok(e) => e,
            Err(e) => {
                notice.set(e);
                return;
            }
        };
        let pid = p.id.clone();
        action(
            "/api/v1/people".into(),
            "POST",
            json!({"project_id":p.id,"display_name":name.get_untracked(),"identity_note":identity.get_untracked(),"evidence":e}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if !chosen_project(snap, project).is_some_and(|p| p.id == pid) {
                    return;
                }
                current.set(v["id"].as_str().unwrap_or_default().into());
                name.set(String::new());
                identity.set(String::new());
                doc.set(String::new());
                query.set(String::new());
                offset.set(0);
                tick.update(|n| *n += 1);
            },
        );
    };
    let update = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen() else {
            return;
        };
        let mode = command.get_untracked();
        let e = if mode == "edit" {
            vec![]
        } else {
            match picked(
                snap,
                project,
                &edit_doc.get_untracked(),
                &edit_line.get_untracked(),
            ) {
                Ok(e) => e,
                Err(e) => {
                    notice.set(e);
                    return;
                }
            }
        };
        let body = json!({"expected_version":version.get_untracked().unwrap_or(p.version),"action":mode,"display_name":if mode=="edit"{Some(first.get_untracked())}else{None},"identity_note":if mode=="edit"{Some(second.get_untracked())}else{None},"alias":if mode=="alias"{Some(json!({"label":first.get_untracked(),"certainty":certainty.get_untracked(),"evidence":e}))}else{None},"role":if mode=="role"{Some(json!({"role":first.get_untracked(),"scope":second.get_untracked(),"valid_from":from.get_untracked(),"valid_until":if until.get_untracked().is_empty(){None}else{Some(until.get_untracked())},"evidence":e}))}else{None},"work":if mode=="work"{Some(json!({"title":first.get_untracked(),"topic":second.get_untracked(),"relationship":relationship.get_untracked(),"evidence":e}))}else{None},"note":note.get_untracked()});
        action(
            format!("/api/v1/people/{}", p.id),
            "PATCH",
            body,
            snap,
            notice,
            busy,
            pending,
            move |_| {
                reset();
                tick.update(|n| *n += 1);
            },
        );
    };
    view! {<div class="brief-layout waiting-layout"><aside class="brief-sidebar"><section class="panel brief-create"><div class="section-heading"><h2>"Register a person"</h2></div><p class="muted">"Create a cited identity record. Task owners and name strings are never automatically joined to a person."</p><form on:submit=create><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Display name"<input required maxlength="200" prop:value=move||name.get() on:input=move|ev|name.set(event_target_value(&ev))/></label><label>"Identity context / uncertainty"<textarea required maxlength="2000" prop:value=move||identity.get() on:input=move|ev|identity.set(event_target_value(&ev)) placeholder="How you know this identity; note any ambiguity"></textarea></label><Picker snap=snap project=project doc=doc line=line notice=notice selected=selected/><button type="submit" class="button primary">"Register cited identity"</button></fieldset></form></section></aside>
    <div class="brief-main"><section class="panel"><div class="section-heading"><h2>"People & expertise lookup"</h2><span>{move||list.get().map(|p|p.total.to_string()).unwrap_or_default()}</span></div><label>"Name, role or work topic"<input maxlength="1000" prop:value=move||query.get() on:input=move|ev|{query.set(event_target_value(&ev));offset.set(0);current.set(String::new());}/></label><label><input type="checkbox" prop:checked=move||archived.get() on:change=move|ev|{archived.set(event_target_checked(&ev));offset.set(0);}/>"Include archived identities"</label><p class="muted">{move||list.get().map(|p|p.caution).unwrap_or_default()}</p>{move||list.get().map(|p|p.items.into_iter().map(|m|{let id=m.person.id;view!{<article class="observation"><button class="task-title" on:click=move|_|current.set(id.clone())>{m.person.display_name}</button><p class="muted">{m.match_basis.join(" · ")}</p><small>{if m.uncertain_identity{"Identity match uncertain; verify before use"}else{"User-curated identity; inspect cited evidence"}}</small></article>}}).collect_view())}<div class="workflow-actions"><button class="button text" disabled=move||offset.get()==0 on:click=move|_|offset.update(|n|*n=n.saturating_sub(10))>"← Previous"</button><button class="button text" disabled=move||list.get().is_none_or(|p|p.next_offset.is_none()) on:click=move|_|{if let Some(n)=list.get_untracked().and_then(|p|p.next_offset){offset.set(n);}}>"Next →"</button></div></section>
    {move||chosen().map(|p|{let review=p.clone();let hist=format!("/api/v1/people/{}/history",p.id);let active=p.status=="active";let owner_id=p.id.clone();let owner_version=p.version;view!{<section class="panel"><div class="section-heading"><h2>{p.display_name}</h2><span>{format!("{} · v{}",p.status,p.version)}</span></div><p>{p.identity_note}</p><Citations evidence=p.evidence selected=selected notice=notice/><h3>"Aliases"</h3>{p.aliases.into_iter().enumerate().map(|(i,a)|{let id=owner_id.clone();view!{<article class="observation"><strong>{a.label}</strong><small>{a.certainty}</small><Citations evidence=a.evidence selected=selected notice=notice/><button class="button text" disabled=move||busy.get()||!active on:click=move|_|{action(format!("/api/v1/people/{id}"),"PATCH",json!({"expected_version":owner_version,"action":"remove_alias","record_index":i,"note":"User withdrew this alias; original retained in history"}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}>"Withdraw alias"</button></article>}}).collect_view()}<h3>"Explicit roles"</h3>{p.roles.into_iter().enumerate().map(|(i,r)|{let id=owner_id.clone();view!{<article class="observation"><strong>{format!("{} · {}",r.role,r.scope)}</strong><small>{format!("Valid {} until {} (exclusive)",r.valid_from,r.valid_until.unwrap_or_else(||"open ended".into()))}</small><Citations evidence=r.evidence selected=selected notice=notice/><button class="button text" disabled=move||busy.get()||!active on:click=move|_|{action(format!("/api/v1/people/{id}"),"PATCH",json!({"expected_version":owner_version,"action":"remove_role","record_index":i,"note":"User withdrew this role; original retained in history"}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}>"Withdraw role"</button></article>}}).collect_view()}<h3>"Authored work"</h3>{p.works.into_iter().enumerate().map(|(i,w)|{let id=owner_id.clone();view!{<article class="observation"><strong>{w.title}</strong><small>{format!("{} · {}",w.relationship,w.topic)}</small><Citations evidence=w.evidence selected=selected notice=notice/><button class="button text" disabled=move||busy.get()||!active on:click=move|_|{action(format!("/api/v1/people/{id}"),"PATCH",json!({"expected_version":owner_version,"action":"remove_work","record_index":i,"note":"User withdrew this attribution; original retained in history"}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}>"Withdraw attribution"</button></article>}}).collect_view()}<button class="button text" disabled=move||busy.get() on:click=move|_|{action(format!("/api/v1/people/{}",review.id),"PATCH",json!({"expected_version":review.version,"action":if review.status=="active"{"archive"}else{"restore"},"note":"Explicit user identity review"}),snap,notice,busy,pending,move|_|{reset();tick.update(|n|*n+=1);});}>{if active{"Archive identity"}else{"Restore identity"}}</button>
    <details><summary>"Maintain cited profile"</summary><form on:submit=update><fieldset class="focus-form-fields" disabled=move||busy.get()||!active><label>"Action"<select prop:value=move||command.get() on:change=move|ev|{reset();command.set(event_target_value(&ev));capture();if command.get_untracked()=="edit"{if let Some(p)=chosen(){first.set(p.display_name);second.set(p.identity_note);}}}><option value="alias">"Add alias"</option><option value="role">"Add dated role"</option><option value="work">"Add authored work"</option><option value="edit">"Edit identity label / context"</option></select></label><label>{move||match command.get().as_str(){"alias"=>"Alias label","role"=>"Role","work"=>"Work title",_=>"Display name"}}<input required maxlength="300" prop:value=move||first.get() on:input=move|ev|{capture();first.set(event_target_value(&ev));}/></label>
    <Show when=move||command.get()!="alias"><label>{move||match command.get().as_str(){"role"=>"Role scope","work"=>"Topic",_=>"Identity context"}}<textarea required maxlength="2000" prop:value=move||second.get() on:input=move|ev|{capture();second.set(event_target_value(&ev));}></textarea></label></Show><Show when=move||command.get()=="alias"><label>"Alias certainty"<select prop:value=move||certainty.get() on:change=move|ev|{capture();certainty.set(event_target_value(&ev));}><option value="uncertain">"Uncertain match"</option><option value="confirmed">"Confirmed by cited evidence"</option></select></label></Show><Show when=move||command.get()=="role"><label>"Valid from"<input type="date" required prop:value=move||from.get() on:input=move|ev|{capture();from.set(event_target_value(&ev));}/></label><label>"Valid until (exclusive, optional)"<input type="date" prop:value=move||until.get() on:input=move|ev|{capture();until.set(event_target_value(&ev));}/></label></Show><Show when=move||command.get()=="work"><label>"Relationship"<select prop:value=move||relationship.get() on:change=move|ev|{capture();relationship.set(event_target_value(&ev));}><option value="author">"Author"</option><option value="reviewer">"Reviewer"</option><option value="maintainer">"Maintainer"</option></select></label></Show><Show when=move||command.get()!="edit"><Picker snap=snap project=project doc=edit_doc line=edit_line notice=notice selected=selected/></Show><label>"Reason / context"<textarea required maxlength="2000" prop:value=move||note.get() on:input=move|ev|{capture();note.set(event_target_value(&ev));}></textarea></label><button type="submit" class="button primary">"Append profile change"</button><button type="button" class="button text" on:click=move|_|reset()>"Discard draft / use current version"</button></fieldset></form></details><History path=hist notice=notice/></section>}})}
    </div></div>}
}
