use super::*;

pub(super) fn provider_name(p: TranscriptionProvider) -> &'static str {
    match p {
        TranscriptionProvider::Bundled => "Local · Base (bundled)",
        TranscriptionProvider::LocalNormal => "Local · Normal",
        TranscriptionProvider::LocalLarge => "Local · Large",
        TranscriptionProvider::Custom => "Custom local model",
        TranscriptionProvider::Deepgram => "Deepgram · Nova-3",
        TranscriptionProvider::Soniox => "Soniox · Async v5",
        TranscriptionProvider::Company => "Company endpoint",
    }
}
fn provider_value(p: TranscriptionProvider) -> String {
    serde_json::to_value(p)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}
#[component]
pub(super) fn TranscriptionSetup(
    selection: RwSignal<TranscriptionSelection>,
    notice: RwSignal<String>,
) -> impl IntoView {
    let catalog = RwSignal::new(None::<TranscriptionCatalog>);
    let busy = RwSignal::new(false);
    let secret = RwSignal::new(String::new());
    if let Some(guard) = use_context::<super::onboarding::SetupDraftGuard>() {
        Effect::new(move |_| {
            guard.0.update(|m| {
                m.insert("transcription", !secret.get().is_empty() || busy.get());
            })
        });
        on_cleanup(move || {
            guard.0.update(|m| {
                m.remove("transcription");
            })
        });
    }
    let fetch = move || {
        spawn_local(async move {
            let result = api("/api/v1/transcription", "GET", None).await;
            if catalog.try_get_untracked().is_none() {
                return;
            }
            match result {
                Ok(v) => {
                    if let Ok(c) = serde_json::from_value(v) {
                        catalog.set(Some(c));
                    }
                }
                Err(e) => notice.set(e),
            }
        })
    };
    fetch();
    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(2000).await;
            if catalog.try_get_untracked().is_none() {
                break;
            }
            // A sequential poll cannot overwrite newer progress with an older request.
            match api("/api/v1/transcription", "GET", None).await {
                Ok(v) => {
                    if catalog.try_get_untracked().is_some() {
                        if let Ok(c) = serde_json::from_value(v) {
                            catalog.set(Some(c));
                        }
                    }
                }
                Err(_) => {}
            }
        }
    });
    let model_action = move |id: String, action: &'static str| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        spawn_local(async move {
            let response = api(
                &format!("/api/v1/transcription/models/{id}"),
                "POST",
                Some(json!({"action":action})),
            )
            .await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match response {
                Ok(_) => {
                    notice.set(
                        if action == "download" {
                            "Download started; recording settings are unchanged"
                        } else {
                            "Cancelling download; partial data will be kept for resume"
                        }
                        .into(),
                    );
                    fetch();
                }
                Err(e) => notice.set(e),
            }
        });
    };
    view! {<div class="transcription-setup">
        <section class="settings-group" aria-labelledby="transcript-title"><div class="settings-group-heading"><h3 id="transcript-title">"Transcript"</h3><span class="settings-badge">{move||if selection.get().provider.is_remote(){"Cloud"}else{"On device"}}</span></div>
        <div class="settings-card">
        <div class="setting-row"><div class="setting-copy"><label for="transcription-provider">"Transcription model"</label><p id="transcription-provider-help">{move||match selection.get().provider {
            TranscriptionProvider::Bundled=>"Included with the app. Works offline.",
            TranscriptionProvider::LocalNormal=>"A larger local model. Download once, use offline.",
            TranscriptionProvider::LocalLarge=>"Our largest local model. Needs more memory and processing time.",
            TranscriptionProvider::Custom=>"Use your own local executable and model file.",
            _=>"Transcribe with your connected provider account.",
        }}</p></div>
        <select id="transcription-provider" aria-label="Transcription provider" aria-describedby="transcription-provider-help" prop:value=move||provider_value(selection.get().provider) on:change=move|ev|{
            let p=serde_json::from_value::<TranscriptionProvider>(json!(event_target_value(&ev))).unwrap_or_default();
            selection.update(|s|{s.provider=p;s.approve_audio_upload=false;s.endpoint=String::new();s.model=String::new();s.use_vad=matches!(p,TranscriptionProvider::LocalNormal|TranscriptionProvider::LocalLarge);s.credential_ref=match p{TranscriptionProvider::Deepgram=>"keychain:transcription.deepgram",TranscriptionProvider::Soniox=>"keychain:transcription.soniox",TranscriptionProvider::Company=>"keychain:transcription.company",_=>""}.into();});secret.set(String::new());
        }>{[TranscriptionProvider::Bundled,TranscriptionProvider::LocalNormal,TranscriptionProvider::LocalLarge,TranscriptionProvider::Deepgram,TranscriptionProvider::Soniox,TranscriptionProvider::Company,TranscriptionProvider::Custom].into_iter().map(|p|view!{<option value=provider_value(p)>{provider_name(p)}</option>}).collect_view()}</select></div>
        <div class="setting-row"><div class="setting-copy"><label for="transcription-language">"Meeting language"</label><p id="transcription-language-help">{move||match selection.get().language.as_str(){"auto"=>"Detect from the first speech for local transcription.","multi"=>"Detect the language in each audio segment.",_=>"Use this language for new recordings."}}</p></div><select id="transcription-language" aria-label="Transcription language" aria-describedby="transcription-language-help" prop:value=move||selection.get().language on:change=move|ev|selection.update(|s|s.language=event_target_value(&ev))>
            <option value="auto">"Automatic"</option><option value="multi">"Multilingual"</option><option value="en">"English"</option><option value="es">"Spanish"</option><option value="fr">"French"</option><option value="de">"German"</option><option value="pt">"Portuguese"</option><option value="ja">"Japanese"</option><option value="zh">"Chinese"</option><option value="ko">"Korean"</option><option value="it">"Italian"</option><option value="hi">"Hindi"</option>
        </select></div></div></section>
        <Show when=move||!selection.get().provider.is_remote()>
            {move||catalog.get().filter(|c|!c.engine_available).map(|_|view!{<p class="settings-alert" role="alert">"The local transcription engine is missing. Install the complete desktop app or configure a custom engine."</p>})}
            <details class="settings-disclosure" open=move||matches!(selection.get().provider,TranscriptionProvider::LocalNormal|TranscriptionProvider::LocalLarge)><summary>"Local models & speech detection"</summary>
            <div class="settings-card"><div class="setting-row"><div class="setting-copy"><label for="transcription-vad">"Speech detection"</label><p id="transcription-vad-help">"Skip silence. Requires the small Silero download below."</p></div><input id="transcription-vad" aria-describedby="transcription-vad-help" class="setting-switch" type="checkbox" role="switch" prop:checked=move||selection.get().use_vad on:change=move|ev|selection.update(|s|s.use_vad=event_target_checked(&ev))/></div>
            <div class="setting-models">{move||catalog.get().map(|c|c.models.into_iter().map(|m|{
                let download=m.id.clone();let cancel=m.id.clone();let active=matches!(m.state.as_str(),"downloading"|"cancelling");let ready=m.state=="ready";
                view!{<article class="setting-model"><div class="setting-model-heading"><strong>{m.name}</strong><span class="settings-badge" class:model-ready=ready>{m.state.clone()}</span></div><p class="muted">{m.detail}</p>
                    {(!ready).then(||view!{<p class="muted" role="status">{format!("{:.1} / {:.1} MiB",m.downloaded_bytes as f64/1048576.,m.bytes as f64/1048576.)}</p>})}
                    {active.then(||view!{<progress aria-label="Model download progress" max=m.bytes as f64 value=m.downloaded_bytes as f64></progress>})}
                    <div class="workflow-actions">{(!ready).then(||view!{<button class="button secondary" type="button" disabled=move||busy.get()||active on:click=move|_|model_action(download.clone(),"download")>{if m.downloaded_bytes>0{"Resume download"}else{"Download"}}</button>})}{active.then(||view!{<button class="button text" type="button" disabled=move||busy.get() on:click=move|_|model_action(cancel.clone(),"cancel")>"Cancel"</button>})}</div></article>}
            }).collect_view())}</div><div class="setting-diagnostics"><p>"Downloads are checksum verified and stay available across app updates. Sizes shown are downloads, not memory requirements."</p>{move||catalog.get().map(|c|view!{<p>{c.hardware}</p>})}</div></div></details>
        </Show>
        <Show when=move||selection.get().provider.is_remote()>
            <section class="settings-group"><div class="settings-group-heading"><h3>"Cloud transcription"</h3><span class="settings-badge">"Audio leaves this device"</span></div><div class="settings-card cloud-transcription"><p class="muted">"The selected microphone and system-audio tracks are uploaded separately. Transcription appears after each completed chunk. Your summary model and its account are configured separately."</p>
            <Show when=move||selection.get().provider==TranscriptionProvider::Deepgram><p class="muted">"Destination: api.deepgram.com · default global endpoint · Nova-3 · model improvement opt-out requested. Review your account's processing location, pricing and retention terms." <a href="https://deepgram.com/pricing" target="_blank" rel="noreferrer">"Provider pricing ↗"</a></p></Show>
            <Show when=move||selection.get().provider==TranscriptionProvider::Soniox><p class="muted">"Destination: api.soniox.com · United States endpoint · Async v5. Uploaded files and jobs are deleted after processing; failed deletion is shown in delivery receipts. Review your account's retention terms." <a href="https://soniox.com/pricing" target="_blank" rel="noreferrer">"Provider pricing ↗"</a></p></Show>
            <Show when=move||selection.get().provider==TranscriptionProvider::Company><label for="transcription-endpoint">"Company listen URL (Deepgram-compatible)"</label><input id="transcription-endpoint" type="url" required prop:value=move||selection.get().endpoint on:input=move|ev|selection.update(|s|{s.endpoint=event_target_value(&ev);s.approve_audio_upload=false;})/><label for="transcription-company-model">"Company model"</label><input id="transcription-company-model" required maxlength="100" prop:value=move||selection.get().model on:input=move|ev|selection.update(|s|{s.model=event_target_value(&ev);s.approve_audio_upload=false;})/><p class="muted">"Confirm your organization's processing region, retention and pricing before authorizing this endpoint."</p></Show>
            <label for="transcription-api-key">"Provider API key"</label><input id="transcription-api-key" type="password" autocomplete="new-password" prop:value=move||secret.get() on:input=move|ev|secret.set(event_target_value(&ev))/>
            <button class="button secondary" type="button" disabled=move||busy.get()||secret.get().is_empty() on:click=move|_|{
                let provider=selection.get_untracked().provider;let endpoint=selection.get_untracked().endpoint;let reference_before=selection.get_untracked().credential_ref;let value=secret.get_untracked();secret.set(String::new());busy.set(true);
                spawn_local(async move {let result=api("/api/v1/transcription/credential","POST",Some(json!({"provider":provider,"endpoint":endpoint,"secret":value}))).await;
                    if busy.try_get_untracked().is_none(){return;}busy.set(false);
                    match result{Ok(v)=>{if selection.get_untracked().provider==provider&&selection.get_untracked().endpoint==endpoint&&selection.get_untracked().credential_ref==reference_before{if let Some(r)=v["credential_ref"].as_str(){selection.update(|s|s.credential_ref=r.into());}}notice.set("API key saved in the host's Keychain; no audio was sent".into());},Err(e)=>notice.set(e)}
                });
            }>"Save API key to Keychain"</button>
            <details><summary>"Credential reference (advanced)"</summary><input aria-label="Transcription credential reference" prop:value=move||selection.get().credential_ref on:input=move|ev|selection.update(|s|{s.credential_ref=event_target_value(&ev);s.approve_audio_upload=false;})/><p class="muted">"Use keychain:NAME or env:NAME from the service's environment. Credentials are resolved on the host."</p></details>
            <label for="transcription-audio-limit">"Daily audio-minute limit (UTC · all cloud routes and tracks)"</label><input id="transcription-audio-limit" type="number" min="1" max="2880" prop:value=move||selection.get().daily_audio_minutes on:input=move|ev|selection.update(|s|{s.daily_audio_minutes=event_target_value(&ev).parse().unwrap_or(0);s.approve_audio_upload=false;})/>
            {move||catalog.get().map(|c|view!{<p class="muted">{format!("{:.1} audio minutes reserved today. Two tracks can use two audio minutes per meeting minute. Failed or uncertain deliveries count; this is a usage cap, not a currency guarantee.",c.remote_audio_seconds_today as f64/60.)}</p>})}
            <label class="settings-consent"><input type="checkbox" prop:checked=move||selection.get().approve_audio_upload on:change=move|ev|selection.update(|s|s.approve_audio_upload=event_target_checked(&ev))/>"Allow future recordings to send audio to this selected destination within this limit. I have reviewed its pricing and data policy."</label>
            <button class="button text" type="button" disabled=move||busy.get() on:click=move|_|{
                busy.set(true);spawn_local(async move{let result=api("/api/v1/transcription/revoke","POST",Some(json!({}))).await;
                    if busy.try_get_untracked().is_none(){return;}busy.set(false);match result{Ok(_)=>{selection.update(|s|s.approve_audio_upload=false);notice.set("Audio upload permission revoked. In-flight requests may finish; queued audio will not be uploaded.".into());},Err(e)=>notice.set(e)}
                });
            }>"Revoke audio uploads now"</button>
        </div></section></Show>
    </div>}
}

