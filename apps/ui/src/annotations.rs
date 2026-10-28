use super::workflows::{Citations, Pending, action};
use super::*;
use std::{cell::Cell, rc::Rc};
#[component]
pub(super) fn DocumentAnnotations(
    document: Document,
    line: RwSignal<String>,
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    let document = StoredValue::new(document);
    let notes = RwSignal::new(None::<NotePage>);
    let offset = RwSignal::new(0usize);
    let tick = RwSignal::new(0u64);
    let body = RwSignal::new(String::new());
    let emphasis = RwSignal::new(false);
    let editing = RwSignal::new(None::<AnchoredNote>);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let history = RwSignal::new(None::<NoteHistoryPage>);
    let history_note = RwSignal::new(String::new());
    let history_offset = RwSignal::new(0usize);
    let archived = RwSignal::new(false);
    let serial = Rc::new(Cell::new(0u64));
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let c = alive.clone();
    on_cleanup(move || c.store(false, std::sync::atomic::Ordering::Relaxed));
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        let start = offset.get();
        let all = archived.get();
        let hn = history_note.get();
        let ho = history_offset.get();
        let d = document.get_value();
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        spawn_local(async move {
            let result = api(
                &format!(
                    "/api/v1/documents/{}/notes?revision={}&offset={start}&include_archived={all}",
                    d.id, d.revision
                ),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<NotePage>(v)
                    .map_err(|_| "Could not read anchored notes".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(v) => notes.set(Some(v)),
                Err(e) => notice.set(e),
            }
            if !hn.is_empty() {
                let result = api(
                    &format!("/api/v1/notes/{hn}/history?offset={ho}&limit=10"),
                    "GET",
                    None,
                )
                .await
                .and_then(|v| {
                    serde_json::from_value::<NoteHistoryPage>(v)
                        .map_err(|_| "Could not read note history".into())
                });
                if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                    return;
                }
                match result {
                    Ok(h) => history.set(Some(h)),
                    Err(e) => notice.set(e),
                }
            }
        });
    });
    let anchor = move || {
        let d = document.get_value();
        let n = line
            .get()
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
            return Err("Choose a nonempty line up to 2,000 bytes");
        }
        Ok(Evidence {
            document_id: d.id,
            revision: d.revision,
            line: n,
            quote: q.into(),
        })
    };
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let old = editing.get_untracked();
        let (path, method, request) = if let Some(n) = old {
            (
                format!("/api/v1/notes/{}", n.id),
                "PATCH",
                json!({"expected_version":n.version,"body":body.get_untracked(),"emphasis":emphasis.get_untracked(),"status":n.status}),
            )
        } else {
            let e = match anchor() {
                Ok(e) => e,
                Err(e) => {
                    notice.set(e.into());
                    return;
                }
            };
            (
                "/api/v1/notes".into(),
                "POST",
                json!({"evidence":e,"body":body.get_untracked(),"emphasis":emphasis.get_untracked()}),
            )
        };
        action(
            path,
            method,
            request,
            snap,
            notice,
            busy,
            pending,
            move |_| {
                editing.set(None);
                body.set(String::new());
                emphasis.set(false);
                tick.update(|n| *n += 1);
            },
        );
    };
    view! {<details class="workflow-manage"><summary>"Conversation notes & corrections"</summary><section class="panel"><div class="section-heading"><h3>"Anchored notes"</h3><span>{move||notes.get().map(|n|format!("{} notes",n.total)).unwrap_or_default()}</span></div><p class="muted">"Click a transcript line to choose an anchor, or enter its line number. Emphasis is preserved as attributed user context in summaries and shared memory."</p>
    <form on:submit=save><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Source line"<input type="number" min="1" disabled=move||editing.get().is_some() prop:value=move||line.get() on:input=move|ev|line.set(event_target_value(&ev))/></label>
    {move||if let Some(n)=editing.get(){view!{<Citations evidence=vec![n.evidence] selected=selected notice=notice/>}.into_any()}else{match anchor(){Ok(e)=>view!{<blockquote>{e.quote}</blockquote>}.into_any(),Err(e)=>view!{<small class="muted">{e}</small>}.into_any()}}}
    <label>"Your note"<textarea required rows="3" maxlength="4000" prop:value=move||body.get() on:input=move|ev|body.set(event_target_value(&ev))></textarea></label><label class="checkbox-label"><input type="checkbox" prop:checked=move||emphasis.get() on:change=move|ev|emphasis.set(event_target_checked(&ev))/><span>"Emphasize in summaries"</span></label><div class="workflow-actions"><button type="submit" class="button primary">{move||if editing.get().is_some(){"Save note edit"}else{"Attach note"}}</button><button type="button" class="button text" on:click=move|_|{editing.set(None);body.set(String::new());emphasis.set(false);pending.set(None);}>"Clear edit"</button></div></fieldset></form>
    <label><input type="checkbox" prop:checked=move||archived.get() on:change=move|ev|{archived.set(event_target_checked(&ev));offset.set(0);}/>"Include archived"</label>
    {move||notes.get().map(|page|page.items.into_iter().map(|n|{let edit=n.clone();let archive=n.clone();let id=n.id.clone();view!{<article class="observation"><small>{format!("{} · line {} · v{} · {}{}",n.contributor,n.evidence.line,n.version,n.status,if n.emphasis{" · EMPHASIS"}else{""})}</small><p>{n.body}</p><div class="workflow-actions"><button class="button text" disabled=move||busy.get() on:click=move|_|{editing.set(Some(edit.clone()));body.set(edit.body.clone());emphasis.set(edit.emphasis);line.set(edit.evidence.line.to_string());pending.set(None);}>"Edit"</button><button class="button text" disabled=move||busy.get() on:click=move|_|{action(format!("/api/v1/notes/{}",archive.id),"PATCH",json!({"expected_version":archive.version,"body":archive.body,"emphasis":archive.emphasis,"status":if archive.status=="active"{"archived"}else{"active"}}),snap,notice,busy,pending,move|_|{editing.set(None);body.set(String::new());tick.update(|n|*n+=1);});}>{if n.status=="active"{"Archive"}else{"Restore"}}</button><button class="button text" on:click=move|_|{history_note.set(id.clone());history_offset.set(0);}>"History"</button></div></article>}}).collect_view())}
    <div class="workflow-actions"><button class="button text" disabled=move||offset.get()==0 on:click=move|_|offset.update(|n|*n=n.saturating_sub(25))>"← Previous"</button><button class="button text" disabled=move||notes.get().is_none_or(|n|n.next_offset.is_none()) on:click=move|_|{if let Some(n)=notes.get_untracked().and_then(|n|n.next_offset){offset.set(n);}}>"Next →"</button></div>
    {move||history.get().map(|h|view!{<details open><summary>{format!("Note history · {} versions",h.total)}</summary>{h.items.into_iter().map(|e|view!{<article class="observation"><small>{format!("{} · v{} · {} · {}",e.action,e.record.version,e.record.contributor,e.record.updated_at)}</small><p>{e.record.body}</p><p class="muted">{format!("{} · emphasis {}",e.record.status,e.record.emphasis)}</p></article>}).collect_view()}<div class="workflow-actions"><button class="button text" disabled=move||history_offset.get()==0 on:click=move|_|history_offset.update(|n|*n=n.saturating_sub(10))>"← Newer"</button><button class="button text" disabled=h.next_offset.is_none() on:click=move|_|{if let Some(n)=h.next_offset{history_offset.set(n);}}>"Older →"</button></div></details>})}
    </section><Corrections source=document.get_value().source_id snap=snap notice=notice/></details>}
}
#[component]
fn Corrections(source: String, snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let source = StoredValue::new(source);
    let set = RwSignal::new(None::<CorrectionSet>);
    let draft = RwSignal::new(None::<Vec<CorrectionRule>>);
    let kind = RwSignal::new("speaker".to_string());
    let from = RwSignal::new(String::new());
    let to = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let preview = RwSignal::new(None::<CorrectionPreview>);
    let audit = RwSignal::new(None::<CorrectionAuditPage>);
    let audit_offset = RwSignal::new(0usize);
    let tick = RwSignal::new(0u64);
    let draft_version = RwSignal::new(None::<i64>);
    let serial = Rc::new(Cell::new(0u64));
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let c = alive.clone();
    on_cleanup(move || c.store(false, std::sync::atomic::Ordering::Relaxed));
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        let offset = audit_offset.get();
        let source = source.get_value();
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/sources/{source}/corrections"),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<CorrectionSet>(v)
                    .map_err(|_| "Could not read correction rules".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(v) => set.set(Some(v)),
                Err(e) => notice.set(e),
            }
            let result = api(
                &format!("/api/v1/sources/{source}/corrections/history?offset={offset}&limit=5"),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<CorrectionAuditPage>(v)
                    .map_err(|_| "Could not read correction history".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match result {
                Ok(v) => audit.set(Some(v)),
                Err(e) => notice.set(e),
            }
        });
    });
    let add = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if draft_version.get_untracked().is_none() {
            draft_version.set(set.get_untracked().map(|s| s.version));
        }
        let mut rules = draft
            .get_untracked()
            .unwrap_or_else(|| set.get_untracked().map(|s| s.rules).unwrap_or_default());
        rules.push(CorrectionRule {
            kind: kind.get_untracked(),
            from: from.get_untracked(),
            to: to.get_untracked(),
        });
        draft.set(Some(rules));
        from.set(String::new());
        to.set(String::new());
        preview.set(None);
    };
    view! {<section class="panel"><div class="section-heading"><h3>"Speaker & terminology corrections"</h3><span>{move||set.get().map(|s|format!("rules v{}",s.version)).unwrap_or_default()}</span></div><p class="muted">"Rules apply to this source. Speaker aliases match exact owner labels; terms replace exact, case sensitive text in extracted titles. Original transcript lines and reviewed tasks remain unchanged."</p><form on:submit=add><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Kind"<select prop:value=move||kind.get() on:change=move|ev|kind.set(event_target_value(&ev))><option value="speaker">"Speaker alias"</option><option value="term">"Company term"</option></select></label><label>"Original"<input required maxlength="100" prop:value=move||from.get() on:input=move|ev|from.set(event_target_value(&ev))/></label><label>"Corrected"<input required maxlength="100" prop:value=move||to.get() on:input=move|ev|to.set(event_target_value(&ev))/></label><button type="submit" class="button secondary">"Add to draft rules"</button></fieldset></form>
        {move||draft.get().or_else(||set.get().map(|s|s.rules)).unwrap_or_default().into_iter().enumerate().map(|(i,r)|view!{<div class="workflow-actions"><code>{format!("{}: {} → {}",r.kind,r.from,r.to)}</code><button class="button text" disabled=move||busy.get() on:click=move|_|{if draft_version.get_untracked().is_none(){draft_version.set(set.get_untracked().map(|s|s.version));}let mut rules=draft.get_untracked().unwrap_or_else(||set.get_untracked().map(|s|s.rules).unwrap_or_default());if i<rules.len(){rules.remove(i);}draft.set(Some(rules));preview.set(None);}>"Remove"</button></div>}).collect_view()}
        <div class="workflow-actions"><button class="button primary" disabled=move||busy.get()||draft.get().is_none() on:click=move|_|{let Some(rules)=draft.get_untracked()else{return;};action(format!("/api/v1/sources/{}/corrections",source.get_value()),"PUT",json!({"expected_version":draft_version.get_untracked().unwrap_or_else(||set.get_untracked().map(|s|s.version).unwrap_or(0)),"rules":rules}),snap,notice,busy,pending,move|v|{if let Ok(v)=serde_json::from_value::<CorrectionSet>(v){set.set(Some(v));}draft.set(None);draft_version.set(None);preview.set(None);tick.update(|n|*n+=1);});}>"Save rules"</button><button class="button text" disabled=move||busy.get() on:click=move|_|{draft.set(None);draft_version.set(None);preview.set(None);pending.set(None);}>"Discard draft"</button>
        <button class="button secondary" disabled=move||busy.get()||draft.get().is_some() on:click=move|_|{if busy.get_untracked(){return;}busy.set(true);spawn_local(async move{let result=api(&format!("/api/v1/sources/{}/corrections/preview",source.get_value()),"GET",None).await.and_then(|v|serde_json::from_value::<CorrectionPreview>(v).map_err(|_|"Could not read correction preview".into()));if busy.try_get_untracked().is_none(){return;}busy.set(false);match result{Ok(v)=>preview.set(Some(v)),Err(e)=>notice.set(e)}});}>"Preview affected extractions"</button></div>
        {move||preview.get().map(|p|{let signature=p.signature.clone();view!{<div class="thread-context-section"><h4>{format!("{} candidates to recompute · {} protected",p.actionable_count,p.protected_count)}</h4>{p.items.into_iter().map(|i|view!{<article class="observation"><small>{format!("{} · {}",i.task.id,if i.actionable{"candidate update"}else{&i.reason})}</small><p>{format!("{} → {}",i.task.title,i.corrected_title)}</p><small>{format!("Owner: {} → {} · original r{} line {}",i.task.owner.unwrap_or_else(||"unknown".into()),i.corrected_owner.unwrap_or_else(||"unknown".into()),i.task.evidence.revision,i.task.evidence.line)}</small><blockquote>{i.task.evidence.quote}</blockquote></article>}).collect_view()}<button class="button primary" disabled=move||busy.get()||p.actionable_count==0 on:click=move|_|{action(format!("/api/v1/sources/{}/corrections/apply",source.get_value()),"POST",json!({"signature":signature}),snap,notice,busy,pending,move|v|{notice.set(format!("Recomputed {} candidates; reviewed tasks unchanged",v["applied"]));preview.set(None);tick.update(|n|*n+=1);});}>"Recompute these candidates"</button></div>}})}
        {move||audit.get().map(|a|view!{<details><summary>{format!("Correction audit · {} entries",a.total)}</summary>{a.items.into_iter().map(|e|view!{<details><summary>{format!("{} · {} · {}",e.kind,e.contributor,e.created_at)}</summary><pre>{serde_json::to_string_pretty(&e.detail).unwrap_or_default()}</pre></details>}).collect_view()}<div class="workflow-actions"><button class="button text" disabled=move||audit_offset.get()==0 on:click=move|_|audit_offset.update(|n|*n=n.saturating_sub(5))>"← Newer"</button><button class="button text" disabled=a.next_offset.is_none() on:click=move|_|{if let Some(n)=a.next_offset{audit_offset.set(n);}}>"Older →"</button></div></details>})}
    </section>}
}
