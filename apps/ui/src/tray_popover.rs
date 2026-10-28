//! Quick controls for the transient tray dropdown; detailed setup stays in the meeting view.
use super::live_meetings::{clock, elapsed, window_action};
use super::workflows::{Pending, action};
use super::*;

#[wasm_bindgen(inline_js = "
export function isTrayPopover(){return new URLSearchParams(location.search).get('tray')==='1';}
export function watchTrayPopover(open,error){
  const root=document.querySelector('.tray-popover');
  if(!root)return;
  const focus=()=>requestAnimationFrame(()=>(root.querySelector('[data-tray-primary]:not(:disabled)')||root.querySelector('button:not(:disabled)'))?.focus({preventScroll:true}));
  if(window.__TAURI__){window.__TAURI__.event.listen('tray-opened',()=>{open();focus();});window.__TAURI__.event.listen('service-error',e=>error(String(e.payload)));}
  focus();
  root.addEventListener('keydown',event=>{
    const controls=[...root.querySelectorAll('button:not(:disabled),input:not(:disabled),select:not(:disabled)')].filter(node=>node.getClientRects().length);
    if(!controls.length)return;
    const index=controls.indexOf(document.activeElement);
    if(event.key==='Tab'){
      if(event.shiftKey&&index<=0){event.preventDefault();controls[controls.length-1].focus();}
      else if(!event.shiftKey&&index===controls.length-1){event.preventDefault();controls[0].focus();}
    }else if(['ArrowDown','ArrowUp'].includes(event.key)&&!['INPUT','TEXTAREA','SELECT'].includes(event.target.tagName)&&!event.metaKey&&!event.ctrlKey&&!event.altKey){
      event.preventDefault();controls[(index+(event.key==='ArrowDown'?1:-1)+controls.length)%controls.length].focus();
    }
  });
  let next=0,last=0,busy=false;
  const resize=async()=>{
    if(busy||!window.__TAURI__)return;
    busy=true;
    try{while(next&&next!==last){const height=next;last=height;await window.__TAURI__.core.invoke('meeting_popover_height',{height});}}
    catch(_){last=0;}
    finally{busy=false;}
  };
  new ResizeObserver(()=>{if(document.hasFocus()&&!root.contains(document.activeElement))focus();next=Math.min(460,Math.max(180,Math.ceil(root.getBoundingClientRect().height)));resize();}).observe(root);
}
")]
extern "C" {
    #[wasm_bindgen(js_name=isTrayPopover)]
    pub fn is_tray() -> bool;
    #[wasm_bindgen(js_name=watchTrayPopover)]
    fn watch(callback: &js_sys::Function, error: &js_sys::Function);
}
fn current(rows: RwSignal<Vec<LiveMeeting>>) -> Option<LiveMeeting> {
    let items = rows.get();
    items
        .iter()
        .find(|s| {
            matches!(
                s.state.as_str(),
                "starting" | "recording" | "pausing" | "stopping"
            )
        })
        .or_else(|| {
            items
                .iter()
                .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
                .filter(|s| s.state == "paused")
        })
        .cloned()
}
#[component]
pub(super) fn TrayPopover() -> impl IntoView {
    let snap = RwSignal::new(None::<Snapshot>);
    let notice = RwSignal::new(String::new());
    let rows = RwSignal::new(Vec::<LiveMeeting>::new());
    let settings = RwSignal::new(None::<LiveRecordingSettings>);
    let loaded = RwSignal::new(false);
    let epoch = RwSignal::new(0u64);
    let now = RwSignal::new(js_sys::Date::now());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let note = RwSignal::new(String::new());
    let feedback = RwSignal::new(String::new());
    let reload = move || {
        epoch.update(|n| *n += 1);
        let ticket = epoch.get_untracked();
        spawn_local(async move {
            let result = api("/api/v1/live-meetings", "GET", None).await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match result {
                Ok(value) => match serde_json::from_value::<Vec<LiveMeeting>>(value) {
                    Ok(items) => {
                        rows.set(items);
                        loaded.set(true);
                    }
                    Err(_) => notice.set("Could not read recording status".into()),
                },
                Err(e) => notice.set(e),
            }
            if let Ok(value) = api("/api/v1/live-meetings/settings", "GET", None).await {
                if epoch.try_get_untracked() == Some(ticket) {
                    if let Ok(value) = serde_json::from_value(value) {
                        settings.set(value);
                    }
                }
            }
        });
    };
    spawn_local(async move {
        if !is_native() {
            if let Ok(v) = api("/api/v1/session", "GET", None).await {
                CSRF.with(|s| *s.borrow_mut() = v["csrf"].as_str().unwrap_or_default().into());
            }
        }
        for _ in 0..20 {
            if let Ok(value) = api("/api/v1/snapshot", "GET", None).await {
                if let Ok(s) = serde_json::from_value(value) {
                    snap.set(Some(s));
                }
                break;
            }
            gloo_timers::future::TimeoutFuture::new(500).await;
        }
        let callback = Closure::<dyn Fn()>::new(move || refresh(snap, notice));
        observe(callback.as_ref().unchecked_ref());
        callback.forget();
        reload();
    });
    Effect::new(move |_| {
        if snap.get().is_some() {
            reload();
        }
    });
    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(1000).await;
            if now.try_get_untracked().is_none() {
                break;
            }
            now.set(js_sys::Date::now());
        }
    });
    let opened = Closure::<dyn Fn()>::new(move || {
        reload();
        refresh(snap, notice);
    });
    // The popover is retained while hidden, so this callback has the window's lifetime.
    let function = opened.as_ref().unchecked_ref::<js_sys::Function>().clone();
    let error = Closure::<dyn Fn(String)>::new(move |e| notice.set(e));
    let error_function = error.as_ref().unchecked_ref::<js_sys::Function>().clone();
    request_animation_frame(move || watch(&function, &error_function));
    opened.forget();
    error.forget();
    let control = move |action_name: &'static str| {
        let Some(s) = current(rows) else {
            return;
        };
        action(
            format!("/api/v1/live-meetings/{}/control", s.id),
            "POST",
            json!({"expected_version":s.version,"action":action_name,"approve_recording":action_name=="resume"}),
            snap,
            notice,
            busy,
            pending,
            move |_| reload(),
        );
    };
    let save_note = move |kind: &'static str| {
        let Some(s) = current(rows) else {
            return;
        };
        let text = note.get_untracked();
        let text = if text.trim().is_empty() && kind == "bookmark" {
            "Bookmark".into()
        } else {
            text
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
                feedback.set(
                    if kind == "bookmark" {
                        "Moment bookmarked"
                    } else {
                        "Note saved"
                    }
                    .into(),
                );
                reload();
            },
        );
    };
    view! {<div class="tray-popover" role="region" aria-label="Meeting quick controls" on:keydown=move|ev:web_sys::KeyboardEvent|{
        if ev.key()=="Escape"{ev.prevent_default();window_action("dismiss",notice);}
        if ev.alt_key()&&ev.key()=="b"{ev.prevent_default();save_note("bookmark");}
    }>
      <header class="tray-heading"><span class="tray-brand">"↪ thesaurus"</span><span class="tray-local">"local"</span><button class="tray-icon-button" aria-label="Close dropdown" on:click=move|_|window_action("dismiss",notice)>"×"</button></header>
      <div class="tray-content">
        <Show when=move||!loaded.get()&&notice.get().is_empty()><p class="tray-muted">"Connecting…"</p></Show>
        <Show when=move||!notice.get().is_empty()><div class="tray-error" role="alert"><p>{move||notice.get()}</p><button class="tray-text-button" on:click=move|_|{notice.set(String::new());reload();}>"Retry"</button></div></Show>
        <Show when=move||loaded.get()&&current(rows).is_some()>
          <div class="tray-session"><div class="tray-state"><span>{move||current(rows).map(|s|format!("{} {}",if matches!(s.state.as_str(),"recording"|"starting"|"pausing"|"stopping"){"●"}else{"Ⅱ"},s.state)).unwrap_or_default()}</span><time>{move||current(rows).map(|s|clock(elapsed(&s,now.get()))).unwrap_or_default()}</time></div>
          <h1>{move||current(rows).map(|s|s.title).unwrap_or_default()}</h1>
          <p class="tray-muted tray-single-line">{move||current(rows).map(|s|{let name=snap.get().and_then(|v|v.briefs.into_iter().find(|p|p.id==s.project_id).map(|p|p.name)).unwrap_or_else(||"Unassigned recording".into());format!("{} · {}{}",name,if s.microphone_id.is_some(){"mic"}else{""},if s.system_audio{if s.microphone_id.is_some(){" + system audio"}else{"system audio"}}else{""})}).unwrap_or_default()}</p>
          <div class="tray-control-row"><button data-tray-primary class="tray-primary" disabled=move||busy.get()||current(rows).is_none_or(|s|!matches!(s.state.as_str(),"recording"|"starting"|"paused")) on:click=move|_|control(if current(rows).is_some_and(|s|s.state=="paused"){"resume"}else{"pause"})>{move||if current(rows).is_some_and(|s|s.state=="paused"){"▶ Resume"}else{"Ⅱ Pause"}}</button><button class="tray-secondary" disabled=move||busy.get()||current(rows).is_none_or(|s|matches!(s.state.as_str(),"pausing"|"stopping")) on:click=move|_|control("stop")>"■ Stop"</button></div>
          {move||current(rows).and_then(|s|s.error).map(|e|view!{<p class="tray-error" role="alert">{e}</p>})}
          <p class="tray-muted tray-processing">{move||current(rows).filter(|s|s.pending_chunks>0).map(|s|format!("Transcribing {} chunk{}…",s.pending_chunks,if s.pending_chunks==1{""}else{"s"})).unwrap_or_default()}</p>
          </div>
          <form class="tray-note" on:submit=move|ev|{ev.prevent_default();save_note("note");}><input aria-label="Quick meeting note" maxlength="4000" placeholder="Quick note… ↵" prop:value=move||note.get() on:input=move|ev|{note.set(event_target_value(&ev));feedback.set(String::new());}/><button type="submit" class="tray-icon-button" aria-label="Save note" disabled=move||busy.get()||note.get().trim().is_empty()>"↵"</button><button type="button" class="tray-icon-button" aria-label="Bookmark this moment" title="Bookmark · Alt+B" disabled=move||busy.get() on:click=move|_|save_note("bookmark")>"⚑"</button></form>
          <Show when=move||!feedback.get().is_empty()><p class="tray-saved" role="status">{move||feedback.get()}</p></Show>
        </Show>
        <Show when=move||loaded.get()&&current(rows).is_none()><div class="tray-idle"><h1>"○ Not recording"</h1><p class="tray-muted">"Record now. Add meeting details afterward."</p><p class="tray-muted">{move||{let pending=rows.get().iter().map(|s|s.pending_chunks).sum::<u32>();if pending>0{format!("Finishing transcription · {pending} queued")}else{String::new()}}}</p>
          <button class="tray-primary" data-tray-primary disabled=move||busy.get()||settings.get().is_none() on:click=move|_|action("/api/v1/live-meetings/quick-start".into(),"POST",json!({"approve_recording":true}),snap,notice,busy,pending,move|v|{if let Ok(s)=serde_json::from_value::<LiveMeeting>(v){rows.update(|r|{r.retain(|old|old.id!=s.id);r.insert(0,s);});}reload();})>{move||if busy.get(){"Starting…"}else{"● Start recording"}}</button>
          <p class="tray-muted">{move||settings.get().map(|s|format!("{} · {}",match(s.microphone_id.is_some(),s.system_audio){(true,true)=>"Microphone + all system audio",(true,false)=>"Microphone",(false,true)=>"All system audio",_=>"No audio input selected"},if s.use_bundled_transcription{"bundled local transcription"}else{"custom local transcription"})).unwrap_or_else(||"Loading recording defaults…".into())}</p>
          <button class="tray-text-button" on:click=move|_|window_action("setup",notice)>"Audio setup…"</button>
          <Show when=move||rows.get().iter().any(|s|s.state=="stopped")><button class="tray-text-button" on:click=move|_|window_action("workspace",notice)>"Add recording details ↗"</button></Show>
        </div></Show>
      </div>
      <footer class="tray-footer"><button class="tray-text-button" on:click=move|_|window_action("pin",notice)>"Pin widget ↗"</button><button class="tray-text-button" on:click=move|_|window_action("workspace",notice)>"Open meeting ↗"</button></footer>
    </div>}
}
