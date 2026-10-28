use super::workflows::{Citations, Pending, action, chosen_project};
use super::*;
use std::{cell::Cell, rc::Rc};

fn evidence(
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
    let quote = d
        .body
        .lines()
        .nth(n as usize - 1)
        .ok_or("Source line does not exist")?;
    if quote.len() > 2000 || quote.trim().is_empty() {
        return Err("Choose a nonempty line of at most 2,000 bytes".into());
    }
    Ok(vec![Evidence {
        document_id: d.id,
        revision: d.revision,
        line: n,
        quote: quote.into(),
    }])
}
#[component]
fn SourcePicker(
    snap: Snap,
    project: RwSignal<String>,
    doc: RwSignal<String>,
    line: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    notice: RwSignal<String>,
) -> impl IntoView {
    view! {<div class="waiting-evidence-picker"><label>"Exact source line"<select prop:value=move||doc.get() on:change=move|ev|{doc.set(event_target_value(&ev));line.set("1".into());}><option value="">"Choose source…"</option>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.documents.into_iter().filter(|d|p.source_ids.contains(&d.source_id)).map(|d|view!{<option value=d.id>{format!("{} · r{}",d.title,d.revision)}</option>}).collect_view())}</select></label><label>"Line"<input type="number" min="1" prop:value=move||line.get() on:input=move|ev|line.set(event_target_value(&ev))/></label>{move||match evidence(snap,project,&doc.get(),&line.get()){Ok(e)=>view!{<Citations evidence=e selected=selected notice=notice/>}.into_any(),Err(e)=>view!{<small class="muted">{e}</small>}.into_any()}}</div>}
}
#[component]
pub(super) fn Delivery(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    project: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let list = RwSignal::new(None::<CommitmentList>);
    let detail = RwSignal::new(None::<Commitment>);
    let picked = RwSignal::new(String::new());
    let filter = RwSignal::new("all".to_string());
    let offset = RwSignal::new(0usize);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let loading = RwSignal::new(false);
    let title = RwSignal::new(String::new());
    let owner = RwSignal::new(String::new());
    let promised = RwSignal::new(super::focus::utc_day());
    let due = RwSignal::new(String::new());
    let task = RwSignal::new(String::new());
    let doc = RwSignal::new(String::new());
    let line = RwSignal::new("1".to_string());
    let create_note = RwSignal::new(String::new());
    let command = RwSignal::new("link".to_string());
    let kind = RwSignal::new("discussion".to_string());
    let edit_title = RwSignal::new(String::new());
    let edit_owner = RwSignal::new(String::new());
    let edit_due = RwSignal::new(String::new());
    let edit_doc = RwSignal::new(String::new());
    let edit_line = RwSignal::new("1".to_string());
    let edit_note = RwSignal::new(String::new());
    let version = RwSignal::new(0i64);
    let link_ids = RwSignal::new(Vec::<String>::new());
    let editing = RwSignal::new(String::new());
    let active_project = RwSignal::new(String::new());
    let history_page = RwSignal::new(None::<CommitmentHistory>);
    let history_offset = RwSignal::new(0usize);
    let serial = Rc::new(Cell::new(0u64));
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let clean = alive.clone();
    on_cleanup(move || clean.store(false, std::sync::atomic::Ordering::Relaxed));
    let reset_edit = move |w: &Commitment| {
        version.set(w.version);
        edit_title.set(w.title.clone());
        edit_owner.set(w.owner.clone());
        edit_due.set(w.due.clone().unwrap_or_default());
        edit_doc.set(String::new());
        edit_line.set("1".into());
        edit_note.set(String::new());
        link_ids.set(vec![]);
        command.set(
            if w.status == "proposed" {
                "confirm_completion"
            } else if ["completed", "cancelled"].contains(&w.status.as_str()) {
                "reopen"
            } else {
                "link"
            }
            .into(),
        );
    };
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        let p = chosen_project(snap, project);
        let selected_id = picked.get();
        let status = filter.get();
        let start = offset.get();
        let history_start = history_offset.get();
        let Some(p) = p else {
            return;
        };
        if active_project.get_untracked() != p.id {
            active_project.set(p.id.clone());
            list.set(None);
            detail.set(None);
            picked.set(String::new());
            offset.set(0);
            editing.set(String::new());
            history_offset.set(0);
            title.set(String::new());
            owner.set(String::new());
            due.set(String::new());
            task.set(String::new());
            doc.set(String::new());
            create_note.set(String::new());
            pending.set(None);
            tick.update(|n| *n += 1);
            return;
        }
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        loading.set(true);
        spawn_local(async move {
            let result = api(
                &format!(
                    "/api/v1/projects/{}/commitments?status={status}&offset={start}&limit=25",
                    p.id
                ),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<CommitmentList>(v)
                    .map_err(|_| "Could not read commitments".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            let data = match result {
                Ok(v) => v,
                Err(e) => {
                    notice.set(e);
                    loading.set(false);
                    return;
                }
            };
            let id = if selected_id.is_empty() {
                data.items.first().map(|w| w.id.clone()).unwrap_or_default()
            } else {
                selected_id
            };
            list.set(Some(data));
            if id.is_empty() {
                detail.set(None);
                history_page.set(None);
                loading.set(false);
                return;
            }
            let result = api(&format!("/api/v1/commitments/{id}"), "GET", None)
                .await
                .and_then(|v| {
                    serde_json::from_value::<Commitment>(v)
                        .map_err(|_| "Could not read commitment".into())
                });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(w) if w.project_id == p.id => {
                    if editing.get_untracked() != w.id {
                        reset_edit(&w);
                        editing.set(w.id.clone());
                    }
                    detail.set(Some(w));
                }
                Ok(_) => detail.set(None),
                Err(e) => {
                    notice.set(e);
                    detail.set(None);
                }
            }
            let result = api(
                &format!("/api/v1/commitments/{id}/history?offset={history_start}&limit=10"),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<CommitmentHistory>(v)
                    .map_err(|_| "Could not read promise history".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(h) => history_page.set(Some(h)),
                Err(e) => notice.set(e),
            }
            loading.set(false);
        });
    });
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let e = match evidence(snap, project, &doc.get_untracked(), &line.get_untracked()) {
            Ok(e) => e,
            Err(e) => {
                notice.set(e);
                return;
            }
        };
        let linked = task.get_untracked();
        if e.is_empty() && linked.is_empty() {
            notice.set("Choose source evidence or an accepted task".into());
            return;
        }
        let project_id = p.id.clone();
        action(
            "/api/v1/commitments".into(),
            "POST",
            json!({"project_id":p.id,"task_id":if linked.is_empty(){None}else{Some(linked)},"title":title.get_untracked(),"owner":owner.get_untracked(),"promised_on":promised.get_untracked(),"due":if due.get_untracked().is_empty(){None}else{Some(due.get_untracked())},"evidence":e,"note":create_note.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if !chosen_project(snap, project).is_some_and(|p| p.id == project_id) {
                    return;
                }
                picked.set(v["id"].as_str().unwrap_or_default().into());
                title.set(String::new());
                owner.set(String::new());
                task.set(String::new());
                doc.set(String::new());
                due.set(String::new());
                create_note.set(String::new());
                filter.set("all".into());
                offset.set(0);
                history_offset.set(0);
                tick.update(|n| *n += 1);
            },
        );
    };
    let update = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(w) = detail.get_untracked() else {
            return;
        };
        let cmd = command.get_untracked();
        let amend = cmd == "amend";
        let attach = amend || cmd == "link";
        let e = if attach {
            match evidence(
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
        } else {
            vec![]
        };
        let selected_id = w.id.clone();
        let project_id = w.project_id.clone();
        action(
            format!("/api/v1/commitments/{}", w.id),
            "PATCH",
            json!({"expected_version":version.get_untracked(),"action":cmd,"title":if amend{Some(edit_title.get_untracked())}else{None},"owner":if amend{Some(edit_owner.get_untracked())}else{None},"due":if amend && !edit_due.get_untracked().is_empty(){Some(edit_due.get_untracked())}else{None},"clear_due":amend && edit_due.get_untracked().is_empty(),"kind":if cmd=="link"{Some(kind.get_untracked())}else{None},"evidence":e,"link_ids":if cmd=="propose_completion"{link_ids.get_untracked()}else{vec![]},"note":edit_note.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if !chosen_project(snap, project).is_some_and(|p| p.id == project_id)
                    || !detail.get_untracked().is_some_and(|d| d.id == selected_id)
                {
                    return;
                }
                if let Ok(w) = serde_json::from_value::<Commitment>(v) {
                    reset_edit(&w);
                    detail.set(Some(w));
                }
                history_offset.set(0);
                tick.update(|n| *n += 1);
            },
        );
    };
    view! {
        <div class="brief-layout waiting-layout"><aside class="brief-sidebar"><section class="panel brief-create"><div class="section-heading"><h2>"Capture a promise"</h2></div><form on:submit=create><fieldset class="focus-form-fields" disabled=move||busy.get()>
            <label>"Commitment"<input required maxlength="300" prop:value=move||title.get() on:input=move|ev|title.set(event_target_value(&ev))/></label><label>"Promised by"<input required maxlength="200" prop:value=move||owner.get() on:input=move|ev|owner.set(event_target_value(&ev))/></label>
            <label>"Promised on (UTC)"<input type="date" required prop:value=move||promised.get() on:input=move|ev|promised.set(event_target_value(&ev))/></label><label>"Due (optional)"<input type="date" prop:value=move||due.get() on:input=move|ev|due.set(event_target_value(&ev))/></label>
            <label>"Accepted task (optional)"<select prop:value=move||task.get() on:change=move|ev|task.set(event_target_value(&ev))><option value="">"Standalone promise"</option>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.tasks.into_iter().filter(|t|t.status=="accepted" && s.documents.iter().any(|d|d.id==t.evidence.document_id && p.source_ids.contains(&d.source_id))).map(|t|view!{<option value=t.id>{t.title}</option>}).collect_view())}</select></label>
            <SourcePicker snap=snap project=project doc=doc line=line selected=selected notice=notice/><small class="muted">"A linked task supplies its original evidence. Source quotations remain pinned to this revision."</small><label>"Context"<textarea maxlength="2000" prop:value=move||create_note.get() on:input=move|ev|create_note.set(event_target_value(&ev))></textarea></label><button type="submit" class="button primary">"Capture promise"</button>
        </fieldset></form></section></aside><div class="brief-main"><section class="panel"><div class="section-heading"><h2>"Commitment ledger"</h2><span>{move||list.get().map(|l|format!("{} matching",l.total)).unwrap_or_default()}</span></div>
            <div class="workflow-actions"><label>"Status"<select prop:value=move||filter.get() on:change=move|ev|{filter.set(event_target_value(&ev));offset.set(0);picked.set(String::new());history_offset.set(0);}><option value="all">"All"</option><option value="open">"Open"</option><option value="proposed">"Completion proposed"</option><option value="completed">"Completed"</option><option value="cancelled">"Cancelled"</option></select></label><span class="muted">{move||if loading.get(){"Refreshing…"}else{"Completion requires your review"}}</span></div>
            <div class="task-table-scroll"><table class="task-table"><thead><tr><th>"Promise"</th><th>"Owner"</th><th>"Due"</th><th>"State"</th></tr></thead><tbody>{move||list.get().map(|l|l.items.into_iter().map(|w|{let id=w.id;view!{<tr><td><button class="task-title" on:click=move|_|{picked.set(id.clone());history_offset.set(0);}>{w.title}</button></td><td>{w.owner}</td><td>{w.due.unwrap_or_else(||"—".into())}</td><td>{w.status}</td></tr>}}).collect_view())}</tbody></table></div>
            <div class="workflow-actions"><button class="button text" disabled=move||offset.get()==0 on:click=move|_|{offset.update(|n|*n=n.saturating_sub(25));picked.set(String::new());}>"← Previous"</button><button class="button text" disabled=move||list.get().is_none_or(|l|l.next_offset.is_none()) on:click=move|_|{if let Some(next)=list.get_untracked().and_then(|l|l.next_offset){offset.set(next);picked.set(String::new());}}>"Next →"</button></div>
        </section>
        {move||detail.get().map(|w|{
            let repository_owner=w.clone();let promise=w.promise_evidence.clone();let initial=w.clone();let links=w.links.clone();let implementations=w.links.iter().filter(|l|l.kind=="implementation").cloned().collect::<Vec<_>>();
            view!{<section class="panel"><div class="section-heading"><h2>{w.title}</h2><span>{format!("{} · v{}",w.status,w.version)}</span></div><p class="muted">{format!("{} promised on {} · task: {}",w.owner,w.promised_on,w.linked_task_status.unwrap_or_else(||"not linked".into()))}</p><Show when=move||initial.stale_evidence><p class="orange">"Some source revisions have changed. Original evidence remains available."</p></Show><h3>"Original promise"</h3><Citations evidence=promise selected=selected notice=notice/>
                <h3>"Delivery trace"</h3>{links.into_iter().map(|l|view!{<details><summary>{format!("{} · {} · {}",l.kind,l.created_at,l.contributor)}</summary><p>{l.note}</p><small class="muted">{format!("Link {}",l.id)}</small><Citations evidence=l.evidence selected=selected notice=notice/>{l.repository.map(|r|view!{<div class="thread-context-section"><p>{format!("Repository {} · report {} · {}",r.repository_id,r.report_id,r.report_created_at)}</p><p>{format!("Range {} → {}",r.from_commit,r.to_commit)}</p><strong>{format!("Commit {} · {}",r.commit_id,r.commit_subject)}</strong><ul>{r.paths.into_iter().map(|p|view!{<li><code>{p}</code></li>}).collect_view()}</ul><p class="muted">{format!("{} commit paths omitted · report digest {}",r.paths_omitted,r.report_hash)}</p>{r.report_omissions.into_iter().map(|o|view!{<small class="muted">{o}</small>}).collect_view()}</div>})}</details>}).collect_view()}
                {w.completion_proposal.map(|p|view!{<div class="thread-context-section"><h3>"Completion proposal"</h3><p>{p.rationale}</p><p class="muted">{format!("{} · {} · {} implementation link(s)",p.contributor,p.proposed_at,p.link_ids.len())}</p></div>})}
                <RepositoryLinker commitment=repository_owner snap=snap notice=notice tick=tick/><form on:submit=update><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Action"<select prop:value=move||command.get() on:change=move|ev|command.set(event_target_value(&ev))><option value="link">"Link later evidence"</option><option value="amend">"Amend promise"</option><option value="propose_completion">"Propose completion"</option><option value="confirm_completion">"Confirm reviewed completion"</option><option value="reject_completion">"Reject completion proposal"</option><option value="cancel">"Cancel promise"</option><option value="reopen">"Reopen promise"</option></select></label>
                <Show when=move||command.get()=="amend"><label>"Updated promise"<input maxlength="300" prop:value=move||edit_title.get() on:input=move|ev|edit_title.set(event_target_value(&ev))/></label><label>"Owner"<input maxlength="200" prop:value=move||edit_owner.get() on:input=move|ev|edit_owner.set(event_target_value(&ev))/></label><label>"Due (blank clears)"<input type="date" prop:value=move||edit_due.get() on:input=move|ev|edit_due.set(event_target_value(&ev))/></label></Show>
                <Show when=move||command.get()=="link"><label>"Evidence kind"<select prop:value=move||kind.get() on:change=move|ev|kind.set(event_target_value(&ev))><option value="discussion">"Later discussion"</option><option value="implementation">"Implementation / delivered work"</option></select></label></Show>
                <Show when=move||["link","amend"].contains(&command.get().as_str())><SourcePicker snap=snap project=project doc=edit_doc line=edit_line selected=selected notice=notice/></Show>
                <Show when=move||command.get()=="propose_completion"><p class="muted">"Select implementation evidence to review:"</p>{implementations.clone().into_iter().map(|l|{let id=l.id.clone();let check=l.id;view!{<label><input type="checkbox" prop:checked=move||link_ids.get().contains(&check) on:change=move|ev|{let yes=event_target_checked(&ev);link_ids.update(|ids|{ids.retain(|v|v!=&id);if yes{ids.push(id.clone());}});}/>{format!("{} · {}",l.created_at,l.note)}</label>}}).collect_view()}</Show>
                <label>"Reason / context"<textarea maxlength="2000" prop:value=move||edit_note.get() on:input=move|ev|edit_note.set(event_target_value(&ev))></textarea></label><p class="muted">"Proposing or confirming completion never changes the linked task. Confirm only after reading the cited implementation evidence."</p><div class="workflow-actions"><button type="submit" class="button primary">"Record action"</button><button type="button" class="button text" on:click=move|_|{if let Some(w)=detail.get_untracked(){reset_edit(&w);pending.set(None);}}>"Discard edits / use current version"</button></div>
                </fieldset></form>
            </section>}
        })}
        {move||history_page.get().map(|h|view!{<section class="panel"><div class="section-heading"><h2>"Promise history"</h2><span>{format!("{} changes",h.total)}</span></div>{h.items.into_iter().map(|e|view!{<details><summary>{format!("v{} · {} · {} · {}",e.version,e.action,e.contributor,e.created_at)}</summary><p>{format!("{} · {} · due {} · {}",e.title,e.owner,e.due.unwrap_or_else(||"none".into()),e.status)}</p><p>{e.note}</p><Citations evidence=e.evidence selected=selected notice=notice/></details>}).collect_view()}<div class="workflow-actions"><button class="button text" disabled=move||history_offset.get()==0 on:click=move|_|history_offset.update(|n|*n=n.saturating_sub(10))>"← Newer"</button><button class="button text" disabled=h.next_offset.is_none() on:click=move|_|{if let Some(n)=h.next_offset{history_offset.set(n);}}>"Older →"</button></div></section>})}
        </div></div>
    }
}

