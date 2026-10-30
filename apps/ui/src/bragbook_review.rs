//! Bragbook review queue and session-local drafts.
use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Draft {
    version: i64,
    title: String,
    contribution: String,
    impact: String,
    date: String,
}
impl Draft {
    fn reconcile(self, saved: &Self) -> Self {
        bragbook_review_state::reconcile(self, saved)
    }
    fn from_entry(entry: &BragbookEntry) -> Self {
        Self {
            version: entry.version,
            title: entry.title.clone(),
            contribution: entry.contribution.clone(),
            impact: entry.impact.clone(),
            date: entry.occurred_on.clone().unwrap_or_default(),
        }
    }
}
impl bragbook_review_state::ReviewDraft for Draft {
    fn content(&self) -> bragbook_review_state::DraftContent<'_> {
        bragbook_review_state::DraftContent {
            title: &self.title,
            contribution: &self.contribution,
            impact: &self.impact,
            date: &self.date,
        }
    }
}
#[derive(Clone, Copy)]
pub(super) struct Drafts {
    items: RwSignal<HashMap<String, Draft>>,
    feedback: RwSignal<String>,
}

#[wasm_bindgen(inline_js = "
const draftKey='thesaurus.preview.bragbook-drafts.v1';
let hasDrafts=false;
function protectDrafts(event){if(hasDrafts){event.preventDefault();event.returnValue='';}}
export function loadReviewDrafts(){try{return sessionStorage.getItem(draftKey)||'{}';}catch{return '{}';}}
export function storeReviewDrafts(value,pending){hasDrafts=pending;window.removeEventListener('beforeunload',protectDrafts);if(pending)window.addEventListener('beforeunload',protectDrafts);try{if(pending)sessionStorage.setItem(draftKey,value);else sessionStorage.removeItem(draftKey);}catch{}}
const fieldWatchers=new Map();
export function watchReviewFields(key){
  if(globalThis.CSS?.supports('field-sizing','content'))return;
  let frame=0,disposed=false,form=null,observer=null;
  const grow=()=>{
    frame=0;
    if(disposed)return;
    form ||= document.querySelector('[data-review-editor=\"'+key+'\"]');
    if(!form)return;
    if(!observer){observer=new ResizeObserver(schedule);observer.observe(form);form.addEventListener('input',schedule);}
    for(const field of form.querySelectorAll('textarea')){
      if(!field.clientWidth || field.scrollHeight<=field.clientHeight)continue;
      const style=getComputedStyle(field);
      const border=(parseFloat(style.borderTopWidth)||0)+(parseFloat(style.borderBottomWidth)||0);
      // Grow only: deleting text must not undo a user's manual enlargement.
      field.style.height=(field.scrollHeight+border)+'px';
    }
  };
  const schedule=()=>{if(!disposed&&!frame)frame=requestAnimationFrame(grow);};
  window.addEventListener('resize',schedule);
  document.fonts?.ready.then(schedule);
  fieldWatchers.set(key,{schedule,stop:()=>{disposed=true;cancelAnimationFrame(frame);observer?.disconnect();form?.removeEventListener('input',schedule);window.removeEventListener('resize',schedule);}});
  schedule();
}
export function refreshReviewFields(key){fieldWatchers.get(key)?.schedule();}
export function stopReviewFields(key){fieldWatchers.get(key)?.stop();fieldWatchers.delete(key);}
")]
extern "C" {
    #[wasm_bindgen(js_name=loadReviewDrafts)]
    fn load_drafts() -> String;
    #[wasm_bindgen(js_name=storeReviewDrafts)]
    fn store_drafts(value: &str, pending: bool);
    #[wasm_bindgen(js_name=watchReviewFields)]
    fn watch_fields(key: &str);
    #[wasm_bindgen(js_name=refreshReviewFields)]
    fn refresh_fields(key: &str);
    #[wasm_bindgen(js_name=stopReviewFields)]
    fn stop_fields(key: &str);
}
pub(super) fn provide_drafts() {
    let saved: HashMap<String, Draft> = serde_json::from_str(&load_drafts()).unwrap_or_default();
    let drafts = RwSignal::new(saved);
    provide_context(Drafts {
        items: drafts,
        feedback: RwSignal::new(String::new()),
    });
    Effect::new(move |_| {
        let value = drafts.get();
        store_drafts(
            &serde_json::to_string(&value).unwrap_or_else(|_| "{}".into()),
            !value.is_empty(),
        );
    });
}

