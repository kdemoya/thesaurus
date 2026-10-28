use super::dot_type::DotText;
use super::*;
mod audio;
mod memory;
mod model;
#[derive(Clone, Copy)]
pub(super) struct SetupDraftGuard(pub RwSignal<std::collections::BTreeMap<&'static str, bool>>);
#[derive(Clone, Copy)]
pub(super) struct SetupTarget {
    pub step: u8,
    pub goal: &'static str,
}
#[derive(Clone, Copy)]
pub(super) struct SetupNavigation(pub Callback<SetupTarget>);
pub(super) fn repair(step: u8, goal: &'static str) {
    if let Some(n) = use_context::<SetupNavigation>() {
        n.0.run(SetupTarget { step, goal });
    }
}
const STEPS: [(&str, &str); 5] = [
    ("HELLO", "Choose your starting point"),
    ("LINK", "Choose your main model"),
    ("TOOLS", "Prepare what you need"),
    ("PROOF", "Get a first useful result"),
    ("READY", "Make room for your work"),
];
#[wasm_bindgen(
    inline_js = "export function focusSetup(){requestAnimationFrame(()=>document.getElementById('onboard-heading')?.focus({preventScroll:true}));} export async function copySetup(text){await navigator.clipboard.writeText(text);}"
)]
extern "C" {
    #[wasm_bindgen(js_name=focusSetup)]
    fn focus_heading();
    #[wasm_bindgen(catch,js_name=copySetup)]
    async fn copy_setup(text: &str) -> Result<(), JsValue>;
}
pub(super) async fn initial_entry(view: RwSignal<String>, notice: RwSignal<String>) {
    match api("/api/v1/onboarding", "GET", None).await {
        Ok(v) => {
            if let Ok(s) = serde_json::from_value::<OnboardingStatus>(v) {
                if ((!s.existing_workspace && s.progress.status == "new")
                    || s.progress.status == "active")
                    && !super::live_meetings::is_companion()
                    && view.get_untracked() == "now"
                {
                    view.set("onboarding".into());
                }
            }
        }
        Err(e) => {
            if !e.contains("could not be completed") {
                notice.set(format!("Setup is unavailable: {e}"));
            }
        }
    }
}
#[component]
pub(super) fn Onboarding(
    snap: Snap,
    notice: RwSignal<String>,
    view: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    target: RwSignal<Option<SetupTarget>>,
) -> impl IntoView {
    let status = RwSignal::new(None::<OnboardingStatus>);
    let draft = RwSignal::new(OnboardingProgress::default());
    let loaded = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let example = RwSignal::new(None::<Document>);
    let note = RwSignal::new(String::new());
    let example_uri = StoredValue::new(format!("onboarding://note/{}", new_id()));
    let dirty = RwSignal::new(std::collections::BTreeMap::<&'static str, bool>::new());
    provide_context(SetupDraftGuard(dirty));
    let epoch = RwSignal::new(0u64);
    let fetching = RwSignal::new(false);
    let reload = move || {
        if fetching.get_untracked() {
            return;
        }
        fetching.set(true);
        epoch.update(|n| *n += 1);
        let ticket = epoch.get_untracked();
        spawn_local(async move {
            let r = api("/api/v1/onboarding", "GET", None).await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            fetching.set(false);
            match r {
                Ok(v) => match serde_json::from_value::<OnboardingStatus>(v) {
                    Ok(s) => {
                        if !loaded.get_untracked() {
                            let mut p = s.progress.clone();
                            if let Some(t) = target.get_untracked() {
                                p.step = t.step;
                                p.goal = t.goal.into();
                                target.set(None);
                            }
                            draft.set(p);
                            loaded.set(true);
                        }
                        status.set(Some(s));
                    }
                    Err(_) => error.set("Unable to read setup status".into()),
                },
                Err(e) => error.set(e),
            }
        });
    };
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        reload();
    });
    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5000).await;
            if loaded.try_get_untracked().is_none() {
                break;
            }
            reload();
        }
    });
    let step = Memo::new(move |_| draft.get().step);
    let dot_heading = Memo::new(move |_| {
        if step.get() == 4
            && status.get().is_some_and(|s| {
                s.readiness.iter().any(|r| {
                    (r.capability == draft.get().goal
                        || r.capability == "audio" && draft.get().goal == "record")
                        && r.state == "needs_setup"
                })
            })
        {
            "CHECK".to_string()
        } else {
            STEPS[step.get().min(4) as usize].0.to_string()
        }
    });
    Effect::new(move |_| {
        if loaded.get() {
            let _ = step.get();
            focus_heading();
        }
    });
    Effect::new(move |_| {
        if loaded.get() {
            if let Some(t) = target.get() {
                draft.update(|p| {
                    p.step = t.step;
                    p.goal = t.goal.into();
                });
                target.set(None);
            }
        }
    });
    let save = move |step: u8, state: &'static str, destination: &'static str| {
        if busy.get_untracked() || !loaded.get_untracked() {
            return;
        }
        if dirty.get_untracked().values().any(|v| *v) {
            error.set("Save your changes or cancel this edit before moving to another step. Your draft is still here.".into());
            return;
        }
        let p = draft.get_untracked();
        busy.set(true);
        error.set(String::new());
        spawn_local(async move {
            let r=api("/api/v1/onboarding","PUT",Some(json!({"expected_version":p.version,"goal":p.goal,"step":step,"primary_connection":p.primary_connection,"status":state,"memory_grant_id":p.memory_grant_id,"idempotency_key":new_id()}))).await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match r {
                Ok(v) => {
                    if let Ok(p) = serde_json::from_value::<OnboardingProgress>(v) {
                        let goal = p.goal.clone();
                        draft.set(p);
                        reload();
                        refresh(snap, notice);
                        if !destination.is_empty() {
                            view.set(
                                if destination == "exit" {
                                    if state == "complete" {
                                        match goal.as_str() {
                                            "record" => "meetings",
                                            "memory" => "memory",
                                            _ => "now",
                                        }
                                    } else {
                                        "now"
                                    }
                                } else {
                                    destination
                                }
                                .into(),
                            );
                        }
                    }
                }
                Err(e) => error.set(e),
            }
        });
    };
    let import_note = move |sample: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let request = if sample {
            None
        } else {
            Some(
                json!({"source_name":"My first note","title":"My first note","body":note.get_untracked(),"kind":"markdown","uri":example_uri.get_value(),"allow_external":false}),
            )
        };
        spawn_local(async move {
            let r = api(
                if sample {
                    "/api/v1/onboarding/example"
                } else {
                    "/api/v1/captures"
                },
                "POST",
                request.or(Some(json!({}))),
            )
            .await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match r {
                Ok(v) => {
                    if let Ok(d) = serde_json::from_value::<Document>(v["document"].clone()) {
                        example.set(Some(d));
                        refresh(snap, notice);
                    }
                }
                Err(e) => error.set(e),
            }
        });
    };
    view! {<section class="onboard" aria-label="Workspace setup">
        <div class="onboard-topline"><span>"WORKSPACE SETUP"</span><span>{move||format!("{:02} / 05",draft.get().step+1)}</span><span>"saved on this workspace"</span></div>
        <Show when=move||loaded.get() fallback=move||view!{<p role="status">"Reading your existing setup…"</p>}>
        <div class="onboard-layout">
        <aside class="onboard-rail"><div class="onboard-number">{move||view!{<DotText text=format!("{:02}",step.get()+1) grid=true/>}}</div>
            <ol class="onboard-steps">{[("Start",0u8),("Connect",1),("Prepare",2),("Try it",3),("Finish",4)].into_iter().map(|(label,n)|view!{<li class:current=move||draft.get().step==n><button type="button" aria-current=move||if draft.get().step==n{"step"}else{"false"} disabled=move||busy.get() on:click=move|_|save(n,"active","")><span>{format!("{:02}",n+1)}</span>{label}<span class="onboard-step-mark">{move||if draft.get().step==n{"←"}else{"·"}}</span></button></li>}).collect_view()}</ol>
            <p class="onboard-rail-note">"Your workspace, at your pace."<br/>"Skip optional setup. Return whenever you need."</p>
        </aside>
        <div class="onboard-content">
            <header class="onboard-heading"><h1 id="onboard-heading" tabindex="-1"><span class="sr-only">{move||STEPS[draft.get().step.min(4) as usize].1}</span>{move||view!{<DotText text=dot_heading.get()/>}}</h1><p>{move||STEPS[draft.get().step.min(4) as usize].1}</p></header>
            <Show when=move||!error.get().is_empty()><p class="onboard-error" role="alert">{move||error.get()}</p></Show>
            <Show when=move||status.get().is_some_and(|s|s.progress.version>draft.get().version)><div class="onboard-error"><p>"Setup changed in another window. Your draft is still here."</p><button type="button" class="button secondary" on:click=move|_|{if let Some(s)=status.get_untracked(){draft.set(s.progress);error.set(String::new());}}>"Reload saved progress"</button></div></Show>
            <Show when=move||draft.get().step==0><div class="onboard-scene"><p class="onboard-lede">"Start with one useful thing."</p><p class="muted">"Choose what you want Thesaurus to help with first. Everything else can wait."</p>
                <div class="onboard-goals" role="group" aria-label="Starting goal">{[("record","01","Capture a meeting","Record now. Organize the meeting afterward."),("context","02","Make sense of work","Bring a note or transcript. Find the next action."),("memory","03","Connect your agent","Give an existing agent a scoped view of your work.")].into_iter().map(|(goal,n,title,detail)|view!{<button type="button" class="onboard-goal" class:chosen=move||draft.get().goal==goal aria-pressed=move||draft.get().goal==goal on:click=move|_|draft.update(|p|p.goal=goal.into())><span class="onboard-goal-index">{n}</span><span><strong>{title}</strong><small>{detail}</small></span><span>{move||if draft.get().goal==goal{"●"}else{"○"}}</span></button>}).collect_view()}</div>
                <p class="onboard-note">"Work stays local until you explicitly choose a destination. Setup never starts a recording or a model request."</p>
            </div></Show>
            <Show when=move||draft.get().step==1><model::ModelSetup status=status draft=draft reload=Callback::new(move|_|reload()) snap=snap error=error/></Show>
            <Show when=move||draft.get().step==2><div class="onboard-scene">
                <Show when=move||draft.get().goal=="record"><audio::AudioSetup error=error reload=Callback::new(move|_|reload()) snap=snap notice=notice/></Show>
                <Show when=move||draft.get().goal=="memory"><memory::MemorySetup snap=snap draft=draft error=error reload=Callback::new(move|_|reload())/></Show>
                <Show when=move||draft.get().goal=="context"><p class="onboard-lede">"A little context goes a long way."</p><p class="muted">"Paste a note in the next step, or use a small example. Local commitment extraction works without a model account."</p><dl class="onboard-facts"><dt>"Work sources"</dt><dd>{move||snap.get().map(|s|s.sources.len()).unwrap_or(0)}</dd><dt>"Primary connection"</dt><dd>{move||draft.get().primary_connection}</dd><dt>"External sharing"</dt><dd>"Off for new context"</dd></dl></Show>
                <details class="onboard-plugin-setup"><summary>"Additional capability readiness"</summary><p class="muted">"These steps come from installed plugins. Optional capabilities can be prepared later."</p>{move||status.get().map(|s|s.plugin_setup.into_iter().map(|r|{let title=r.step.title.clone();view!{<div class="onboard-check"><strong>{format!("{} / {}",r.plugin_id,title)}</strong><span>{format!("{:?}",r.state).to_lowercase()}</span><p>{r.detail}</p><button type="button" class="source-link" on:click=move|_|{save(2,"active","plugins");}>"Open plugin setup →"</button></div>}}).collect_view())}</details>
            </div></Show>
            <Show when=move||draft.get().step==3><div class="onboard-scene">
                <Show when=move||draft.get().goal=="record"><p class="onboard-lede">"Your first recording starts with one click."</p><p class="muted">"The meeting companion uses your saved inputs and transcription choice. After you stop recording, your saved model automatically summarizes the transcript and notes. Start recording to request any missing OS permissions."</p><button type="button" class="button primary" on:click=move|_|{save(3,"active","meetings");}>"Open recording companion →"</button></Show>
                <Show when=move||draft.get().goal!="record"><p class="onboard-lede">"See a commitment become an action."</p><p class="muted">"This preview uses a tiny synthetic meeting. Importing it creates one local source; no model account is used."</p><blockquote class="onboard-example">"Maya: I'll share the pilot checklist."<br/>"Action: Review the deployment notes."<br/>"Decision: Start with a staged rollout."</blockquote><div class="onboard-preview"><span>"LOCAL PREVIEW / LINE 1"</span><p>"Share the pilot checklist"</p><small>"Owner: Maya · review before accepting"</small></div><button type="button" class="button secondary" disabled=move||busy.get() on:click=move|_|import_note(true)>"Import this example"</button><details><summary>"Use my own note"</summary><label for="onboard-note">"Work note · stays local"</label><textarea id="onboard-note" rows="5" maxlength="200000" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))/><button type="button" class="button secondary" disabled=move||busy.get()||note.get().trim().is_empty() on:click=move|_|import_note(false)>"Save and extract locally"</button></details>
                    {move||example.get().map(|d|{let shown=d.clone();view!{<div class="onboard-result" role="status"><p>"Context saved. Local extraction is queued; inspect its cited suggestions in Work."</p>{move||snap.get().map(|s|s.tasks.into_iter().filter(|t|t.evidence.document_id==d.id).map(|t|view!{<div class="onboard-proof-row"><p>{t.title}</p><blockquote>{t.evidence.quote.clone()}</blockquote><button type="button" class="source-link" on:click=move|_|navigate_evidence(t.evidence.clone(),notice)>"Open cited passage →"</button></div>}).collect_view())}<button class="source-link" type="button" on:click=move|_|selected.set(Some(shown.clone()))>"Read the source →"</button></div>}})}
                </Show>
            </div></Show>
            <Show when=move||draft.get().step==4><div class="onboard-scene"><p class="onboard-lede">"A starting point. Room to grow."</p><p class="muted">"Your progress is saved across desktop and browser. Meeting summaries run automatically with your saved model once transcription finishes. You can change automation, models and recording preferences in Settings."</p>{move||status.get().map(|s|s.readiness.into_iter().filter(|r|r.capability==draft.get().goal||r.capability=="audio"&&draft.get().goal=="record").map(|r|view!{<div class="onboard-check"><strong>{r.capability}</strong><span>{r.state}</span><p>{r.detail}</p><small>{format!("Checked {}",r.checked_at)}</small></div>}).collect_view())}<p class="onboard-note">{move||format!("Primary connection: {}. Model access is only verified by a successful explicit check. Optional setup: {}.",draft.get().primary_connection,match draft.get().goal.as_str(){"record"=>"sources and shared memory","memory"=>"recording and extra models",_=>"recording and shared memory"})}</p></div></Show>
            <footer class="onboard-actions"><button type="button" class="button text" disabled=move||busy.get()||draft.get().step==0 on:click=move|_|save(draft.get_untracked().step.saturating_sub(1),"active","")>"← Back"</button><button type="button" class="button text" disabled=move||busy.get() on:click=move|_|save(draft.get_untracked().step,"deferred","exit")>"Set up later"</button><button type="button" class="button primary" disabled=move||busy.get() on:click=move|_|{let step=draft.get_untracked().step;if step<4{save(step+1,"active","")}else{save(4,"complete","exit")}}>{move||if busy.get(){"Saving…"}else if draft.get().step==4{"Open my workspace →"}else{"Continue →"}}</button></footer>
        </div></div></Show>
        <Show when=move||!loaded.get()&&!error.get().is_empty()><p role="alert">{move||error.get()}</p><button class="button secondary" type="button" on:click=move|_|reload()>"Retry setup"</button></Show>
    </section>}
}
