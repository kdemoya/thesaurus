use super::workflows::{Citations, Pending, action};
use super::*;

fn heading(kind: &str) -> &'static str {
    match kind {
        "key_points" => "Key points",
        "decisions" => "Decisions",
        "action_items" => "Action items",
        "open_questions" => "Open questions",
        _ => "Risks and blockers",
    }
}
fn current_document(snap: Snap, session: Signal<Option<LiveMeeting>>) -> Option<Document> {
    let id = session.get()?.document_id?;
    snap.get()?.documents.into_iter().find(|d| d.id == id)
}
fn ready(session: Signal<Option<LiveMeeting>>) -> bool {
    session.get().is_some_and(|s| {
        s.state == "stopped"
            && !s.processing
            && s.pending_chunks == 0
            && !s.transcript.trim().is_empty()
    })
}
#[component]
pub(super) fn MeetingSummaries(
    session: Signal<Option<LiveMeeting>>,
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    let rows = RwSignal::new(Vec::<MeetingSummary>::new());
    let processing = RwSignal::new(None::<MeetingProcessingState>);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let epoch = RwSignal::new(0u64);
    let reload = RwSignal::new(0u64);
    let chosen = RwSignal::new(String::new());
    let session_id = Memo::new(move |_| session.get().map(|s| s.id));
    Effect::new(move |_| {
        let _ = session_id.get();
        chosen.set(String::new());
        rows.set(vec![]);
        processing.set(None);
    });
    Effect::new(move |_| {
        let s = session.get();
        let _ = snap.get().map(|s| s.watermark);
        let _ = reload.get();
        epoch.update(|n| *n += 1);
        let ticket = epoch.get_untracked();
        let Some(s) = s else {
            return;
        };
        spawn_local(async move {
            let result = async {
                let summaries = api(
                    &format!("/api/v1/live-meetings/{}/summaries", s.id),
                    "GET",
                    None,
                )
                .await?;
                let state = api(
                    &format!("/api/v1/live-meetings/{}/processing", s.id),
                    "GET",
                    None,
                )
                .await?;
                Ok::<_, String>((
                    serde_json::from_value::<Vec<MeetingSummary>>(summaries)
                        .map_err(|_| "Could not read meeting summaries".to_string())?,
                    serde_json::from_value::<Option<MeetingProcessingState>>(state)
                        .map_err(|_| "Could not read meeting processing".to_string())?,
                ))
            }
            .await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match result {
                Ok((list, state)) => {
                    rows.set(list);
                    processing.set(state);
                }
                Err(e) => notice.set(e),
            }
        });
    });
    let active = move || {
        rows.get().iter().any(|s| s.status == "generating")
            || processing
                .get()
                .is_some_and(|s| matches!(s.status.as_str(), "waiting" | "processing"))
    };
    let selected_summary = move || {
        let rows = rows.get();
        rows.iter()
            .find(|s| s.id == chosen.get())
            .or_else(|| rows.first())
            .cloned()
    };
    view! {<Show when=move||session.get().is_some()><section class="meeting-summary">
        <div class="section-heading"><h3>"Meeting summary"</h3><button class="source-link" on:click=move|_|super::workspace::open_settings()>"Settings"</button></div>
        <Show when=move||!ready(session)><p class="muted">"Your summary appears here after the recording and transcription finish."</p></Show>
        <Show when=move||ready(session)&&active()&&!rows.get().iter().any(|s|s.status=="generating")><p role="status" class="muted">{move||if processing.get().is_some_and(|s|s.status=="waiting"){"Summary queued. You can leave this screen."}else{"Summarizing your meeting…"}}</p></Show>
        <Show when=move||ready(session)&&processing.get().is_some_and(|s|s.status=="disabled")><p class="muted">"Automatic summaries are off. You can enable them in Settings."</p></Show>
        <Show when=move||processing.get().is_some_and(|s|s.status=="empty")><p class="muted">"No speech was transcribed in this recording."</p></Show>
        {move||processing.get().and_then(|s|{if rows.get().iter().any(|r|Some(&r.id)==s.run_id.as_ref()&&r.error.is_some()){None}else{s.error}}).map(|e|view!{<p class="runtime-error" role="alert">{e}</p>})}

        <Show when=move||rows.get().len().gt(&1)><label>"Saved summaries"</label><select aria-label="Saved meeting summaries" prop:value=move||selected_summary().map(|s|s.id).unwrap_or_default() on:change=move|ev|chosen.set(event_target_value(&ev))>{move||rows.get().into_iter().map(|s|view!{<option value=s.id>{format!("{} · r{} · {} · {}",s.created_at,s.revision,s.connection,s.status)}</option>}).collect_view()}</select></Show>
        {move||selected_summary().map(|summary|{
            let stale=current_document(snap,session).is_none_or(|d|d.revision!=summary.revision);
            let run_id=summary.id.clone();
            view!{<article class="meeting-summary-result"><p class="muted">{format!("{} · transcript r{} · {}",if summary.status=="draft"{"Ready"}else{summary.status.as_str()},summary.revision,if summary.connection=="local"{"Local extractive summary"}else{summary.connection.as_str()})}</p>
                {stale.then(||view!{<p class="runtime-error">"This summary describes an earlier transcript revision. Generate a new summary before accepting its action items."</p>})}
                {summary.error.map(|e|view!{<p class="runtime-error" role="alert">{e}</p>})}
                {(summary.status=="generating").then(||view!{<p role="status">{move||snap.get().and_then(|s|s.runtime_runs.into_iter().find(|r|r.id==run_id)).map(|r|r.progress).unwrap_or("Processing transcript…".into())}</p><button class="button secondary" disabled=move||busy.get() on:click=move|_|action(format!("/api/v1/runtime/runs/{}/cancel",summary.id),"POST",json!({}),snap,notice,busy,pending,move|_|{reload.update(|n|*n+=1);})>"Cancel processing"</button>})}
                {summary.sections.into_iter().map(|section|view!{<section><h4>{heading(&section.kind)}</h4>{section.items.is_empty().then(||view!{<p class="muted">"None identified."</p>})}{section.items.into_iter().map(|item|{
                    let task_id=item.task_id.clone();
                    view!{<article class="focus-change"><p>{item.text}</p>{(item.owner.is_some()||item.due.is_some()).then(||view!{<p class="muted">{format!("Owner: {} · Due: {}",item.owner.unwrap_or("Unspecified".into()),item.due.unwrap_or("Unspecified".into()))}</p>})}<Citations evidence=vec![item.evidence] selected=selected notice=notice/>
                    {move||task_id.as_ref().and_then(|id|snap.get().and_then(|s|s.tasks.into_iter().find(|t|&t.id==id))).map(|t|{let accept=t.id.clone();let dismiss=t.id;let version=t.version;view!{<p class="muted">{format!("Action status: {}",t.status)}</p>{(t.status=="candidate").then(||view!{<div class="workflow-actions"><button class="button secondary" disabled=move||busy.get()||stale on:click=move|_|action(format!("/api/v1/tasks/{accept}"),"PATCH",json!({"expected_version":version,"status":"accepted"}),snap,notice,busy,pending,move|_|{})>"Accept action"</button><button class="button text" disabled=move||busy.get() on:click=move|_|action(format!("/api/v1/tasks/{dismiss}"),"PATCH",json!({"expected_version":version,"status":"dismissed"}),snap,notice,busy,pending,move|_|{})>"Dismiss"</button></div>})}}})}
                    </article>}
                }).collect_view()}</section>}).collect_view()}
            </article>}
        })}
        <Show when=move||ready(session)&&!active()><button class="button text" disabled=move||busy.get() on:click=move|_|{
            if let Some(s)=session.get_untracked(){action(format!("/api/v1/live-meetings/{}/processing",s.id),"POST",json!({}),snap,notice,busy,pending,move|v|{if let Ok(state)=serde_json::from_value(v){processing.set(Some(state));}reload.update(|n|*n+=1);});}
        }>{move||if rows.get().is_empty(){"Create summary"}else{"Regenerate summary"}}</button></Show>
    </section></Show>}
}