#[component]
pub(super) fn ReviewQueue(snap: Snap) -> impl IntoView {
    let draft_store = expect_context::<Drafts>().items;
    let entries = RwSignal::new(None::<BragbookList>);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(String::new());
    let offset = RwSignal::new(0usize);
    let index = RwSignal::new(0usize);
    let tick = RwSignal::new(0u64);
    let request = RwSignal::new(0u64);
    let message = expect_context::<Drafts>().feedback;
    let loaded_offset = RwSignal::new(0usize);
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        let start = offset.get();
        let active_id = entries
            .get_untracked()
            .and_then(|v| v.items.get(index.get_untracked()).map(|e| e.id.clone()));
        if start != loaded_offset.get_untracked() {
            entries.set(None);
        }
        request.update(|n| *n += 1);
        let ticket = request.get_untracked();
        loading.set(true);
        error.set(String::new());
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/bragbook?status=candidate&offset={start}"),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<BragbookList>(v)
                    .map_err(|_| "Could not read the review queue. Try again.".into())
            });
            if request.try_get_untracked() != Some(ticket) {
                return;
            }
            loading.set(false);
            match result {
                Ok(v) => {
                    if v.items.is_empty() && start > 0 {
                        offset.set(start.saturating_sub(25));
                        index.set(0);
                        return;
                    }
                    let ids = v.items.iter().map(|e| e.id.as_str()).collect::<Vec<_>>();
                    index.set(bragbook_review_state::queue_index(
                        active_id.as_deref(),
                        &ids,
                        index.get_untracked(),
                    ));
                    loaded_offset.set(start);
                    entries.set(Some(v));
                }
                Err(e) => {
                    error.set(e);
                    entries.set(None);
                }
            }
        });
    });
    view! {
        <section class="review-queue" aria-label="Review one accomplishment at a time">
            <header class="review-queue-heading">
                <div><h2>"Review your contribution"</h2><p>"Read the evidence. Make the record yours."</p></div>
                <span class="review-position">{move||entries.get().filter(|v|v.total>0).map(|v|format!("{} of {} to review",offset.get()+index.get()+1,v.total))}</span>
            </header>
            <div class="review-feedback" role="status" aria-live="polite">{move||message.get()}</div>
            <Show when=move||!error.get().is_empty()><div class="review-error" role="alert"><p>{move||error.get()}</p><button class="button secondary" on:click=move|_|tick.update(|n|*n+=1)>"Try again"</button></div></Show>
            <Show when=move||loading.get()&&entries.get().is_none()><p role="status">"Loading your review queue…"</p></Show>
            <Show when=move||!loading.get()&&error.get().is_empty()&&entries.get().is_some_and(|v|v.total==0)>
                <div class="review-complete"><h3>"You're caught up."</h3><p>"There are no more candidates to review. Accepted entries are ready for your next set of highlights."</p></div>
            </Show>
            <For each={move||entries.get().and_then(|v|v.items.get(index.get()).cloned()).into_iter().collect::<Vec<_>>()} key=|e|(e.id.clone(),e.version,e.support_changed,e.stale_evidence) children=move|entry|view!{<QueueEntry entry=entry snap=snap/>}/>
            <Show when=move||!draft_store.get().is_empty()>
                <details class="review-retained-drafts"><summary>{move||format!("{} local draft(s) kept in this window",draft_store.get().len())}</summary>
                    <p>"A saved response can be interrupted. If an entry has left the queue, your draft stays here so you can compare it with Accepted & highlights or Dismissed. Discarding a local draft does not change the saved record."</p>
                    {move||{let current=entries.get().and_then(|v|v.items.get(index.get()).map(|e|e.id.clone()));draft_store.get().into_iter().filter(|(id,_)|Some(id)!=current.as_ref()).map(|(key,value)|{
                        let discard_id=key.clone();
                        view!{<details><summary>{value.title.clone()}</summary><pre>{format!("{}\n\n{}\n\nImpact: {}\nDate: {}",value.title,value.contribution,value.impact,value.date)}</pre><button class="button text" type="button" on:click=move|_|{if web_sys::window().is_some_and(|w|w.confirm_with_message("Discard this local draft? The saved entry will not change.").unwrap_or(false)){draft_store.update(|all|{all.remove(&discard_id);});}}>"Discard this local draft"</button></details>}
                    }).collect_view()}}
                </details>
            </Show>
            <Show when=move||entries.get().is_some_and(|v|v.total>0)>
                <nav class="review-pagination" aria-label="Review queue">
                    <button class="button text" disabled=move||loading.get()||(offset.get()==0&&index.get()==0) on:click=move|_|{message.set(String::new());if index.get()>0{index.update(|i|*i-=1);}else{offset.update(|o|*o=o.saturating_sub(25));index.set(24);}}>"Previous candidate"</button>
                    <span>"Your unsaved edits stay in this window."</span>
                    <button class="button secondary" disabled=move||loading.get()||entries.get().is_none_or(|v|index.get()+1>=v.items.len()&&v.next_offset.is_none()) on:click=move|_|{message.set(String::new());if let Some(v)=entries.get_untracked(){if index.get()+1<v.items.len(){index.update(|i|*i+=1);}else if let Some(next)=v.next_offset{offset.set(next);index.set(0);}}}>"Next candidate"</button>
                </nav>
            </Show>
        </section>
    }
}

