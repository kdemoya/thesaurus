use super::workflows::{Pending, action};
use super::*;
#[component]
pub(super) fn UsageControls(
    snap: Snap,
    notice: RwSignal<String>,
    view: RwSignal<String>,
) -> impl IntoView {
    let usages = RwSignal::new(Vec::<ConnectionUsage>::new());
    let tick = RwSignal::new(0u64);
    let epoch = RwSignal::new(0u64);
    let primary = RwSignal::new(String::new());
    let preview = RwSignal::new(None::<FallbackPreview>);
    let approved = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        epoch.update(|n| *n += 1);
        let e = epoch.get_untracked();
        spawn_local(async move {
            match api("/api/v1/usage", "GET", None).await {
                Ok(v) => {
                    if epoch.try_get_untracked() == Some(e) {
                        if let Ok(items) = serde_json::from_value::<Vec<ConnectionUsage>>(v) {
                            if serde_json::to_value(&items).ok()
                                != serde_json::to_value(usages.get_untracked()).ok()
                            {
                                usages.set(items);
                            }
                        }
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    });
    view! {<section class="panel"><div class="section-heading"><h2>"Usage budgets"</h2><button class="source-link" on:click=move|_|tick.update(|n|*n+=1)>"Refresh"</button></div><p class="muted">"Reserve requests for interactive work and pause queued background model work. Limits count attempted/reserved requests per UTC day; account credits and monetary spend are reported by the provider."</p>{move||usages.get().into_iter().map(|usage|view!{<UsageEditor usage=usage snap=snap notice=notice tick=tick/>}).collect_view()}
    <details class="workflow-manage"><summary>"Explicit fallback after a stopped run"</summary><label>"Failed, interrupted or cancelled primary run ID"<input prop:value=move||primary.get() on:input=move|ev|{primary.set(event_target_value(&ev));preview.set(None);approved.set(false);}/></label><button class="button secondary" disabled=move||busy.get() on:click=move|_|{let id=primary.get_untracked();if id.is_empty()||!id.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'){notice.set("Invalid primary run ID".into());return;}busy.set(true);spawn_local(async move{let r=api(&format!("/api/v1/runtime/runs/{id}/fallback"),"GET",None).await;busy.set(false);if primary.get_untracked()!=id{return;}match r{Ok(v)=>if let Ok(p)=serde_json::from_value(v){preview.set(Some(p));},Err(e)=>notice.set(e)}});}>"Preview selected fallback"</button>
    {move||preview.get().map(|p|view!{<p>{format!("{} → {} · policy v{} · ~{} input tokens",p.primary_connection,p.fallback_connection,p.policy_version,p.approximate_input_tokens)}</p><p class="muted">{p.note}</p><details><summary>"Exact retry prompt and source scope"</summary><pre>{format!("Sources: {}\n\n{}",p.source_ids.join(", "),p.prompt)}</pre></details><label><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>"Approve this separate request on the selected fallback account"</label><button class="button primary" disabled=move||busy.get()||!approved.get() on:click=move|_|{action(format!("/api/v1/runtime/runs/{}/fallback",p.primary_run_id),"POST",json!({"signature":p.signature,"approve_usage":approved.get_untracked()}),snap,notice,busy,pending,move|_|{approved.set(false);preview.set(None);view.set("activity".into());});}>"Start approved fallback"</button>})}
    </details></section>}
}
#[component]
fn UsageEditor(
    usage: ConnectionUsage,
    snap: Snap,
    notice: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let id = usage.policy.connection.clone();
    let version = usage.policy.version;
    let daily = RwSignal::new(usage.policy.daily_limit);
    let reserve = RwSignal::new(usage.policy.interactive_reserve);
    let paused = RwSignal::new(usage.policy.background_paused);
    let fallback = RwSignal::new(usage.policy.fallback_connection.unwrap_or_default());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    view! {<details class="workflow-manage"><summary>{format!("{} · {} reserved · {} interactive / {} background remaining",id,usage.requests_reserved_today,usage.interactive_remaining,usage.background_remaining)}</summary><p class="muted">{usage.billing}</p><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let f=fallback.get_untracked();action(format!("/api/v1/usage/{id}"),"PUT",json!({"expected_version":version,"daily_limit":daily.get_untracked(),"interactive_reserve":reserve.get_untracked(),"background_paused":paused.get_untracked(),"fallback_connection":if f.is_empty(){None}else{Some(f)}}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Daily attempt limit"<input type="number" min="1" max=usage.configured_limit prop:value=move||daily.get() on:input=move|ev|daily.set(event_target_value(&ev).parse().unwrap_or(1))/></label><label>"Reserve for interactive requests"<input type="number" min="0" max=usage.configured_limit prop:value=move||reserve.get() on:input=move|ev|reserve.set(event_target_value(&ev).parse().unwrap_or(0))/></label><label><input type="checkbox" prop:checked=move||paused.get() on:change=move|ev|paused.set(event_target_checked(&ev))/>"Pause background model jobs"</label><label>"Optional explicit fallback ACP connection"<select prop:value=move||fallback.get() on:change=move|ev|fallback.set(event_target_value(&ev))><option value="">"No fallback rule"</option>{move||snap.get().map(|s|s.connections.into_iter().filter(|c|c.kind=="acp").map(|c|view!{<option value=c.id.clone()>{c.id.clone()}</option>}).collect_view())}</select></label><small class="muted">"Fallback rules enable a reviewed separate request; they never cause automatic fallback."</small><button class="button secondary" type="submit">"Save usage controls"</button></fieldset></form></details>}
}
