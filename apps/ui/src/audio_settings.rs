use super::workflows::{Pending, action};
use super::*;

/// Recording preferences are independent of the recording companion: opening
/// Settings should not mount sessions, notes or the companion's polling clock.
#[component]
pub(super) fn AudioSettings(
    snap: Snap,
    notice: RwSignal<String>,
    #[prop(optional)] after: Option<Callback<()>>,
) -> impl IntoView {
    let settings = RwSignal::new(LiveRecordingSettings {
        transcription: None,
        use_bundled_transcription: true,
        microphone_id: Some(DEFAULT_MEETING_MICROPHONE.into()),
        system_audio: false,
        whisper_command: String::new(),
        model_path: String::new(),
        chunk_seconds: 20,
    });
    let transcription = RwSignal::new(TranscriptionSelection {
        use_vad: false,
        ..Default::default()
    });
    let baseline = RwSignal::new(None::<Value>);
    let loaded = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let scanning = RwSignal::new(false);
    let inputs = RwSignal::new(None::<MeetingAudioInputs>);
    let pending = RwSignal::new(None::<Pending>);
    let draft = move || {
        let mut s = settings.get();
        let t = transcription.get();
        s.use_bundled_transcription = t.provider == TranscriptionProvider::Bundled;
        s.transcription = Some(t);
        serde_json::to_value(s).unwrap_or_default()
    };
    let dirty = move || loaded.get() && baseline.get().as_ref() != Some(&draft());
    let apply = move |s: LiveRecordingSettings| {
        transcription.set(s.transcription.clone().unwrap_or(TranscriptionSelection {
            provider: if s.use_bundled_transcription {
                TranscriptionProvider::Bundled
            } else {
                TranscriptionProvider::Custom
            },
            use_vad: false,
            ..Default::default()
        }));
        settings.set(s);
        baseline.set(Some(draft()));
    };
    let load = move || {
        error.set(String::new());
        spawn_local(async move {
            let result = api("/api/v1/live-meetings/settings", "GET", None)
                .await
                .and_then(|v| {
                    serde_json::from_value::<Option<LiveRecordingSettings>>(v)
                        .map_err(|_| "Could not read audio preferences".to_string())
                });
            if loaded.try_get_untracked().is_none() {
                return;
            }
            match result {
                Ok(s) => {
                    if let Some(s) = s {
                        apply(s);
                    }
                    loaded.set(true);
                }
                Err(e) => error.set(e),
            }
        });
    };
    let scan = move || {
        if scanning.get_untracked() {
            return;
        }
        scanning.set(true);
        spawn_local(async move {
            let result = api("/api/v1/live-meetings/inputs", "GET", None)
                .await
                .and_then(|v| {
                    serde_json::from_value::<MeetingAudioInputs>(v)
                        .map_err(|_| "Could not read audio inputs".to_string())
                });
            if scanning.try_get_untracked().is_none() {
                return;
            }
            scanning.set(false);
            match result {
                Ok(i) => inputs.set(Some(i)),
                Err(e) => notice.set(e),
            }
        });
    };
    load();
    scan();
    view! {<div class="settings-page audio-preferences">
        <header class="settings-intro"><h2>"Audio & transcription"</h2><p>"Choose what to capture. We’ll take care of the transcript."</p></header>
        <Show when=move||!error.get().is_empty()><div class="settings-alert" role="alert"><p>{move||error.get()}</p><button type="button" class="button secondary" on:click=move|_|load()>"Try again"</button></div></Show>
        <form class="settings-form" on:submit=move|ev|{
            ev.prevent_default();
            if !loaded.get_untracked() || !dirty() { return; }
            action("/api/v1/live-meetings/settings".into(),"PUT",json!({"settings":draft()}),snap,notice,busy,pending,move|v|{
                if let Ok(s)=serde_json::from_value::<LiveRecordingSettings>(v){apply(s);if let Some(after)=after{after.run(());}}
            });
        }>
        <fieldset class="settings-fields" disabled=move||busy.get()||!loaded.get()>
            <section class="settings-group" aria-labelledby="audio-inputs-title"><div class="settings-group-heading"><h3 id="audio-inputs-title">"Audio inputs"</h3><button type="button" class="button text" disabled=move||scanning.get() on:click=move|_|scan()>{move||if scanning.get(){"Refreshing…"}else{"Refresh devices"}}</button></div>
                <div class="settings-card">
                    <div class="setting-row"><div class="setting-copy"><label for="audio-microphone">"Microphone"</label><p id="microphone-help">"Your voice, from the input you prefer."</p></div><select id="audio-microphone" aria-describedby="microphone-help" prop:value=move||settings.get().microphone_id.unwrap_or_default() on:change=move|ev|{let id=event_target_value(&ev);settings.update(|s|s.microphone_id=if id.is_empty(){None}else{Some(id)});}>
                        <option value=DEFAULT_MEETING_MICROPHONE>"System default"</option><option value="">"Off"</option>
                        {move||{let i=inputs.get();let saved=settings.get().microphone_id;let missing=saved.filter(|id|id!=DEFAULT_MEETING_MICROPHONE&&i.as_ref().is_some_and(|i|!i.microphones.iter().any(|d|d.id==*id)));view!{
                            {missing.map(|id|view!{<option value=id>"Saved microphone · unavailable"</option>})}
                            {i.map(|i|i.microphones.into_iter().map(|d|view!{<option value=d.id>{d.name}</option>}).collect_view())}
                        }}}
                    </select></div>
                    <div class="setting-row"><div class="setting-copy"><label for="audio-system">"Include system audio"</label><p id="system-audio-help">"Hear the other side of the meeting. Excludes Thesaurus."</p></div><input class="setting-switch" type="checkbox" role="switch" id="audio-system" aria-describedby="system-audio-help" prop:checked=move||settings.get().system_audio disabled=move||inputs.get().is_some_and(|i|!i.system_audio_available)&&!settings.get().system_audio on:change=move|ev|settings.update(|s|s.system_audio=event_target_checked(&ev))/></div>
                </div>
                {move||inputs.get().filter(|i|!i.available).map(|i|view!{<p class="settings-alert" role="alert">{i.detail}</p>})}
            </section>
            <super::transcription_setup::TranscriptionSetup selection=transcription notice=notice/>
            <Show when=move||transcription.get().provider==TranscriptionProvider::Custom><section class="settings-group"><div class="settings-group-heading"><h3>"Custom local engine"</h3></div><div class="settings-card"><div class="setting-row"><label for="audio-engine">"Whisper executable"</label><input id="audio-engine" required placeholder="/absolute/path/to/whisper-cli" prop:value=move||settings.get().whisper_command on:input=move|ev|settings.update(|s|s.whisper_command=event_target_value(&ev))/></div><div class="setting-row"><label for="audio-model-path">"Model file"</label><input id="audio-model-path" required placeholder="/absolute/path/to/model.bin" prop:value=move||settings.get().model_path on:input=move|ev|settings.update(|s|s.model_path=event_target_value(&ev))/></div></div></section></Show>
            <details class="settings-disclosure"><summary>"Advanced recording settings"</summary><div class="settings-card">
                <div class="setting-row"><div class="setting-copy"><label for="audio-chunk">"Processing interval"</label><p id="audio-chunk-help">"How often audio is sent for transcription. 10–60 seconds."</p></div><div class="setting-unit"><input id="audio-chunk" aria-describedby="audio-chunk-help" type="number" required min="10" max="60" prop:value=move||settings.get().chunk_seconds on:input=move|ev|settings.update(|s|s.chunk_seconds=event_target_value(&ev).parse().unwrap_or(0))/><span>"sec"</span></div></div>
                <div class="setting-diagnostics">{move||inputs.get().map(|i|view!{<p>{format!("Microphone permission: {}",i.microphone_permission)}</p><p>{i.detail}</p>})}<p>"Permissions are requested when you start recording. Temporary local audio is deleted after processing."</p></div>
            </div></details>
        </fieldset>
        <footer class="settings-savebar"><div role="status" aria-live="polite"><span class="settings-save-state" class:settings-unsaved=move||dirty()>{move||if busy.get(){"Saving…"}else if !loaded.get(){"Loading preferences…"}else if dirty(){"Unsaved changes"}else{"✓ All changes saved"}}</span><p>"Applies to new recordings. Saving never starts a recording."</p></div><button class="button primary" type="submit" disabled=move||busy.get()||!dirty()||inputs.get().is_none_or(|i|!i.available)||(!settings.get().system_audio&&settings.get().microphone_id.is_none())>"Save changes"</button></footer>
        </form>
    </div>}
}