#[component]
fn RepositoryLinker(
    commitment: Commitment,
    snap: Snap,
    notice: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let owner = StoredValue::new(commitment);
    let repositories = RwSignal::new(Vec::<RepositoryConnection>::new());
    let repository = RwSignal::new(String::new());
    let reports = RwSignal::new(Vec::<RepositoryReport>::new());
    let report_id = RwSignal::new(String::new());
    let commit_id = RwSignal::new(String::new());
    let paths = RwSignal::new(None::<Vec<String>>);
    let note = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let serial = Rc::new(Cell::new(0u64));
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let clean = alive.clone();
    on_cleanup(move || clean.store(false, std::sync::atomic::Ordering::Relaxed));
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let rid = repository.get();
        let project = owner.get_value().project_id;
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        spawn_local(async move {
            let result = api("/api/v1/repositories", "GET", None)
                .await
                .and_then(|v| {
                    serde_json::from_value::<Vec<RepositoryConnection>>(v)
                        .map_err(|_| "Could not read repositories".into())
                });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            let list = match result {
                Ok(list) => list
                    .into_iter()
                    .filter(|r| r.project_id == project)
                    .collect::<Vec<_>>(),
                Err(e) => {
                    notice.set(e);
                    return;
                }
            };
            let id = if list.iter().any(|r| r.id == rid) {
                rid
            } else {
                list.first().map(|r| r.id.clone()).unwrap_or_default()
            };
            repositories.set(list);
            if repository.get_untracked() != id {
                repository.set(id.clone());
                reports.set(vec![]);
                report_id.set(String::new());
                commit_id.set(String::new());
                paths.set(None);
                return;
            }
            if id.is_empty() {
                return;
            }
            let result = api(&format!("/api/v1/repositories/{id}/reports"), "GET", None)
                .await
                .and_then(|v| {
                    serde_json::from_value::<Vec<RepositoryReport>>(v)
                        .map_err(|_| "Could not read immutable repository reports".into())
                });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(v) => reports.set(v),
                Err(e) => notice.set(e),
            }
        });
    });
    let report = move || {
        reports
            .get()
            .into_iter()
            .find(|r| r.id == report_id.get())
            .or_else(|| reports.get().first().cloned())
    };
    let commit = move || {
        report().and_then(|r| {
            r.commits
                .iter()
                .find(|c| c.id == commit_id.get())
                .or_else(|| r.commits.first())
                .cloned()
        })
    };
    let picked_paths = move || {
        paths.get().unwrap_or_else(|| {
            commit()
                .map(|c| c.files.into_iter().take(20).collect())
                .unwrap_or_default()
        })
    };
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(report) = report() else {
            notice.set("Generate a repository report first".into());
            return;
        };
        let Some(commit) = commit() else {
            notice.set("Select a commit from this report".into());
            return;
        };
        let picked = picked_paths();
        if picked.is_empty() {
            notice.set("Choose at least one reported file path".into());
            return;
        }
        let w = owner.get_value();
        action(
            format!("/api/v1/commitments/{}/repository-links", w.id),
            "POST",
            json!({"expected_version":w.version,"report_id":report.id,"commit_id":commit.id,"paths":picked,"note":note.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |_| {
                note.set(String::new());
                tick.update(|n| *n += 1);
            },
        );
    };
    view! {<details><summary>"Link a repository report / commit"</summary><p class="muted">"Select an existing immutable report and exact commit/files. This links recorded implementation evidence; it performs no Git operation and does not imply deployment or completion."</p><form on:submit=submit><fieldset class="focus-form-fields" disabled=move||busy.get()||["completed","cancelled"].contains(&owner.get_value().status.as_str())><label>"Repository"<select prop:value=move||repository.get() on:change=move|ev|{repository.set(event_target_value(&ev));reports.set(vec![]);report_id.set(String::new());commit_id.set(String::new());paths.set(None);}>{move||repositories.get().into_iter().map(|r|view!{<option value=r.id>{r.name}</option>}).collect_view()}</select></label><label>"Pinned report"<select prop:value=move||report().map(|r|r.id).unwrap_or_default() on:change=move|ev|{report_id.set(event_target_value(&ev));commit_id.set(String::new());paths.set(None);}>{move||reports.get().into_iter().map(|r|view!{<option value=r.id>{format!("{} · {} → {}",r.created_at,r.from_commit,r.to_commit)}</option>}).collect_view()}</select></label><label>"Exact commit"<select prop:value=move||commit().map(|c|c.id).unwrap_or_default() on:change=move|ev|{commit_id.set(event_target_value(&ev));paths.set(None);}>{move||report().map(|r|r.commits.into_iter().map(|c|view!{<option value=c.id.clone()>{format!("{} · {}",c.id,c.subject)}</option>}).collect_view())}</select></label><fieldset class="runtime-sources"><legend>"Reported file paths (choose 1–20)"</legend>{move||commit().map(|c|c.files.into_iter().map(|path|{let check=path.clone();let change=path.clone();view!{<label><input type="checkbox" prop:checked=move||picked_paths().contains(&check) on:change=move|ev|{let yes=event_target_checked(&ev);let mut ids=picked_paths();ids.retain(|p|p!=&change);if yes{if ids.len()>=20{notice.set("At most 20 paths per delivery link".into());return;}ids.push(change.clone());}paths.set(Some(ids));}/>{path}</label>}}).collect_view())}</fieldset><label>"Why this implements the commitment"<textarea required maxlength="2000" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))></textarea></label><button type="submit" class="button primary">"Attach pinned implementation evidence"</button></fieldset></form></details>}
}
