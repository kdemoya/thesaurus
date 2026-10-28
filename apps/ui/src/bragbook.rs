use super::*;
const PLUGIN: &str = "thesaurus.bragbook";

#[component]
pub(super) fn Bragbook(
    snap: Snap,
    notice: RwSignal<String>,
    section: RwSignal<String>,
) -> impl IntoView {
    let entries = RwSignal::new(None::<BragbookList>);
    let error = RwSignal::new(String::new());
    let loading = RwSignal::new(false);
    let offset = RwSignal::new(0usize);
    let tick = RwSignal::new(0u64);
    let request = RwSignal::new(0u64);
    let selected = RwSignal::new(Vec::<String>::new());
    let format = RwSignal::new("markdown".to_string());
    let export = RwSignal::new(None::<Value>);
    let exporting = RwSignal::new(false);
    Effect::new(move |_| {
        let _ = section.get();
        offset.set(0);
        selected.set(vec![]);
        export.set(None);
        entries.set(None);
    });
    Effect::new(move |_| {
        let status = section.get();
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        let start = offset.get();
        if status == "collection" {
            return;
        }
        request.update(|n| *n += 1);
        let ticket = request.get_untracked();
        loading.set(true);
        error.set(String::new());
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/bragbook?status={status}&offset={start}"),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<BragbookList>(v)
                    .map_err(|_| "Invalid bragbook response".into())
            });
            if request.try_get_untracked() != Some(ticket) {
                return;
            }
            loading.set(false);
            match result {
                Ok(v) => entries.set(Some(v)),
                Err(e) => {
                    error.set(e);
                    entries.set(None);
                }
            }
        });
    });
    view! {
        <Show when=move||section.get()=="collection"><Collection snap=snap notice=notice tick=tick/></Show>
        <Show when=move||section.get()!="collection">
            <section class="panel bragbook-intro">
                <div class="section-heading"><h2>{move||match section.get().as_str(){"accepted"=>"Accepted accomplishments","dismissed"=>"Dismissed candidates",_=>"Accomplishments to review"}}</h2>
                    <button class="source-link" disabled=move||loading.get() on:click=move|_|tick.update(|n|*n+=1)>"Refresh"</button></div>
                <p class="muted">"Check the source, describe your contribution, and add the impact you can support. Acceptance records your review."</p>
                <Show when=move||loading.get()><p role="status">"Loading entries…"</p></Show>
                <Show when=move||!error.get().is_empty()><p role="alert">{move||error.get()}</p></Show>
                <Show when=move||!loading.get()&&error.get().is_empty()&&entries.get().is_some_and(|v|v.total==0)>
                    <p>{move||if section.get()=="accepted"{"Accepted accomplishments will appear here, ready for highlights."}else if section.get()=="dismissed"{"No dismissed candidates."}else{"No candidates to review yet. Choose sources in Collection, then collect existing work or let new accomplishments arrive."}}</p>
                    <button class="button secondary" on:click=move|_|section.set("collection".into())>"Collection settings"</button>
                </Show>
                <Show when=move||section.get()=="accepted"&&entries.get().is_some_and(|v|v.total>0)>
                    <div class="bragbook-export-controls">
                        <p>{move||format!("{} selected for highlights",selected.get().len())}</p>
                        <label>"Export format"<select prop:value=move||format.get() on:change=move|ev|{format.set(event_target_value(&ev));export.set(None);}>
                            <option value="markdown">"Markdown"</option><option value="json">"JSON"</option></select></label>
                        <button class="button primary" disabled=move||selected.get().is_empty()||exporting.get() on:click=move|_|{
                            exporting.set(true);export.set(None);
                            let body=json!({"entry_ids":selected.get_untracked(),"format":format.get_untracked()});
                            spawn_local(async move{let r=api("/api/v1/bragbook/export","POST",Some(body)).await;
                                if exporting.try_get_untracked().is_none(){return;}exporting.set(false);
                                match r{Ok(v)=>export.set(Some(v)),Err(e)=>notice.set(e)}});
                        }>{move||if exporting.get(){"Preparing…"}else{"Prepare highlights"}}</button>
                    </div>
                    {move||export.get().map(|v|{
                        let body=v["body"].as_str().unwrap_or_default().to_string();let download=body.clone();
                        let name=v["filename"].as_str().unwrap_or("bragbook.md").to_string();let mime=v["content_type"].as_str().unwrap_or("text/markdown").to_string();
                        view!{<div class="bragbook-preview"><h3>"Highlights preview"</h3><p class="muted">{format!("{} entries · prepared {}",v["entry_count"],v["exported_at"].as_str().unwrap_or_default())}</p><pre>{body}</pre>
                            <button class="button secondary" on:click=move|_|crate::exports::save_export(&name,&mime,&download)>"Download highlights"</button></div>}
                    })}
                </Show>
            </section>
            <div class="bragbook-entries" aria-busy=move||loading.get()>
                <For each=move||entries.get().map(|v|v.items).unwrap_or_default() key=|e|(e.id.clone(),e.version) children=move|entry|view!{
                    <Entry entry=entry selected=selected export=export snap=snap notice=notice tick=tick/>
                }/>
            </div>
            <Show when=move||entries.get().is_some_and(|v|v.total>0)>
                <div class="workflow-actions"><button class="button secondary" disabled=move||offset.get()==0||loading.get() on:click=move|_|offset.update(|n|*n=n.saturating_sub(25))>"Previous"</button>
                    <span class="muted">{move||entries.get().map(|v|format!("{}–{} of {}",offset.get()+1,offset.get()+v.items.len(),v.total)).unwrap_or_default()}</span>
                    <button class="button secondary" disabled=move||loading.get()||entries.get().is_none_or(|v|v.next_offset.is_none()) on:click=move|_|{if let Some(n)=entries.get_untracked().and_then(|v|v.next_offset){offset.set(n);}}>"Next"</button>
                </div>
            </Show>
        </Show>
    }
}

