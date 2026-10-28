use super::workflows::{Citations, Pending, action};
use super::*;

#[wasm_bindgen(inline_js = "
export function companionView(){return new URLSearchParams(location.search).get('companion')==='1';}
export function requestedMeetingSetup(){return new URLSearchParams(location.search).get('setup')==='1';}
export function listenMeetingSetup(callback){if(window.__TAURI__)return window.__TAURI__.event.listen('meeting-setup',callback);}
export async function meetingWindow(mode){if(window.__TAURI__)return window.__TAURI__.core.invoke('meeting_window',{mode});}
")]
extern "C" {
    #[wasm_bindgen(js_name=companionView)]
    pub fn is_companion() -> bool;
    #[wasm_bindgen(js_name=requestedMeetingSetup)]
    fn requested_setup() -> bool;
    #[wasm_bindgen(js_name=listenMeetingSetup)]
    fn listen_setup(callback: &js_sys::Function);
    #[wasm_bindgen(catch,js_name=meetingWindow)]
    async fn meeting_window(mode: &str) -> Result<JsValue, JsValue>;
}
pub(super) fn clock(ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        ms / 3_600_000,
        (ms / 60_000) % 60,
        (ms / 1000) % 60
    )
}
pub(super) fn elapsed(s: &LiveMeeting, now: f64) -> u64 {
    s.recorded_ms
        + s.active_since
            .as_ref()
            .map(|v| (now - js_sys::Date::parse(v)).max(0.) as u64)
            .unwrap_or(0)
}
pub(super) fn window_action(mode: &'static str, notice: RwSignal<String>) {
    spawn_local(async move {
        if let Err(e) = meeting_window(mode).await {
            if notice.try_get_untracked().is_some() {
                notice.set(
                    e.as_string()
                        .unwrap_or_else(|| "Unable to open meeting window".into()),
                );
            }
        }
    });
}
fn chosen(rows: RwSignal<Vec<LiveMeeting>>, id: RwSignal<String>) -> Option<LiveMeeting> {
    let rows = rows.get();
    rows.iter()
        .find(|r| r.id == id.get())
        .or_else(|| {
            rows.iter()
                .find(|r| matches!(r.state.as_str(), "starting" | "recording"))
        })
        .or_else(|| rows.first())
        .cloned()
}

