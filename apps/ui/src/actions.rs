use super::workflows::{Pending, action};
use super::*;
fn pretty(v: &impl serde::Serialize) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}
#[component]
pub(super) fn Actions(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let provider = RwSignal::new("task_status".to_string());
    let target = RwSignal::new(String::new());
    let status = RwSignal::new("completed".to_string());
    let preview = RwSignal::new(None::<ActionPreview>);
    let approved = RwSignal::new(false);
    let receipts = RwSignal::new(Vec::<ActionReceipt>::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let tick = RwSignal::new(0u64);
    let epoch = RwSignal::new(0u64);
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        epoch.update(|n| *n += 1);
        let e = epoch.get_untracked();
        spawn_local(async move {
            if let Ok(v) = api("/api/v1/actions/receipts", "GET", None).await {
                if epoch.try_get_untracked() == Some(e) {
                    if let Ok(r) = serde_json::from_value(v) {
                        receipts.set(r);
                    }
                }
            }
        });
    });
    let prepare = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let id = target.get_untracked();
        let task = snap
            .get_untracked()
            .and_then(|s| s.tasks.into_iter().find(|t| t.id == id));
        let is_task = provider.get_untracked() == "task_status";
        action(
            "/api/v1/actions/previews".into(),
            "POST",
            json!({"provider":provider.get_untracked(),"target_id":id,"expected_version":if is_task{task.map(|t|t.version)}else{None},"status":if is_task{Some(status.get_untracked())}else{None}}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if let Ok(p) = serde_json::from_value(v) {
                    preview.set(Some(p));
                    approved.set(false);
                }
            },
        );
    };
    view! {<section class="panel"><div class="section-heading"><h2>"Preview actions"</h2><span>"Explicit effects · durable receipts"</span></div><form class="workflow-form" on:submit=prepare><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Provider"<select prop:value=move||provider.get() on:change=move|ev|{provider.set(event_target_value(&ev));target.set(String::new());preview.set(None);approved.set(false);}><option value="task_status">"Local task status (undo supported)"</option><option value="delete_activity_frame">"Delete captured frame (permanent)"</option></select></label><Show when=move||provider.get()=="task_status"><label>"Task"<select prop:value=move||target.get() on:change=move|ev|{target.set(event_target_value(&ev));preview.set(None);}><option value="">"Choose task"</option>{move||snap.get().map(|s|s.tasks.into_iter().filter(|t|t.status!="superseded").map(|t|view!{<option value=t.id>{format!("{} · {}",t.status,t.title)}</option>}).collect_view())}</select></label><label>"Proposed status"<select prop:value=move||status.get() on:change=move|ev|{status.set(event_target_value(&ev));preview.set(None);}><option value="accepted">"Accepted"</option><option value="completed">"Completed"</option><option value="dismissed">"Dismissed"</option><option value="candidate">"Candidate"</option></select></label></Show><Show when=move||provider.get()=="delete_activity_frame"><label>"Retained frame ID"<input required prop:value=move||target.get() on:input=move|ev|{target.set(event_target_value(&ev));preview.set(None);}/></label></Show><button class="button secondary" type="submit">"Prepare exact effect preview"</button></fieldset></form>
    {move||preview.get().map(|p|view!{<article class="focus-change"><h3>{format!("{} → {}",p.provider,p.target_id)}</h3><p>{p.explanation}</p><details open><summary>"Before"</summary><pre>{pretty(&p.before)}</pre></details><details open><summary>"Proposed effects"</summary><pre>{pretty(&p.proposed)}</pre></details><label><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>"Approve exactly these effects"</label><button class="button primary" disabled=move||busy.get()||!approved.get() on:click=move|_|action(format!("/api/v1/actions/previews/{}/execute",p.id),"POST",json!({"fingerprint":p.fingerprint,"approve_effects":approved.get_untracked()}),snap,notice,busy,pending,move|_|{preview.set(None);approved.set(false);tick.update(|n|*n+=1);})>"Execute approved action"</button></article>})}
    <details class="workflow-manage"><summary>"Execution receipts"</summary>{move||receipts.get().into_iter().map(|r|view!{<Receipt receipt=r snap=snap notice=notice tick=tick/>}).collect_view()}</details></section>}
}
#[component]
fn Receipt(
    receipt: ActionReceipt,
    snap: Snap,
    notice: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let approved = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let receipt_id = StoredValue::new(receipt.id.clone());
    let can_undo = receipt.supports_undo && receipt.status == "executed";
    let before = pretty(&receipt.before);
    let after = pretty(&receipt.after);
    let undo = pretty(&receipt.undo_effect);
    view! {<details><summary>{format!("{} · {} · {}",receipt.provider,receipt.status,receipt.executed_at)}</summary><code>{receipt.id.clone()}</code><pre>{format!("Before:\n{before}\nAfter:\n{after}\nUndo effect:\n{undo}")}</pre><Show when=move||can_undo><p class="muted">"Undo restores the previous task status as a new revision. Newer task changes prevent undo."</p><label><input type="checkbox" prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/>"Approve restoring the recorded previous status"</label><button class="button secondary" disabled=move||busy.get()||!approved.get() on:click=move|_|{action(format!("/api/v1/actions/receipts/{}/undo",receipt_id.get_value()),"POST",json!({"expected_receipt_version":receipt.version,"approve_effects":approved.get_untracked()}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}>"Undo supported action"</button></Show><Show when=move||!receipt.supports_undo><p class="muted">"This provider does not support undo."</p></Show></details>}
}