#[component]
fn Entry(
    entry: BragbookEntry,
    selected: RwSignal<Vec<String>>,
    export: RwSignal<Option<Value>>,
    snap: Snap,
    notice: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let title = RwSignal::new(entry.title.clone());
    let contribution = RwSignal::new(entry.contribution.clone());
    let impact = RwSignal::new(entry.impact.clone());
    let date = RwSignal::new(entry.occurred_on.clone().unwrap_or_default());
    let status = RwSignal::new(if entry.status == "accepted" {
        "accepted".to_string()
    } else {
        "candidate".into()
    });
    let attested = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let key = RwSignal::new(new_id());
    let pending = RwSignal::new(None::<Value>);
    let id = StoredValue::new(entry.id.clone());
    let expected = entry.version;
    let save = move |decision: String| {
        if busy.get_untracked() {
            return;
        }
        let request = json!({"expected_version":expected,"status":decision,"title":title.get_untracked(),"contribution":contribution.get_untracked(),"impact":impact.get_untracked(),"occurred_on":if date.get_untracked().is_empty(){None}else{Some(date.get_untracked())},"idempotency_key":key.get_untracked()});
        if pending
            .get_untracked()
            .as_ref()
            .is_some_and(|old| old != &request)
        {
            key.set(new_id());
        }
        let mut request = request;
        request["idempotency_key"] = json!(key.get_untracked());
        pending.set(Some(request.clone()));
        busy.set(true);
        error.set(String::new());
        spawn_local(async move {
            let r = api(
                &format!("/api/v1/bragbook/{}", id.get_value()),
                "PATCH",
                Some(request),
            )
            .await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match r {
                Ok(_) => {
                    export.set(None);
                    selected.update(|ids| ids.retain(|x| x != &id.get_value()));
                    tick.update(|n| *n += 1);
                    refresh(snap, notice);
                }
                Err(e) => error.set(e),
            }
        });
    };
    let select_id = entry.id.clone();
    let read_id = select_id.clone();
    view! {
        <article class="panel bragbook-entry">
            <div class="section-heading"><h2>{entry.title.clone()}</h2><span class="muted">{entry.status.clone()}</span></div>
            <p class="muted">{format!("{} · collected {}",entry.origin_kind,&entry.collected_at[..10])}</p>
            <p class="workflow-body">{entry.origin_note}</p>
            <Show when=move||entry.support_changed><p class="notice">"This work was reopened or is no longer completed. Review its current state before accepting it."</p></Show>
            <Show when=move||entry.stale_evidence><p class="muted">"Evidence is pinned to an earlier revision. Open the citation to review the original wording."</p></Show>
            <div class="bragbook-evidence">{entry.evidence.into_iter().map(|e|{let target=e.clone();view!{
                <blockquote><button class="source-link" on:click=move|_|navigate_evidence(target.clone(),notice)>{format!("Open source · revision {} · line {}",e.revision,e.line)}</button><p class="workflow-body">{e.quote}</p></blockquote>
            }}).collect_view()}</div>
            {(entry.status=="accepted"&&!entry.support_changed).then(||view!{
                <label class="bragbook-selection"><input type="checkbox" prop:checked=move||selected.get().contains(&read_id) on:change=move|ev|{
                    let checked=event_target_checked(&ev);selected.update(|ids|{ids.retain(|x|x!=&select_id);if checked&&ids.len()<100{ids.push(select_id.clone());}});export.set(None);
                }/>"Include in highlights"</label>
            })}
            <details class="workflow-manage" open=entry.status=="candidate">
                <summary>{if entry.status=="candidate"{"Review candidate"}else{"Edit or review entry"}}</summary>
                <form class="bragbook-form" on:submit=move|ev|{ev.prevent_default();save(status.get_untracked());}>
                    <fieldset disabled=move||busy.get()>
                        <label>"Accomplishment"<input required maxlength="300" prop:value=move||title.get() on:input=move|ev|{title.set(event_target_value(&ev));attested.set(false);}/></label>
                        <label>"Your contribution"<textarea required rows="3" maxlength="4000" prop:value=move||contribution.get() on:input=move|ev|{contribution.set(event_target_value(&ev));attested.set(false);}/></label>
                        <label>"Impact (optional)"<textarea rows="2" maxlength="4000" placeholder="Describe the outcome you can support with the evidence." prop:value=move||impact.get() on:input=move|ev|{impact.set(event_target_value(&ev));attested.set(false);}/></label>
                        <label>"Accomplishment date (optional)"<input type="date" prop:value=move||date.get() on:input=move|ev|{date.set(event_target_value(&ev));attested.set(false);}/></label>
                        <label>"Review decision"<select prop:value=move||status.get() on:change=move|ev|status.set(event_target_value(&ev))>
                            <option value="candidate">"Keep as candidate"</option><option value="accepted">"Accept for my bragbook"</option><option value="dismissed">"Dismiss"</option></select></label>
                        <Show when=move||status.get()=="accepted"><label class="bragbook-attestation"><input type="checkbox" prop:checked=move||attested.get() on:change=move|ev|attested.set(event_target_checked(&ev))/>"I reviewed the evidence, my contribution, and the stated impact."</label></Show>
                        <button type="submit" class="button primary" disabled=move||busy.get()||(status.get()=="accepted"&&(!attested.get()||entry.support_changed))>{move||if busy.get(){"Saving…"}else if status.get()=="accepted"{"Accept entry"}else if status.get()=="dismissed"{"Dismiss entry"}else{"Save candidate"}}</button>
                    </fieldset>
                    <Show when=move||!error.get().is_empty()><p role="alert">{move||error.get()}</p></Show>
                </form>
            </details>
        </article>
    }
}

