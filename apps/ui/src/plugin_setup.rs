use super::workflows::{Pending, action};
use super::*;

fn label(state: &PluginSetupState) -> &'static str {
    match state {
        PluginSetupState::Ready => "ready",
        PluginSetupState::NeedsSetup => "needs setup",
        PluginSetupState::Checking => "checking",
        PluginSetupState::Unavailable => "unavailable",
        PluginSetupState::Unverified => "not checked",
    }
}
fn repair(action: PluginSetupRepair, view: RwSignal<String>) {
    let selector = match action {
        PluginSetupRepair::Configuration { key } => {
            format!(".plugin-editor [data-plugin-setting=\"{key}\"]")
        }
        PluginSetupRepair::Grants => ".plugin-editor [data-plugin-grants]".into(),
        PluginSetupRepair::Provider { capability } => {
            format!(".plugin-capabilities select[aria-label=\"Provider for {capability}\"]")
        }
        PluginSetupRepair::Feature { feature } => match feature {
            PluginSetupFeature::Audio => {
                view.set("meetings".into());
                return;
            }
            PluginSetupFeature::Capture => {
                view.set("activity".into());
                return;
            }
            PluginSetupFeature::Models => {
                view.set("connections".into());
                return;
            }
            PluginSetupFeature::Notifications => ".native-notification-setup".into(),
        },
    };
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Ok(Some(element)) = document.query_selector(&selector) {
            if element.tag_name() == "DETAILS" {
                let _ = element.set_attribute("open", "");
            }
            element.scroll_into_view();
            let target = element
                .query_selector("input,select,summary,button")
                .ok()
                .flatten()
                .unwrap_or(element);
            if let Some(target) = target.dyn_ref::<web_sys::HtmlElement>() {
                let _ = target.focus();
            }
        }
    }
}
#[component]
pub(super) fn PluginSetup(
    plugin: PluginRecord,
    snap: Snap,
    notice: RwSignal<String>,
    view: RwSignal<String>,
) -> impl IntoView {
    let rows = RwSignal::new(Vec::<PluginSetupStatus>::new());
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let loading = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let tick = RwSignal::new(0u64);
    let id = StoredValue::new(plugin.id.clone());
    let refresh = move || {
        if loading.get_untracked() {
            return;
        }
        loading.set(true);
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/plugins/{}/setup", id.get_value()),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<Vec<PluginSetupStatus>>(v)
                    .map_err(|_| "Invalid setup status".into())
            });
            if rows.try_get_untracked().is_none() {
                return;
            }
            loading.set(false);
            match result {
                Ok(v) => {
                    rows.set(v);
                    error.set(String::new());
                }
                Err(e) => error.set(e),
            }
        });
    };
    Effect::new(move |_| {
        let _ = tick.get();
        refresh();
    });
    let timer = gloo_timers::callback::Interval::new(2000, refresh);
    let _timer = StoredValue::new_local(timer);
    view! {<Show when=move||!rows.get().is_empty()||!error.get().is_empty()>
        <details class="workflow-manage plugin-setup"><summary>"Setup and readiness"</summary>
            <p class="muted">"Checks are explicit and do not start recording, sync data or use a model. Results apply to the checked configuration and host and expire after ten minutes. Configuration changes require a new check."</p>
            <Show when=move||!plugin.builtin><p class="muted">"An executable check runs this trusted local plugin with your OS account's privileges, using its saved configuration. The host supplies no work data or resolved credentials."</p></Show>
            <button class="source-link" disabled=move||loading.get() on:click=move|_|tick.update(|v|*v+=1)>"Refresh setup"</button>
            <Show when=move||!error.get().is_empty()><p role="alert">{move||error.get()}</p></Show>
            {move||rows.get().into_iter().map(|s|{
                let repair_action=s.step.repair.clone();let step=s.step.id.clone();let state=label(&s.state);let check=s.step.check;
                let disabled=s.configuration_version!=plugin.version||matches!(s.state,PluginSetupState::Checking|PluginSetupState::Unavailable)||!plugin.enabled;
                view!{<article data-setup-step=s.step.id><h3>{s.step.title}</h3><p class="muted">{s.step.description}</p><p role="status">{format!("{}{} · {}",state,if s.step.optional {" · optional"}else{""},s.detail)}</p>
                    {s.checked_at.map(|t|view!{<small>{format!("Checked {t}")}</small>})}
                    {check.then(||view!{<button class="button secondary" disabled=move||busy.get()||disabled on:click=move|_|action(format!("/api/v1/plugins/{}/setup/{step}/check",id.get_value()),"POST",json!({"expected_version":plugin.version}),snap,notice,busy,pending,move|_|tick.update(|v|*v+=1))>"Run setup check"</button>})}
                    {repair_action.map(|r|view!{<button class="source-link" on:click=move|_|repair(r.clone(),view)>"Open setup"</button>})}
                </article>}
            }).collect_view()}
        </details>
    </Show>}
}
