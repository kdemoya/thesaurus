use super::workflows::{Pending, action};
use super::*;

#[component]
pub(super) fn Settings(
    snap: Snap,
    notice: RwSignal<String>,
    section: RwSignal<String>,
    view: RwSignal<String>,
    theme_open: RwSignal<bool>,
) -> impl IntoView {
    view! {
        <div class="settings-page" hidden=move||section.get()!="processing"><header class="settings-intro"><h2>"A little setup. A lot less work."</h2><p>"Your preferences keep things moving in the background."</p></header><MeetingAutomation snap=snap notice=notice/>
            <section class="settings-group"><div class="settings-group-heading"><h3>"Workspace"</h3></div><div class="settings-card"><div class="setting-row"><div class="setting-copy"><span class="setting-label">"Initial setup"</span><p>"Revisit your model, recording inputs and agent memory."</p></div><button class="button secondary" on:click=move|_|view.set("onboarding".into())>"Revisit onboarding →"</button></div></div></section>
        </div>
        <Show when=move||section.get()=="audio"><audio_settings::AudioSettings snap=snap notice=notice/></Show>
        <Show when=move||section.get()=="models"><div class="settings-page"><header class="settings-intro"><h2>"Models & usage"</h2><p>"Choose your connections. Keep background work within your limits."</p></header><section class="settings-group"><div class="settings-group-heading"><h3>"Model connections"</h3></div><div class="settings-card"><div class="setting-row"><div class="setting-copy"><span class="setting-label">"Connected accounts"</span><p>"Add a model or manage the accounts available to your workspace."</p></div><button class="button secondary" on:click=move|_|view.set("connections".into())>"Manage connections →"</button></div></div></section><usage_controls::UsageControls snap=snap notice=notice view=view/></div></Show>
        <Show when=move||section.get()=="appearance"><div class="settings-page"><header class="settings-intro"><h2>"Make yourself at home."</h2><p>"A quiet workspace, in your preferred palette."</p></header><section class="settings-group"><div class="settings-group-heading"><h3>"Appearance"</h3><span class="settings-badge">"This device"</span></div><div class="settings-card"><div class="setting-row"><div class="setting-copy"><span class="setting-label">"App theme"</span><p>"Terminal, Midnight, Phosphor, Paper or follow your system."</p></div><button class="button secondary" on:click=move|_|theme_open.set(true)>"Preview themes →"</button></div></div><p class="settings-footnote">"Preview a palette across the app before you apply it."</p></section></div></Show>
    }
}

#[component]
fn MeetingAutomation(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let settings = RwSignal::new(MeetingProcessingSettings::default());
    let baseline = RwSignal::new(None::<Value>);
    let loaded = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let dirty = move || loaded.get() && baseline.get() != serde_json::to_value(settings.get()).ok();
    spawn_local(async move {
        match api("/api/v1/settings/meetings", "GET", None)
            .await
            .and_then(|v| {
                serde_json::from_value::<MeetingProcessingSettings>(v)
                    .map_err(|_| "Could not read meeting settings".to_string())
            }) {
            Ok(s) => {
                if loaded.try_get_untracked().is_none() {
                    return;
                }
                baseline.set(serde_json::to_value(&s).ok());
                settings.set(s);
                loaded.set(true);
            }
            Err(e) => {
                if loaded.try_get_untracked().is_some() {
                    notice.set(e);
                }
            }
        }
    });
    view! {<form class="settings-form" on:submit=move|ev|{ev.prevent_default();if !dirty(){return;}action("/api/v1/settings/meetings".into(),"PUT",json!({"settings":settings.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(s)=serde_json::from_value::<MeetingProcessingSettings>(v){baseline.set(serde_json::to_value(&s).ok());settings.set(s);}});}>
        <fieldset class="settings-fields" disabled=move||!loaded.get()||busy.get()><section class="settings-group" aria-labelledby="summary-settings-title"><div class="settings-group-heading"><h3 id="summary-settings-title">"Meeting summaries"</h3><span class="settings-badge">"Automatic by default"</span></div><div class="settings-card">
            <div class="setting-row"><div class="setting-copy"><label for="meeting-automatic">"Summarize automatically"</label><p id="meeting-automatic-help">"Finish a recording. Get a summary and cited action items."</p></div><input id="meeting-automatic" class="setting-switch" type="checkbox" role="switch" aria-describedby="meeting-automatic-help" prop:checked=move||settings.get().automatic_summaries on:change=move|ev|settings.update(|s|s.automatic_summaries=event_target_checked(&ev))/></div>
            <div class="setting-row"><div class="setting-copy"><label for="meeting-default-model">"Summary model"</label><p>"Use your setup choice, or pick a different connection."</p></div><select id="meeting-default-model" prop:value=move||settings.get().connection.unwrap_or_default() on:change=move|ev|{let c=event_target_value(&ev);settings.update(|s|s.connection=if c.is_empty(){None}else{Some(c)});}>
                <option value="">"Use my onboarding model"</option>
                {move||snap.get().map(|s|s.connections.into_iter().map(|c|view!{<option value=c.id.clone()>{if c.id=="local"{"Local · extractive summary".into()}else{c.id.clone()}}</option>}).collect_view())}
            </select></div></div></section>
            <details class="settings-disclosure"><summary>"How automatic summaries work"</summary><div class="setting-diagnostics"><p>"After transcription finishes, your saved model receives the transcript and meeting notes. Local mode creates an extractive summary on this device. Connected models use their normal account allowance; source destination blocks and daily limits still apply."</p><p>"Model choices apply to new recordings. Turning automation off also stops queued summaries. A summary already running can finish."</p></div></details>
        </fieldset>
        <footer class="settings-savebar"><div role="status" aria-live="polite"><span class="settings-save-state" class:settings-unsaved=move||dirty()>{move||if busy.get(){"Saving…"}else if !loaded.get(){"Loading preferences…"}else if dirty(){"Unsaved changes"}else{"✓ All changes saved"}}</span><p>"Set it once. Future meetings follow your preferences."</p></div><button class="button primary" type="submit" disabled=move||!dirty()||busy.get()>"Save changes"</button></footer>
    </form>}
}
