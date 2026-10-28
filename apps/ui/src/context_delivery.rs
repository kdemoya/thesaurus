use super::workflows::{Pending, action};
use super::*;
#[wasm_bindgen(
    inline_js = "export function restoreRunObserver(){try{return JSON.parse(localStorage.getItem('thesaurus.runObserver')||'{}')}catch{return {}}} export function rememberRunObserver(id,cursor){try{localStorage.setItem('thesaurus.runObserver',JSON.stringify({id,cursor}))}catch{}}"
)]
extern "C" {
    #[wasm_bindgen(js_name=restoreRunObserver)]
    fn restore_observer() -> JsValue;
    #[wasm_bindgen(js_name=rememberRunObserver)]
    fn remember_observer(id: &str, cursor: &str);
}
#[component]
pub(super) fn RunObserver(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let saved = js_sys::JSON::stringify(&restore_observer())
        .ok()
        .and_then(|s| s.as_string())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(json!({}));
    let input = RwSignal::new(saved["id"].as_str().unwrap_or_default().to_string());
    let attached = RwSignal::new(input.get_untracked());
    let cursor = RwSignal::new(saved["cursor"].as_str().unwrap_or_default().to_string());
    let observed = RwSignal::new(None::<RuntimeObservation>);
    let polling = RwSignal::new(true);
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let cleanup = alive.clone();
    on_cleanup(move || cleanup.store(false, std::sync::atomic::Ordering::Relaxed));
    spawn_local(async move {
        loop {
            if !alive.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let id = attached.get_untracked();
            if polling.get_untracked() && !id.is_empty() {
                let c = cursor.get_untracked();
                let path = format!(
                    "/api/v1/runtime/runs/{id}/observe{}",
                    if c.is_empty() {
                        String::new()
                    } else {
                        format!("?cursor={c}")
                    }
                );
                match api(&path, "GET", None).await {
                    Ok(v) => {
                        if !alive.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        if attached.get_untracked() == id {
                            if let Ok(o) = serde_json::from_value::<RuntimeObservation>(v) {
                                cursor.set(o.cursor.clone());
                                remember_observer(&id, &o.cursor);
                                let active = matches!(
                                    o.run.status.as_str(),
                                    "starting" | "running" | "waiting_permission" | "cancelling"
                                );
                                observed.set(Some(o));
                                if !active {
                                    polling.set(false);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if !alive.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        if attached.get_untracked() == id {
                            notice.set(e);
                            polling.set(false);
                        }
                    }
                }
            }
            gloo_timers::future::TimeoutFuture::new(2000).await;
        }
    });
    view! {<details class="workflow-manage"><summary>"Reconnect to an existing run"</summary><p class="muted">"Attach by any stored run ID, including older runs. This restores observation and never starts another task. Browser restart restores the last ID and cursor; service restart reports interrupted runs."</p><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let id=input.get_untracked();if id.is_empty() || id.len()>100 || !id.bytes().all(|b|b.is_ascii_alphanumeric() || b"_-".contains(&b)){notice.set("Invalid run ID".into());return;}if attached.get_untracked()!=id{cursor.set(String::new());observed.set(None);}attached.set(id.clone());remember_observer(&id,&cursor.get_untracked());polling.set(true);}>
        <label>"Stored run ID"</label><input required maxlength="100" prop:value=move||input.get() on:input=move|ev|input.set(event_target_value(&ev))/><div class="workflow-actions"><button class="button secondary" type="submit">"Attach / refresh"</button><button class="button text" type="button" on:click=move|_|{polling.set(false);attached.set(String::new());cursor.set(String::new());observed.set(None);remember_observer("","");}>"Detach"</button></div></form>
        {move||observed.get().map(|o|view!{<article class="runtime-run"><div class="runtime-topline"><strong>{o.run.connection}</strong><span class="runtime-status">{o.run.status.clone()}</span></div><p>{o.run.progress}</p><code>{format!("Run {} · session {} · event watermark {}",o.run.id,o.run.session_id.unwrap_or("pending".into()),o.watermark)}</code><small class="muted">{format!("Cursor {} · observed {}",o.cursor,o.observed_at)}</small>{o.run.error.map(|e|view!{<p class="runtime-error">{e}</p>})}<pre class="runtime-output">{o.run.output}</pre>
            {matches!(o.run.status.as_str(),"starting"|"running"|"waiting_permission"|"cancelling").then(||{let cancel=o.run.id.clone();view!{<button class="button text" disabled=o.run.status=="cancelling" on:click=move|_|{mutate(format!("/api/v1/runtime/runs/{cancel}/cancel"),"POST",json!({}),snap,notice);polling.set(true);}>"Cancel this run"</button>}})}
            {o.run.permissions.clone().into_iter().filter(|p|p.decision.is_none()).map(|p|{let run_id=o.run.id.clone();let pid=p.id.clone();view!{<section class="runtime-permission"><strong>{p.title}</strong><small>{p.kind}</small><pre>{p.input}</pre><div>{p.options.into_iter().map(|option|{let path=format!("/api/v1/runtime/runs/{run_id}/permissions/{pid}");view!{<button class="button compact" on:click=move|_|{mutate(path.clone(),"POST",json!({"option_id":option.id}),snap,notice);polling.set(true);}>{option.label}</button>}}).collect_view()}</div></section>}}).collect_view()}
            <p class="muted">{format!("{} pending permission requests. Controls above affect this stored run only.",o.run.permissions.iter().filter(|p|p.decision.is_none()).count())}</p></article>})}
    </details>}
}
#[component]
pub(super) fn HandoffReceipts(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let thread = RwSignal::new(String::new());
    let context = RwSignal::new(String::new());
    let receipt = RwSignal::new(None::<HandoffReceipt>);
    let note = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    view! {<section class="panel"><div class="section-heading"><h2>"Handoff receipts"</h2><span>"Delivery ≠ acknowledgement ≠ completed work"</span></div><div class="workflow-form"><label>"Work thread"</label><select prop:value=move||thread.get() on:change=move|ev|{thread.set(event_target_value(&ev));context.set(String::new());receipt.set(None);}><option value="">"Choose thread"</option>{move||snap.get().map(|s|s.threads.into_iter().map(|t|view!{<option value=t.id>{t.title}</option>}).collect_view())}</select><label>"Delivered context"</label><select prop:value=move||context.get() on:change=move|ev|{context.set(event_target_value(&ev));receipt.set(None);}><option value="">"Choose delivery"</option>{move||snap.get().and_then(|s|s.threads.into_iter().find(|t|t.id==thread.get())).map(|t|t.runs.into_iter().map(|r|view!{<option value=r.context_id>{format!("{} · {} · {}",r.created_at,r.connection,r.status)}</option>}).collect_view())}</select><button class="button secondary" disabled=move||context.get().is_empty() on:click=move|_|{let t=thread.get_untracked();let c=context.get_untracked();spawn_local(async move{match api(&format!("/api/v1/threads/{t}/contexts/{c}/receipt"),"GET",None).await{Ok(v)=>{if thread.get_untracked()==t && context.get_untracked()==c{if let Ok(r)=serde_json::from_value(v){receipt.set(Some(r));}}},Err(e)=>notice.set(e)}});}>"Inspect receipt"</button><label>"Acknowledgement / completion review note"</label><textarea rows="3" maxlength="2000" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))></textarea></div>
        {move||receipt.get().map(|r|view!{<p>{format!("{} · version {} · runtime {}",r.state,r.version,r.runtime_status)}</p><code>{format!("Context {} · revision {} · watermark {}",r.context_id,r.context_revision,r.context_watermark)}</code><p class="muted">"A completed agent runtime only means the agent stopped normally. Verify the work explicitly here; this receipt never completes source tasks."</p><div class="workflow-actions">{["acknowledged","completion_claimed","completed","reopened"].into_iter().map(|a|view!{<button class="button secondary" disabled=move||busy.get() on:click=move|_|{if let Some(r)=receipt.get_untracked(){action(format!("/api/v1/threads/{}/contexts/{}/receipt",r.thread_id,r.context_id),"POST",json!({"expected_version":r.version,"action":a,"note":note.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(r)=serde_json::from_value(v){receipt.set(Some(r));note.set(String::new());}});}}>{match a{"acknowledged"=>"Acknowledge delivery","completion_claimed"=>"Record completion claim","completed"=>"Verify work completed",_=>"Reopen work"}}</button>}).collect_view()}</div>{r.history.into_iter().map(|e|view!{<article class="focus-change"><strong>{format!("{} · {}",e.action,e.contributor)}</strong><p>{e.note}</p><small>{e.created_at}</small></article>}).collect_view()}})}
    </section>}
}
#[component]
pub(super) fn ContextBuilder(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let sources = RwSignal::new(Vec::<String>::new());
    let characters = RwSignal::new("12000".to_string());
    let tokens = RwSignal::new("4000".to_string());
    let result = RwSignal::new(None::<ContextBundle>);
    let busy = RwSignal::new(false);
    view! {<section class="panel"><div class="section-heading"><h2>"Budgeted context"</h2><span>"Local · exact citations"</span></div><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let(c,t)=match(characters.get_untracked().parse::<usize>(),tokens.get_untracked().parse::<usize>()){(Ok(c),Ok(t))=>(c,t),_=>{notice.set("Invalid context budgets".into());return;}};if busy.get_untracked(){return;}busy.set(true);let scope=sources.get_untracked();let body=json!({"query":query.get_untracked(),"character_budget":c,"token_budget":t,"source_ids":if scope.is_empty(){None}else{Some(scope)}});spawn_local(async move{let response=api("/api/v1/context/budgeted","POST",Some(body)).await;busy.set(false);match response{Ok(v)=>{if let Ok(b)=serde_json::from_value(v){result.set(Some(b));}},Err(e)=>notice.set(e)}});}><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Query"</label><input required maxlength="1024" prop:value=move||query.get() on:input=move|ev|query.set(event_target_value(&ev))/><label>"Character budget (500–16000)"</label><input type="number" min="500" max="16000" prop:value=move||characters.get() on:input=move|ev|characters.set(event_target_value(&ev))/><label>"Estimated token budget (125–16000)"</label><input type="number" min="125" max="16000" prop:value=move||tokens.get() on:input=move|ev|tokens.set(event_target_value(&ev))/><fieldset class="runtime-sources"><legend>"Source scope (empty means all permitted)"</legend>{move||snap.get().map(|s|s.sources.into_iter().map(|s|{let id=s.id.clone();view!{<label><input type="checkbox" prop:checked=move||sources.get().contains(&id) on:change=move|ev|{let checked=event_target_checked(&ev);sources.update(|ids|{ids.retain(|i|i!=&s.id);if checked{ids.push(s.id.clone());}});}/>{s.name}</label>}}).collect_view())}</fieldset><button class="button primary" type="submit">"Build within budgets"</button></fieldset></form>{move||result.get().map(|b|view!{<p class="muted">{format!("≈{} / {} tokens · {} source(s) · watermark {} · {}",b.estimated_tokens,b.token_budget,b.source_ids.len(),b.watermark,if b.truncated{"omissions reported"}else{"all considered records included"})}</p><pre style="white-space:pre-wrap;overflow-wrap:anywhere">{serde_json::to_string_pretty(&b).unwrap_or_default()}</pre>})}</section>}
}
