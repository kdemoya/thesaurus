use super::*;
#[component]
pub(super) fn AudioSetup(
    error: RwSignal<String>,
    reload: Callback<()>,
    snap: Snap,
    notice: RwSignal<String>,
) -> impl IntoView {
    let inputs = RwSignal::new(None::<MeetingAudioInputs>);
    let ready = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let mic = RwSignal::new(DEFAULT_MEETING_MICROPHONE.to_string());
    let system = RwSignal::new(false);
    let transcription = RwSignal::new(TranscriptionSelection {
        use_vad: false,
        ..Default::default()
    });
    let whisper = RwSignal::new(String::new());
    let model = RwSignal::new(String::new());
    let seconds = RwSignal::new(20u32);
    let message = RwSignal::new(String::new());
    let baseline = RwSignal::new(None::<LiveRecordingSettings>);
    let original_mic = RwSignal::new(String::new());
    let original_system = RwSignal::new(false);
    let original_t = RwSignal::new(None::<TranscriptionSelection>);
    let guard = use_context::<SetupDraftGuard>();
    if let Some(guard) = guard {
        Effect::new(move |_| {
            guard.0.update(|m| {
                m.insert(
                    "audio",
                    ready.get()
                        && (mic.get() != original_mic.get()
                            || system.get() != original_system.get()
                            || Some(transcription.get()) != original_t.get()
                            || baseline.get().is_some_and(|b| {
                                b.whisper_command != whisper.get() || b.model_path != model.get()
                            })),
                );
            });
        });
        on_cleanup(move || {
            guard.0.update(|m| {
                m.remove("audio");
            })
        });
    }
    spawn_local(async move {
        let settings = api("/api/v1/live-meetings/settings", "GET", None).await;
        let devices = api("/api/v1/live-meetings/inputs", "GET", None).await;
        if ready.try_get_untracked().is_none() {
            return;
        }
        match settings {
            Ok(v) => {
                if let Ok(Some(s)) = serde_json::from_value::<Option<LiveRecordingSettings>>(v) {
                    baseline.set(Some(s.clone()));
                    mic.set(s.microphone_id.unwrap_or_default());
                    system.set(s.system_audio);
                    transcription.set(s.transcription.unwrap_or(TranscriptionSelection {
                        provider: if s.use_bundled_transcription {
                            TranscriptionProvider::Bundled
                        } else {
                            TranscriptionProvider::Custom
                        },
                        use_vad: false,
                        ..Default::default()
                    }));
                    whisper.set(s.whisper_command);
                    model.set(s.model_path);
                    seconds.set(s.chunk_seconds);
                    original_mic.set(mic.get_untracked());
                    original_system.set(system.get_untracked());
                    original_t.set(Some(transcription.get_untracked()));
                }
            }
            Err(e) => {
                error.set(e);
                return;
            }
        }
        match devices {
            Ok(v) => {
                if let Ok(d) = serde_json::from_value(v) {
                    inputs.set(Some(d));
                }
            }
            Err(e) => error.set(e),
        }
        ready.set(true);
    });
    view! {<div class="onboard-audio"><p class="onboard-lede">"Choose what you want to hear."</p><p class="muted">"Your system microphone and bundled local transcription are prepared by default. These settings belong to the connected computer."</p>
        <form class="onboard-form" on:submit=move|ev|{ev.prevent_default();if busy.get_untracked(){return;}busy.set(true);let m=mic.get_untracked();let t=transcription.get_untracked();let request=json!({"settings":{"microphone_id":if m.is_empty(){None}else{Some(m)},"system_audio":system.get_untracked(),"use_bundled_transcription":t.provider==TranscriptionProvider::Bundled,"transcription":t,"whisper_command":whisper.get_untracked(),"model_path":model.get_untracked(),"chunk_seconds":seconds.get_untracked()},"idempotency_key":new_id()});spawn_local(async move{let r=api("/api/v1/live-meetings/settings","PUT",Some(request)).await;if busy.try_get_untracked().is_none(){return;}busy.set(false);match r{Ok(v)=>{if let Ok(s)=serde_json::from_value::<LiveRecordingSettings>(v){baseline.set(Some(s.clone()));whisper.set(s.whisper_command);model.set(s.model_path);}original_mic.set(mic.get_untracked());original_system.set(system.get_untracked());original_t.set(Some(transcription.get_untracked()));message.set("Audio configuration saved. Recording has not started.".into());reload.run(());refresh(snap,notice);},Err(e)=>error.set(e)}});}><fieldset disabled=move||busy.get()||!ready.get()>
            <label>"Microphone"</label><select prop:value=move||mic.get() on:change=move|ev|mic.set(event_target_value(&ev))><option value=DEFAULT_MEETING_MICROPHONE>"System default microphone"</option><option value="">"Off"</option>{move||inputs.get().map(|i|i.microphones.into_iter().map(|m|view!{<option value=m.id>{m.name}</option>}).collect_view())}</select>
            <label class="onboard-checkbox"><input type="checkbox" prop:checked=move||system.get() on:change=move|ev|system.set(event_target_checked(&ev))/>"Include system audio · all apps except Thesaurus"</label>
            {move||inputs.get().map(|i|view!{<p class="onboard-note">{format!("Microphone permission: {}. {}",i.microphone_permission,i.detail)}</p>})}
            <super::super::transcription_setup::TranscriptionSetup selection=transcription notice=notice/>
            <Show when=move||transcription.get().provider==TranscriptionProvider::Custom><label>"Absolute whisper executable"</label><input required prop:value=move||whisper.get() on:input=move|ev|whisper.set(event_target_value(&ev))/><label>"Absolute model path"</label><input required prop:value=move||model.get() on:input=move|ev|model.set(event_target_value(&ev))/></Show>
            <p class="muted">"OS permissions are requested when you explicitly Start recording. Selecting inputs or saving setup does not open them."</p><button type="submit" class="button secondary">"Save audio choices"</button><button type="button" class="button text" on:click=move|_|{if let Some(s)=baseline.get_untracked(){mic.set(original_mic.get_untracked());system.set(original_system.get_untracked());if let Some(t)=original_t.get_untracked(){transcription.set(t);}whisper.set(s.whisper_command);model.set(s.model_path);}}>"Discard audio edits"</button>
        </fieldset></form><Show when=move||!message.get().is_empty()><p role="status" class="onboard-note">{move||message.get()}</p></Show>
    </div>}
}
