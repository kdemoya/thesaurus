use super::workflows::{Pending, action, chosen_project};
use super::*;
#[component]
pub(super) fn Capture(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let project = RwSignal::new(String::new());
    let title = RwSignal::new(String::new());
    let audio = RwSignal::new(String::new());
    let whisper = RwSignal::new(String::new());
    let model = RwSignal::new(String::new());
    let reasoning = RwSignal::new(String::new());
    let external = RwSignal::new(false);
    let local_approved = RwSignal::new(false);
    let reasoning_approved = RwSignal::new(None::<String>);
    let jobs = RwSignal::new(Vec::<AudioCapture>::new());
    let windows = RwSignal::new(Vec::<CaptureWindow>::new());
    let chosen_window = RwSignal::new(String::new());
    let interval = RwSignal::new("60".to_string());
    let retention = RwSignal::new("24".to_string());
    let max_frames = RwSignal::new("50".to_string());
    let ocr = RwSignal::new(true);
    let capture_approved = RwSignal::new(false);
    let sessions = RwSignal::new(Vec::<ActivitySession>::new());
    let session_id = RwSignal::new(String::new());
    let query = RwSignal::new(String::new());
    let frames = RwSignal::new(Vec::<ActivityFrame>::new());
    let image = RwSignal::new(None::<(String, String)>);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let reload_epoch = RwSignal::new(0u64);
    let frame_epoch = RwSignal::new(0u64);
    let image_epoch = RwSignal::new(0u64);
    let window_epoch = RwSignal::new(0u64);
    let permission_epoch = RwSignal::new(0u64);
    let reload = move || {
        if reload_epoch.try_get_untracked().is_none() {
            return;
        }
        reload_epoch.update(|n| *n += 1);
        let ticket = reload_epoch.get_untracked();
        spawn_local(async move {
            let response = api("/api/v1/capture/audio", "GET", None).await;
            if reload_epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match response {
                Ok(v) => {
                    if let Ok(rows) = serde_json::from_value(v) {
                        jobs.set(rows);
                    }
                }
                Err(e) => notice.set(e),
            }
            let response = api("/api/v1/capture/sessions", "GET", None).await;
            if reload_epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match response {
                Ok(v) => {
                    if let Ok(rows) = serde_json::from_value(v) {
                        sessions.set(rows);
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
    let load_frames = move || {
        let Some(id) = session_id.try_get_untracked() else {
            return;
        };
        let Some(raw_query) = query.try_get_untracked() else {
            return;
        };
        if id.is_empty() {
            return;
        }
        frame_epoch.update(|n| *n += 1);
        let ticket = frame_epoch.get_untracked();
        let q = js_sys::encode_uri_component(&raw_query)
            .as_string()
            .unwrap_or_default();
        spawn_local(async move {
            let response = api(
                &format!("/api/v1/capture/sessions/{id}/frames?q={q}"),
                "GET",
                None,
            )
            .await;
            if frame_epoch.try_get_untracked() != Some(ticket)
                || session_id.try_get_untracked().as_deref() != Some(id.as_str())
                || query.try_get_untracked().as_deref() != Some(raw_query.as_str())
            {
                return;
            }
            match response {
                Ok(v) => {
                    if let Ok(rows) = serde_json::from_value(v) {
                        frames.set(rows);
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    };
    view! {<div class="focus-toolbar"><label>"Project scope"</label><select prop:value=move||chosen_project(snap,project).map(|p|p.id).unwrap_or_default() on:change=move|ev|{project.set(event_target_value(&ev));session_id.set(String::new());frames.set(vec![]);image.set(None);frame_epoch.update(|n|*n+=1);image_epoch.update(|n|*n+=1);}>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select><button class="button text" on:click=move|_|reload()>"Refresh retained data"</button></div>
    <div class="focus-detail-columns"><section class="panel"><div class="section-heading"><h2>"Audio → local transcript"</h2><span>"Local whisper.cpp"</span></div><p class="muted">"Choose a WAV file, installed whisper.cpp executable and existing local model. Thesaurus sends audio only to that local process. No model download, remote audio route or microphone recording is started."</p><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let Some(p)=chosen_project(snap,project)else{return;};let r=reasoning.get_untracked();action("/api/v1/capture/audio".into(),"POST",json!({"project_id":p.id,"title":title.get_untracked(),"audio_path":audio.get_untracked(),"whisper_command":whisper.get_untracked(),"model_path":model.get_untracked(),"reasoning_connection":if r.is_empty(){None}else{Some(r)},"allow_external":external.get_untracked(),"approve_local_processing":local_approved.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(j)=serde_json::from_value::<AudioCapture>(v){reload_epoch.update(|n|*n+=1);jobs.update(|rows|rows.insert(0,j));local_approved.set(false);}});}>
        <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Transcript title"</label><input required maxlength="200" prop:value=move||title.get() on:input=move|ev|title.set(event_target_value(&ev))/><label>"Absolute WAV path (≤50MB)"</label><input required prop:value=move||audio.get() on:input=move|ev|audio.set(event_target_value(&ev))/><label>"Absolute whisper-cli executable"</label><input required prop:value=move||whisper.get() on:input=move|ev|whisper.set(event_target_value(&ev))/><label>"Absolute local model path"</label><input required prop:value=move||model.get() on:input=move|ev|model.set(event_target_value(&ev))/><label>"Separate reasoning connection"</label><select prop:value=move||reasoning.get() on:change=move|ev|reasoning.set(event_target_value(&ev))><option value="">"No reasoning destination"</option>{move||snap.get().map(|s|s.connections.into_iter().filter(|c|c.kind=="acp").map(|c|view!{<option value=c.id.clone()>{format!("{} · {} · {}",c.id,c.model,c.billing)}</option>}).collect_view())}</select><label><input type="checkbox" prop:checked=move||external.get() on:change=move|ev|external.set(event_target_checked(&ev))/>" Allow external agents to read the resulting transcript source"</label><label><input type="checkbox" prop:checked=move||local_approved.get() on:change=move|ev|local_approved.set(event_target_checked(&ev))/>" Approve this local transcription process"</label><button class="button primary" type="submit" disabled=move||!local_approved.get()>"Transcribe locally"</button></fieldset></form>
        {move||jobs.get().into_iter().map(|j|{let approve_id=j.id.clone();let check=j.id.clone();let selected_id=j.id.clone();let reasoning_id=j.id.clone();let c=j.reasoning_connection.clone();view!{<details class="focus-influence"><summary>{format!("{} · {}",j.title,j.status)}</summary><p>{j.transcription_destination}</p><p class="orange">{j.reasoning_destination}</p>{j.error.map(|e|view!{<p class="runtime-error">{e}</p>})}<pre style="white-space:pre-wrap">{j.transcript}</pre><p class="muted">{format!("Source {} · reasoning runs {}",j.source_id.unwrap_or("pending".into()),j.reasoning_runs.join(", "))}</p>{c.map(|connection|view!{<label><input type="checkbox" prop:checked=move||reasoning_approved.get().as_ref()==Some(&check) on:change=move|ev|reasoning_approved.set(if event_target_checked(&ev){Some(approve_id.clone())}else{None})/>" Approve one transcript-only reasoning request using this selected account"</label><button class="button secondary" disabled=move||busy.get() || reasoning_approved.get().as_ref()!=Some(&selected_id) on:click=move|_|{action(format!("/api/v1/capture/audio/{reasoning_id}/reason"),"POST",json!({"connection":connection,"approve_usage":true}),snap,notice,busy,pending,move|v|{reasoning_approved.set(None);notice.set(format!("Reasoning run {} started; inspect Agents",v["id"]));});}>"Reason on transcript"</button>})}</details>}}).collect_view()}
    </section><section class="panel"><div class="section-heading"><h2>"Selected activity"</h2><span>"macOS · explicit window"</span></div><p class="muted">"Capture only a chosen named window associated with this project. A title/app change pauses capture. Retained PNGs and Vision OCR remain local, searchable here, and expire by your retention settings. macOS Screen Recording permission is required. Configuration alone never starts capture."</p><button class="button text" on:click=move|_|{permission_epoch.update(|n|*n+=1);let ticket=permission_epoch.get_untracked();spawn_local(async move{let response=api("/api/v1/capture/permission","POST",Some(json!({}))).await;if permission_epoch.try_get_untracked()!=Some(ticket){return;}match response{Ok(v)=>notice.set(if v["granted"]==true{"Screen recording permission granted; list windows again".into()}else{"Grant Screen Recording in macOS Settings for the daemon/terminal, then list windows again".into()}),Err(e)=>notice.set(e)}});}>"Request macOS capture permission"</button><button class="button secondary" on:click=move|_|{window_epoch.update(|n|*n+=1);let ticket=window_epoch.get_untracked();spawn_local(async move{let response=api("/api/v1/capture/windows","GET",None).await;if window_epoch.try_get_untracked()!=Some(ticket){return;}match response{Ok(v)=>if let Ok(rows)=serde_json::from_value(v){windows.set(rows);},Err(e)=>notice.set(e)}});}>"List windows for selection"</button>
        <form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let Some(p)=chosen_project(snap,project)else{return;};let original_project=project.get_untracked();let original_session=session_id.get_untracked();let Some(w)=windows.get_untracked().into_iter().find(|w|w.window_id.to_string()==chosen_window.get_untracked())else{notice.set("Select an explicitly listed window".into());return;};let(i,h,m)=match(interval.get_untracked().parse::<u32>(),retention.get_untracked().parse::<u32>(),max_frames.get_untracked().parse::<u32>()){(Ok(i),Ok(h),Ok(m))=>(i,h,m),_=>{notice.set("Invalid capture limits".into());return;}};action("/api/v1/capture/sessions".into(),"POST",json!({"project_id":p.id,"window":w,"interval_seconds":i,"retention_hours":h,"max_frames":m,"use_local_ocr":ocr.get_untracked(),"approve_capture":capture_approved.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(s)=serde_json::from_value::<ActivitySession>(v){reload_epoch.update(|n|*n+=1);if project.try_get_untracked().as_ref()==Some(&original_project)&&session_id.try_get_untracked().as_ref()==Some(&original_session){session_id.set(s.id.clone());frames.set(vec![]);image.set(None);frame_epoch.update(|n|*n+=1);image_epoch.update(|n|*n+=1);}sessions.update(|rows|rows.insert(0,s));capture_approved.set(false);}});}>
        <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"App / window"</label><select prop:value=move||chosen_window.get() on:change=move|ev|{chosen_window.set(event_target_value(&ev));capture_approved.set(false);}><option value="">"Choose listed window"</option>{move||windows.get().into_iter().map(|w|view!{<option value=w.window_id.to_string()>{format!("{} · {} · PID {}",w.app,w.title,w.process_id)}</option>}).collect_view()}</select><label>"Interval seconds (15–3600)"</label><input type="number" min="15" max="3600" prop:value=move||interval.get() on:input=move|ev|interval.set(event_target_value(&ev))/><label>"Retention hours (1–168)"</label><input type="number" min="1" max="168" prop:value=move||retention.get() on:input=move|ev|retention.set(event_target_value(&ev))/><label>"Maximum retained frames (1–100)"</label><input type="number" min="1" max="100" prop:value=move||max_frames.get() on:input=move|ev|max_frames.set(event_target_value(&ev))/><label><input type="checkbox" prop:checked=move||ocr.get() on:change=move|ev|ocr.set(event_target_checked(&ev))/>" Local Apple Vision OCR (requires installed Swift toolchain)"</label><label><input type="checkbox" prop:checked=move||capture_approved.get() on:change=move|ev|capture_approved.set(event_target_checked(&ev))/>" Start capturing this selected window now"</label><button class="button primary" type="submit" disabled=move||!capture_approved.get()>"Start visible capture"</button></fieldset></form>
        {move||sessions.get().into_iter().map(|s|{let id=s.id.clone();let inspect=s.id.clone();view!{<article class="focus-change"><strong class="orange">{format!("{} · {} · {}",s.state,s.window.app,s.window.title)}</strong><p class="muted">{format!("{} frames · {} bytes · {}h retention · every {}s",s.retained_frames,s.retained_bytes,s.retention_hours,s.interval_seconds)}</p>{s.error.map(|e|view!{<p class="runtime-error">{e}</p>})}<div class="workflow-actions">{["pause","resume","stop"].into_iter().map(|a|{let id=id.clone();view!{<button class="button secondary" disabled=move||busy.get() on:click=move|_|action(format!("/api/v1/capture/sessions/{id}/control"),"POST",json!({"expected_version":s.version,"action":a,"approve_capture":a=="resume"}),snap,notice,busy,pending,move|_|reload())>{match a{"pause"=>"Pause capture","resume"=>"Resume selected window",_=>"Stop session"}}</button>}}).collect_view()}<button class="button text" on:click=move|_|{session_id.set(inspect.clone());frames.set(vec![]);image.set(None);image_epoch.update(|n|*n+=1);load_frames();}>"Inspect retained frames"</button></div></article>}}).collect_view()}
    </section></div>
    <section class="panel"><div class="section-heading"><h2>"Retained activity"</h2><span>"Local OCR search · inspect · delete"</span></div><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();load_frames();}><label>"Search selected session's retained text"</label><input prop:value=move||query.get() on:input=move|ev|{query.set(event_target_value(&ev));frames.set(vec![]);image.set(None);frame_epoch.update(|n|*n+=1);image_epoch.update(|n|*n+=1);}/><button class="button secondary" type="submit">"Search / refresh"</button></form>{move||frames.get().into_iter().map(|f|{let image_id=f.id.clone();let delete_id=f.id.clone();let shown=f.id.clone();view!{<details class="focus-influence"><summary>{format!("{} · {} · {} bytes",f.created_at,f.app,f.png_bytes)}</summary><p>{f.window_title}</p><pre style="white-space:pre-wrap">{f.text}</pre><button class="button text" on:click=move|_|{let id=image_id.clone();let session=session_id.get_untracked();let raw_query=query.get_untracked();image_epoch.update(|n|*n+=1);let ticket=image_epoch.get_untracked();image.set(None);spawn_local(async move{let response=api(&format!("/api/v1/capture/frames/{id}/image"),"GET",None).await;if image_epoch.try_get_untracked()!=Some(ticket)||session_id.try_get_untracked().as_deref()!=Some(session.as_str())||query.try_get_untracked().as_deref()!=Some(raw_query.as_str()){return;}match response{Ok(v)=>image.set(Some((id,v["png_base64"].as_str().unwrap_or_default().into()))),Err(e)=>notice.set(e)}});}>"Inspect PNG"</button><button class="button text" on:click=move|_|action(format!("/api/v1/capture/frames/{delete_id}"),"DELETE",json!({}),snap,notice,busy,pending,move|_|{image_epoch.update(|n|*n+=1);image.set(None);load_frames();})>"Delete retained frame and OCR"</button>{move||image.get().filter(|(id,_)|id==&shown).map(|(_,data)|view!{<img alt="Explicitly selected retained activity frame" style="max-width:100%" src=format!("data:image/png;base64,{data}")/>})}</details>}}).collect_view()}</section>}
}
#[component]
pub(super) fn ActivityIndicator(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let active = RwSignal::new(Vec::<ActivitySession>::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let epoch = RwSignal::new(0u64);
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        epoch.update(|n| *n += 1);
        let ticket = epoch.get_untracked();
        spawn_local(async move {
            let response = api("/api/v1/capture/sessions", "GET", None).await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            if let Ok(v) = response {
                if let Ok(s) = serde_json::from_value::<Vec<ActivitySession>>(v) {
                    active.set(s.into_iter().filter(|s| s.state == "active").collect());
                }
            }
        });
    });
    view! {<div role="status">{move||active.get().into_iter().map(|s|view!{<span class="orange">{format!("● CAPTURING {} · {} ",s.window.app,s.window.title)}</span><button class="button compact" disabled=move||busy.get() on:click=move|_|action(format!("/api/v1/capture/sessions/{}/control",s.id),"POST",json!({"expected_version":s.version,"action":"pause","approve_capture":false}),snap,notice,busy,pending,move|_|{})>"Pause capture"</button>}).collect_view()}</div>}
}
