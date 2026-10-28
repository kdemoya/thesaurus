mod actions;
mod annotations;
mod audio_settings;
mod bragbook;
mod briefs;
mod capture;
mod context_delivery;
mod delivery;
mod discovery;
mod dot_type;
mod evaluations;
mod exports;
mod focus;
mod insights;
mod knowledge;
mod live_meetings;
mod meeting_summaries;
mod meetings;
mod memory_review;
mod native_notifications;
mod onboarding;
mod plugin_action_review;
mod plugin_evaluation_results;
mod plugin_model_review;
mod plugin_runs;
mod plugin_setup;
mod plugins;
mod priorities;
mod procedures;
mod recipes;
mod repository;
mod retrieval;
mod settings;
mod source_controls;
mod themes;
mod threads;
mod transcription_setup;
mod tray_popover;
mod usage_controls;
mod waiting;
mod workflows;
mod workspace;
use leptos::{prelude::*, task::spawn_local};
use serde_json::{Value, json};
use std::cell::RefCell;
use thesaurus_contracts::*;
use wasm_bindgen::prelude::*;

thread_local! {static CSRF:RefCell<String>=const {RefCell::new(String::new())};}
#[derive(Clone, Debug, PartialEq)]
struct EvidenceTarget {
    document_id: String,
    revision: i64,
    line: i64,
}
thread_local! {
    static EVIDENCE_TARGET:RefCell<Option<RwSignal<Option<EvidenceTarget>>>>=const{RefCell::new(None)};
    static DOCUMENT_REQUEST:RefCell<u64>=const{RefCell::new(0)};
    static DOCUMENT_VIEW:RefCell<Option<RwSignal<Option<Document>>>>=const{RefCell::new(None)};
}
fn navigate_evidence(e: Evidence, notice: RwSignal<String>) {
    DOCUMENT_VIEW.with(|signal| {
        if let Some(selected) = *signal.borrow() {
            briefs::open_evidence(e, selected, notice);
        }
    });
}
fn document_request() -> u64 {
    DOCUMENT_REQUEST.with(|request| {
        let mut value = request.borrow_mut();
        *value += 1;
        *value
    })
}
fn current_document_request(ticket: u64) -> bool {
    DOCUMENT_REQUEST.with(|value| *value.borrow() == ticket)
}
fn evidence_target(target: Option<EvidenceTarget>) {
    EVIDENCE_TARGET.with(|signal| {
        if let Some(signal) = *signal.borrow() {
            signal.set(target);
        }
    });
}

#[wasm_bindgen(
    inline_js = "export function scrollEvidence(documentId,revision,line){requestAnimationFrame(()=>{const body=document.querySelector('.document-body[data-evidence-document]');if(!body||body.dataset.evidenceDocument!==documentId||body.dataset.evidenceRevision!==String(revision))return;const row=document.getElementById('evidence-line-'+line);if(!row)return;row.scrollIntoView({block:'center',inline:'nearest'});row.focus({preventScroll:true});});}"
)]
extern "C" {
    #[wasm_bindgen(js_name=scrollEvidence)]
    fn scroll_evidence(document_id: &str, revision: i64, line: i64);
}

#[wasm_bindgen(
    inline_js = "export function isNative(){return !!window.__TAURI__;} export async function nativeRequest(path,method,body){return JSON.stringify(await window.__TAURI__.core.invoke('api_request',{path,method,body}));} export function observe(callback){let timer=null;const changed=()=>{if(timer===null){timer=setTimeout(()=>{timer=null;callback();},100);}};if(window.__TAURI__){window.__TAURI__.event.listen('data-changed',changed);}else{const s=new EventSource('/api/v1/events');s.addEventListener('change',changed);window.__thesaurusEvents=s;}} export function newId(){return crypto.randomUUID();} export function meetingEvents(open,error,connection){if(window.__TAURI__){window.__TAURI__.event.listen('meeting-workspace',()=>open());window.__TAURI__.event.listen('service-error',e=>error(String(e.payload)));window.__TAURI__.event.listen('desktop-recovery',e=>connection(JSON.stringify(e.payload)));}}"
)]
extern "C" {
    #[wasm_bindgen(js_name=isNative)]
    fn is_native() -> bool;
    #[wasm_bindgen(catch,js_name=nativeRequest)]
    async fn native_request(path: &str, method: &str, body: JsValue) -> Result<JsValue, JsValue>;
    fn observe(callback: &js_sys::Function);
    #[wasm_bindgen(js_name=newId)]
    fn new_id() -> String;
    #[wasm_bindgen(js_name=meetingEvents)]
    fn meeting_events(
        open: &js_sys::Function,
        error: &js_sys::Function,
        connection: &js_sys::Function,
    );
}

async fn api(path: &str, method: &str, body: Option<Value>) -> Result<Value, String> {
    if is_native() {
        let body = js_sys::JSON::parse(&body.unwrap_or(Value::Null).to_string())
            .map_err(|_| "Invalid request".to_string())?;
        let value = native_request(path, method, body).await.map_err(|e| {
            e.as_string()
                .unwrap_or_else(|| "Desktop connection unavailable".into())
        })?;
        return serde_json::from_str(&value.as_string().unwrap_or_default())
            .map_err(|_| "Invalid service response".into());
    }
    let mut request = gloo_net::http::RequestBuilder::new(path)
        .method(
            method
                .parse()
                .map_err(|_| "Invalid request method".to_string())?,
        )
        .header("Content-Type", "application/json");
    let csrf = CSRF.with(|s| s.borrow().clone());
    if !csrf.is_empty() {
        request = request.header("X-Thesaurus-CSRF", &csrf);
    }
    let response = if let Some(body) = body {
        request
            .body(body.to_string())
            .map_err(|e| e.to_string())?
            .send()
            .await
    } else {
        request.send().await
    }
    .map_err(|_| {
        "The local service is unavailable. Check that thesaurus serve is running.".to_string()
    })?;
    let status = response.status();
    let value = response.json::<Value>().await.unwrap_or(json!({}));
    if status >= 400 {
        return Err(value["error"]
            .as_str()
            .unwrap_or("Request could not be completed")
            .into());
    }
    Ok(value)
}

