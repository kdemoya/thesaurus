use crate::{Snap, api, new_id, refresh};
use leptos::{prelude::*, task::spawn_local};
use serde_json::json;
use thesaurus_contracts::*;

#[component]
pub fn MemoryReview(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let records = RwSignal::new(Vec::<MemoryInspection>::new());
    let conflicts = RwSignal::new(Vec::<MemoryConflict>::new());
    let selected = RwSignal::new(String::new());
    let detail = RwSignal::new(None::<MemoryInspection>);
    let marked = RwSignal::new(Vec::<String>::new());
    let offset = RwSignal::new(0u32);
    let reload = RwSignal::new(0u64);
    let epoch = RwSignal::new(0u64);
    let busy = RwSignal::new(false);
    let reason = RwSignal::new(String::new());
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = reload.get();
        let page = offset.get();
        let ticket = epoch.get_untracked() + 1;
        epoch.set(ticket);
        spawn_local(async move {
            let response = async {
                let list = api(
                    &format!("/api/v1/memory/inspect?offset={page}"),
                    "GET",
                    None,
                )
                .await?;
                let reviews = api("/api/v1/memory/conflicts", "GET", None).await?;
                Ok::<_, String>((
                    serde_json::from_value::<Vec<MemoryInspection>>(list)
                        .map_err(|_| "Invalid memory catalog".to_string())?,
                    serde_json::from_value::<Vec<MemoryConflict>>(reviews)
                        .map_err(|_| "Invalid conflict review".to_string())?,
                ))
            }
            .await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match response {
                Ok((list, reviews)) => {
                    records.set(list);
                    conflicts.set(reviews);
                }
                Err(e) => notice.set(e),
            }
        });
    });
    Effect::new(move |_| {
        let id = selected.get();
        let _ = reload.get();
        let _ = snap.get().map(|s| s.watermark);
        detail.set(None);
        if id.is_empty() {
            return;
        }
        spawn_local(async move {
            match api(&format!("/api/v1/memory/inspect/{id}"), "GET", None).await {
                Ok(value) => {
                    if selected.try_get_untracked().as_deref() == Some(id.as_str()) {
                        match serde_json::from_value(value) {
                            Ok(value) => detail.set(Some(value)),
                            Err(_) => notice.set("Invalid memory inspection".into()),
                        }
                    }
                }
                Err(e) => {
                    if selected.try_get_untracked().as_deref() == Some(id.as_str()) {
                        notice.set(e)
                    }
                }
            }
        });
    });
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let request = json!({"observation_ids":marked.get_untracked(),"reason":reason.get_untracked(),"idempotency_key":new_id()});
        spawn_local(async move {
            let result = api("/api/v1/memory/conflicts", "POST", Some(request)).await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match result {
                Ok(_) => {
                    marked.set(vec![]);
                    reason.set(String::new());
                    reload.update(|n| *n += 1);
                    refresh(snap, notice);
                }
                Err(e) => notice.set(e),
            }
        });
    };
    view! {
        <section class="panel memory-inspector"><div class="section-heading"><h2>"Memory inspector"</h2><button class="source-link" on:click=move|_|reload.update(|n|*n+=1)>"refresh"</button></div>
            <p class="muted">"Inspect original contributions and evidence. Corrections and suppression affect shared memory; originals and review history remain available here."</p>
            <div class="workflow-actions"><button class="button text" disabled=move||offset.get()==0 on:click=move|_|{offset.update(|n|*n=n.saturating_sub(25));selected.set(String::new());}>"← Previous"</button><small class="muted">{move||format!("Page {}",offset.get()/25+1)}</small><button class="button text" disabled=move||records.get().len()<25 on:click=move|_|{offset.update(|n|*n+=25);selected.set(String::new());}>"Next →"</button></div>
            <Show when=move||records.get().is_empty()><p class="muted">"No memory on this page."</p></Show>
            {move||records.get().into_iter().map(|item|{let id=item.original.id.clone();let inspect=id.clone();let mark=id.clone();let marked_id=id.clone();let active=id.clone();view!{<div class="memory-inspector-row" class:active=move||selected.get()==active><label class="focus-check"><input type="checkbox" prop:checked=move||marked.get().contains(&marked_id) on:change=move|ev|{let checked=event_target_checked(&ev);marked.update(|list|{list.retain(|x|x!=&mark);if checked{list.push(mark.clone());}});}/><span class="muted">{format!("v{} · {}",item.current.version,item.current.status)}</span></label><button class="source-link" on:click=move|_|selected.set(inspect.clone())>{item.current.body.chars().take(180).collect::<String>()}</button><small class="muted">{format!("{} · {}",item.original.contributor,item.original.created_at)}</small></div>}}).collect_view()}
            {move||detail.get().map(|item|view!{<MemoryDetail item=item snap=snap notice=notice reload=reload/>})}
            <details class="workflow-manage"><summary>"Flag incompatible claims for review"</summary><p class="muted">"Select 2–8 observations above, including across pages. Explain the contradiction. Both claims remain available while the review is pending."</p><form class="workflow-form" on:submit=create><fieldset class="focus-form-fields" disabled=move||busy.get()><small>{move||format!("{} selected",marked.get().len())}</small><label for="conflict-reason">"Why do these claims conflict?"</label><textarea id="conflict-reason" rows="2" maxlength="2000" required prop:value=move||reason.get() on:input=move|ev|reason.set(event_target_value(&ev))></textarea><button class="button secondary" type="submit" disabled=move||marked.get().len()<2 || (marked.get().len()>8)>"Open conflict review"</button></fieldset></form></details>
        </section>
        <section class="panel memory-conflicts"><div class="section-heading"><h2>"Conflicting memory"</h2><span class="muted">"user review"</span></div><Show when=move||conflicts.get().is_empty()><p class="muted">"No conflict reviews. Flag incompatible observations in the inspector."</p></Show>{move||conflicts.get().into_iter().map(|item|view!{<ConflictDetail item=item snap=snap notice=notice reload=reload/>}).collect_view()}</section>
    }
}

