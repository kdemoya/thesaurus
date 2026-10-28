use super::workflows::{Pending, action};
use super::*;

#[component]
pub(super) fn PluginModelReview(
    plugin: String,
    id: String,
    connections: Vec<String>,
    snap: Snap,
    notice: RwSignal<String>,
) -> impl IntoView {
    let proposal = RwSignal::new(None::<PluginModelProposal>);
    let preview = RwSignal::new(None::<PluginModelPreview>);
    let connection = RwSignal::new(String::new());
    let approved = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let refresh = RwSignal::new(0u64);
    let receipt = RwSignal::new(None::<RuntimeRun>);
    let base = StoredValue::new(format!("/api/v1/plugins/{plugin}/models/{id}"));
    Effect::new(move |_| {
        let _ = refresh.get();
        spawn_local(async move {
            let result = api(&base.with_value(|s| s.clone()), "GET", None)
                .await
                .and_then(|v| {
                    serde_json::from_value(v).map_err(|_| "Invalid model proposal".into())
                });
            if proposal.try_get_untracked().is_none() {
                return;
            }
            match result {
                Ok(p) => proposal.set(Some(p)),
                Err(e) => notice.set(e),
            }
        });
    });
    let timer = gloo_timers::callback::Interval::new(1000, move || {
        if proposal
            .get_untracked()
            .is_some_and(|p| p.status == "running")
        {
            refresh.update(|v| *v += 1);
        }
    });
    let _timer = StoredValue::new_local(timer);
    view! {<details class="workflow-manage"><summary>"Model request and approval"</summary>
        {move||proposal.get().map(|p|{let awaiting=p.status=="pending_approval";view!{<p>{format!("{} · {}",p.request.key,p.status)}</p><p>{p.error.unwrap_or_default()}</p>
            {awaiting.then(||{let connections=connections.clone();view!{<label>"Model connection"<select aria-label="Plugin model connection" prop:value=move||connection.get() on:change=move|ev|{connection.set(event_target_value(&ev));preview.set(None);approved.set(false);}><option value="">"Choose a granted connection"</option>{move||snap.get().map(|s|s.connections.into_iter().filter(|c|connections.contains(&c.id)&&matches!(c.kind.as_str(),"acp"|"openai_compatible")).map(|c|view!{<option value=c.id>{format!("{} · {}",c.model,c.billing)}</option>}).collect_view())}</select></label>
                <button class="source-link" disabled=move||busy.get()||connection.get().is_empty() on:click=move|_|{busy.set(true);preview.set(None);approved.set(false);spawn_local(async move{let result=api(&format!("{}/preview",base.get_value()),"POST",Some(json!({"connection":connection.get_untracked()}))).await.and_then(|v|serde_json::from_value(v).map_err(|_|"Invalid model preview".into()));if busy.try_get_untracked().is_none(){return;}busy.set(false);match result{Ok(p)=>preview.set(Some(p)),Err(e)=>notice.set(e)}});}>"Review exact model request"</button>
                {move||preview.get().map(|p|view!{<p>{format!("{} · {} · ~{} input tokens · {} · {} host output bytes",p.connection,p.model,p.approximate_input_tokens,if p.connection_kind=="openai_compatible"{format!("API token request limit {}",p.proposal.request.max_output_tokens)}else{"ACP token cap unavailable".into()},p.proposal.request.max_output_bytes)}</p><p class="muted">{p.note}</p><details><summary>"Exact prompt and source data"</summary><pre>{p.prompt}</pre></details><label><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>"Approve this one request and return its bounded result to this plugin"</label><button class="button secondary" disabled=move||busy.get()||!approved.get() on:click=move|_|action(format!("{}/approve",base.get_value()),"POST",json!({"connection":p.connection,"signature":p.signature,"approve_usage":approved.get_untracked(),"approve_result_delivery":approved.get_untracked(),"max_approximate_input_tokens":p.approximate_input_tokens}),snap,notice,busy,pending,move|v|{if let Ok(r)=serde_json::from_value(v){receipt.set(Some(r));}preview.set(None);approved.set(false);refresh.update(|v|*v+=1);})>"Run approved model request"</button>})}
                <button class="source-link" disabled=move||busy.get() on:click=move|_|action(format!("{}/reject",base.get_value()),"POST",json!({}),snap,notice,busy,pending,move|_|{preview.set(None);refresh.update(|v|*v+=1);})>"Reject without usage"</button>
            }})}
            {p.runtime_id.map(|id|view!{<p>{format!("Runtime {id}")}</p><button class="source-link" disabled=move||busy.get() on:click=move|_|{let id=id.clone();spawn_local(async move{match api(&format!("/api/v1/runtime/runs/{id}"),"GET",None).await.and_then(|v|serde_json::from_value(v).map_err(|_|"Invalid runtime receipt".into())){Ok(r)=>receipt.set(Some(r)),Err(e)=>notice.set(e)}});}>"Inspect runtime receipt"</button>})}
        }})}
        {move||receipt.get().map(|r|view!{<details><summary>"Runtime result and provenance"</summary><pre>{serde_json::to_string_pretty(&r).unwrap_or_default()}</pre></details>})}
    </details>}
}