type Snap = RwSignal<Option<Snapshot>>;
thread_local! {
    // Replayed source events can arrive in a burst after reconnecting. Keep one
    // snapshot request active and one catch-up request, instead of fetching a
    // complete workspace for every event in that burst.
    static REFRESH_IN_FLIGHT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static REFRESH_REQUESTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn refresh(snap: Snap, notice: RwSignal<String>) {
    if REFRESH_IN_FLIGHT.replace(true) {
        REFRESH_REQUESTED.set(true);
        return;
    }
    spawn_local(async move {
        loop {
            REFRESH_REQUESTED.set(false);
            match api("/api/v1/snapshot", "GET", None).await.and_then(|v| {
                serde_json::from_value(v).map_err(|_| "Invalid workspace response".into())
            }) {
                Ok(s) => snap.set(Some(s)),
                Err(e) => notice.set(e),
            }
            if !REFRESH_REQUESTED.get() {
                REFRESH_IN_FLIGHT.set(false);
                break;
            }
        }
    })
}
fn mutate(path: String, method: &'static str, body: Value, snap: Snap, notice: RwSignal<String>) {
    spawn_local(async move {
        match api(&path, method, Some(body)).await {
            Ok(_) => {
                notice.set(String::new());
                refresh(snap, notice)
            }
            Err(e) => notice.set(e),
        }
    })
}

fn active_tasks(snap: Option<Snapshot>) -> Vec<Task> {
    snap.map(|s| {
        s.tasks
            .into_iter()
            .filter(|t| t.status == "candidate" || t.status == "accepted")
            .collect()
    })
    .unwrap_or_default()
}
fn focused_task(snap: Option<Snapshot>, selected: Option<String>) -> Option<Task> {
    let tasks = active_tasks(snap);
    tasks
        .iter()
        .find(|t| Some(&t.id) == selected.as_ref())
        .cloned()
        .or_else(|| tasks.into_iter().next())
}

#[component]
fn App() -> impl IntoView {
    let snap = RwSignal::new(None::<Snapshot>);
    let notice = RwSignal::new(String::new());
    let view = RwSignal::new(
        if live_meetings::is_companion() {
            "companion"
        } else {
            "now"
        }
        .to_string(),
    );
    let section = RwSignal::new("sources".to_string());
    Effect::new(move |_| {
        let page = view.get();
        section.set(
            workspace::sections(&page)
                .first()
                .map(|s| s.0)
                .unwrap_or_default()
                .into(),
        );
    });
    Effect::new(move |_| {
        let _ = view.get();
        let _ = section.get();
        workspace::reset_scroll();
    });
    provide_context(workspace::SettingsNavigation(Callback::new(move |_| {
        view.set("settings".into())
    })));
    let setup_target = RwSignal::new(None::<onboarding::SetupTarget>);
    provide_context(onboarding::SetupNavigation(Callback::new(
        move |t: onboarding::SetupTarget| {
            setup_target.set(Some(t));
            view.set("onboarding".into());
        },
    )));
    let meeting_open = Closure::<dyn FnMut()>::new(move || view.set("meetings".into()));
    let service_error = Closure::<dyn FnMut(String)>::new(move |e| notice.set(e));
    let connection_notice = RwSignal::new(String::new());
    let connection = Closure::<dyn FnMut(String)>::new(move |value: String| {
        let Ok(status) = serde_json::from_str::<Value>(&value) else {
            return;
        };
        if status["phase"] == "ready" {
            let previous = connection_notice.get_untracked();
            if !previous.is_empty() && notice.get_untracked() == previous {
                notice.set(String::new());
            }
            connection_notice.set(String::new());
        } else if let (Some(phase), Some(headline), Some(detail)) = (
            status["phase"].as_str(),
            status["headline"].as_str(),
            status["detail"].as_str(),
        ) {
            if !matches!(phase, "connecting" | "starting") {
                connection_notice.set(format!("{headline}: {detail}"));
            }
        }
    });
    meeting_events(
        meeting_open.as_ref().unchecked_ref(),
        service_error.as_ref().unchecked_ref(),
        connection.as_ref().unchecked_ref(),
    );
    meeting_open.forget();
    service_error.forget();
    connection.forget();
    let importing = RwSignal::new(false);
    let saving = RwSignal::new(false);
    let saving_memory = RwSignal::new(false);
    let selected = RwSignal::new(None::<Document>);
    DOCUMENT_VIEW.with(|signal| *signal.borrow_mut() = Some(selected));
    let annotation_line = RwSignal::new("1".to_string());
    let cited = RwSignal::new(None::<EvidenceTarget>);
    EVIDENCE_TARGET.with(|signal| *signal.borrow_mut() = Some(cited));
    Effect::new(move |_| {
        let target = cited.get();
        let document = selected.get();
        if let (Some(target), Some(doc)) = (target, document) {
            if target.document_id == doc.id && target.revision == doc.revision {
                annotation_line.set(target.line.to_string());
                scroll_evidence(&target.document_id, target.revision, target.line);
            }
        }
    });
    let selected_task = RwSignal::new(None::<String>);
    let help = RwSignal::new(false);
    let theme_open = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    let hits = RwSignal::new(None::<Vec<SearchHit>>);
    let title = RwSignal::new(String::new());
    let body = RwSignal::new(String::new());
    let uri = RwSignal::new(String::new());
    let kind = RwSignal::new("transcript".to_string());
    let external = RwSignal::new(false);
    let memory_body = RwSignal::new(String::new());
    let memory_source = RwSignal::new(String::new());
    spawn_local(async move {
        let result = async {
            if !is_native() {
                let location = web_sys::window().unwrap().location();
                let hash = location.hash().unwrap_or_default();
                let response = if let Some(code) = hash.strip_prefix("#pair=") {
                    let r = api("/api/v1/pair", "POST", Some(json!({"code":code}))).await?;
                    let _ = web_sys::window()
                        .unwrap()
                        .history()
                        .unwrap()
                        .replace_state_with_url(&JsValue::NULL, "", Some("/"));
                    r
                } else {
                    api("/api/v1/session", "GET", None).await?
                };
                CSRF.with(|s| {
                    *s.borrow_mut() = response["csrf"].as_str().unwrap_or_default().into()
                });
            }
            let mut response = api("/api/v1/snapshot", "GET", None).await;
            if is_native() {
                for _ in 0..20 {
                    if response.is_ok() {
                        break;
                    }
                    gloo_timers::future::TimeoutFuture::new(500).await;
                    response = api("/api/v1/snapshot", "GET", None).await;
                }
            }
            let value = response?;
            let loaded: Snapshot =
                serde_json::from_value(value).map_err(|_| "Invalid workspace data".to_string())?;
            snap.set(Some(loaded));
            onboarding::initial_entry(view, notice).await;
            let callback = Closure::<dyn Fn()>::new(move || refresh(snap, notice));
            observe(callback.as_ref().unchecked_ref());
            callback.forget();
            Ok::<_, String>(())
        }
        .await;
        if let Err(e) = result {
            notice.set(e)
        }
    });
    let open_import = move |_| {
        title.set(String::new());
        body.set(String::new());
        uri.set(String::new());
        external.set(false);
        importing.set(true);
    };
    let submit_import = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if saving.get_untracked() {
            return;
        }
        saving.set(true);
        let request = ImportRequest {
            source_name: title.get(),
            title: title.get(),
            body: body.get(),
            kind: kind.get(),
            uri: if uri.get().trim().is_empty() {
                format!("capture://{}", new_id())
            } else {
                uri.get()
            },
            allow_external: external.get(),
        };
        spawn_local(async move {
            let result = api(
                "/api/v1/captures",
                "POST",
                Some(serde_json::to_value(request).unwrap()),
            )
            .await;
            saving.set(false);
            match result {
                Ok(_) => {
                    importing.set(false);
                    notice.set(String::new());
                    refresh(snap, notice)
                }
                Err(e) => notice.set(e),
            }
        });
    };
    let run_search = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        view.set("library".into());
        section.set("sources".into());
        spawn_local(async move {
            match api(
                "/api/v1/search",
                "POST",
                Some(json!({"query":query.get(),"limit":30})),
            )
            .await
            {
                Ok(v) => match serde_json::from_value(v) {
                    Ok(results) => hits.set(Some(results)),
                    Err(_) => notice.set("Could not read search results".into()),
                },
                Err(e) => notice.set(e),
            }
        });
    };
    let read_doc = move |id: String, revision: Option<i64>| {
        let ticket = document_request();
        evidence_target(None);
        annotation_line.set("1".into());
        spawn_local(async move {
            let suffix = revision.map_or(String::new(), |r| format!("?revision={r}"));
            match api(&format!("/api/v1/documents/{id}{suffix}"), "GET", None).await {
                Ok(v) => match serde_json::from_value(v) {
                    Ok(doc) => {
                        if current_document_request(ticket) {
                            selected.set(Some(doc));
                        }
                    }
                    Err(_) => notice.set("Could not read document".into()),
                },
                Err(e) => notice.set(e),
            }
        })
    };
    // Match terminal navigation without intercepting typing or browser shortcuts.
    let keys = Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |ev: web_sys::KeyboardEvent| {
        if ev.meta_key() && ev.key() == "," {
            ev.prevent_default();
            view.set("settings".into());
            return;
        }
        if ev.ctrl_key() || ev.meta_key() || ev.alt_key() {
            return;
        }
        if ev.key() == "Escape" {
            theme_open.set(false);
            importing.set(false);
            selected.set(None);
            help.set(false);
            return;
        }
        let typing = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|e| {
                e.closest("input, textarea, select, [contenteditable='true']")
                    .ok()
                    .flatten()
                    .is_some()
            });
        if view.get_untracked() == "onboarding" {
            return;
        }
        if typing
            || importing.get_untracked()
            || selected.get_untracked().is_some()
            || help.get_untracked()
            || theme_open.get_untracked()
        {
            return;
        }
        if live_meetings::is_companion() {
            return;
        }
        match ev.key().as_str() {
            "1" => view.set("now".into()),
            "2" => view.set("library".into()),
            "3" => view.set("memory".into()),
            "4" => view.set("activity".into()),
            "5" => view.set("connections".into()),
            "6" => view.set("briefs".into()),
            "7" => view.set("threads".into()),
            "8" => view.set("meetings".into()),
            "9" => view.set("focus".into()),
            "0" => view.set("recipes".into()),
            "e" => view.set("evaluations".into()),
            "p" => view.set("plugins".into()),
            "b" => view.set("bragbook".into()),
            "n" => {
                title.set(String::new());
                body.set(String::new());
                uri.set(String::new());
                external.set(false);
                importing.set(true);
            }
            "/" => {
                ev.prevent_default();
                view.set("library".into());
                section.set("sources".into());
                spawn_local(async {
                    gloo_timers::future::TimeoutFuture::new(0).await;
                    if let Some(input) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("context-search"))
                        .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
                    {
                        let _ = input.focus();
                    }
                });
            }
            "?" => help.set(true),
            "j" | "k" if view.get_untracked() == "now" => {
                ev.prevent_default();
                let tasks = active_tasks(snap.get_untracked());
                if !tasks.is_empty() {
                    let current = selected_task.get_untracked();
                    let index = tasks
                        .iter()
                        .position(|t| Some(&t.id) == current.as_ref())
                        .unwrap_or(0);
                    let next = if ev.key() == "j" {
                        (index + 1).min(tasks.len() - 1)
                    } else {
                        index.saturating_sub(1)
                    };
                    selected_task.set(Some(tasks[next].id.clone()));
                }
            }
            "Enter" if view.get_untracked() == "now" => {
                if let Some(task) =
                    focused_task(snap.get_untracked(), selected_task.get_untracked())
                {
                    briefs::open_evidence(task.evidence, selected, notice);
                }
            }
            _ => {}
        }
    });
    let _ = web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("keydown", keys.as_ref().unchecked_ref());
    keys.forget();
    view! {
        <div class="app-shell" class:companion-shell=live_meetings::is_companion() class:onboarding-shell=move||view.get()=="onboarding">
            <header class="app-header">
                <a class="brand" href="#" on:click=move|_|view.set("now".into())><span>"↪"</span>"thesaurus"<small>"0.1.0"</small></a>
                <workspace::Navigation view=view hits=hits/>
                <div class="sidebar-footer">
                    <span class="instance"><span class="local-dot">"●"</span>"Personal · local"</span>
                    <div class="sidebar-settings"><button class="button text setup-trigger" on:click=move|_|view.set("settings".into())>"Settings"</button><button class="button text theme-trigger" on:click=move|_|theme_open.set(true)>"Theme"</button></div>
                </div>
            </header>
            <div class="workspace-body">
            <capture::ActivityIndicator snap=snap notice=notice/><live_meetings::MeetingIndicator snap=snap notice=notice/><div class="main-shell">
                <main class:settings-view=move||view.get()=="settings">
                    <Show when=move||!notice.get().is_empty()><div class="notice" role="alert"><span>{move||notice.get()}</span><button aria-label="Dismiss notice" on:click=move|_|notice.set(String::new())>"×"</button></div></Show>
                    <Show when=move||snap.get().is_none()>
                        <section class="connect-state"><span class="eyebrow">"Workspace"</span><h1>"Pair this browser"</h1><p>"Open this page from the local app or pair your browser to access your work."</p><code>"thesaurus open"</code><button class="button secondary" on:click=move|_|refresh(snap,notice)>"Reconnect"</button></section>
                    </Show>
                    <Show when=move||snap.get().is_some()>
                        <Show when=move||view.get()=="onboarding"><onboarding::Onboarding snap=snap notice=notice view=view selected=selected target=setup_target/></Show>
                        <div class="page-toolbar">
                            <div class="page-heading"><div><h1>{move||workspace::title(&view.get())}</h1><p>{move||workspace::description(&view.get())}</p></div><button class="button primary" on:click=open_import><kbd>"n"</kbd>"Capture"</button></div>
                            <workspace::Sections view=view section=section/>
                        </div>
                        <div class="page-content" id="page-content">
                        <Show when=move||view.get()=="now">
                            <div class="work-layout">
                                <aside class="work-aside">
                                    <section class="panel workspace-panel"><div class="section-heading"><h2>"Workspace"</h2><button class="source-link" on:click=move|_|view.set("focus".into())>"focus →"</button></div>
                                        <dl class="workspace-counts">
                                            <dt>"To review"</dt><dd class="orange">{move||snap.get().map(|s|s.tasks.iter().filter(|t|t.status=="candidate").count()).unwrap_or(0)}</dd>
                                            <dt>"Accepted"</dt><dd class="violet">{move||snap.get().map(|s|s.tasks.iter().filter(|t|t.status=="accepted").count()).unwrap_or(0)}</dd>
                                            <dt>"Completed"</dt><dd>{move||snap.get().map(|s|s.tasks.iter().filter(|t|t.status=="completed").count()).unwrap_or(0)}</dd>
                                        </dl>
                                        <details class="workspace-processing"><summary>"Processing status"</summary><div class="processing-meter"><span>"Processed"</span><span>{move||snap.get().map(|s|format!("{} / {}",s.jobs.iter().filter(|j|j.status=="completed").count(),s.jobs.len())).unwrap_or_default()}</span><progress aria-label="Completed processing jobs" value=move||snap.get().map(|s|s.jobs.iter().filter(|j|j.status=="completed").count()).unwrap_or(0) max=move||snap.get().map(|s|s.jobs.len().max(1)).unwrap_or(1)></progress></div>
                                        <div class="workspace-jobs"><span>"Pending"</span><span>{move||snap.get().map(|s|s.jobs.iter().filter(|j|j.status=="queued"||j.status=="processing").count()).unwrap_or(0)}</span><span>"Failed"</span><span>{move||snap.get().map(|s|s.jobs.iter().filter(|j|j.status=="failed").count()).unwrap_or(0)}</span></div></details>
                                    </section>
                                    <section class="panel context-panel"><div class="section-heading"><h2>"Sources"</h2><button class="source-link" on:click=move|_|view.set("library".into())>"all →"</button></div>
                                        {move||snap.get().map(|s|s.documents.into_iter().take(4).map(|d|{let id=d.id;view!{<button class="source-tree-row" on:click=move|_|read_doc(id.clone(),None)><span class="tree-branch">"├─"</span><span>{d.title}</span><small>{format!("r{}",d.revision)}</small></button>}}).collect_view())}
                                        <Show when=move||snap.get().is_some_and(|s|s.documents.is_empty())><p class="muted empty-small">"No sources imported."</p></Show>
                                    </section>
                                    <section class="panel notes-panel"><div class="section-heading"><h2>"Memory"</h2><button class="source-link" on:click=move|_|view.set("memory".into())>"all →"</button></div>
                                        {move||snap.get().map(|s|s.observations.into_iter().take(1).map(|o|view!{<div class="memory-line"><p>{o.body}</p><small>{o.contributor}</small></div>}).collect_view())}
                                        <Show when=move||snap.get().is_some_and(|s|s.observations.is_empty())><p class="muted empty-small">"No observations yet."</p></Show>
                                    </section>
                                </aside>
                                <section class="panel actions-panel"><div class="section-heading"><h2>"Actions"</h2><span class="muted">{move||format!("{} active",active_tasks(snap.get()).len())}</span></div>
                                    <Show when=move||!active_tasks(snap.get()).is_empty() fallback=move||view!{<div class="empty-state"><p>"No actions to review."</p><p class="muted">"Capture a transcript or note to extract commitments."</p><button class="button secondary" on:click=move|_|mutate("/api/v1/demo".into(),"POST",json!({}),snap,notice)>"Load synthetic examples"</button></div>}>
                                        <div class="task-table-scroll"><table class="task-table"><thead><tr><th>"State"</th><th>"Action"</th><th>"Owner"</th><th>"Due"</th></tr></thead><tbody>
                                            {move||active_tasks(snap.get()).into_iter().map(|t|{let id=t.id.clone();let click=t.id.clone();view!{<tr class:selected=move||focused_task(snap.get(),selected_task.get()).is_some_and(|t|t.id==id) on:click=move|_|selected_task.set(Some(click.clone()))><td class=if t.status=="candidate"{"state-review"}else{"state-accepted"}>{if t.status=="candidate"{"? review"}else{"+ accepted"}}</td><td><button class="task-title" title=t.title.clone() on:click=move|_|selected_task.set(Some(t.id.clone()))>{t.title.clone()}</button></td><td class="task-owner">{t.owner.unwrap_or_else(||"—".into())}</td><td class="task-due">{t.due.unwrap_or_else(||"—".into())}</td></tr>}}).collect_view()}
                                        </tbody></table></div>
                                        {move||focused_task(snap.get(),selected_task.get()).map(|t|{let id=t.id.clone();let dismiss=t.id.clone();let ev=t.evidence.clone();let version=t.version;let candidate=t.status=="candidate";let source_name=snap.get().and_then(|s|s.documents.into_iter().find(|d|d.id==t.evidence.document_id)).map(|d|d.title).unwrap_or_else(||"Source".into());view!{<div class="task-detail"><h3 class="selected-action-title">{t.title.clone()}</h3><div class="detail-heading"><span>"└ Evidence"</span><small>{if t.method=="local"{"local extraction"}else{"model suggestion"}}</small></div><blockquote>{t.evidence.quote}</blockquote><div class="detail-source"><button class="source-link" on:click=move|_|briefs::open_evidence(ev.clone(),selected,notice)>{source_name}<span>{format!("line {} · rev {}",ev.line,ev.revision)}</span></button><div class="task-actions">{candidate.then(||view!{<button class="button text" on:click=move|_|mutate(format!("/api/v1/tasks/{dismiss}"),"PATCH",json!({"status":"dismissed","expected_version":version}),snap,notice)>"Dismiss"</button>})}<button class="button compact" on:click=move|_|mutate(format!("/api/v1/tasks/{id}"),"PATCH",json!({"status":if candidate{"accepted"}else{"completed"},"expected_version":version}),snap,notice)>{if candidate{"Accept"}else{"Complete"}}</button></div></div></div>}})}
                                    </Show>
                                    <div class="table-note"><span>"? review"</span><span>"+ accepted"</span><span>"Select a row to inspect its evidence."</span></div>
                                </section>
                            </div>
                        </Show>
                        <Show when=move||view.get()=="library">
                            <div hidden=move||section.get()!="retrieval"><retrieval::Retrieval snap=snap notice=notice selected=selected/></div>
                            <div hidden=move||section.get()!="repositories"><repository::Repositories snap=snap notice=notice selected=selected/></div>
                            <div hidden=move||section.get()!="capture"><capture::Capture snap=snap notice=notice/></div>
                            <div hidden=move||section.get()!="access"><source_controls::SourceControls snap=snap notice=notice/></div>
                            <div hidden=move||section.get()!="context"><context_delivery::ContextBuilder snap=snap notice=notice/><exports::Exports snap=snap notice=notice/></div>
                            <div hidden=move||section.get()!="sources">
                            <form class="search-form" on:submit=run_search><span>"⌕"</span><input id="context-search" aria-label="Search work context" placeholder="Search sources…" prop:value=move||query.get() on:input=move|ev|query.set(event_target_value(&ev))/><button class="button compact" type="submit">"Search context"</button></form>
                            {move||hits.get().map(|results|view!{<section class="panel search-results"><div class="section-heading"><h2>{format!("{} results",results.len())}</h2><button class="source-link" on:click=move|_|hits.set(None)>"Clear search"</button></div>{results.into_iter().map(|hit|{let id=hit.document_id;view!{<button class="search-hit" on:click=move|_|read_doc(id.clone(),Some(hit.revision))><h3>{hit.title}</h3><p>{hit.excerpt}</p><small>{format!("Revision {} · source evidence",hit.revision)}</small></button>}}).collect_view()}</section>})}
                            <div class="section-heading library-heading"><h2>"Sources"</h2><span class="muted">"Select a source to inspect its content."</span></div>
                            <div class="source-table-head"><span>"Type"</span><span>"Source"</span><span>"Access / revision"</span><span>"Document"</span><span>"Analysis"</span></div><div class="source-grid library-sources">{move||snap.get().map(|s|{let sources=s.sources; s.documents.into_iter().map(|doc|{let source=sources.iter().find(|x|x.id==doc.source_id).cloned();let read=doc.id.clone();let run_id=doc.id.clone();let edit=doc.clone();let source_edit=source.clone();let analysis_connection=RwSignal::new("local".to_string());view!{<article class="panel source-card"><div class="source-card-top"><span class="document-icon">"▤"</span><span class="subtle-pill">{source.as_ref().map(|s|s.kind.to_uppercase()).unwrap_or_default()}</span></div><h3>{doc.title.clone()}</h3><p>{doc.body.lines().filter(|l|!l.is_empty()&&!l.starts_with('#')).take(2).collect::<Vec<_>>().join(" ").chars().take(170).collect::<String>()}</p><div class="source-policy"><span class="status-dot"></span>{if source.as_ref().is_some_and(|s|s.allow_external){"External allowed"}else{"Local only"}}<span>{format!("rev {}",doc.revision)}</span></div><div class="source-card-actions"><button class="source-link" on:click=move|_|read_doc(read.clone(),None)>"Open"</button><button class="source-link" on:click=move|_|{title.set(edit.title.clone());body.set(edit.body.clone());uri.set(source_edit.as_ref().map(|s|s.uri.clone()).unwrap_or_default());kind.set(source_edit.as_ref().map(|s|s.kind.clone()).unwrap_or_else(||"markdown".into()));external.set(source_edit.as_ref().is_some_and(|s|s.allow_external));importing.set(true);}>"Edit"</button></div><div class="analyze-row"><select aria-label="Analysis connection" on:change=move|ev|analysis_connection.set(event_target_value(&ev))>{move||snap.get().map(|s|s.connections.into_iter().filter(|c|c.available && c.kind!="acp").map(|c|view!{<option value=c.id.clone()>{c.id.clone()}</option>}).collect_view())}</select><button class="button text" on:click=move|_|{let connection=analysis_connection.get();let approved=connection=="local"||web_sys::window().unwrap().confirm_with_message(&format!("Use {connection} for this document? This can consume your agent subscription or API allowance. The source disclosure permission must allow it.")).unwrap_or(false);if approved{mutate(format!("/api/v1/documents/{run_id}/analyze"),"POST",json!({"connection":connection,"approve_usage":approved}),snap,notice);}}>"Analyze ↗"</button></div></article>}}).collect_view()})}</div>
                            </div>
                        </Show>
                        <Show when=move||view.get()=="memory">
                            <div hidden=move||section.get()!="review"><memory_review::MemoryReview snap=snap notice=notice/></div>
                            <div hidden=move||section.get()!="observations">
                            <div class="info-banner"><span>"◎"</span><div><strong>"Agent memory"</strong><p>"Agents can search authorized sources and contribute observations through MCP. Each connection has its own revocable access."</p></div></div>
                            <div class="overview-columns"><section class="panel"><div class="section-heading"><div><h2>"Observations"</h2><p>"Notes and discoveries, with attribution."</p></div></div>{move||snap.get().map(|s|s.observations.into_iter().map(|o|view!{<article class="observation"><span class="task-label">"OBSERVATION"</span><p>{o.body}</p><small>{format!("{} · {} source(s)",o.contributor,o.source_ids.len())}</small></article>}).collect_view())}<form class="memory-form" on:submit=move|ev|{ev.prevent_default();let source=memory_source.get();let source=if source.is_empty(){snap.get().and_then(|s|s.sources.first().map(|s|s.id.clone())).unwrap_or_default()}else{source};let request=json!({"body":memory_body.get(),"source_ids":if source.is_empty(){vec![]}else{vec![source]},"evidence":[],"idempotency_key":new_id()});if saving_memory.get_untracked(){return;}saving_memory.set(true);spawn_local(async move{let result=api("/api/v1/observations","POST",Some(request)).await;saving_memory.set(false);match result{Ok(_)=>{memory_body.set(String::new());notice.set(String::new());refresh(snap,notice)},Err(e)=>notice.set(e)}});}><label for="memory-note">"Add an observation"</label><textarea id="memory-note" required rows="4" placeholder="Observation…" prop:value=move||memory_body.get() on:input=move|ev|memory_body.set(event_target_value(&ev))></textarea><select aria-label="Observation source" on:change=move|ev|memory_source.set(event_target_value(&ev))>{move||snap.get().map(|s|s.sources.into_iter().map(|s|view!{<option value=s.id>{s.name}</option>}).collect_view())}</select><button class="button primary" type="submit" disabled=move||saving_memory.get()>{move||if saving_memory.get(){"Saving…"}else{"Save observation"}}</button></form></section><aside class="panel setup-panel"><span class="eyebrow">"CONNECT AN AGENT"</span><h2>"MCP access"</h2><p>"Create a separate grant for each agent. Choose the source IDs it can use."</p><code>"thesaurus agent add my-agent \\n  --source SOURCE_ID --write"</code><p>"The CLI returns an MCP configuration snippet. Your agent configuration is yours to apply."</p><details><summary>"Available source IDs"</summary>{move||snap.get().map(|s|s.sources.into_iter().map(|s|view!{<div class="source-id"><strong>{s.name}</strong><code>{s.id}</code></div>}).collect_view())}</details><small>"External agents can only read sources you have allowed to be shared. Use --local-agent for an agent that processes entirely on your device."</small></aside></div>
                            </div>
                        </Show>
                        <Show when=move||view.get()=="briefs"><briefs::Briefs snap=snap notice=notice selected=selected view=view/></Show>
                        <Show when=move||view.get()=="threads"><div hidden=move||section.get()!="threads"><threads::Threads snap=snap notice=notice selected=selected view=view/></div><div hidden=move||section.get()!="handoffs"><context_delivery::HandoffReceipts snap=snap notice=notice/></div></Show>
                        <Show when=move||view.get()=="companion"><live_meetings::LiveMeetings snap=snap notice=notice selected=selected/></Show>
                        <Show when=move||view.get()=="meetings"><div hidden=move||section.get()!="live"><live_meetings::LiveMeetings snap=snap notice=notice selected=selected/></div><div hidden=move||section.get()!="preparation"><meetings::Meetings snap=snap notice=notice selected=selected/></div></Show>
                        <Show when=move||view.get()=="focus"><focus::Focus snap=snap notice=notice selected=selected/></Show>
                        <Show when=move||view.get()=="recipes"><recipes::Recipes snap=snap notice=notice view=view/></Show>
                        <Show when=move||view.get()=="evaluations"><evaluations::Evaluations snap=snap notice=notice/></Show>
                        <Show when=move||view.get()=="settings"><settings::Settings snap=snap notice=notice section=section view=view theme_open=theme_open/></Show>
                        <Show when=move||view.get()=="plugins"><plugins::Plugins snap=snap notice=notice view=view section=section/></Show>
                        <Show when=move||view.get()=="bragbook"><bragbook::Bragbook snap=snap notice=notice section=section/></Show>
                        <Show when=move||view.get()=="connections">
                            <div hidden=move||section.get()!="usage"><usage_controls::UsageControls snap=snap notice=notice view=view/></div>
                            <div hidden=move||section.get()!="agent"><RuntimeSetup snap=snap notice=notice view=view/></div>
                            <div hidden=move||section.get()!="models">
                            <button class="button secondary" on:click=move|_|onboarding::repair(1,"context")>"Set up or repair a model →"</button><div class="info-banner"><span>"⌘"</span><div><strong>"Model runtimes"</strong><p>"Add an existing agent, API account or local model through Setup. Nothing runs without your request."</p></div></div>
                            <div class="source-grid">{move||snap.get().map(|s|s.connections.into_iter().map(|c|view!{<article class="panel connection-card"><span class="document-icon">"⌘"</span><h3>{c.id}</h3><span class="connection-state">{if c.available{"Available"}else{"Needs setup"}}</span><p>{c.detail}</p><dl><dt>"Model"</dt><dd>{c.model}</dd><dt>"Usage"</dt><dd>{c.billing}</dd></dl></article>}).collect_view())}</div><details class="panel config-guide"><summary>"Advanced configuration"</summary><p>"Use Setup to add a connection immediately. Advanced config.toml connections are also supported; file edits require a service restart. API keys stay in Keychain or the named environment variable."</p><div class="config-examples"><pre>"[[connections]]\nid = \"my-claude\"\nkind = \"claude_cli\"\ndaily_limit = 10"</pre><pre>"[[connections]]\nid = \"local-model\"\nkind = \"openai_compatible\"\nendpoint = \"http://127.0.0.1:11434/v1\"\nmodel = \"your-installed-model\"\ndaily_limit = 20"</pre></div><p class="muted">"Agent accounts manage their own billing and authentication. A connected executable does not prove model access. Subscription access follows the provider's supported integration rules."</p></details>
                            </div>
                        </Show>
                        <Show when=move||view.get()=="activity">
                            <div hidden=move||section.get()!="actions"><actions::Actions snap=snap notice=notice/></div>
                            <div hidden=move||section.get()!="notifications"><source_controls::WorkNotifications snap=snap notice=notice/></div>
                            <div hidden=move||section.get()!="runs"><RuntimeHistory snap=snap notice=notice/></div>
                            <div hidden=move||section.get()!="processing">
                            <section class="panel"><div class="section-heading"><h2>"Processing history"</h2><span class="subtle-pill">"PERSISTENT JOBS"</span></div>{move||snap.get().map(|s|{let documents=s.documents;s.jobs.into_iter().map(|j|{let name=documents.iter().find(|d|d.id==j.document_id).map(|d|d.title.clone()).unwrap_or(j.document_id);view!{<article class="job-row"><span class="job-status">{j.status}</span><div><h3>{name}</h3><p>{if j.summary.is_empty(){"Waiting for extraction".into()}else{j.summary}}</p><small>{format!("{} · revision {} · attempt {}",j.connection,j.revision,j.attempt)}</small></div></article>}}).collect_view()})}</section></div><div hidden=move||section.get()!="completed"><section class="panel completed-panel"><h2>"Completed actions"</h2>{move||snap.get().map(|s|s.tasks.into_iter().filter(|t|t.status=="completed").map(|t|{let id=t.id;view!{<div class="completed-row"><span>"✓ "{t.title}</span><button class="source-link" on:click=move|_|mutate(format!("/api/v1/tasks/{id}"),"PATCH",json!({"status":"accepted","expected_version":t.version}),snap,notice)>"Reopen"</button></div>}}).collect_view())}</section>
                            </div>
                        </Show>
                        </div>
                    </Show>
                </main>
            </div>
            </div>
            <footer class="keybar"><button on:click=open_import><kbd>"n"</kbd>"Capture"</button><button on:click=move|_|{view.set("library".into());section.set("sources".into());}><kbd>"/"</kbd>"Search"</button><Show when=move||view.get()=="now"><span><kbd>"j/k"</kbd>"Select"</span><span><kbd>"enter"</kbd>"Source"</span></Show><button on:click=move|_|help.set(true)><kbd>"?"</kbd>"Keys"</button><span class="keybar-status">{move||snap.get().map(|s|format!("local · {} sources",s.sources.len())).unwrap_or_else(||"unpaired".into())}</span></footer>
            <themes::ThemeSettings open=theme_open/>
            <Show when=move||help.get()><div class="modal-overlay"><section class="modal keys-modal" role="dialog" aria-modal="true" aria-labelledby="keys-title"><div class="modal-heading"><h2 id="keys-title">"Keys"</h2><button class="close-button" aria-label="Close keyboard shortcuts" on:click=move|_|help.set(false)>"×"</button></div><dl class="key-list"><dt>"1–9 / 0"</dt><dd>"Switch tabs"</dd><dt>"n"</dt><dd>"Capture context"</dd><dt>"/"</dt><dd>"Search sources"</dd><dt>"j / k"</dt><dd>"Select next / previous action"</dd><dt>"enter"</dt><dd>"Open selected action's source"</dd><dt>"esc"</dt><dd>"Close a dialog"</dd></dl></section></div></Show>
            <Show when=move||importing.get()><div class="modal-overlay"><section class="modal" role="dialog" aria-modal="true" aria-labelledby="import-title"><div class="modal-heading"><div><span class="eyebrow">"Capture"</span><h2 id="import-title">"Add context"</h2></div><button class="close-button" aria-label="Close import" on:click=move|_|importing.set(false)>"×"</button></div><form on:submit=submit_import><label for="capture-title">"Title"</label><input id="capture-title" required maxlength="300" placeholder="A meeting, a note, a piece of knowledge" prop:value=move||title.get() on:input=move|ev|title.set(event_target_value(&ev))/><label for="capture-kind">"Type of context"</label><select id="capture-kind" prop:value=move||kind.get() on:change=move|ev|kind.set(event_target_value(&ev))><option value="transcript">"Meeting transcript"</option><option value="markdown">"Work note"</option><option value="knowledge">"Company knowledge"</option><option value="repository">"Repository context"</option></select><label for="capture-uri">"Source link or stable reference (optional)"</label><input id="capture-uri" placeholder="https://… or a stable source reference" prop:value=move||uri.get() on:input=move|ev|uri.set(event_target_value(&ev))/><label for="capture-body">"Content"</label><textarea id="capture-body" required rows="10" maxlength="200000" placeholder="Paste the transcript or notes. Existing references are preserved." prop:value=move||body.get() on:input=move|ev|body.set(event_target_value(&ev))></textarea><label class="checkbox-label"><input type="checkbox" prop:checked=move||external.get() on:change=move|ev|external.set(event_target_checked(&ev))/><span><strong>"Allow this source in external agents and models"</strong><small>"Off by default. Local extraction works without sharing."</small></span></label><div class="modal-footer"><button class="button text" type="button" on:click=move|_|importing.set(false)>"Cancel"</button><button class="button primary" type="submit" disabled=move||saving.get()>{move||if saving.get(){"Saving…"}else{"Save context"}}</button></div></form></section></div></Show>
            {move||selected.get().map(|doc|view!{<div class="modal-overlay"><section class="modal document-modal" role="dialog" aria-modal="true" aria-labelledby="document-title"><div class="modal-heading"><div><span class="eyebrow">{format!("SOURCE EVIDENCE · REVISION {}",doc.revision)}</span><h2 id="document-title">{doc.title.clone()}</h2></div><button class="close-button" aria-label="Close source" on:click=move|_|selected.set(None)>"×"</button></div><div class="document-body" data-evidence-document=doc.id.clone() data-evidence-revision=doc.revision.to_string()>{doc.body.lines().enumerate().map(|(i,l)|{let doc_id=doc.id.clone();let revision=doc.revision;view!{<div class="document-line" id=format!("evidence-line-{}",i+1) tabindex="-1" class:cited=move||cited.get().is_some_and(|target|target.document_id==doc_id&&target.revision==revision&&target.line==(i+1) as i64) class:annotation-anchor=move||annotation_line.get()==(i+1).to_string() on:click=move|_|annotation_line.set((i+1).to_string())><span>{i+1}</span><pre>{l.to_string()}</pre></div>}}).collect_view()}</div><annotations::DocumentAnnotations document=doc.clone() line=annotation_line snap=snap notice=notice selected=selected/><div class="modal-footer"><small>{format!("{} · immutable revision {}",doc.id,doc.revision)}</small><button class="button secondary" on:click=move|_|selected.set(None)>"Done"</button></div></section></div>})}
        </div>
    }
}

