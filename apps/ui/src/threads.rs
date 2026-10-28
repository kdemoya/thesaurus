use super::workflows::{Citations, Pending, action, chosen_project};
use super::*;

fn thread(snap: Snap, id: RwSignal<String>) -> Option<WorkThread> {
    let threads = snap.get()?.threads;
    threads
        .iter()
        .find(|t| t.id == id.get())
        .or_else(|| threads.iter().find(|t| t.status == "active"))
        .or_else(|| threads.first())
        .cloned()
}
#[component]
pub(super) fn Threads(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    view: RwSignal<String>,
) -> impl IntoView {
    let current = RwSignal::new(String::new());
    let project = RwSignal::new(String::new());
    let title = RwSignal::new(String::new());
    let scope = RwSignal::new(None::<Vec<String>>);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let note = RwSignal::new(String::new());
    let entry_kind = RwSignal::new("task".to_string());
    let record = RwSignal::new(String::new());
    let chosen_entries = RwSignal::new(Vec::<String>::new());
    let split_title = RwSignal::new(String::new());
    let merge_id = RwSignal::new(String::new());
    let renamed = RwSignal::new(None::<String>);
    let added_sources = RwSignal::new(Vec::<String>::new());
    let preview = RwSignal::new(None::<ThreadContext>);
    let preview_busy = RwSignal::new(false);
    let budget = RwSignal::new(8000usize);
    let connection = RwSignal::new(String::new());
    let approved = RwSignal::new(false);
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            notice.set("Create a project in Briefs first.".into());
            return;
        };
        let ids = scope.get_untracked().unwrap_or(p.source_ids);
        action(
            "/api/v1/threads".into(),
            "POST",
            json!({"project_id":p.id,"title":title.get_untracked(),"source_ids":ids}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                current.set(v["id"].as_str().unwrap_or_default().into());
                title.set(String::new());
                scope.set(None);
                preview.set(None);
                chosen_entries.set(vec![]);
            },
        );
    };
    let add_note = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if let Some(t) = thread(snap, current) {
            action(
                format!("/api/v1/threads/{}/entries", t.id),
                "POST",
                json!({"expected_version":t.version,"kind":"checkpoint","body":note.get_untracked(),"record_id":null,"evidence":[]}),
                snap,
                notice,
                busy,
                pending,
                move |_| {
                    note.set(String::new());
                    preview.set(None);
                },
            );
        }
    };
    let load_preview = move |_| {
        if preview_busy.get_untracked() {
            return;
        }
        let Some(t) = thread(snap, current) else {
            return;
        };
        preview_busy.set(true);
        approved.set(false);
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/threads/{}/context", t.id),
                "POST",
                Some(json!({"byte_budget":budget.get_untracked()})),
            )
            .await;
            preview_busy.set(false);
            if !thread(snap, current).is_some_and(|now| now.id == t.id) {
                return;
            }
            match result {
                Ok(value) => match serde_json::from_value(value) {
                    Ok(ctx) => {
                        preview.set(Some(ctx));
                        notice.set(String::new());
                    }
                    Err(_) => notice.set("Could not read context preview".into()),
                },
                Err(e) => notice.set(e),
            }
        });
    };
    view! {
        <div class="brief-layout workflow-layout">
            <aside class="brief-sidebar">
                <section class="panel"><div class="section-heading"><h2>"Threads"</h2></div>{move||snap.get().map(|s|s.threads.into_iter().map(|t|{let id=t.id.clone();let active=t.id;view!{<button class="brief-project" class:active=move||thread(snap,current).is_some_and(|t|t.id==active) on:click=move|_|{current.set(id.clone());preview.set(None);approved.set(false);chosen_entries.set(vec![]);renamed.set(None);added_sources.set(vec![]);record.set(String::new());}><span>{t.title}</span><small>{t.status}</small></button>}}).collect_view())}</section>
                <section class="panel brief-create"><div class="section-heading"><h2>"New thread"</h2></div><form on:submit=create>
                    <label for="thread-project">"Project"</label><select id="thread-project" prop:value=move||chosen_project(snap,project).map(|p|p.id).unwrap_or_default() on:change=move|ev|{project.set(event_target_value(&ev));scope.set(None);}>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select>
                    <label for="thread-title">"Work to continue"</label><input id="thread-title" required maxlength="200" prop:value=move||title.get() on:input=move|ev|title.set(event_target_value(&ev)) placeholder="Release readiness"/>
                    <fieldset class="runtime-sources"><legend>"Sources"</legend>{move||chosen_project(snap,project).map(|p|{let all=p.source_ids.clone();p.source_ids.into_iter().map(|id|{let all=all.clone();let check=id.clone();let label=snap.get().and_then(|s|s.sources.into_iter().find(|s|s.id==id)).map(|s|s.name).unwrap_or_else(||id.clone());view!{<label><input type="checkbox" prop:checked=move||scope.get().unwrap_or_else(||all.clone()).contains(&check) on:change=move|ev|{let checked=event_target_checked(&ev);let mut ids=scope.get_untracked().unwrap_or_else(||chosen_project(snap,project).map(|p|p.source_ids).unwrap_or_default());ids.retain(|s|s!=&id);if checked{ids.push(id.clone());}scope.set(Some(ids));}/><span>{label}</span></label>}}).collect_view()})}</fieldset>
                    <button class="button secondary" disabled=move||busy.get() type="submit">"Create thread"</button>
                </form></section>
            </aside>
            <div class="brief-main">
            {move||thread(snap,current).map(|t|{
                let thread_id=t.id.clone();let note_active=t.status=="active";let merged=t.status=="merged";let ids=t.source_ids.clone();let attach=t.id.clone();let split=t.id.clone();let merge=t.id.clone();let update=t.id.clone();let scope_update=t.id.clone();
                let selected_t=t.clone();let update_t=t.clone();let scope_t=t.clone();let resume_t=t.clone();
                view!{
                    <section class="panel"><div class="section-heading"><h2>{t.title.clone()}</h2><span>{format!("{} · v{}",t.status,t.version)}</span></div>
                        <p class="muted">{format!("Project: {} · {} sources",snap.get().and_then(|s|s.briefs.into_iter().find(|p|p.id==t.project_id)).map(|p|p.name).unwrap_or_else(||t.project_id.clone()),t.source_ids.len())}</p>
                        <details class="workflow-manage"><summary>"Manage thread"</summary>
                            <label for="thread-rename">"Title"</label><input id="thread-rename" maxlength="200" prop:value=move||renamed.get().unwrap_or_else(||selected_t.title.clone()) on:input=move|ev|renamed.set(Some(event_target_value(&ev)))/>
                            <div class="workflow-actions"><button class="button secondary" disabled=move||busy.get() || merged on:click=move|_|{action(format!("/api/v1/threads/{update}"),"PATCH",json!({"expected_version":update_t.version,"title":renamed.get_untracked().unwrap_or_else(||update_t.title.clone()),"status":update_t.status,"source_ids":update_t.source_ids}),snap,notice,busy,pending,move|_|renamed.set(None));}>"Rename"</button>
                                {let t=t.clone();view!{<button class="button text" disabled=move||busy.get() || merged on:click=move|_|action(format!("/api/v1/threads/{}",t.id),"PATCH",json!({"expected_version":t.version,"title":t.title,"status":if t.status=="closed"{"active"}else{"closed"},"source_ids":t.source_ids}),snap,notice,busy,pending,move|_|preview.set(None))>{if note_active{"Close thread"}else{"Reopen thread"}}</button>}}
                            </div>
                            <fieldset class="runtime-sources"><legend>"Add project sources"</legend>{snap.get().map(|s|{let project=s.briefs.iter().find(|p|p.id==t.project_id);project.map(|p|p.source_ids.iter().filter(|id|!ids.contains(id)).map(|id|{let check=id.clone();let change=id.clone();let label=s.sources.iter().find(|s|&s.id==id).map(|s|s.name.clone()).unwrap_or_else(||id.clone());view!{<label><input type="checkbox" prop:checked=move||added_sources.get().contains(&check) on:change=move|ev|{let checked=event_target_checked(&ev);added_sources.update(|ids|{ids.retain(|i|i!=&change);if checked{ids.push(change.clone());}});}/><span>{label}</span></label>}}).collect_view())})}</fieldset>
                            <button class="button secondary" disabled=move||busy.get() || added_sources.get().is_empty() || !note_active on:click=move|_|{let mut ids=scope_t.source_ids.clone();ids.extend(added_sources.get_untracked());action(format!("/api/v1/threads/{scope_update}"),"PATCH",json!({"expected_version":scope_t.version,"title":scope_t.title,"status":scope_t.status,"source_ids":ids}),snap,notice,busy,pending,move|_|{added_sources.set(vec![]);preview.set(None);});}>"Expand thread scope"</button>
                            <p class="muted">"Scope changes invalidate unused resume previews. Merged threads keep their history."</p>
                        </details>
                    </section>
                    <section class="panel"><div class="section-heading"><h2>"Checkpoints"</h2><span>{t.entries.len()}</span></div>
                        {t.entries.iter().cloned().map(|e|{let check=e.id.clone();let change=e.id.clone();view!{<article class="thread-entry"><div class="workflow-topline"><label><input type="checkbox" disabled=!note_active prop:checked=move||chosen_entries.get().contains(&check) on:change=move|ev|{let checked=event_target_checked(&ev);chosen_entries.update(|ids|{ids.retain(|i|i!=&change);if checked{ids.push(change.clone());}});}/><span>{e.kind}</span></label><small>{format!("{} · {}",e.contributor,e.created_at)}</small></div><p class="workflow-body">{e.body}</p>{e.truncated.then(||view!{<small class="orange">"Excerpt truncated · original record remains available."</small>})}<Citations evidence=e.evidence selected=selected notice=notice/></article>}}).collect_view()}
                        {t.entries.is_empty().then(||view!{<p class="muted">"No checkpoints yet. Save what to continue, or attach an existing task, observation, or agent result."</p>})}
                        {note_active.then(||view!{
                            <form class="workflow-form" on:submit=add_note><label for="thread-note">"Checkpoint"</label><textarea id="thread-note" required maxlength="4000" rows="3" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev)) placeholder="Current state, next step, and unresolved questions…"></textarea><small class="muted">"Saved as your note. It does not replace original source evidence."</small><button class="button secondary" disabled=move||busy.get() type="submit">"Save checkpoint"</button></form>
                            <div class="workflow-attach"><label for="thread-record-kind">"Attach existing"</label><select id="thread-record-kind" prop:value=move||entry_kind.get() on:change=move|ev|{entry_kind.set(event_target_value(&ev));record.set(String::new());}><option value="task">"Task"</option><option value="observation">"Observation"</option><option value="agent_run">"Agent output"</option></select>
                                <select aria-label="Record to attach" prop:value=move||record.get() on:change=move|ev|record.set(event_target_value(&ev))><option value="">"Choose record…"</option>{move||snap.get().zip(thread(snap,current)).map(|(s,t)|{
                                    let records:Vec<(String,String)>=match entry_kind.get().as_str(){"observation"=>s.observations.into_iter().filter(|o|!o.source_ids.is_empty() && o.source_ids.iter().all(|id|t.source_ids.contains(id))).map(|o|(o.id,o.body.chars().take(90).collect())).collect(),"agent_run"=>s.runtime_runs.into_iter().filter(|r|!matches!(r.status.as_str(),"starting"|"running"|"waiting_permission"|"cancelling") && r.source_ids.iter().all(|id|t.source_ids.contains(id))).map(|r|(r.id,format!("{} · {} · {}",r.connection,r.status,r.created_at))).collect(),_=>s.tasks.into_iter().filter(|task|s.documents.iter().any(|d|d.id==task.evidence.document_id && t.source_ids.contains(&d.source_id))).map(|task|(task.id,task.title)).collect()};records.into_iter().map(|(id,label)|view!{<option value=id>{label}</option>}).collect_view()
                                })}</select><button class="button secondary" disabled=move||busy.get() || record.get().is_empty() on:click=move|_|{action(format!("/api/v1/threads/{attach}/entries"),"POST",json!({"expected_version":t.version,"kind":entry_kind.get_untracked(),"record_id":record.get_untracked(),"body":"","evidence":[]}),snap,notice,busy,pending,move|_|{record.set(String::new());preview.set(None);});}>"Attach"</button>
                            </div>
                            <details class="workflow-manage"><summary>"Split or merge"</summary><p class="muted">"Split moves the checked checkpoints into a new thread with the same source scope. Merge moves another thread's checkpoints here, combines both source scopes, and preserves its history."</p>
                                <label for="thread-split-title">"New thread title"</label><input id="thread-split-title" maxlength="200" prop:value=move||split_title.get() on:input=move|ev|split_title.set(event_target_value(&ev))/><button class="button secondary" disabled=move||busy.get() || chosen_entries.get().is_empty() || split_title.get().trim().is_empty() on:click=move|_|action(format!("/api/v1/threads/{split}/split"),"POST",json!({"expected_version":t.version,"title":split_title.get_untracked(),"entry_ids":chosen_entries.get_untracked()}),snap,notice,busy,pending,move|v|{current.set(v["id"].as_str().unwrap_or_default().into());chosen_entries.set(vec![]);split_title.set(String::new());preview.set(None);} )>"Split selected checkpoints"</button>
                                <label for="thread-merge">"Merge into this thread"</label><select id="thread-merge" prop:value=move||merge_id.get() on:change=move|ev|merge_id.set(event_target_value(&ev))><option value="">"Choose another thread…"</option>{snap.get().map(|s|s.threads.into_iter().filter(|other|other.id!=t.id && other.project_id==t.project_id && other.status=="active").map(|other|view!{<option value=other.id>{other.title}</option>}).collect_view())}</select><button class="button secondary" disabled=move||busy.get() || merge_id.get().is_empty() on:click=move|_|{if let Some(other)=snap.get_untracked().and_then(|s|s.threads.into_iter().find(|other|other.id==merge_id.get_untracked())){action(format!("/api/v1/threads/{merge}/merge"),"POST",json!({"expected_version":t.version,"other_id":other.id,"other_expected_version":other.version}),snap,notice,busy,pending,move|_|{merge_id.set(String::new());chosen_entries.set(vec![]);preview.set(None);});}}>"Merge selected thread"</button>
                            </details>
                        })}
                    </section>
                    <section class="panel"><div class="section-heading"><h2>"Resume with an agent"</h2><span>"Preview → approve → run"</span></div><p class="muted">"Start a new session with a bounded snapshot of this thread. Sources, accepted brief sections, open work and attributed notes stay identifiable. Attach a finished agent output to a checkpoint when you want to carry it into a later resume."</p>
                        <div class="workflow-actions"><label for="thread-budget">"Context bytes"</label><select id="thread-budget" prop:value=move||budget.get().to_string() on:change=move|ev|{budget.set(event_target_value(&ev).parse().unwrap_or(8000));preview.set(None);approved.set(false);}>
                            <option value="4000">"4,000"</option><option value="8000">"8,000"</option><option value="12000">"12,000"</option></select><button class="button secondary" disabled=move||preview_busy.get() || !note_active on:click=load_preview>"Preview context · local"</button></div>
                        {move||preview.get().filter(|p|p.thread_id==thread_id).map(|p|{let ctx_id=p.id.clone();let bytes=serde_json::to_vec(&p).map(|v|v.len()).unwrap_or(0);let fresh=p.thread_version==resume_t.version;let has_indexed_sources=p.retrieval_policies.iter().any(|policy|policy.policy!="live");let shared=snap.get().is_some_and(|s|p.retrieval_policies.iter().filter(|policy|policy.policy!="live").all(|policy|s.sources.iter().any(|s|s.id==policy.source_id && s.allow_external)));let has_agent=snap.get().is_some_and(|s|s.connections.iter().any(|c|c.kind=="acp" && c.available));let delivery=resume_t.clone();view!{
                            <div class="context-preview"><p class="muted">{format!("{} selected sources · {} included revision pins · {} live-only exclusions",p.selected_source_ids.len(),p.sources.len(),p.omitted_source_ids.len())}</p><p>{format!("{} bytes · {} checkpoints · {} open tasks · {} observations",bytes,p.entries.len(),p.tasks.len(),p.observations.len())}</p><p class="muted">{format!("Omitted: {} checkpoints, {} tasks, {} observations, {} brief items, {} excerpt lines, {} waiting requests, {} preferences",p.omissions.entries,p.omissions.tasks,p.omissions.observations,p.omissions.brief_items,p.omissions.source_lines,p.omissions.waiting_for,p.omissions.priority_preferences)}</p>{p.omissions.notes.iter().map(|n|view!{<small class="muted">{n.clone()}</small>}).collect_view()}<details><summary>"Exact delivery snapshot"</summary><pre class="runtime-output">{serde_json::to_string_pretty(&p).unwrap_or_default()}</pre></details>
                            {(!fresh).then(||view!{<p class="orange">"Thread changed. Preview again before resuming."</p>})}{(!shared).then(||view!{<p class="muted">"Enable external sharing for every included indexed source before resuming."</p>})}{(!has_agent).then(||view!{<p class="muted">"Configure an ACP connection in Connections."</p>})}{(!has_indexed_sources).then(||view!{<p class="muted">"No indexed sources remain. Live-only cached content cannot be delivered as current context."</p>})}
                            <label for="thread-agent">"Agent connection"</label><select id="thread-agent" prop:value=move||{let c=connection.get();if !c.is_empty(){c}else{snap.get().and_then(|s|s.connections.into_iter().find(|c|c.kind=="acp" && c.available)).map(|c|c.id).unwrap_or_default()}} on:change=move|ev|{connection.set(event_target_value(&ev));approved.set(false);}>{move||snap.get().map(|s|s.connections.into_iter().filter(|c|c.kind=="acp" && c.available).map(|c|view!{<option value=c.id.clone()>{c.id.clone()}</option>}).collect_view())}</select>
                            <label class="checkbox-label"><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/><span>"Send this snapshot, allow retrieval from its included indexed sources, and use the selected agent's account allowance. Live-only caches are excluded."</span></label>
                            <button class="button primary" disabled=move||busy.get() || !approved.get() || !fresh || !shared || !has_indexed_sources || !has_agent || !note_active on:click=move|_|{let c=connection.get_untracked();let c=if !c.is_empty(){c}else{snap.get_untracked().and_then(|s|s.connections.into_iter().find(|c|c.kind=="acp" && c.available)).map(|c|c.id).unwrap_or_default()};action(format!("/api/v1/threads/{}/resume",delivery.id),"POST",json!({"context_id":ctx_id,"expected_version":delivery.version,"connection":c,"approve_usage":true}),snap,notice,busy,pending,move|_|{approved.set(false);preview.set(None);view.set("activity".into());});}>"Resume work"</button>
                            </div>
                        }})}
                    </section>
                    <section class="panel"><div class="section-heading"><h2>"Continuity history"</h2></div>{t.runs.into_iter().map(|r|{let thread_id=t.id.clone();let context_id=r.context_id;view!{<div class="workflow-history"><span>{format!("{} · {}",r.connection,r.status)}</span><code>{r.run_id}</code><small>{r.session_id.map(|id|format!("Session {id}")).unwrap_or_default()}</small><button class="source-link" on:click=move|_|view.set("activity".into())>"Activity →"</button><button class="source-link" on:click=move|_|{let path=format!("/api/v1/threads/{thread_id}/contexts/{context_id}");approved.set(false);spawn_local(async move{match api(&path,"GET",None).await{Ok(v)=>match serde_json::from_value(v["context"].clone()){Ok(ctx)=>preview.set(Some(ctx)),Err(_)=>notice.set("Could not read delivery snapshot".into())},Err(e)=>notice.set(e)}});}>"Delivered context"</button></div>}}).collect_view()}<details><summary>"Thread operations"</summary>{t.history.into_iter().map(|h|view!{<p class="muted">{format!("{} · {} · {} checkpoints",h.created_at,h.action,h.entry_ids.len())}{h.related_id.map(|id|view!{<code>{id}</code>})}</p>}).collect_view()}</details></section>
                }
            })}
            <Show when=move||thread(snap,current).is_none()><section class="panel"><div class="section-heading"><h2>"Continue your work"</h2></div><p class="muted">"Create a project in Briefs, then save a work thread here. Checkpoints and source evidence can travel to another authorized agent."</p></section></Show>
            </div>
        </div>
    }
}