#[component]
fn MemoryDetail(
    item: MemoryInspection,
    snap: Snap,
    notice: RwSignal<String>,
    reload: RwSignal<u64>,
) -> impl IntoView {
    let body = RwSignal::new(item.current.body.clone());
    let reason = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let action = RwSignal::new("correct".to_string());
    let restore = RwSignal::new("0".to_string());
    let id = StoredValue::new(item.original.id.clone());
    let version = item.current.version;
    let revise = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let selected_action = action.get_untracked();
        let request = json!({"expected_version":version,"action":selected_action,"body":if selected_action=="correct"{Some(body.get_untracked())}else{None},"restore_version":if selected_action=="restore"{restore.get_untracked().parse::<i64>().ok()}else{None},"reason":reason.get_untracked(),"idempotency_key":new_id()});
        let path = format!("/api/v1/memory/inspect/{}", id.get_value());
        spawn_local(async move {
            let result = api(&path, "PATCH", Some(request)).await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match result {
                Ok(_) => {
                    reload.update(|n| *n += 1);
                    refresh(snap, notice);
                }
                Err(e) => notice.set(e),
            }
        });
    };
    view! {
        <section class="memory-detail"><h3 class="focus-section-title">"Contribution and lineage"</h3><code>{item.original.id.clone()}</code><p class="muted">{format!("Original contributor: {} · {} · current v{}",item.original.contributor,item.original.created_at,version)}</p><p class="muted">{format!("Source scope: {}",if item.original.source_ids.is_empty(){"personal / unscoped".into()}else{item.original.source_ids.join(", ")})}</p>
            {item.stale_evidence.then(||view!{<p class="brief-stale">"The source has changed since this memory's evidence revision."</p>})}
            <details class="brief-evidence"><summary>"Original memory and source evidence"</summary><p class="workflow-body">{item.original.body.clone()}</p>{item.original.evidence.iter().map(|e|view!{<blockquote><small>{format!("{} · r{} · line {}",e.document_id,e.revision,e.line)}</small><p>{e.quote.clone()}</p></blockquote>}).collect_view()}</details>
            <form class="workflow-form" on:submit=revise><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Action"<select prop:value=move||action.get() on:change=move|ev|action.set(event_target_value(&ev))><option value="correct">"Correct / supersede body"</option><option value="suppress">"Suppress from shared memory"</option><option value="restore">"Undo to an earlier revision"</option></select></label><Show when=move||action.get()=="correct"><label>"Corrected memory"</label><textarea rows="4" maxlength="16000" required prop:value=move||body.get() on:input=move|ev|body.set(event_target_value(&ev))></textarea></Show><Show when=move||action.get()=="restore"><label>"Restore version (0 is original)"</label><input type="number" min="0" max=version prop:value=move||restore.get() on:input=move|ev|restore.set(event_target_value(&ev))/></Show><label>"Reason"</label><textarea rows="2" maxlength="2000" required prop:value=move||reason.get() on:input=move|ev|reason.set(event_target_value(&ev))></textarea><button class="button primary" type="submit">"Save reviewed change"</button></fieldset></form>
            <h3 class="focus-section-title">{format!("History · {} revisions · {} shown",item.history_count,item.history.len())}</h3><p class="muted">"Up to 30 recent revisions shown within the response budget. Any earlier version can be inspected through the CLI or API and restored above."</p>{item.history.into_iter().map(|entry|view!{<details class="workflow-history"><summary>{format!("v{} · {} · {} · {}",entry.version,entry.action,entry.contributor,entry.created_at)}</summary><p>{entry.reason}</p><p class="workflow-body">{entry.body}</p><small>{entry.status}</small></details>}).collect_view()}
        </section>
    }
}

