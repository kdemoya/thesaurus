use super::workflows::{Citations, chosen_project};
use super::*;

pub(super) fn utc_day() -> String {
    js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default()
        .chars()
        .take(10)
        .collect()
}
pub(super) fn plus_days(day: &str, days: i64) -> Option<String> {
    let date = js_sys::Date::new(&JsValue::from_str(&format!("{day}T00:00:00Z")));
    let time = date.get_time();
    if !time.is_finite() {
        return None;
    }
    js_sys::Date::new(&JsValue::from_f64(time + days as f64 * 86400000.0))
        .to_iso_string()
        .as_string()
        .map(|s| s.chars().take(10).collect())
}
#[component]
pub(super) fn Reference(
    reference: EvidenceReference,
    selected: RwSignal<Option<Document>>,
    notice: RwSignal<String>,
) -> impl IntoView {
    if let Some(quote) = reference.quote {
        view!{<Citations evidence=vec![Evidence{document_id:reference.document_id,revision:reference.revision,line:reference.line,quote}] selected=selected notice=notice/>}.into_any()
    } else {
        view!{<p class="muted">"Long quote omitted from summary."</p><button class="source-link" on:click=move|_|super::briefs::open_evidence(Evidence{document_id:reference.document_id.clone(),revision:reference.revision,line:reference.line,quote:String::new()},selected,notice)>{format!("Open r{} · line {}",reference.revision,reference.line)}</button>}.into_any()
    }
}
#[component]
pub(super) fn Focus(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    let project = RwSignal::new(String::new());
    let mode = RwSignal::new("priorities".to_string());
    let tick = RwSignal::new(0u64);
    let waiting_id = RwSignal::new(String::new());
    let initial_task = RwSignal::new(String::new());
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let cleanup = alive.clone();
    on_cleanup(move || cleanup.store(false, std::sync::atomic::Ordering::Relaxed));
    spawn_local(async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(60000).await;
            if !alive.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            tick.update(|n| *n += 1);
        }
    });
    Effect::new(move |_| {
        if project.get().is_empty() {
            if let Some(id) = snap
                .get()
                .and_then(|s| s.briefs.first().map(|p| p.id.clone()))
            {
                project.set(id);
            }
        }
    });
    view! {
        <div class="focus-toolbar"><label for="focus-project">"Project"</label><select id="focus-project" prop:value=move||chosen_project(snap,project).map(|p|p.id).unwrap_or_default() on:change=move|ev|{project.set(event_target_value(&ev));waiting_id.set(String::new());initial_task.set(String::new());}>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select>
            <div class="focus-tabs"><button class:active=move||mode.get()=="priorities" on:click=move|_|mode.set("priorities".into())>"Priorities"</button><button class:active=move||mode.get()=="waiting" on:click=move|_|mode.set("waiting".into())>"Waiting for"</button><button class:active=move||mode.get()=="delivery" on:click=move|_|mode.set("delivery".into())>"Delivery"</button><button class:active=move||mode.get()=="discovery" on:click=move|_|mode.set("discovery".into())>"Questions & gaps"</button><button class:active=move||mode.get()=="knowledge" on:click=move|_|mode.set("knowledge".into())>"Knowledge"</button><button class:active=move||mode.get()=="insights" on:click=move|_|mode.set("insights".into())>"Insights"</button><button class:active=move||mode.get()=="procedures" on:click=move|_|mode.set("procedures".into())>"Procedures"</button></div>
            <button class="button text" on:click=move|_|tick.update(|n|*n+=1)>"Refresh · local"</button><small class="muted">"Dates use UTC · updates each minute"</small>
        </div>
        <Show when=move||chosen_project(snap,project).is_none()><section class="panel"><div class="section-heading"><h2>"Project focus"</h2></div><p class="muted">"Create a project in Briefs and accept actions from its sources to start a priority review. Waiting requests can also be captured directly from source evidence."</p></section></Show>
        <Show when=move||chosen_project(snap,project).is_some() && mode.get()=="priorities"><super::priorities::Priorities snap=snap notice=notice selected=selected project=project tick=tick mode=mode waiting_id=waiting_id initial_task=initial_task/></Show>
        <Show when=move||chosen_project(snap,project).is_some() && mode.get()=="waiting"><super::waiting::Waiting snap=snap notice=notice selected=selected project=project tick=tick waiting_id=waiting_id initial_task=initial_task/></Show>
        <Show when=move||chosen_project(snap,project).is_some() && mode.get()=="delivery"><super::delivery::Delivery snap=snap notice=notice selected=selected project=project tick=tick/></Show>
        <Show when=move||chosen_project(snap,project).is_some() && mode.get()=="discovery"><super::discovery::Discovery snap=snap notice=notice selected=selected project=project tick=tick/></Show>
        <Show when=move||chosen_project(snap,project).is_some() && mode.get()=="knowledge"><super::knowledge::Knowledge snap=snap notice=notice selected=selected project=project tick=tick/></Show>
        <Show when=move||chosen_project(snap,project).is_some() && mode.get()=="insights"><super::insights::Insights snap=snap notice=notice/></Show>
        <Show when=move||chosen_project(snap,project).is_some() && mode.get()=="procedures"><super::procedures::Procedures snap=snap notice=notice project=project tick=tick/></Show>
    }
}