#[component]
fn RuntimeSetup(snap: Snap, notice: RwSignal<String>, view: RwSignal<String>) -> impl IntoView {
    let connection = RwSignal::new(String::new());
    let prompt = RwSignal::new(String::new());
    let sources = RwSignal::new(Vec::<String>::new());
    let approved = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let probe = RwSignal::new(String::new());
    let last_request = RwSignal::new(None::<RuntimeRequest>);
    let chosen = move || {
        let selected = connection.get();
        if !selected.is_empty() {
            selected
        } else {
            snap.get()
                .and_then(|s| {
                    s.connections
                        .into_iter()
                        .find(|c| c.kind == "acp" && c.available)
                })
                .map(|c| c.id)
                .unwrap_or_default()
        }
    };
    let start = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        let selected = chosen();
        if selected.is_empty() || sources.get_untracked().is_empty() || !approved.get_untracked() {
            notice
                .set("Choose an ACP connection, select sources, and approve account usage.".into());
            return;
        }
        let request = last_request
            .get_untracked()
            .filter(|r| {
                r.connection == selected
                    && r.prompt == prompt.get_untracked()
                    && r.source_ids == sources.get_untracked()
            })
            .unwrap_or_else(|| RuntimeRequest {
                connection: selected,
                prompt: prompt.get_untracked(),
                source_ids: sources.get_untracked(),
                approve_usage: true,
                idempotency_key: new_id(),
            });
        last_request.set(Some(request.clone()));
        busy.set(true);
        spawn_local(async move {
            let result = api(
                "/api/v1/runtime/runs",
                "POST",
                Some(serde_json::to_value(request).unwrap()),
            )
            .await;
            busy.set(false);
            match result {
                Ok(_) => {
                    last_request.set(None);
                    approved.set(false);
                    notice.set(String::new());
                    view.set("activity".into());
                    refresh(snap, notice);
                }
                Err(e) => notice.set(e),
            }
        });
    };
    view! {
        <section class="panel runtime-setup"><div class="section-heading"><h2>"ACP agent"</h2><span class="muted">"Read-only memory"</span></div>
            <Show when=move||snap.get().is_some_and(|s|s.connections.iter().any(|c|c.kind=="acp" && c.available)) fallback=move||view!{<p class="muted">"Configure an ACP connection to run an existing agent with selected work memory. See docs/acp-integration.md for setup."</p><pre class="runtime-install">"./scripts/install-acp.sh"</pre>}>
                <form on:submit=start>
                    <div class="runtime-connection"><label for="runtime-connection">"Connection"</label><select id="runtime-connection" prop:value=move||chosen() on:change=move|ev|{connection.set(event_target_value(&ev));probe.set(String::new());}>{move||snap.get().map(|s|s.connections.into_iter().filter(|c|c.kind=="acp" && c.available).map(|c|view!{<option value=c.id.clone()>{c.id.clone()}</option>}).collect_view())}</select>
                        <button class="button secondary" type="button" disabled=move||busy.get() on:click=move|_|{let id=chosen();busy.set(true);spawn_local(async move{let result=api(&format!("/api/v1/runtime/connections/{id}/probe"),"POST",Some(json!({}))).await;busy.set(false);match result{Ok(v)=>probe.set(serde_json::to_string_pretty(&v).unwrap_or_default()),Err(e)=>notice.set(e)}});}>"Probe · no model usage"</button>
                    </div>
                    <Show when=move||!probe.get().is_empty()><pre class="runtime-probe">{move||probe.get()}</pre></Show>
                    <label for="runtime-prompt">"Request"</label><textarea id="runtime-prompt" required maxlength="16000" rows="3" placeholder="Summarize open commitments in the selected sources. Cite the evidence." prop:value=move||prompt.get() on:input=move|ev|prompt.set(event_target_value(&ev))></textarea>
                    <fieldset class="runtime-sources"><legend>"Memory sources"</legend>{move||snap.get().map(|s|s.sources.into_iter().map(|source|{let id=source.id.clone();let checked=source.id;view!{<label><input type="checkbox" disabled=!source.allow_external prop:checked=move||sources.get().contains(&checked) on:change=move|ev|{let checked=event_target_checked(&ev);sources.update(|list|{list.retain(|x|x!=&id);if checked{list.push(id.clone());}});}/><span>{source.name}</span>{(!source.allow_external).then(||view!{<small>"local only · enable sharing in Sources"</small>})}</label>}}).collect_view())}</fieldset>
                    <label class="checkbox-label"><input type="checkbox" required prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/><span><strong>"Use this runtime's account allowance for this request"</strong><small>"The request and selected memory may be sent to its model provider. No automatic fallback or retry. Tools that request permission wait for your decision in Activity."</small></span></label>
                    <button class="button primary" type="submit" disabled=move||busy.get()>{move||if busy.get(){"Starting…"}else{"Start agent run"}}</button>
                </form>
            </Show>
        </section>
    }
}
#[component]
fn RuntimeHistory(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    view! {
        <section class="panel runtime-history"><div class="section-heading"><h2>"Agent runs"</h2><span class="muted">"ACP"</span></div><context_delivery::RunObserver snap=snap notice=notice/>
            <Show when=move||snap.get().is_some_and(|s|!s.runtime_runs.is_empty()) fallback=move||view!{<p class="muted">"No agent runs yet. Start one from Connections."</p>}>
                {move||snap.get().map(|s|s.runtime_runs.into_iter().map(|run|{
                    let cancel=run.id.clone();let permission_run=run.id.clone();let active=matches!(run.status.as_str(),"starting"|"running"|"waiting_permission"|"cancelling");
                    view!{<article class="runtime-run"><div class="runtime-topline"><span class="runtime-status">{run.status.clone()}</span><strong>{run.connection}</strong><span class="muted">{format!("{} source(s)",run.source_ids.len())}</span>{active.then(||view!{<button class="button text" disabled=run.status=="cancelling" on:click=move|_|mutate(format!("/api/v1/runtime/runs/{cancel}/cancel"),"POST",json!({}),snap,notice)>"Cancel"</button>})}</div><p class="runtime-request">{run.prompt}</p><div class="muted">{run.progress}</div>
                        {run.permissions.into_iter().filter(|p|p.decision.is_none()).map(|p|{let run_id=permission_run.clone();let pid=p.id.clone();view!{<section class="runtime-permission"><strong>"Permission requested: "{p.title}</strong><small>{p.kind}</small><pre>{p.input}</pre><div>{p.options.into_iter().map(|option|{let path=format!("/api/v1/runtime/runs/{run_id}/permissions/{pid}");view!{<button class="button compact" on:click=move|_|mutate(path.clone(),"POST",json!({"option_id":option.id}),snap,notice)>{option.label}</button>}}).collect_view()}<button class="button text" on:click=move|_|mutate(format!("/api/v1/runtime/runs/{run_id}/cancel"),"POST",json!({}),snap,notice)>"Cancel run"</button></div></section>}}).collect_view()}
                        {run.error.map(|error|view!{<p class="runtime-error" role="alert">{error}</p>})}
                        {(!run.output.is_empty()).then(||view!{<pre class="runtime-output">{run.output}</pre>})}
                        <details class="runtime-receipt"><summary>"Run details"</summary><code>{run.id}</code>{run.session_id.map(|id|view!{<code>{format!("Session: {id}")}</code>})}<small>{format!("Started {} · {}",run.created_at,run.stop_reason.unwrap_or_else(||"No stop reason yet".into()))}</small></details>
                    </article>}
                }).collect_view())}
            </Show>
        </section>
    }
}

#[component]
fn Root() -> impl IntoView {
    if tray_popover::is_tray() {
        view! {<tray_popover::TrayPopover/>}.into_any()
    } else {
        view! {<App/>}.into_any()
    }
}
fn main() {
    console_error_panic_hook::set_once();
    themes::initialize();
    leptos::mount::mount_to_body(Root);
}
