use crate::{Snap, api, new_id, refresh};
use leptos::{prelude::*, task::spawn_local};
use serde_json::{Value, json};
use thesaurus_contracts::*;

fn send(
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
        let result = api(&path, method, Some(body)).await;
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
}
#[component]
pub fn SourceControls(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let source = RwSignal::new(String::new());
    let reload = RwSignal::new(0u64);
    let epoch = RwSignal::new(0u64);
    let health = RwSignal::new(None::<SourceHealth>);
    let preview = RwSignal::new(None::<SourceDisclosurePreview>);
    Effect::new(move |_| {
        if source.get().is_empty() {
            if let Some(s) = snap.get().and_then(|s| s.sources.first().cloned()) {
                source.set(s.id);
            }
        }
    });
    Effect::new(move |_| {
        let id = source.get();
        let _ = snap.get().map(|s| s.watermark);
        let _ = reload.get();
        if id.is_empty() {
            return;
        }
        let ticket = epoch.get_untracked() + 1;
        epoch.set(ticket);
        spawn_local(async move {
            let response = async {
                let health = api(&format!("/api/v1/sources/{id}/health"), "GET", None).await?;
                let preview = api(&format!("/api/v1/sources/{id}/disclosure"), "GET", None).await?;
                Ok::<_, String>((
                    serde_json::from_value::<SourceHealth>(health)
                        .map_err(|_| "Invalid source health".to_string())?,
                    serde_json::from_value::<SourceDisclosurePreview>(preview)
                        .map_err(|_| "Invalid source disclosure".to_string())?,
                ))
            }
            .await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match response {
                Ok((h, p)) => {
                    health.set(Some(h));
                    preview.set(Some(p));
                }
                Err(e) => notice.set(e),
            }
        });
    });
    view! {<section class="panel source-controls"><div class="section-heading"><h2>"Source health and disclosure"</h2><button class="source-link" on:click=move|_|reload.update(|n|*n+=1)>"refresh"</button></div><label for="controlled-source">"Source"</label><select id="controlled-source" prop:value=move||source.get() on:change=move|ev|source.set(event_target_value(&ev))>{move||snap.get().map(|s|s.sources.into_iter().map(|s|{let id=s.id.clone();view!{<option value=s.id prop:selected=move||source.get()==id>{s.name}</option>}}).collect_view())}</select>
        <For each={move||health.get().filter(|item|item.coverage.source_id==source.get()).into_iter().collect::<Vec<_>>()} key={|item|(item.coverage.source_id.clone(),item.coverage.version)} children={move|item|view!{<CoverageForm health=item live_health=health snap=snap notice=notice reload=reload/>}}/>
        <For each={move||preview.get().filter(|item|item.policy.source_id==source.get()).into_iter().collect::<Vec<_>>()} key={|item|(item.policy.source_id.clone(),item.policy.version)} children={move|item|view!{<PolicyForm preview=item live_preview=preview snap=snap notice=notice reload=reload/>}}/>
    </section>}
}

#[component]
fn PolicyForm(
    preview: SourceDisclosurePreview,
    live_preview: RwSignal<Option<SourceDisclosurePreview>>,
    snap: Snap,
    notice: RwSignal<String>,
    reload: RwSignal<u64>,
) -> impl IntoView {
    let external = RwSignal::new(preview.policy.external_enabled);
    let connections = RwSignal::new(preview.policy.denied_connections.clone());
    let grants = RwSignal::new(preview.policy.denied_grants.clone());
    let busy = RwSignal::new(false);
    let source = StoredValue::new(preview.policy.source_id.clone());
    let version = preview.policy.version;
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        send(
            format!("/api/v1/sources/{}/policy", source.get_value()),
            "PUT",
            json!({"expected_version":version,"external_enabled":external.get_untracked(),"denied_connections":connections.get_untracked(),"denied_grants":grants.get_untracked(),"idempotency_key":new_id()}),
            busy,
            snap,
            notice,
            reload,
        );
    };
    view! {<details class="workflow-manage"><summary>"Data destinations · preview and revoke"</summary><p class="muted">{preview.scope_note}</p><p class="source-control-uri">{preview.source_uri}</p><small class="muted">{format!("Source documents: {}",preview.document_ids.join(", "))}</small><form class="workflow-form" on:submit=submit><fieldset class="focus-form-fields" disabled=move||busy.get()><label class="focus-check"><input type="checkbox" prop:checked=move||external.get() on:change=move|ev|external.set(event_target_checked(&ev))/><span>"Allow this source to reach external destinations"</span></label><p class="muted">"Destination checkboxes below permit access when this external sharing switch and the destination's existing grant/usage permissions also allow it. Local indexing continues."</p>
    {move||live_preview.get().filter(|item|item.policy.source_id==source.get_value()).map(|item|item.destinations.into_iter().map(|dest|{let id=dest.destination_id.clone();let checked=id.clone();let blocked=if dest.kind=="grant"{grants}else{connections};let local=dest.kind=="connection"&&id=="local";view!{<div class="source-destination"><label class="focus-check"><input type="checkbox" disabled=local prop:checked=move||!blocked.get().contains(&checked) on:change=move|ev|{let allow=event_target_checked(&ev);blocked.update(|list|{list.retain(|x|x!=&id);if !allow{list.push(id.clone());}});}/><span>{format!("{} · {} · {}",dest.name,dest.kind,if dest.external{"external"}else{"local"})}</span></label><small class="muted">{format!("Currently {}. {}",if dest.allowed{"allowed"}else{"blocked"},dest.reason)}</small></div>}}).collect_view())}<button class="button primary" type="submit">"Apply destination policy"</button></fieldset></form></details>}
}