#[component]
pub(super) fn TranscriptionReceipts(session_id: String, notice: RwSignal<String>) -> impl IntoView {
    let rows = RwSignal::new(Vec::<TranscriptionReceipt>::new());
    let id = StoredValue::new(session_id);
    let busy = RwSignal::new(false);
    let refresh = move || {
        spawn_local(async move {
            let result = api(
                &format!(
                    "/api/v1/live-meetings/{}/transcription-receipts",
                    id.get_value()
                ),
                "GET",
                None,
            )
            .await;
            if rows.try_get_untracked().is_none() {
                return;
            }
            match result {
                Ok(v) => {
                    if let Ok(r) = serde_json::from_value(v) {
                        rows.set(r)
                    }
                }
                Err(e) => notice.set(e),
            }
        })
    };
    view! {<details on:toggle=move|_|refresh()><summary>"Transcription delivery receipts"</summary><button type="button" class="button text" on:click=move|_|refresh()>"Refresh deliveries"</button>
        {move||rows.get().into_iter().map(|r|{let chunk=r.chunk_id.clone();view!{<article class="focus-change"><strong>{format!("{} · {} · {}s · {}",provider_name(r.provider),r.track,r.audio_seconds,r.status)}</strong><p class="muted">{format!("{} · {} · {}",r.created_at,r.model,r.destination)}</p><p class="muted">{format!("Request: {} · File: {}",r.provider_request_id.as_deref().unwrap_or("unknown / none"),r.provider_file_id.as_deref().unwrap_or("none"))}</p>{r.error.map(|e|view!{<p class="runtime-error">{e}</p>})}
            {r.cleanup_pending.then(||view!{<p class="runtime-error">"Provider cleanup is pending. Audio or job results may still be retained."</p><button type="button" class="button secondary" disabled=move||busy.get() on:click=move|_|{let chunk=chunk.clone();busy.set(true);spawn_local(async move{let result=api(&format!("/api/v1/live-meetings/{}/transcription-receipts/{chunk}/cleanup",id.get_value()),"POST",Some(json!({}))).await;if busy.try_get_untracked().is_none(){return;}busy.set(false);match result{Ok(_)=>refresh(),Err(e)=>notice.set(e)}});}>"Delete provider file and job"</button>})}
        </article>}}).collect_view()}
    </details>}
}