#[component]
fn QueueEntry(entry: BragbookEntry, snap: Snap) -> impl IntoView {
    let state = expect_context::<Drafts>();
    let drafts = state.items;
    let message = state.feedback;
    let notice = expect_context::<ReviewNotice>().0;
    let id = StoredValue::new(entry.id.clone());
    let original = StoredValue::new(Draft::from_entry(&entry));
    let restored = drafts
        .get_untracked()
        .get(&entry.id)
        .cloned()
        .unwrap_or_else(|| Draft::from_entry(&entry));
    let draft = RwSignal::new(restored.reconcile(&Draft::from_entry(&entry)));
    let attested = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let pending = RwSignal::new(None::<Value>);
    let key = RwSignal::new(new_id());
    let supported = !entry.support_changed;
    let dirty = move || draft.get() != original.get_value();
    let conflict = move || draft.get().version != original.get_value().version;
    let sizing_key = new_id();
    watch_fields(&sizing_key);
    let sizing_cleanup = sizing_key.clone();
    on_cleanup(move || stop_fields(&sizing_cleanup));
    let sizing_updates = sizing_key.clone();
    Effect::new(move |_| {
        let value = draft.get();
        refresh_fields(&sizing_updates);
        drafts.update(|all| {
            if value != original.get_value() {
                all.insert(id.get_value(), value);
            } else {
                all.remove(&id.get_value());
            }
        });
    });
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() || conflict() {
            return;
        }
        let choice = ev
            .submitter()
            .and_then(|b| b.get_attribute("data-decision"))
            .unwrap_or_else(|| "candidate".into());
        if choice == "accepted" && (!attested.get_untracked() || !supported) {
            return;
        }
        let value = draft.get_untracked();
        let submitted = value.clone();
        let entry_id = id.get_value();
        let body = json!({"expected_version":value.version,"status":choice,"title":value.title,"contribution":value.contribution,"impact":value.impact,"occurred_on":if value.date.is_empty(){None}else{Some(value.date)}});
        if pending.get_untracked().as_ref() != Some(&body) {
            key.set(new_id());
            pending.set(Some(body.clone()));
        }
        let mut body = body;
        body["idempotency_key"] = json!(key.get_untracked());
        busy.set(true);
        error.set(String::new());
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/bragbook/{}", entry_id),
                "PATCH",
                Some(body),
            )
            .await;
            // Drafts belong to App, so successful requests can settle even after navigation.
            if let Ok(_) = &result {
                if drafts.try_get_untracked().is_some() {
                    drafts.update(|all| {
                        bragbook_review_state::clear_submitted(all, &entry_id, &submitted);
                    });
                }
            }
            if result.is_ok() && message.try_get_untracked().is_some() {
                message.set(
                    match choice.as_str() {
                        "accepted" => "Entry accepted. It is ready in Accepted & highlights.",
                        "dismissed" => "Entry dismissed. You can review it again in Dismissed.",
                        _ => "Candidate saved. Continue here or review the next entry.",
                    }
                    .into(),
                );
                if snap.try_get_untracked().is_some() {
                    refresh(snap, notice);
                }
            }
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            if let Err(e) = result {
                error.set(format!("Your draft is kept. {e}"));
            }
        });
    };
    view! {
        <article class="review-entry">
            <div class="review-evidence">
                <h3>"Source evidence"</h3>
                <p class="review-source-meta">{format!("Collected {} · {}",entry.collected_at.get(..10).unwrap_or(&entry.collected_at),entry.origin_kind)}</p>
                <Show when=move||!supported><p class="review-error">"This work was reopened or is no longer completed. Acceptance is paused until its current state supports the claim."</p></Show>
                <Show when=move||entry.stale_evidence><p class="review-source-note">"This citation keeps the original revision. Open it to inspect the saved wording."</p></Show>
                {entry.evidence.into_iter().map(|e|{let target=e.clone();view!{
                    <blockquote><p>{e.quote}</p><button type="button" class="source-link" on:click=move|_|{navigate_evidence(target.clone(),notice);}>{format!("Open source · revision {} · line {}",e.revision,e.line)}</button></blockquote>
                }}).collect_view()}
                <details class="review-provenance"><summary>"How this candidate was collected"</summary><p>{entry.origin_note}</p></details>
            </div>
            <form class="review-editor" data-review-editor=sizing_key on:submit=save>
                <header><h3>"Your record"</h3><span role="status">{move||if busy.get(){"Saving…"}else if dirty(){"Unsaved changes · draft kept"}else{"Saved candidate"}}</span></header>
                <fieldset disabled=move||busy.get()>
                    <label>"Accomplishment"<textarea required rows="2" maxlength="300" prop:value=move||draft.get().title on:input=move|ev|{draft.update(|d|d.title=event_target_value(&ev));attested.set(false);}/></label>
                    <label>"Your contribution"<textarea required rows="3" maxlength="4000" prop:value=move||draft.get().contribution on:input=move|ev|{draft.update(|d|d.contribution=event_target_value(&ev));attested.set(false);}/></label>
                    <details class="review-optional"><summary>"Add impact or a date (optional)"</summary>
                        <label>"Impact"<textarea rows="2" maxlength="4000" placeholder="An outcome you can support with the evidence." prop:value=move||draft.get().impact on:input=move|ev|{draft.update(|d|d.impact=event_target_value(&ev));attested.set(false);}/></label>
                        <label>"Accomplishment date"<input type="date" prop:value=move||draft.get().date on:input=move|ev|{draft.update(|d|d.date=event_target_value(&ev));attested.set(false);}/></label>
                    </details>
                    <Show when=conflict><p class="review-error" role="alert">"The saved entry changed while you were editing. Your draft is kept here. Compare it with the saved record before discarding your changes and reviewing again."</p></Show>
                    <Show when=conflict><details class="review-optional"><summary>"Compare saved record"</summary><p class="review-saved-copy">{move||format!("Accomplishment: {}\n\nContribution: {}\n\nImpact: {}\n\nDate: {}",original.get_value().title,original.get_value().contribution,original.get_value().impact,original.get_value().date)}</p></details></Show>
                    <div class="review-decision">
                        <label class="review-attestation"><input type="checkbox" prop:checked=move||attested.get() on:change=move|ev|attested.set(event_target_checked(&ev))/>"I reviewed the evidence, my contribution, and the stated impact."</label>
                        <div class="review-actions">
                            <button type="submit" class="button primary review-accept" disabled=move||busy.get()||!attested.get()||!supported||conflict() data-decision="accepted">"Accept entry"</button>
                            <button type="submit" class="button secondary" disabled=move||busy.get()||conflict() data-decision="candidate">"Save candidate"</button>
                            <button type="submit" class="button text" disabled=move||busy.get()||conflict() data-decision="dismissed">"Dismiss"</button>
                        </div>
                        <p class="review-decision-hint">{move||if !supported{"Acceptance is paused because the supporting work changed."}else if !attested.get(){"Confirm your review above to enable acceptance."}else{"Acceptance records your review. It does not verify an unsupported claim."}}</p>
                    </div>
                    <Show when=dirty><button type="button" class="button text review-discard" on:click=move|_|{if web_sys::window().is_some_and(|w|w.confirm_with_message("Discard your unsaved edits and restore the saved candidate?").unwrap_or(false)){draft.set(original.get_value());attested.set(false);error.set(String::new());}}>"Discard unsaved edits"</button></Show>
                </fieldset>
                <Show when=move||!error.get().is_empty()><p role="alert" class="review-error">{move||error.get()}</p></Show>
            </form>
        </article>
    }
}
#[derive(Clone, Copy)]
pub(super) struct ReviewNotice(pub RwSignal<String>);