#[component]
fn CoverageForm(
    health: SourceHealth,
    live_health: RwSignal<Option<SourceHealth>>,
    snap: Snap,
    notice: RwSignal<String>,
    reload: RwSignal<u64>,
) -> impl IntoView {
    let source = StoredValue::new(health.coverage.source_id.clone());
    let version = health.coverage.version;
    let interval = RwSignal::new(
        health
            .coverage
            .expected_interval_hours
            .map(|n| n.to_string())
            .unwrap_or_default(),
    );
    let excluded = RwSignal::new(health.coverage.excluded_content.join("\n"));
    let gaps = RwSignal::new(health.coverage.capture_gaps.clone());
    let start = RwSignal::new(String::new());
    let end = RwSignal::new(String::new());
    let gap_note = RwSignal::new(String::new());
    let retry_query = RwSignal::new(String::new());
    let retry_approved = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let hours = if interval.get_untracked().trim().is_empty() {
            None
        } else {
            match interval.get_untracked().parse::<u32>() {
                Ok(h) => Some(h),
                Err(_) => {
                    notice.set("Expected interval must be a whole number of hours".into());
                    return;
                }
            }
        };
        send(
            format!("/api/v1/sources/{}/coverage", source.get_value()),
            "PUT",
            json!({"expected_version":version,"expected_interval_hours":hours,"excluded_content":excluded.get_untracked().lines().map(str::trim).filter(|s|!s.is_empty()).collect::<Vec<_>>(),"capture_gaps":gaps.get_untracked(),"idempotency_key":new_id()}),
            busy,
            snap,
            notice,
            reload,
        );
    };
    view! {{move||live_health.get().filter(|item|item.coverage.source_id==source.get_value()).map(|health|view!{<div class="source-health"><p class="workflow-topline"><span class=if health.stale{"orange"}else{"muted"}>{if health.stale{"past expected interval"}else{"captured snapshot"}}</span><span>{format!("Last ingestion: {}",health.last_ingested_at)}</span></p><p class="muted">{health.ingestion_mode}</p>{health.retrieval_policy.clone().map(|policy|view!{<p class="muted">{format!("Retrieval policy: {} · last successful sync: {}",policy,health.last_synced_at.clone().unwrap_or_else(||"never".into()))}</p>})}{health.upstream_error.map(|error|view!{<p class="orange">{error}</p>})}<p>{format!("{} current documents processed · {} pending jobs · {} failed jobs",health.current_documents_processed,health.pending_current_jobs,health.failed_current_jobs)}</p><ul>{health.effect_on_answers.into_iter().map(|effect|view!{<li>{effect}</li>}).collect_view()}</ul><div class="workflow-actions">{health.document_ids.into_iter().map(|document|{let id=document.clone();view!{<button class="button secondary" disabled=move||busy.get() on:click=move|_|send(format!("/api/v1/sources/{}/retry",source.get_value()),"POST",json!({"document_id":id}),busy,snap,notice,reload)>{format!("Retry local extraction · {}",document.chars().take(8).collect::<String>())}</button>}}).collect_view()}</div></div>})}
    {health.upstream_id.map(|id|view!{<details class="workflow-manage"><summary>"Retry this upstream source"</summary><p class="muted">"Send the query below to this source's configured upstream. Existing destination permissions and usage limits apply."</p><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();if !retry_approved.get_untracked(){notice.set("Approve the upstream read for this query".into());return;}let approved=retry_approved.get_untracked();retry_approved.set(false);send(format!("/api/v1/upstreams/{id}/refresh"),"POST",json!({"query":retry_query.get_untracked(),"approve_upstream":approved,"idempotency_key":new_id()}),busy,snap,notice,reload);}>
    <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Query"<input required maxlength="1024" prop:value=move||retry_query.get() on:input=move|ev|{retry_query.set(event_target_value(&ev));retry_approved.set(false);}/></label><label class="focus-check"><input type="checkbox" required prop:checked=move||retry_approved.get() on:change=move|ev|retry_approved.set(event_target_checked(&ev))/><span>"Approve this upstream read and query transmission"</span></label><button class="button secondary" type="submit" disabled=move||busy.get()||!retry_approved.get()>"Retry upstream read"</button></fieldset></form></details>})}
    <details class="workflow-manage"><summary>"Expected freshness, exclusions and capture gaps"</summary><form class="workflow-form" on:submit=submit><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Expected update interval in hours (blank for no expectation)"</label><input type="number" min="1" max="87600" prop:value=move||interval.get() on:input=move|ev|interval.set(event_target_value(&ev))/><label>"Excluded content, one description per line"</label><textarea rows="3" prop:value=move||excluded.get() on:input=move|ev|excluded.set(event_target_value(&ev))></textarea><h3 class="focus-section-title">"Capture gaps"</h3>{move||gaps.get().into_iter().enumerate().map(|(index,gap)|view!{<div class="source-gap"><span>{format!("{} → {} · {}",gap.started_at,gap.ended_at.unwrap_or_else(||"ongoing".into()),gap.note)}</span><button class="button text" type="button" on:click=move|_|gaps.update(|g|{g.remove(index);})>"Remove"</button></div>}).collect_view()}<label>"Gap started (timestamp with timezone)"</label><input placeholder="2026-10-17T09:00:00-04:00" prop:value=move||start.get() on:input=move|ev|start.set(event_target_value(&ev))/><label>"Gap ended (blank for ongoing)"</label><input placeholder="2026-10-17T10:00:00-04:00" prop:value=move||end.get() on:input=move|ev|end.set(event_target_value(&ev))/><label>"What was missed?"</label><input maxlength="1000" prop:value=move||gap_note.get() on:input=move|ev|gap_note.set(event_target_value(&ev))/><button type="button" class="button text" on:click=move|_|{if start.get_untracked().trim().is_empty()||gap_note.get_untracked().trim().is_empty(){notice.set("Add a start timestamp and a description of the capture gap".into());return;}gaps.update(|g|g.push(CaptureGap{started_at:start.get_untracked(),ended_at:(!end.get_untracked().trim().is_empty()).then(||end.get_untracked()),note:gap_note.get_untracked()}));start.set(String::new());end.set(String::new());gap_note.set(String::new());}>"Add gap to draft"</button><button class="button primary" type="submit">"Save coverage notes"</button></fieldset></form></details>}
}

#[component]
pub fn WorkNotifications(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let items = RwSignal::new(Vec::<WorkNotification>::new());
    let rules = RwSignal::new(Vec::<NotificationMute>::new());
    let reload = RwSignal::new(0u64);
    let epoch = RwSignal::new(0u64);
    let busy = RwSignal::new(false);
    let kind = RwSignal::new("*".to_string());
    let source = RwSignal::new(String::new());
    let until = RwSignal::new(String::new());
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let cleanup = alive.clone();
    on_cleanup(move || cleanup.store(false, std::sync::atomic::Ordering::Relaxed));
    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(30000).await;
            if !alive.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            reload.update(|n| *n += 1);
        }
    });
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = reload.get();
        let ticket = epoch.get_untracked() + 1;
        epoch.set(ticket);
        spawn_local(async move {
            let response = async {
                let list = api("/api/v1/notifications", "GET", None).await?;
                let mutes = api("/api/v1/notification-mutes", "GET", None).await?;
                Ok::<_, String>((
                    serde_json::from_value::<Vec<WorkNotification>>(list)
                        .map_err(|_| "Invalid notifications".to_string())?,
                    serde_json::from_value::<Vec<NotificationMute>>(mutes)
                        .map_err(|_| "Invalid mute rules".to_string())?,
                ))
            }
            .await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match response {
                Ok((list, mutes)) => {
                    items.set(list);
                    rules.set(mutes);
                }
                Err(e) => notice.set(e),
            }
        });
    });
    let mute = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let source_id = (!source.get_untracked().is_empty()).then(|| source.get_untracked());
        let version = rules
            .get_untracked()
            .iter()
            .find(|r| r.kind == kind.get_untracked() && r.source_id == source_id)
            .map(|r| r.version)
            .unwrap_or(0);
        send(
            "/api/v1/notification-mutes".into(),
            "PUT",
            json!({"expected_version":version,"kind":kind.get_untracked(),"source_id":source_id,"active":true,"until":(!until.get_untracked().trim().is_empty()).then(||until.get_untracked()),"idempotency_key":new_id()}),
            busy,
            snap,
            notice,
            reload,
        );
    };
    view! {<section class="panel work-notifications"><div class="section-heading"><h2>"Notifications"</h2><button class="source-link" on:click=move|_|reload.update(|n|*n+=1)>"refresh"</button></div><p class="muted">"Local suggestions and reminders with their triggering evidence. Updated every 30 seconds. Follow-ups send no messages."</p><Show when=move||items.get().is_empty()><p class="muted">"No current notifications."</p></Show>{move||items.get().into_iter().map(|item|{let read=item.id.clone();let dismiss=item.id.clone();let version=item.version;view!{<article class="work-notification"><div class="workflow-topline"><strong>{item.title}</strong><small>{format!("{} · {}",item.status,item.triggered_at)}</small></div><p>{item.why_now}</p><details class="brief-evidence"><summary>"Triggering evidence"</summary>{item.evidence.into_iter().map(|e|view!{<blockquote><small>{format!("{} r{} line {}",e.document_id,e.revision,e.line)}</small>{e.quote.map(|q|view!{<p>{q}</p>})}</blockquote>}).collect_view()}<small>{format!("Sources: {} · record: {}",item.source_ids.join(", "),item.record_id)}</small></details><div class="workflow-actions"><button class="button text" disabled=move||busy.get() on:click=move|_|send(format!("/api/v1/notifications/{read}"),"PATCH",json!({"expected_version":version,"status":"read","idempotency_key":new_id()}),busy,snap,notice,reload)>"Mark read"</button><button class="button text" disabled=move||busy.get() on:click=move|_|send(format!("/api/v1/notifications/{dismiss}"),"PATCH",json!({"expected_version":version,"status":"dismissed","idempotency_key":new_id()}),busy,snap,notice,reload)>"Dismiss"</button></div></article>}}).collect_view()}
        <details class="workflow-manage"><summary>"Mute similar notifications"</summary><form class="workflow-form" on:submit=mute><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Kind"<select prop:value=move||kind.get() on:change=move|ev|kind.set(event_target_value(&ev))><option value="*">"All kinds"</option><option value="action_candidate">"New action candidates"</option><option value="source_changed">"Source changes"</option><option value="processing_failed">"Processing failures"</option><option value="waiting_due">"Follow-up dates"</option></select></label><label>"Source"<select prop:value=move||source.get() on:change=move|ev|source.set(event_target_value(&ev))><option value="">"All sources"</option>{move||snap.get().map(|s|s.sources.into_iter().map(|s|view!{<option value=s.id>{s.name}</option>}).collect_view())}</select></label><label>"Mute until (timestamp with timezone, blank for indefinitely)"</label><input prop:value=move||until.get() on:input=move|ev|until.set(event_target_value(&ev)) placeholder="2026-10-18T09:00:00-04:00"/><button class="button secondary" type="submit">"Save mute"</button></fieldset></form>{move||rules.get().into_iter().filter(|r|r.active).map(|rule|view!{<div class="source-gap"><span>{format!("{} · {} · until {}",rule.kind,rule.source_id.clone().unwrap_or_else(||"all sources".into()),rule.until.clone().unwrap_or_else(||"unmuted".into()))}</span><button class="button text" disabled=move||busy.get() on:click=move|_|send("/api/v1/notification-mutes".into(),"PUT",json!({"expected_version":rule.version,"kind":rule.kind,"source_id":rule.source_id,"active":false,"until":rule.until,"idempotency_key":new_id()}),busy,snap,notice,reload)>"Unmute"</button></div>}).collect_view()}</details>
    </section>}
}