#[component]
pub(super) fn LiveMeetings(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    let microphone = RwSignal::new(DEFAULT_MEETING_MICROPHONE.to_string());
    let transcription = RwSignal::new(TranscriptionSelection {
        use_vad: false,
        ..Default::default()
    });
    let system = RwSignal::new(false);
    let settings_loaded = RwSignal::new(false);
    let rows = RwSignal::new(Vec::<LiveMeeting>::new());
    let current = RwSignal::new(String::new());
    let details = RwSignal::new(None::<LiveMeetingDetails>);
    let note = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let epoch = RwSignal::new(0u64);
    let detail_epoch = RwSignal::new(0u64);
    let now = RwSignal::new(js_sys::Date::now());
    let configure = RwSignal::new(requested_setup());
    let collapsed = RwSignal::new(false);
    let setup_callback = Closure::<dyn Fn()>::new(move || {
        if configure.try_get_untracked().is_some() {
            configure.set(true);
            collapsed.set(false);
        }
    });
    listen_setup(setup_callback.as_ref().unchecked_ref());
    setup_callback.forget();
    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(1000).await;
            if now.try_get_untracked().is_none() {
                break;
            }
            now.set(js_sys::Date::now());
        }
    });
    let reload = move || {
        epoch.update(|n| *n += 1);
        let ticket = epoch.get_untracked();
        spawn_local(async move {
            let result = api("/api/v1/live-meetings", "GET", None).await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match result {
                Ok(v) => {
                    if let Ok(s) = serde_json::from_value(v) {
                        rows.set(s)
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    };
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        reload();
    });
    Effect::new(move |_| {
        let session = chosen(rows, current);
        let _ = snap.get().map(|s| s.watermark);
        detail_epoch.update(|n| *n += 1);
        let ticket = detail_epoch.get_untracked();
        let Some(session) = session else {
            details.set(None);
            return;
        };
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/live-meetings/{}", session.id),
                "GET",
                None,
            )
            .await;
            if detail_epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match result {
                Ok(v) => {
                    if let Ok(d) = serde_json::from_value(v) {
                        details.set(Some(d));
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    });
    spawn_local(async move {
        let result = api("/api/v1/live-meetings/settings", "GET", None).await;
        if settings_loaded.try_get_untracked().is_none() {
            return;
        }
        if let Ok(value) = result {
            if let Ok(Some(settings)) =
                serde_json::from_value::<Option<LiveRecordingSettings>>(value)
            {
                transcription.set(settings.transcription.unwrap_or(TranscriptionSelection {
                    provider: if settings.use_bundled_transcription {
                        TranscriptionProvider::Bundled
                    } else {
                        TranscriptionProvider::Custom
                    },
                    use_vad: false,
                    ..Default::default()
                }));
                microphone.set(settings.microphone_id.unwrap_or_default());
                system.set(settings.system_audio);
            }
        }
        settings_loaded.set(true);
    });
    let start_recording = move || {
        action(
            "/api/v1/live-meetings/quick-start".into(),
            "POST",
            json!({"approve_recording":true}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if let Ok(s) = serde_json::from_value::<LiveMeeting>(v) {
                    current.set(s.id.clone());
                    rows.update(|r| {
                        r.retain(|old| old.id != s.id);
                        r.insert(0, s);
                    });
                }
            },
        )
    };
    let add_note = move |kind: &'static str| {
        let Some(s) = chosen(rows, current) else {
            return;
        };
        let text = if kind == "bookmark" && note.get_untracked().trim().is_empty() {
            "Bookmark".into()
        } else {
            note.get_untracked()
        };
        action(
            format!("/api/v1/live-meetings/{}/notes", s.id),
            "POST",
            json!({"kind":kind,"text":text}),
            snap,
            notice,
            busy,
            pending,
            move |_| {
                note.set(String::new());
                reload();
            },
        );
    };
    view! {<section class="panel live-meeting" class:meeting-collapsed=move||collapsed.get() on:keydown=move|ev:web_sys::KeyboardEvent|{if ev.alt_key()&&ev.key()=="b"{ev.prevent_default();add_note("bookmark");}}>
      <div class="section-heading"><h2>"Recording companion"</h2><div class="workflow-actions">{is_native().then(||view!{<button class="source-link" on:click=move|_|window_action("panel",notice)>"Pop out ↗"</button><button class="source-link" on:click=move|_|window_action("workspace",notice)>"Workspace ↗"</button>})}<button class="button primary" disabled=move||busy.get()||!settings_loaded.get()||rows.get().iter().any(|s|matches!(s.state.as_str(),"starting"|"recording"|"pausing"|"stopping")) on:click=move|_|start_recording()>{move||if busy.get(){"Working…"}else{"● Start recording"}}</button><button class="source-link" on:click=move|_|super::workspace::open_settings()>"Settings"</button></div></div>
      <Show when=move||chosen(rows,current).is_none()><p class="muted">{move||if settings_loaded.get(){format!("Microphone: {} · System audio: {} · {}",if microphone.get()==DEFAULT_MEETING_MICROPHONE{"system default"}else if microphone.get().is_empty(){"off"}else{"saved input"},if system.get(){"all apps except Thesaurus"}else{"off"},super::transcription_setup::provider_name(transcription.get().provider))}else{"Loading recording defaults…".into()}}</p></Show>
      <label>"Session"<select aria-label="Meeting session" prop:value=move||chosen(rows,current).map(|s|s.id).unwrap_or_default() on:change=move|ev|{current.set(event_target_value(&ev));details.set(None);}>{move||rows.get().into_iter().map(|s|{let id=s.id.clone();let check=id.clone();view!{<option value=id selected=move||chosen(rows,current).is_some_and(|s|s.id==check)>{format!("{} · {}",s.title,s.state)}</option>}}).collect_view()}</select></label>
      {move||chosen(rows,current).map(|s|{let id=s.id.clone();let elapsed_session=s.clone();let controls=s.clone();view!{
        <div class="meeting-status" role="status"><strong class="orange">{format!("{} {}",if matches!(s.state.as_str(),"recording"|"pausing"|"stopping"){"●"}else{"○"},s.state.to_uppercase())}</strong><span>{move||clock(elapsed(&elapsed_session,now.get()))}</span></div>
        <p class="muted">{format!("{} · {} completed chunks · {} queued/processing",s.title,s.chunks_completed,s.pending_chunks)}</p><p class="muted">{s.transcription.as_ref().map(|t|format!("{} · language: {}{}",super::transcription_setup::provider_name(t.provider),t.language,s.detected_language.as_ref().map(|l|format!(" ({l} detected)")).unwrap_or_default())).unwrap_or_else(||"Legacy local transcription".into())}</p>
        <p class="muted">{format!("Microphone: {} · System audio: {} · summaries use your saved preferences",s.microphone_id.as_deref().map(|id|if id==DEFAULT_MEETING_MICROPHONE{"system default"}else{id}).unwrap_or("off"),if s.system_audio{"all apps except Thesaurus"}else{"off"})}</p>
        {s.error.map(|e|view!{<p class="runtime-error" role="alert">{e}</p>})}
        <div class="workflow-actions meeting-controls">{["pause","resume","stop"].into_iter().map(move|a|{let s=controls.clone();let enabled=match a{"pause"=>matches!(s.state.as_str(),"starting"|"recording"),"resume"=>s.state=="paused",_=>s.state!="stopped"};view!{<button class="button secondary" disabled=move||busy.get()||!enabled on:click=move|_|action(format!("/api/v1/live-meetings/{}/control",s.id),"POST",json!({"expected_version":s.version,"action":a,"approve_recording":a=="resume"}),snap,notice,busy,pending,move|_|reload())>{match a{"pause"=>"Pause","resume"=>"Resume selected inputs",_=>"Stop"}}</button>}}).collect_view()}</div>
        <p class="muted">"Pause/Stop closes the audio inputs and finishes the final chunk. Queued chunks use this session's selected transcription provider; a forced stop or crash can leave a gap. Restart never resumes recording."</p>

        <super::transcription_setup::TranscriptionReceipts session_id=s.id.clone() notice=notice/><details open={s.state!="stopped"}><summary>"Transcript"</summary><pre class="meeting-transcript">{s.transcript}</pre><p class="muted">{format!("{}s chunks · microphone and system tracks are labeled separately, not speaker identities",s.chunk_seconds)}</p></details>
        {move||details.get().filter(|d|d.session.id==id).map(|d|view!{<div class="meeting-notes">{d.notes.into_iter().map(|n|view!{<p><span class="muted">{format!("{} · {} ",clock(n.offset_ms),n.kind)}</span>{n.text}</p>}).collect_view()}</div>})}
      }})}
      <Show when=move||chosen(rows,current).is_some()>        <form class="workflow-form" on:submit=move|ev|{ev.prevent_default();add_note("note");}><label>"Quick note / bookmark"</label><input maxlength="4000" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))/><div class="workflow-actions"><button class="button secondary" type="submit" disabled=move||busy.get()||note.get().trim().is_empty()>"Save note"</button><button class="button text" type="button" disabled=move||busy.get() on:click=move|_|add_note("bookmark")>"Bookmark · Alt+B"</button></div></form></Show>
      <super::meeting_summaries::MeetingSummaries session=Signal::derive(move||chosen(rows,current)) snap=snap notice=notice selected=selected/>
      {move||chosen(rows,current).map(|s|view!{<RecordingDetails session=s snap=snap notice=notice after=Callback::new(move |_|reload())/>})}
      <Show when=move||configure.get()><super::audio_settings::AudioSettings snap=snap notice=notice after=Callback::new(move |_|configure.set(false))/></Show>
      {move||chosen(rows,current).and_then(|session|snap.get().and_then(|s|s.briefs.into_iter().find(|p|p.id==session.project_id))).map(|p|{
          let scope=p.source_ids.clone();let tasks=snap.get().map(|s|{let ids=s.documents.into_iter().filter(|d|scope.contains(&d.source_id)).map(|d|d.id).collect::<Vec<_>>();s.tasks.into_iter().filter(|t|ids.contains(&t.evidence.document_id)&&matches!(t.status.as_str(),"candidate"|"accepted")).take(8).collect::<Vec<_>>()}).unwrap_or_default();
          let revision=p.accepted_revision.and_then(|r|p.revisions.into_iter().find(|v|v.revision==r));
          view!{<details class="meeting-context"><summary>{format!("Context · {}",p.name)}</summary>{revision.map(|r|view!{<p class="muted">{if r.stale{"Saved brief is stale; verify cited evidence"}else{"Accepted project brief"}}</p>{r.sections.into_iter().take(6).map(|s|view!{<h3>{s.kind}</h3>{s.items.into_iter().map(|item|view!{<p>{item.text}</p><Citations evidence=item.evidence selected=selected notice=notice/>}).collect_view()}}).collect_view()}})}<h3>"Open commitments / suggestions"</h3>{tasks.into_iter().map(|t|{let id=t.id.clone();let version=t.version;let candidate=t.status=="candidate";view!{<article class="focus-change"><span class="muted">{t.status}</span><p>{t.title}</p><Citations evidence=vec![t.evidence] selected=selected notice=notice/>{candidate.then(||view!{<button class="button secondary" disabled=move||busy.get() on:click=move|_|action(format!("/api/v1/tasks/{id}"),"PATCH",json!({"expected_version":version,"status":"accepted"}),snap,notice,busy,pending,move|_|{})>"Review: accept action"</button>})}</article>}}).collect_view()}</details>}
      })}
      {is_native().then(||view!{<div class="meeting-dock workflow-actions">{is_companion().then(||view!{<button class="button text" on:click=move|_|{collapsed.update(|c|*c=!*c);window_action(if collapsed.get_untracked(){"collapse"}else{"expand"},notice);}>{move||if collapsed.get(){"Expand"}else{"Compact"}}</button>})}<span class="muted">"Companion:"</span>{[("panel","Panel"),("left","Dock left"),("right","Dock right")].into_iter().map(|(mode,label)|view!{<button class="button text" on:click=move|_|window_action(mode,notice)>{label}</button>}).collect_view()}</div>})}
    </section>}
}
#[component]
fn RecordingDetails(
    session: LiveMeeting,
    snap: Snap,
    notice: RwSignal<String>,
    after: Callback<()>,
) -> impl IntoView {
    let ready = session.state == "stopped" && session.pending_chunks == 0 && !session.processing;
    let unassigned = session.project_id.is_empty();
    let title = RwSignal::new(session.title.clone());
    let project = RwSignal::new(session.project_id.clone());
    let series = RwSignal::new(session.series_id.clone().unwrap_or_default());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let saved = RwSignal::new(false);
    view! {<details class="meeting-details" open=ready&&unassigned><summary>"Meeting details"</summary>
      <p class="muted">{if ready{"Add a title, project or meeting series to organize this recording."}else{"Add meeting details after stopping and finishing transcription."}}</p>
      <form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let series_id=series.get_untracked();action(format!("/api/v1/live-meetings/{}",session.id),"PATCH",json!({"expected_version":session.version,"title":title.get_untracked(),"project_id":project.get_untracked(),"series_id":if series_id.is_empty(){None}else{Some(series_id)}}),snap,notice,busy,pending,move|_|{saved.set(true);after.run(());});}>
        <fieldset class="focus-form-fields" disabled=move||busy.get()||!ready>
          <label>"Title"</label><input required maxlength="200" prop:value=move||title.get() on:input=move|ev|{title.set(event_target_value(&ev));saved.set(false);}/>
          <label>"Project (optional)"</label><select disabled=!unassigned prop:value=move||project.get() on:change=move|ev|{project.set(event_target_value(&ev));series.set(String::new());saved.set(false);}>
            <option value="">"Unassigned"</option>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|{let id=p.id.clone();view!{<option value=p.id selected=move||project.get()==id>{p.name}</option>}}).collect_view())}
          </select>
          <label>"Meeting series (optional)"</label><select prop:value=move||series.get() on:change=move|ev|{series.set(event_target_value(&ev));saved.set(false);}><option value="">"One-off recording"</option>{move||snap.get().map(|s|s.meeting_series.into_iter().filter(|s|s.project_id==project.get()).map(|s|{let id=s.id.clone();view!{<option value=s.id selected=move||series.get()==id>{s.name}</option>}}).collect_view())}</select>
          <button class="button secondary" type="submit">"Save details"</button>
          <Show when=move||saved.get()><p class="muted" role="status">"Details saved"</p></Show>
        </fieldset>
      </form>
    </details>}
}
#[component]
pub(super) fn MeetingIndicator(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let sessions = RwSignal::new(Vec::<LiveMeeting>::new());
    let epoch = RwSignal::new(0u64);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        epoch.update(|n| *n += 1);
        let ticket = epoch.get_untracked();
        spawn_local(async move {
            let r = api("/api/v1/live-meetings", "GET", None).await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            if let Ok(v) = r {
                if let Ok(rows) = serde_json::from_value::<Vec<LiveMeeting>>(v) {
                    sessions.set(
                        rows.into_iter()
                            .filter(|s| {
                                matches!(
                                    s.state.as_str(),
                                    "starting" | "recording" | "pausing" | "stopping"
                                )
                            })
                            .collect(),
                    );
                }
            }
        });
    });
    view! {<div class="meeting-recording-banner" role="status">{move||sessions.get().into_iter().map(|s|view!{<strong class="orange">{format!("● MEETING {} · {} ",s.state.to_uppercase(),s.title)}</strong><button class="button compact" disabled=move||busy.get() on:click=move|_|action(format!("/api/v1/live-meetings/{}/control",s.id),"POST",json!({"expected_version":s.version,"action":"pause","approve_recording":false}),snap,notice,busy,pending,move|_|{})>"Pause recording"</button>}).collect_view()}</div>}
}
