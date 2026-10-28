use super::focus::{plus_days, utc_day};
use super::workflows::{Citations, Pending, action, chosen_project};
use super::*;
use std::{cell::Cell, rc::Rc};

fn picked(
    snap: Snap,
    project: RwSignal<String>,
    id: &str,
    line: &str,
) -> Result<Vec<Evidence>, String> {
    if id.is_empty() {
        return Ok(vec![]);
    }
    let p = chosen_project(snap, project).ok_or("Choose a project")?;
    let doc = snap
        .get()
        .and_then(|s| {
            s.documents
                .into_iter()
                .find(|d| d.id == id && p.source_ids.contains(&d.source_id))
        })
        .ok_or("Choose a project source")?;
    let number = line
        .parse::<i64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("Choose a positive source line")?;
    let quote = doc
        .body
        .lines()
        .nth(number as usize - 1)
        .ok_or("Source line does not exist")?;
    if quote.len() > 2000 {
        return Err("Choose a source line of at most 2,000 UTF-8 bytes.".into());
    }
    Ok(vec![Evidence {
        document_id: doc.id,
        revision: doc.revision,
        line: number,
        quote: quote.into(),
    }])
}
#[component]
fn EvidencePicker(
    snap: Snap,
    project: RwSignal<String>,
    doc: RwSignal<String>,
    line: RwSignal<String>,
    prefix: &'static str,
    selected: RwSignal<Option<Document>>,
    notice: RwSignal<String>,
) -> impl IntoView {
    let doc_id = format!("{prefix}-doc");
    let line_id = format!("{prefix}-line");
    view! {<div class="waiting-evidence-picker"><label for=doc_id.clone()>"Source evidence"</label><select id=doc_id prop:value=move||doc.get() on:change=move|ev|{doc.set(event_target_value(&ev));line.set("1".into());}><option value="">"Choose source…"</option>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.documents.into_iter().filter(|d|p.source_ids.contains(&d.source_id)).map(|d|view!{<option value=d.id>{format!("{} · r{}",d.title,d.revision)}</option>}).collect_view())}</select><label for=line_id.clone()>"Line"</label><input id=line_id type="number" min="1" prop:value=move||line.get() on:input=move|ev|line.set(event_target_value(&ev))/>
        {move||match picked(snap,project,&doc.get(),&line.get()){Ok(e)=>view!{<Citations evidence=e selected=selected notice=notice/>}.into_any(),Err(e)=>view!{<small class="muted">{e}</small>}.into_any()}}
    </div>}
}
#[component]
pub(super) fn Waiting(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    project: RwSignal<String>,
    tick: RwSignal<u64>,
    waiting_id: RwSignal<String>,
    initial_task: RwSignal<String>,
) -> impl IntoView {
    let queue = RwSignal::new(None::<WaitingList>);
    let detail = RwSignal::new(None::<WaitingFor>);
    let filter = RwSignal::new("waiting".to_string());
    let due_only = RwSignal::new(false);
    let offset = RwSignal::new(0usize);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let loading = RwSignal::new(false);
    let active_project = RwSignal::new(String::new());
    let task = RwSignal::new(initial_task.get_untracked());
    let title = RwSignal::new(None::<String>);
    let person = RwSignal::new(String::new());
    let requested = RwSignal::new(utc_day());
    let interval = RwSignal::new("3".to_string());
    let create_note = RwSignal::new(String::new());
    let doc = RwSignal::new(String::new());
    let line = RwSignal::new("1".to_string());
    let update_action = RwSignal::new("edit".to_string());
    let edit_title = RwSignal::new(None::<String>);
    let edit_person = RwSignal::new(None::<String>);
    let next = RwSignal::new(None::<String>);
    let note = RwSignal::new(String::new());
    let edit_version = RwSignal::new(None::<i64>);
    let edit_doc = RwSignal::new(String::new());
    let edit_line = RwSignal::new("1".to_string());
    let draft = RwSignal::new(None::<String>);
    let editing_id = RwSignal::new(String::new());
    let reset = move || {
        edit_title.set(None);
        edit_person.set(None);
        next.set(None);
        note.set(String::new());
        edit_version.set(None);
        edit_doc.set(String::new());
        edit_line.set("1".into());
        draft.set(None);
        update_action.set("edit".into());
    };
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let cleanup = alive.clone();
    on_cleanup(move || cleanup.store(false, std::sync::atomic::Ordering::Relaxed));
    let serial = Rc::new(Cell::new(0u64));
    let load: Rc<dyn Fn()> = Rc::new(move || {
        let Some(p) = chosen_project(snap, project) else {
            queue.set(None);
            detail.set(None);
            return;
        };
        if active_project.get_untracked() != p.id {
            active_project.set(p.id.clone());
            queue.set(None);
            detail.set(None);
            offset.set(0);
            task.set(initial_task.get_untracked());
            title.set(None);
            person.set(String::new());
            requested.set(utc_day());
            interval.set("3".into());
            create_note.set(String::new());
            doc.set(String::new());
            line.set("1".into());
            reset();
        }
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        loading.set(true);
        let path = format!(
            "/api/v1/projects/{}/waiting?status={}&due_only={}&offset={}&limit=25",
            p.id,
            filter.get_untracked(),
            due_only.get_untracked(),
            offset.get_untracked()
        );
        let selected_id = waiting_id.get_untracked();
        spawn_local(async move {
            let result = api(&path, "GET", None).await;
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            let data: WaitingList = match result.and_then(|v| {
                serde_json::from_value(v).map_err(|_| "Could not read waiting queue".into())
            }) {
                Ok(data) => data,
                Err(e) => {
                    loading.set(false);
                    notice.set(e);
                    return;
                }
            };
            let id = if selected_id.is_empty() {
                data.items.first().map(|w| w.id.clone()).unwrap_or_default()
            } else {
                selected_id
            };
            queue.set(Some(data));
            if id.is_empty() {
                detail.set(None);
                loading.set(false);
                return;
            }
            match api(&format!("/api/v1/waiting/{id}"), "GET", None)
                .await
                .and_then(|v| {
                    serde_json::from_value::<WaitingFor>(v)
                        .map_err(|_| "Could not read waiting request".into())
                }) {
                Ok(w) if w.project_id == p.id => {
                    if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                        return;
                    }
                    if editing_id.get_untracked() != w.id {
                        reset();
                        editing_id.set(w.id.clone());
                    }
                    if detail.get_untracked().is_none_or(|old| {
                        old.id != w.id
                            || old.version != w.version
                            || old.follow_up_due != w.follow_up_due
                            || old.stale_evidence != w.stale_evidence
                            || old.linked_task_status != w.linked_task_status
                    }) {
                        detail.set(Some(w));
                    }
                }
                Ok(_) => {
                    if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                        return;
                    }
                    detail.set(None);
                }
                Err(e) => {
                    if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                        return;
                    }
                    notice.set(e);
                    detail.set(None);
                }
            }
            loading.set(false);
        });
    });
    Effect::new({
        let load = load.clone();
        move |_| {
            let _ = snap.get().map(|s| s.watermark);
            let _ = project.get();
            let _ = tick.get();
            let _ = filter.get();
            let _ = due_only.get();
            let _ = offset.get();
            let _ = waiting_id.get();
            load();
        }
    });
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let days = match interval.get_untracked().parse::<i64>() {
            Ok(n) if (0..=365).contains(&n) => n,
            _ => {
                notice.set("Choose a follow-up interval from 0 to 365 days.".into());
                return;
            }
        };
        let Some(follow) = plus_days(&requested.get_untracked(), days) else {
            notice.set("Choose a valid request date.".into());
            return;
        };
        let linked = task.get_untracked();
        let evidence = if linked.is_empty() {
            match picked(snap, project, &doc.get_untracked(), &line.get_untracked()) {
                Ok(e) if !e.is_empty() => e,
                Ok(_) => {
                    notice.set("Choose source evidence or link an accepted task.".into());
                    return;
                }
                Err(e) => {
                    notice.set(e);
                    return;
                }
            }
        } else {
            vec![]
        };
        let body_title = title.get_untracked().unwrap_or_else(|| {
            snap.get_untracked()
                .and_then(|s| s.tasks.into_iter().find(|t| t.id == linked))
                .map(|t| t.title)
                .unwrap_or_default()
        });
        let creation_project = p.id.clone();
        action(
            "/api/v1/waiting".into(),
            "POST",
            json!({"project_id":p.id,"task_id":if linked.is_empty(){None}else{Some(linked)},"title":body_title,"person":person.get_untracked(),"requested_on":requested.get_untracked(),"follow_up_on":follow,"evidence":evidence,"note":create_note.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if !chosen_project(snap, project).is_some_and(|p| p.id == creation_project) {
                    return;
                }
                waiting_id.set(v["id"].as_str().unwrap_or_default().into());
                task.set(String::new());
                initial_task.set(String::new());
                title.set(None);
                person.set(String::new());
                create_note.set(String::new());
                doc.set(String::new());
                offset.set(0);
                filter.set("waiting".into());
                due_only.set(false);
                tick.update(|n| *n += 1);
            },
        );
    };
    let update = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(w) = detail.get_untracked() else {
            return;
        };
        let command = if w.status == "waiting" {
            update_action.get_untracked()
        } else {
            "reopen".into()
        };
        let evidence = match picked(
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
        let edit = command == "edit";
        let close = command == "resolve" || command == "cancel";
        let default_date = if command == "snooze" || command == "follow_up" || command == "reopen" {
            plus_days(&utc_day(), 3).unwrap_or_else(utc_day)
        } else {
            w.follow_up_on.clone()
        };
        let date = if close {
            None
        } else {
            Some(next.get_untracked().unwrap_or(default_date))
        };
        action(
            format!("/api/v1/waiting/{}", w.id),
            "PATCH",
            json!({"expected_version":edit_version.get_untracked().unwrap_or(w.version),"action":command,"title":if edit{Some(edit_title.get_untracked().unwrap_or(w.title))}else{None},"person":if edit{Some(edit_person.get_untracked().unwrap_or(w.person))}else{None},"follow_up_on":date,"note":note.get_untracked(),"evidence":evidence}),
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
    view! {
        <div class="brief-layout waiting-layout">
            <aside class="brief-sidebar"><section class="panel brief-create"><div class="section-heading"><h2>"Track a request"</h2></div><form on:submit=create><fieldset class="focus-form-fields" disabled=move||busy.get()>
                <label for="waiting-task">"Accepted task (optional)"</label><select id="waiting-task" prop:value=move||task.get() on:change=move|ev|{task.set(event_target_value(&ev));title.set(None);}>
                    <option value="">"Standalone request"</option>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.tasks.into_iter().filter(|t|t.status=="accepted" && s.documents.iter().any(|d|d.id==t.evidence.document_id && p.source_ids.contains(&d.source_id))).map(|t|view!{<option value=t.id>{t.title}</option>}).collect_view())}</select>
                <label for="waiting-title">"Waiting for"</label><input id="waiting-title" required maxlength="300" prop:value=move||title.get().unwrap_or_else(||snap.get().and_then(|s|s.tasks.into_iter().find(|t|t.id==task.get())).map(|t|t.title).unwrap_or_default()) on:input=move|ev|title.set(Some(event_target_value(&ev))) placeholder="An approval, answer, or deliverable"/>
                <label for="waiting-person">"Person or team"</label><input id="waiting-person" required maxlength="200" prop:value=move||person.get() on:input=move|ev|person.set(event_target_value(&ev)) placeholder="Name as you know it"/>
                <label for="waiting-requested">"Requested on (UTC date)"</label><input id="waiting-requested" type="date" required prop:value=move||requested.get() on:input=move|ev|requested.set(event_target_value(&ev))/>
                <label for="waiting-interval">"Follow up after (days)"</label><input id="waiting-interval" type="number" min="0" max="365" required prop:value=move||interval.get() on:input=move|ev|interval.set(event_target_value(&ev))/><small class="muted">{move||interval.get().parse().ok().and_then(|n|plus_days(&requested.get(),n)).map(|d|format!("Follow-up date: {d}")).unwrap_or_default()}</small>
                {move||if task.get().is_empty(){view!{<EvidencePicker snap=snap project=project doc=doc line=line prefix="waiting-create" selected=selected notice=notice/>}.into_any()}else{view!{<p class="muted">"Uses the accepted task's original source evidence."</p>}.into_any()}}
                <label for="waiting-note">"Context (optional)"</label><textarea id="waiting-note" maxlength="1000" rows="3" prop:value=move||create_note.get() on:input=move|ev|create_note.set(event_target_value(&ev))></textarea><button class="button primary" type="submit" disabled=move||busy.get()>"Track request"</button>
            </fieldset></form></section></aside>
            <div class="brief-main">
                <section class="panel"><div class="section-heading"><h2>"Waiting for"</h2><span>{move||queue.get().map(|q|format!("{} matching",q.total)).unwrap_or_default()}</span></div>
                    <div class="workflow-actions"><label for="waiting-filter">"Status"</label><select id="waiting-filter" prop:value=move||filter.get() on:change=move|ev|{filter.set(event_target_value(&ev));offset.set(0);waiting_id.set(String::new());}><option value="waiting">"Waiting"</option><option value="resolved">"Response received"</option><option value="cancelled">"Cancelled"</option><option value="all">"All"</option></select><label class="focus-check"><input type="checkbox" prop:checked=move||due_only.get() on:change=move|ev|{due_only.set(event_target_checked(&ev));offset.set(0);waiting_id.set(String::new());}/><span>"Follow-up due"</span></label></div>
                    <Show when=move||loading.get()><p class="muted" role="status">"Refreshing queue…"</p></Show>
                    <div class="task-table-scroll"><table class="task-table"><thead><tr><th>"Request"</th><th>"Person"</th><th>"Follow up"</th><th>"State"</th></tr></thead><tbody>{move||queue.get().map(|q|q.items.into_iter().map(|w|{let id=w.id.clone();let click=w.id;view!{<tr class:selected=move||detail.get().is_some_and(|w|w.id==id) on:click=move|_|waiting_id.set(click.clone())><td><button class="task-title" type="button">{w.title}</button></td><td>{w.person}</td><td class:orange=w.follow_up_due>{w.follow_up_on}</td><td>{if w.follow_up_due{"follow-up due".into()}else{w.status}}</td></tr>}}).collect_view())}</tbody></table></div>
                    <Show when=move||queue.get().is_some_and(|q|q.items.is_empty())><p class="muted">"No requests match this view."</p></Show>
                    <div class="workflow-actions"><button class="button text" disabled=move||loading.get() || offset.get()==0 on:click=move|_|{offset.update(|n|*n=n.saturating_sub(25));waiting_id.set(String::new());}>"← Previous"</button><span class="muted">{move||queue.get().map(|q|format!("{}–{} / {}",if q.total==0{0}else{q.offset+1},(q.offset+q.items.len()).min(q.total),q.total)).unwrap_or_default()}</span><button class="button text" disabled=move||loading.get() || queue.get().is_none_or(|q|q.offset+q.items.len()>=q.total) on:click=move|_|{offset.update(|n|*n+=25);waiting_id.set(String::new());}>"Next →"</button></div>
                </section>
                {move||detail.get().map(|w|{
                    let version=w.version;let status=w.status.clone();let default_title=w.title.clone();let default_person=w.person.clone();let default_date=w.follow_up_on.clone();let suggestion=w.suggested_follow_up.clone();let original_sources=w.evidence.clone();
                    view!{
                        <section class="panel"><div class="section-heading"><h2>"Request details"</h2><span>{format!("{} · v{}",w.status,w.version)}</span></div><h3 class="focus-task-title">{w.title.clone()}</h3><p class="muted">{format!("Waiting on {} · requested {} · next follow-up {}",w.person,w.requested_on,w.follow_up_on)}</p>
                            {w.stale_evidence.then(||view!{<p class="brief-stale">"The original request evidence has a newer revision. Verify it before following up."</p>})}
                            {w.linked_task_status.clone().filter(|s|s!="accepted").map(|s|view!{<p class="orange">{format!("Linked task is {s}. This waiting request remains separate until you resolve or cancel it.")}</p>})}
                            <Citations evidence=original_sources selected=selected notice=notice/>
                            {suggestion.map(|suggestion|view!{<section class="follow-up-draft"><h3 class="focus-section-title">"Suggested follow-up"</h3><textarea aria-label="Editable follow-up draft" rows="3" prop:value=move||draft.get().unwrap_or_else(||suggestion.clone()) on:input=move|ev|draft.set(Some(event_target_value(&ev)))></textarea><small class="muted">"Edit and send through your usual tool. Nothing is sent by this app."</small></section>})}
                            <form class="workflow-form" on:submit=update><fieldset class="focus-form-fields" disabled=move||busy.get()>
                                <label for="waiting-action">"Update request"</label><select id="waiting-action" prop:value=move||if status=="waiting"{update_action.get()}else{"reopen".into()} on:change=move|ev|{if edit_version.get_untracked().is_none(){edit_version.set(Some(version));}update_action.set(event_target_value(&ev));next.set(None);}>
                                    {(w.status=="waiting").then(||view!{<option value="edit">"Edit request"</option><option value="snooze">"Snooze follow-up"</option><option value="follow_up">"Record a follow-up I sent"</option><option value="resolve">"Response received"</option><option value="cancel">"Cancel request"</option>})}{(w.status!="waiting").then(||view!{<option value="reopen">"Reopen request"</option>})}
                                </select>
                                {let editable=w.status=="waiting";view!{<Show when=move||editable && update_action.get()=="edit">{let default_title=default_title.clone();let default_person=default_person.clone();view!{<label for="waiting-edit-title">"Request"</label><input id="waiting-edit-title" maxlength="300" prop:value=move||edit_title.get().unwrap_or_else(||default_title.clone()) on:input=move|ev|{if edit_version.get_untracked().is_none(){edit_version.set(Some(version));}edit_title.set(Some(event_target_value(&ev)));}/><label for="waiting-edit-person">"Person or team"</label><input id="waiting-edit-person" maxlength="200" prop:value=move||edit_person.get().unwrap_or_else(||default_person.clone()) on:input=move|ev|{if edit_version.get_untracked().is_none(){edit_version.set(Some(version));}edit_person.set(Some(event_target_value(&ev)));}/>}}</Show>}}
                                {let open=w.status!="waiting";view!{<Show when=move||open || !["resolve","cancel"].contains(&update_action.get().as_str())>{let default_date=default_date.clone();view!{<label for="waiting-next">"Next follow-up date (UTC)"</label><input id="waiting-next" type="date" prop:value=move||next.get().unwrap_or_else(||if open || ["snooze","follow_up"].contains(&update_action.get().as_str()){plus_days(&utc_day(),3).unwrap_or_else(utc_day)}else{default_date.clone()}) on:input=move|ev|{if edit_version.get_untracked().is_none(){edit_version.set(Some(version));}next.set(Some(event_target_value(&ev)));}/>}}</Show>}}
                                <label for="waiting-update-note">"Note (optional)"</label><textarea id="waiting-update-note" maxlength="1000" rows="3" prop:value=move||note.get() on:input=move|ev|{if edit_version.get_untracked().is_none(){edit_version.set(Some(version));}note.set(event_target_value(&ev));}></textarea>
                                <details class="workflow-manage" on:change=move|_|{if edit_version.get_untracked().is_none(){edit_version.set(Some(version));}} on:input=move|_|{if edit_version.get_untracked().is_none(){edit_version.set(Some(version));}}><summary>"Add supporting evidence (optional)"</summary><EvidencePicker snap=snap project=project doc=edit_doc line=edit_line prefix="waiting-update" selected=selected notice=notice/></details>
                                <div class="workflow-actions"><button class="button secondary" type="submit" disabled=move||busy.get()>"Save update"</button><button class="button text" type="button" on:click=move|_|reset()>"Reset edits"</button></div><small class="muted">"Recording a follow-up or response is your confirmation of what happened. It does not send a message or mark a linked task complete."</small>
                            </fieldset></form>
                            <details class="workflow-manage"><summary>{format!("History · {} updates",w.history_count)}</summary>{w.history.into_iter().map(|h|view!{<article class="focus-change"><div class="workflow-topline"><span>{format!("{} · {}",h.action,h.contributor)}</span><small>{h.created_at}</small></div><p>{format!("{} · {} · {} · next {}",h.person,h.title,h.status,h.follow_up_on)}</p><p class="workflow-body">{h.note}</p><Citations evidence=h.evidence selected=selected notice=notice/></article>}).collect_view()}{(w.history_omitted>0).then(||view!{<p class="muted">{format!("{} older events are retained but omitted from this detail response.",w.history_omitted)}</p>})}</details>
                        </section>
                    }
                })}
            </div>
        </div>
    }
}
