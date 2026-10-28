use super::*;
#[component]
pub(super) fn ModelSetup(
    status: RwSignal<Option<OnboardingStatus>>,
    draft: RwSignal<OnboardingProgress>,
    reload: Callback<()>,
    snap: Snap,
    error: RwSignal<String>,
) -> impl IntoView {
    let form = RwSignal::new(false);
    let kind = RwSignal::new("acp".to_string());
    let id = RwSignal::new(String::new());
    let model = RwSignal::new(String::new());
    let endpoint = RwSignal::new(String::new());
    let command = RwSignal::new(String::new());
    let args = RwSignal::new("[]".to_string());
    let key = RwSignal::new(String::new());
    let reference = RwSignal::new(String::new());
    let limit = RwSignal::new("10".to_string());
    let busy = RwSignal::new(false);
    let approved = RwSignal::new(false);
    let message = RwSignal::new(String::new());
    let probe = RwSignal::new(String::new());
    Effect::new(move |_| {
        let _ = draft.get().primary_connection;
        approved.set(false);
        probe.set(String::new());
    });
    if let Some(guard) = use_context::<SetupDraftGuard>() {
        Effect::new(move |_| {
            guard.0.update(|m| {
                m.insert("model", form.get());
            })
        });
        on_cleanup(move || {
            guard.0.update(|m| {
                m.remove("model");
            })
        });
    }
    let choose_form = move |route: &'static str| {
        form.set(true);
        kind.set(
            if route == "agent" {
                "acp"
            } else {
                "openai_compatible"
            }
            .into(),
        );
        endpoint.set(
            if route == "local" {
                "http://127.0.0.1:11434/v1"
            } else {
                ""
            }
            .into(),
        );
        key.set(String::new());
    };
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let parsed = match serde_json::from_str::<Vec<String>>(&args.get_untracked()) {
            Ok(v) => v,
            Err(_) => {
                error.set("Agent arguments must be a JSON string array, such as []".into());
                return;
            }
        };
        let daily = match limit.get_untracked().parse::<i64>() {
            Ok(v) => v,
            Err(_) => {
                error.set("Choose a daily request limit".into());
                return;
            }
        };
        let secret = key.get_untracked();
        let credential = reference.get_untracked();
        let route = kind.get_untracked();
        let raw_command = command.get_untracked();
        let raw_endpoint = endpoint.get_untracked();
        let request = json!({"id":id.get_untracked(),"kind":route,"model":model.get_untracked(),"endpoint":if route=="openai_compatible"{Some(raw_endpoint)}else{None},"command":if route!="openai_compatible"&&!raw_command.is_empty(){Some(raw_command)}else{None},"args":parsed,"api_key":if secret.is_empty(){None}else{Some(secret)},"credential_ref":if credential.is_empty(){None}else{Some(credential)},"daily_limit":daily,"idempotency_key":new_id()});
        key.set(String::new());
        busy.set(true);
        error.set(String::new());
        spawn_local(async move {
            let r = api("/api/v1/onboarding/connections", "POST", Some(request)).await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match r {
                Ok(v) => {
                    if let Ok(c) = serde_json::from_value::<ConnectionInfo>(v) {
                        draft.update(|p| p.primary_connection = c.id);
                        form.set(false);
                        message.set("Connection saved. Authentication and model access have not been verified.".into());
                        reload.run(());
                        refresh(snap, error);
                    }
                }
                Err(e) => error.set(e),
            }
        });
    };
    let selected = move || {
        status.get().and_then(|s| {
            s.connections
                .into_iter()
                .find(|c| c.connection.id == draft.get().primary_connection)
        })
    };
    view! {<div class="onboard-scene"><p class="onboard-lede">"Use the tools you already trust."</p><p class="muted">"Keep an existing agent account, connect an API, or use a model on your computer. Local recording and basic action extraction work without a model."</p>
        <div class="onboard-model-options" role="group" aria-label="Primary model connection">{move||status.get().map(|s|s.connections.into_iter().map(|c|{let id=c.connection.id.clone();let check=id.clone();let pressed=id.clone();view!{<button type="button" class="onboard-model-option" class:chosen=move||draft.get().primary_connection==check aria-pressed=move||draft.get().primary_connection==pressed on:click=move|_|draft.update(|p|p.primary_connection=id.clone())><span>{c.connection.id}</span><small>{if c.connection.kind=="local_rules"{"Local rules · no account".into()}else{format!("{} · {}",c.connection.kind,if c.connection.available{"configured"}else{"needs attention"})}}</small></button>}}).collect_view())}</div>
        {move||selected().map(|c|{let local=c.connection.kind=="local_rules";let callable=c.check.as_ref().is_some_and(|r|r.status=="completed");let active=c.check.as_ref().is_some_and(|r|matches!(r.status.as_str(),"starting"|"running"|"cancelling"));let cancel_id=c.check.as_ref().filter(|_|active).map(|r|r.id.clone());let acp=c.connection.kind=="acp";let supports=acp||c.connection.kind=="openai_compatible";view!{<div class="onboard-connection-detail"><dl class="onboard-facts"><dt>"Model"</dt><dd>{if c.connection.model.is_empty(){"Selected by runtime · unknown until response".into()}else{c.connection.model.clone()}}</dd><dt>"Account / billing"</dt><dd>{c.connection.billing.clone()}</dd><dt>"Destination"</dt><dd>{c.destination}</dd><dt>"Credentials"</dt><dd>{c.credential_state}</dd><dt>"Model access"</dt><dd>{if local{"No model needed"}else if callable{"Responded to the synthetic check"}else{"Unverified"}}</dd></dl>
            {c.check.map(|r|view!{<p class="onboard-note" role="status">{format!("Last request: {} · {}",r.status,r.error.unwrap_or(r.progress))}</p>})}
            <Show when=move||!local&&supports><p class="muted">"Optional check: one short synthetic request, no work sources and no tools. It counts toward this connection's request allowance; API pricing still applies."</p><label class="onboard-checkbox"><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>"Use this account for one connection check"</label>
                <div class="workflow-actions">{acp.then(||view!{<button class="button secondary" type="button" disabled=move||busy.get()||active on:click=move|_|{let id=draft.get_untracked().primary_connection;busy.set(true);spawn_local(async move{let r=api(&format!("/api/v1/runtime/connections/{id}/probe"),"POST",Some(json!({}))).await;if busy.try_get_untracked().is_none(){return;}busy.set(false);match r{Ok(v)=>probe.set(format!("Protocol initialized: {} {}. Authentication and a model response remain unverified.",v["agent_name"].as_str().unwrap_or("agent"),v["agent_version"].as_str().unwrap_or("unknown version"))),Err(e)=>error.set(e)}});}>"Check protocol · no model request"</button>})}
                <button class="button secondary" type="button" disabled=move||busy.get()||active||!approved.get() on:click=move|_|{let connection=draft.get_untracked().primary_connection;busy.set(true);approved.set(false);spawn_local(async move{let r=api("/api/v1/onboarding/check","POST",Some(json!({"connection":connection,"approve_usage":true,"idempotency_key":new_id()}))).await;if busy.try_get_untracked().is_none(){return;}busy.set(false);match r{Ok(_)=>{message.set("Connection check started. Its actual result will appear here.".into());reload.run(());refresh(snap,error);},Err(e)=>error.set(e)}});}>"Send synthetic check"</button></div>
            </Show>
            {cancel_id.map(|id|view!{<button class="button text" type="button" disabled=move||busy.get() on:click=move|_|{let id=id.clone();busy.set(true);spawn_local(async move{let r=api(&format!("/api/v1/runtime/runs/{id}/cancel"),"POST",Some(json!({}))).await;if busy.try_get_untracked().is_none(){return;}busy.set(false);match r{Ok(_)=>{reload.run(());refresh(snap,error);},Err(e)=>error.set(e)}});}>"Cancel check"</button>})}
        </div>}})}
        <Show when=move||!probe.get().is_empty()><p class="onboard-note" role="status">{move||probe.get()}</p></Show><Show when=move||!message.get().is_empty()><p class="onboard-note" role="status">{move||message.get()}</p></Show>
        <div class="onboard-add-model"><span>"Add a connection"</span><div class="workflow-actions"><button class="button text" type="button" on:click=move|_|choose_form("agent")>"Existing agent"</button><button class="button text" type="button" on:click=move|_|choose_form("api")>"API account"</button><button class="button text" type="button" on:click=move|_|choose_form("local")>"Local model"</button></div></div>
        <Show when=move||form.get()><form class="onboard-form" on:submit=save><fieldset disabled=move||busy.get()>
            <label for="setup-connection-id">"Connection name"</label><input id="setup-connection-id" required maxlength="80" placeholder="my-main-model" prop:value=move||id.get() on:input=move|ev|id.set(event_target_value(&ev))/>
            <Show when=move||kind.get()!="openai_compatible"><label>"Installed agents (discovery does not run them)"</label><div class="workflow-actions">{move||status.get().map(|s|s.detected_agents.into_iter().map(|a|view!{<button type="button" class="button secondary" on:click=move|_|{kind.set(a.kind.clone());command.set(a.command.clone());message.set(a.detail.clone());}>{a.name}</button>}).collect_view())}</div><label>"Agent connection"</label><select prop:value=move||kind.get() on:change=move|ev|kind.set(event_target_value(&ev))><option value="acp">"ACP adapter · summaries and agent runs"</option><option value="codex_cli">"Codex CLI · document extraction"</option><option value="claude_cli">"Claude CLI · document extraction"</option></select><label>"Absolute executable path"</label><input required prop:value=move||command.get() on:input=move|ev|command.set(event_target_value(&ev))/><label>"Arguments (JSON array)"</label><input prop:value=move||args.get() on:input=move|ev|args.set(event_target_value(&ev))/><p class="muted">"The installed agent owns login and subscription access. Nothing is installed or authenticated automatically."</p></Show>
            <Show when=move||kind.get()=="openai_compatible"><label>"Compatible API base URL"</label><input type="url" required placeholder="https://your-provider.example/v1" prop:value=move||endpoint.get() on:input=move|ev|endpoint.set(event_target_value(&ev))/><label>"API key (optional for a local endpoint)"</label><input type="password" autocomplete="new-password" prop:value=move||key.get() on:input=move|ev|key.set(event_target_value(&ev))/><details><summary>"Use a credential reference instead"</summary><input aria-label="Model credential reference" placeholder="env:MODEL_API_KEY" prop:value=move||reference.get() on:input=move|ev|reference.set(event_target_value(&ev))/></details></Show>
            <label>"Model ID (required for APIs)"</label><input required=move||kind.get()=="openai_compatible" prop:value=move||model.get() on:input=move|ev|model.set(event_target_value(&ev))/><label>"Daily request limit"</label><input type="number" min="1" max="1000" required prop:value=move||limit.get() on:input=move|ev|limit.set(event_target_value(&ev))/>
            <p class="muted">"Saving configures this route without making a model request. Existing accounts stay unchanged; new connections are available immediately."</p><div class="workflow-actions"><button class="button secondary" type="submit">"Save connection"</button><button class="button text" type="button" on:click=move|_|{key.set(String::new());form.set(false);}>"Cancel"</button></div>
        </fieldset></form></Show>
    </div>}
}
