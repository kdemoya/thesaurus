use super::*;
#[component]
pub(super) fn MemorySetup(
    snap: Snap,
    draft: RwSignal<OnboardingProgress>,
    error: RwSignal<String>,
    reload: Callback<()>,
) -> impl IntoView {
    let name = RwSignal::new("My work agent".to_string());
    let sources = RwSignal::new(Vec::<String>::new());
    let local = RwSignal::new(false);
    let approved = RwSignal::new(false);
    let config = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let copied = RwSignal::new(false);
    let saved = RwSignal::new(String::new());
    if let Some(guard) = use_context::<SetupDraftGuard>() {
        Effect::new(move |_| {
            let signature = format!("{}:{}:{:?}", name.get(), local.get(), sources.get());
            guard.0.update(|m| {
                m.insert(
                    "memory",
                    !sources.get().is_empty() && saved.get() != signature,
                );
            });
        });
        on_cleanup(move || {
            guard.0.update(|m| {
                m.remove("memory");
            })
        });
    }
    Effect::new(move |_| {
        let _ = sources.get();
        let _ = local.get();
        approved.set(false);
    });
    if let Some(id) = draft.get_untracked().memory_grant_id {
        spawn_local(async move {
            let r = api(&format!("/api/v1/onboarding/memory/{id}"), "GET", None).await;
            if config.try_get_untracked().is_none() {
                return;
            }
            match r {
                Ok(v) => config.set(
                    serde_json::to_string_pretty(&json!({"mcpServers":v["mcpServers"]}))
                        .unwrap_or_default(),
                ),
                Err(e) => error.set(e),
            }
        });
    }

    view! {<div class="onboard-memory"><p class="onboard-lede">"Give your agent the right context."</p><p class="muted">"Choose exactly which sources it can read. Access is read-only; your primary model and recording permissions stay separate."</p>
        <Show when=move||snap.get().is_some_and(|s|!s.sources.is_empty()) fallback=move||view!{<p class="onboard-note">"There are no sources yet. Continue to import a small example, then return to Prepare to select it."</p>}>
            <form class="onboard-form" on:submit=move|ev|{ev.prevent_default();if busy.get_untracked()||!approved.get_untracked(){return;}busy.set(true);let request=json!({"name":name.get_untracked(),"source_ids":sources.get_untracked(),"local_agent":local.get_untracked(),"approve_access":true,"idempotency_key":new_id()});spawn_local(async move{let r=api("/api/v1/onboarding/memory","POST",Some(request)).await;if busy.try_get_untracked().is_none(){return;}busy.set(false);match r{Ok(v)=>{saved.set(format!("{}:{}:{:?}",name.get_untracked(),local.get_untracked(),sources.get_untracked()));if let Some(id)=v["grant"]["id"].as_str(){draft.update(|p|p.memory_grant_id=Some(id.into()));}config.set(serde_json::to_string_pretty(&json!({"mcpServers":v["mcpServers"]})).unwrap_or_default());reload.run(());refresh(snap,error);},Err(e)=>error.set(e)}});}><fieldset disabled=move||busy.get()>
            <label>"Agent name"</label><input required maxlength="100" prop:value=move||name.get() on:input=move|ev|name.set(event_target_value(&ev))/><label class="onboard-checkbox"><input type="checkbox" prop:checked=move||local.get() on:change=move|ev|{local.set(event_target_checked(&ev));sources.set(vec![]);}/>"This agent processes entirely on this computer"</label>
            <div class="onboard-source-choices">{move||snap.get().map(|s|s.sources.into_iter().map(|s|{let id=s.id.clone();let check=id.clone();view!{<label class="onboard-checkbox"><input type="checkbox" disabled=move||!s.allow_external&&!local.get() prop:checked=move||sources.get().contains(&check) on:change=move|ev|{let yes=event_target_checked(&ev);sources.update(|ids|{ids.retain(|v|v!=&id);if yes{ids.push(id.clone());}});}/><span>{s.name}{(!s.allow_external).then(||view!{<small>"local only · sharing can be changed in Sources"</small>})}</span></label>}}).collect_view())}</div>
            <label class="onboard-checkbox"><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>"Create read-only access to these selected sources"</label><button class="button secondary" type="submit" disabled=move||!approved.get()||sources.get().is_empty()>"Prepare MCP access"</button><button class="button text" type="button" on:click=move|_|{sources.set(vec![]);approved.set(false);}>"Clear selection"</button></fieldset></form>
        </Show>
        <Show when=move||!config.get().is_empty()><div class="onboard-result"><p>"Access is prepared. Add this configuration in your chosen agent to connect. The private grant file stays on this computer."</p><pre>{move||config.get()}</pre><button class="button text" type="button" on:click=move|_|{let text=config.get_untracked();spawn_local(async move{match copy_setup(&text).await{Ok(_)=>copied.set(true),Err(_)=>error.set("Clipboard is unavailable; select and copy the configuration above".into())}});}>{move||if copied.get(){"Copied"}else{"Copy MCP configuration"}}</button></div></Show>
        <Show when=move||draft.get().memory_grant_id.is_some()><p class="onboard-note">"A memory grant is selected. Revoke it from your agent access controls or the CLI if you no longer need it."</p></Show>
    </div>}
}