#[component]
fn Collection(snap: Snap, notice: RwSignal<String>, tick: RwSignal<u64>) -> impl IntoView {
    let plugin = RwSignal::new(None::<PluginRecord>);
    let load = RwSignal::new(0u64);
    let error = RwSignal::new(String::new());
    Effect::new(move |_| {
        let _ = load.get();
        spawn_local(async move {
            match api(&format!("/api/v1/plugins/{PLUGIN}"), "GET", None)
                .await
                .and_then(|v| {
                    serde_json::from_value(v).map_err(|_| "Invalid plugin settings".into())
                }) {
                Ok(p) => plugin.set(Some(p)),
                Err(e) => error.set(e),
            }
        });
    });
    view! {<section class="panel bragbook-collection"><h2>"Collection settings"</h2>
        <p>"Choose the sources Bragbook can read. Once enabled, new completed work and marked accomplishments become candidates automatically."</p>
        <p class="muted">"Use Accomplishment:, Achievement:, Impact: or Shipped: at the start of a source line to mark an accomplishment. Collection runs locally."</p>
        <Show when=move||!error.get().is_empty()><p role="alert">{move||error.get()}</p></Show>
        <Show when=move||plugin.get().is_none()&&error.get().is_empty()><p role="status">"Loading collection settings…"</p></Show>
        <For each={move||plugin.get().into_iter().collect::<Vec<_>>()} key=|p|p.version children={move|p|view!{<CollectionForm plugin=p snap=snap notice=notice tick=tick load=load/>}}/>
    </section>}
}
#[component]
fn CollectionForm(
    plugin: PluginRecord,
    snap: Snap,
    notice: RwSignal<String>,
    tick: RwSignal<u64>,
    load: RwSignal<u64>,
) -> impl IntoView {
    let enabled = RwSignal::new(plugin.enabled);
    let sources = RwSignal::new(plugin.grant.source_ids.clone());
    let projects = RwSignal::new(plugin.grant.project_ids.clone());
    let tasks = RwSignal::new(
        plugin.configuration["completed-tasks"]
            .as_bool()
            .unwrap_or(true),
    );
    let deliveries = RwSignal::new(
        plugin.configuration["confirmed-deliveries"]
            .as_bool()
            .unwrap_or(true),
    );
    let markers = RwSignal::new(
        plugin.configuration["source-markers"]
            .as_bool()
            .unwrap_or(true),
    );
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let message = RwSignal::new(String::new());
    let saved_enabled = plugin.enabled;
    let version = plugin.version;
    let external = plugin.grant.external;
    view! {<form class="bragbook-form" on:submit=move|ev|{
        ev.prevent_default();busy.set(true);error.set(String::new());
        let body=json!({"expected_version":version,"enabled":enabled.get_untracked(),"grant":{"source_ids":sources.get_untracked(),"project_ids":projects.get_untracked(),"operations":["sources.read"],"external":external},"configuration":{"completed-tasks":tasks.get_untracked(),"confirmed-deliveries":deliveries.get_untracked(),"source-markers":markers.get_untracked()},"idempotency_key":new_id()});
        spawn_local(async move{let r=api(&format!("/api/v1/plugins/{PLUGIN}"),"PATCH",Some(body)).await;
            if busy.try_get_untracked().is_none(){return;}busy.set(false);match r{Ok(_)=>{load.update(|n|*n+=1);tick.update(|n|*n+=1);notice.set("Bragbook collection settings saved.".into());refresh(snap,notice);},Err(e)=>error.set(e)}});
    }>
        <fieldset disabled=move||busy.get()>
            <label class="bragbook-attestation"><input type="checkbox" prop:checked=move||enabled.get() on:change=move|ev|enabled.set(event_target_checked(&ev))/>"Automatically collect new candidates"</label>
            <fieldset class="runtime-sources"><legend>"Sources to include"</legend>{move||snap.get().map(|s|s.sources.into_iter().map(|s|{let read=s.id.clone();let id=s.id;view!{
                <label><input type="checkbox" prop:checked=move||sources.get().contains(&read) on:change=move|ev|{let checked=event_target_checked(&ev);sources.update(|v|{v.retain(|x|x!=&id);if checked{v.push(id.clone());}});}/>{s.name}</label>
            }}).collect_view())}</fieldset>
            <fieldset class="runtime-sources"><legend>"Projects (current sources)"</legend>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|{let read=p.id.clone();let id=p.id;view!{
                <label><input type="checkbox" prop:checked=move||projects.get().contains(&read) on:change=move|ev|{let checked=event_target_checked(&ev);projects.update(|v|{v.retain(|x|x!=&id);if checked{v.push(id.clone());}});}/>{p.name}</label>
            }}).collect_view())}</fieldset>
            <fieldset class="runtime-sources"><legend>"Collect candidates from"</legend>
                <label><input type="checkbox" prop:checked=move||tasks.get() on:change=move|ev|tasks.set(event_target_checked(&ev))/>"Completed tasks"</label>
                <label><input type="checkbox" prop:checked=move||deliveries.get() on:change=move|ev|deliveries.set(event_target_checked(&ev))/>"Confirmed deliveries"</label>
                <label><input type="checkbox" prop:checked=move||markers.get() on:change=move|ev|markers.set(event_target_checked(&ev))/>"Marked source lines"</label>
            </fieldset>
            <button type="submit" class="button primary" disabled=move||busy.get()||(enabled.get()&&sources.get().is_empty()&&projects.get().is_empty())>{move||if busy.get(){"Saving…"}else{"Save collection settings"}}</button>
        </fieldset>
        <Show when=move||!error.get().is_empty()><p role="alert">{move||error.get()}</p></Show>
    </form>
    <div class="bragbook-backfill"><h3>"Collect existing work"</h3><p class="muted">"Scan the saved source selection for current completed work and marked lines. Repeated scans keep your reviews and avoid duplicate entries. Save changes above before scanning."</p>
        <button class="button secondary" disabled=move||busy.get()||!saved_enabled on:click=move|_|{
            busy.set(true);error.set(String::new());message.set(String::new());
            spawn_local(async move{let r=api("/api/v1/bragbook/collect","POST",Some(json!({}))).await;
                if busy.try_get_untracked().is_none(){return;}busy.set(false);
                match r{Ok(v)=>{message.set(format!("Collected {} new candidates. {} source documents checked; {} marked lines skipped by size or count limits.",v["created"],v["documents_considered"],v["skipped_lines"]));tick.update(|n|*n+=1);refresh(snap,notice);},Err(e)=>error.set(e)}});
        }>{move||if busy.get(){"Working…"}else{"Collect existing work"}}</button>
        <p role="status">{move||message.get()}</p>
    </div>}
}
