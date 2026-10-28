use super::plugin_runs::Citations;
use super::workflows::{Pending, action};
use super::*;

#[component]
pub(super) fn PluginActionReview(
    id: String,
    snap: Snap,
    notice: RwSignal<String>,
) -> impl IntoView {
    let preview = RwSignal::new(None::<ActionPreview>);
    let receipt = RwSignal::new(None::<ActionReceipt>);
    let approved = RwSignal::new(false);
    let undo_approved = RwSignal::new(false);
    let busy = RwSignal::new(true);
    let error = RwSignal::new(String::new());
    let pending = RwSignal::new(None::<Pending>);
    Effect::new(move |_| {
        let id = id.clone();
        spawn_local(async move {
            let result: Result<(ActionPreview, Option<ActionReceipt>), String> = async {
                let p = serde_json::from_value(
                    api(&format!("/api/v1/actions/previews/{id}"), "GET", None).await?,
                )
                .map_err(|_| "Invalid action preview")?;
                let r = serde_json::from_value(
                    api(
                        &format!("/api/v1/actions/previews/{id}/receipt"),
                        "GET",
                        None,
                    )
                    .await?,
                )
                .map_err(|_| "Invalid action receipt")?;
                Ok((p, r))
            }
            .await;
            if preview.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match result {
                Ok((p, r)) => {
                    preview.set(Some(p));
                    receipt.set(r);
                }
                Err(e) => error.set(e),
            }
        });
    });
    view! {<details class="workflow-manage"><summary>"Review proposed action"</summary><Show when=move||!error.get().is_empty()><p role="alert">{move||error.get()}</p></Show>
        {move||preview.get().map(|p|{let proposed=p.status=="proposed";view!{<h4>{format!("{} · {}",p.provider,p.status)}</h4><p>{p.explanation}</p>
            {p.origin.map(|o|view!{<p>{format!("Proposed by {} · {}",o.plugin_id,o.rationale)}</p><Citations evidence=o.evidence notice=notice/>})}
            <details><summary>"Before"</summary><pre>{serde_json::to_string_pretty(&p.before).unwrap_or_default()}</pre></details><details open><summary>"Proposed changes"</summary><pre>{serde_json::to_string_pretty(&p.proposed).unwrap_or_default()}</pre></details>
            {proposed.then(||view!{<label><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>"Approve exactly these changes"</label><button class="button secondary" disabled=move||busy.get()||!approved.get() on:click=move|_|action(format!("/api/v1/actions/previews/{}/execute",p.id),"POST",json!({"fingerprint":p.fingerprint,"approve_effects":approved.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(r)=serde_json::from_value::<ActionReceipt>(v){receipt.set(Some(r));preview.update(|p|{if let Some(p)=p {p.status="executed".into();}});approved.set(false);}})>"Execute approved action"</button>})}
        }})}
        {move||receipt.get().map(|r|{let can_undo=r.status=="executed"&&r.supports_undo;view!{<p>{format!("Receipt {} · {}",r.id,r.status)}</p><details><summary>"Recorded receipt"</summary><pre>{serde_json::to_string_pretty(&r).unwrap_or_default()}</pre></details>
            {can_undo.then(||view!{<label><input type="checkbox" prop:checked=move||undo_approved.get() on:change=move|ev|undo_approved.set(event_target_checked(&ev))/>"Approve restoring the previous status"</label><button class="source-link" disabled=move||busy.get()||!undo_approved.get() on:click=move|_|action(format!("/api/v1/actions/receipts/{}/undo",r.id),"POST",json!({"expected_receipt_version":r.version,"approve_effects":undo_approved.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(r)=serde_json::from_value(v){receipt.set(Some(r));undo_approved.set(false);}})>"Undo approved action"</button>})}
        }})}
    </details>}
}
