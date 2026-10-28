use super::*;

#[component]
pub(super) fn NativeNotifications(
    capabilities: RwSignal<Vec<PluginCapabilityStatus>>,
    notice: RwSignal<String>,
) -> impl IntoView {
    let permission = RwSignal::new("not_checked".to_string());
    let detail = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let request_pending = RwSignal::new(false);
    let available = move || {
        capabilities
            .get()
            .iter()
            .any(|c| c.capability == "native.notifications" && c.available)
    };
    let call = move |command: &'static str, input: Value| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        spawn_local(async move {
            let result = api(
                "/api/v1/plugin-capabilities/native.notifications/invoke",
                "POST",
                Some(json!({"command":command,"input":input,"idempotency_key":new_id()})),
            )
            .await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match result {
                Ok(value) => {
                    if let Some(state) = value["permission"].as_str() {
                        permission.set(state.into());
                    }
                    request_pending.set(
                        value["permission_request"]["state"] == "pending"
                            || (command == "request_permission" && value["requested"] == true),
                    );
                    detail.set(value["detail"].as_str().unwrap_or(if command=="status" {"OS settings checked. Alert presentation also depends on Focus and notification settings."}else{""}).into());
                }
                Err(error) => notice.set(error),
            }
        });
    };
    let timer = gloo_timers::callback::Interval::new(1000, move || {
        if request_pending.get_untracked() && available() && !busy.get_untracked() {
            call("status", json!({}));
        }
    });
    let _timer = StoredValue::new_local(timer);
    view! {<Show when=move||capabilities.get().iter().any(|c|c.capability=="native.notifications")>
        <details class="workflow-manage native-notification-setup"><summary>"OS notification setup"</summary>
            <p class="muted">"Notifications use the connected desktop host. Request permission explicitly, then check the OS result. Registration and enabling the plugin do not send alerts."</p>
            <p aria-live="polite">{move||format!("Permission: {}",permission.get().replace('_'," "))}</p><p class="muted">{move||detail.get()}</p>
            <fieldset disabled=move||busy.get()||!available()>
                <button class="button secondary" on:click=move|_|call("status",json!({}))>"Check notification status"</button>
                <button class="button secondary" disabled=move||request_pending.get() on:click=move|_|call("request_permission",json!({"approve":true}))>"Request notification permission"</button>
                <button class="button secondary" disabled=move||!matches!(permission.get().as_str(),"authorized"|"provisional"|"ephemeral") on:click=move|_|call("show",json!({"id":format!("test-{}",new_id()),"title":"Thesaurus","body":"Your notification provider is connected. This is a test notification."}))>"Send test notification"</button>
            </fieldset>
        </details>
    </Show>}
}