#[component]
fn ConflictDetail(
    item: MemoryConflict,
    snap: Snap,
    notice: RwSignal<String>,
    reload: RwSignal<u64>,
) -> impl IntoView {
    let decision = RwSignal::new("keep_both".to_string());
    let note = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let id = StoredValue::new(item.id.clone());
    let version = item.version;
    let history = RwSignal::new(None::<MemoryConflictHistory>);
    let resolve = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let path = format!("/api/v1/memory/conflicts/{}", id.get_value());
        let request = json!({"expected_version":version,"decision":decision.get_untracked(),"note":note.get_untracked(),"idempotency_key":new_id()});
        spawn_local(async move {
            let result = api(&path, "PATCH", Some(request)).await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match result {
                Ok(_) => {
                    reload.update(|n| *n += 1);
                    refresh(snap, notice);
                }
                Err(e) => notice.set(e),
            }
        });
    };
    view! {<article class="memory-conflict"><div class="workflow-topline"><strong class="orange">{item.status.clone()}</strong><small>{format!("v{} · opened {}",item.version,item.created_at)}</small></div><p>{item.reason.clone()}</p>
        {item.claims.iter().map(|claim|view!{<details class="brief-evidence" open><summary>{format!("Originally {} · {} · pinned memory v{}",claim.contributor,claim.contributed_at,claim.version)}</summary><p class="workflow-body">{claim.body.clone()}</p>{(claim.version>0).then(||view!{<p class="muted">"This pinned body includes reviewed memory changes. The contributor and date above describe the original contribution; inspect its memory history for correction dates. Original evidence remains below."</p>})}<small>{format!("Observation: {} · sources: {}",claim.observation_id,claim.source_ids.join(", "))}</small>{claim.evidence.iter().map(|e|view!{<blockquote><small>{format!("{} · r{} · line {}",e.document_id,e.revision,e.line)}</small><p>{e.quote.clone()}</p></blockquote>}).collect_view()}</details>}).collect_view()}
        {item.resolution_note.clone().map(|text|view!{<p class="muted">{format!("Last review: {} · {} · {}",item.decision.clone().unwrap_or_else(||"reopened".into()),item.updated_at,text)}</p>})}
        <form class="workflow-form" on:submit=resolve><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Review decision"<select prop:value=move||decision.get() on:change=move|ev|decision.set(event_target_value(&ev))><option value="keep_both">"Keep all claims, acknowledge uncertainty"</option>{item.claims.iter().map(|claim|view!{<option value=format!("keep:{}",claim.observation_id)>{format!("Keep {} · {}",claim.contributor,claim.body.chars().take(65).collect::<String>())}</option>}).collect_view()}<option value="suppress_all">"Suppress all claims"</option><option value="reopen">"Reopen / refresh claim snapshots"</option></select></label><label>"Reason and supporting context"</label><textarea rows="2" required maxlength="2000" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))></textarea><small class="muted">"Keeping one claim suppresses the others. Reopening retains prior corrections; use the inspector to undo them. Changed claims must be refreshed before a decision."</small><button class="button secondary" type="submit">"Save explicit decision"</button></fieldset></form>
        <button class="source-link" on:click=move|_|{let path=format!("/api/v1/memory/conflicts/{}/history",id.get_value());spawn_local(async move{let result=api(&path,"GET",None).await;if history.try_get_untracked().is_none(){return;}match result{Ok(value)=>match serde_json::from_value(value){Ok(value)=>history.set(Some(value)),Err(_)=>notice.set("Invalid conflict history".into())},Err(e)=>notice.set(e)}});}>"Load decision history"</button>
        {move||history.get().map(|h|view!{<div class="memory-conflict-history"><small class="muted">{format!("{} total decisions / snapshots; latest 20 shown",h.total)}</small>{h.records.into_iter().map(|r|view!{<details class="workflow-history"><summary>{format!("v{} · {} · {}",r.version,r.status,r.updated_at)}</summary><p>{r.resolution_note.unwrap_or(r.reason)}</p><small>{r.decision.unwrap_or_else(||"pending".into())}</small></details>}).collect_view()}</div>})}
    </article>}
}
