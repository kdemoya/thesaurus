use super::focus::Reference;
use super::workflows::{Pending, action, chosen_project};
use super::*;
use std::{cell::Cell, rc::Rc};

fn chosen(board: RwSignal<Option<PriorityBoard>>, id: RwSignal<String>) -> Option<PriorityItem> {
    let data = board.get()?;
    data.items
        .iter()
        .find(|i| i.current.task.id == id.get())
        .or_else(|| data.items.first())
        .cloned()
}
#[component]
pub(super) fn Priorities(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    project: RwSignal<String>,
    tick: RwSignal<u64>,
    mode: RwSignal<String>,
    waiting_id: RwSignal<String>,
    initial_task: RwSignal<String>,
) -> impl IntoView {
    let board = RwSignal::new(None::<PriorityBoard>);
    let loading = RwSignal::new(false);
    let current = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let importance = RwSignal::new(None::<String>);
    let note = RwSignal::new(None::<String>);
    let edit_version = RwSignal::new(None::<i64>);
    let active_project = RwSignal::new(String::new());
    let history = RwSignal::new(Vec::<PriorityReviewSummary>::new());
    let past = RwSignal::new(None::<PriorityReview>);
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let cleanup = alive.clone();
    on_cleanup(move || cleanup.store(false, std::sync::atomic::Ordering::Relaxed));
    let serial = Rc::new(Cell::new(0u64));
    let load: Rc<dyn Fn()> = Rc::new(move || {
        let Some(p) = chosen_project(snap, project) else {
            board.set(None);
            return;
        };
        if active_project.get_untracked() != p.id {
            active_project.set(p.id.clone());
            board.set(None);
            current.set(String::new());
            importance.set(None);
            note.set(None);
            edit_version.set(None);
            history.set(vec![]);
            past.set(None);
        }
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        loading.set(true);
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/projects/{}/priorities", p.id),
                "GET",
                None,
            )
            .await;
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            loading.set(false);
            match result {
                Ok(value) => match serde_json::from_value::<PriorityBoard>(value) {
                    Ok(data) => {
                        if board
                            .get_untracked()
                            .is_none_or(|old| old.signature != data.signature)
                        {
                            if !data
                                .items
                                .iter()
                                .any(|i| i.current.task.id == current.get_untracked())
                            {
                                current.set(
                                    data.items
                                        .first()
                                        .map(|i| i.current.task.id.clone())
                                        .unwrap_or_default(),
                                );
                                importance.set(None);
                                note.set(None);
                                edit_version.set(None);
                            }
                            board.set(Some(data));
                        }
                    }
                    Err(_) => notice.set("Could not read priorities".into()),
                },
                Err(error) => notice.set(error),
            }
        });
    });
    Effect::new({
        let load = load.clone();
        move |_| {
            let _ = snap.get().map(|s| s.watermark);
            let _ = project.get();
            let _ = tick.get();
            load();
        }
    });
    let save_preference = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(item) = chosen(board, current) else {
            return;
        };
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        action(
            format!(
                "/api/v1/projects/{}/priorities/tasks/{}",
                p.id, item.current.task.id
            ),
            "PATCH",
            json!({"expected_version":edit_version.get_untracked().unwrap_or(item.current.preference.version),"importance":importance.get_untracked().unwrap_or(item.current.preference.importance),"note":note.get_untracked().unwrap_or(item.current.preference.note)}),
            snap,
            notice,
            busy,
            pending,
            move |_| {
                importance.set(None);
                note.set(None);
                edit_version.set(None);
                tick.update(|n| *n += 1);
            },
        );
    };
    view! {
        <section class="panel focus-priority-table"><div class="section-heading"><h2>"Priority review"</h2><span>"Accepted work · rule v1"</span></div>
            <div class="focus-review-heading"><p class="muted">{move||board.get().and_then(|b|b.baseline).map(|r|format!("Compared with your review saved {}",r.created_at)).unwrap_or_else(||"No saved review yet. Current scores establish the first baseline.".into())}</p>
                <button class="button primary" disabled=move||busy.get() || loading.get() || board.get().is_none() on:click=move|_|{if let Some(b)=board.get_untracked(){action(format!("/api/v1/projects/{}/priorities/reviews",b.project_id),"POST",json!({"signature":b.signature,"expected_review_id":b.baseline.map(|r|r.id)}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}}>"Mark reviewed"</button>
            </div>
            <Show when=move||loading.get()><p class="muted" role="status">"Refreshing priorities…"</p></Show>
            <div class="task-table-scroll"><table class="task-table priority-table"><thead><tr><th>"Rank"</th><th>"Change"</th><th>"Task"</th><th>"Score"</th><th>"Next step"</th><th>"Due"</th></tr></thead><tbody>
                {move||board.get().map(|b|b.items.into_iter().map(|i|{let id=i.current.task.id.clone();let click=id.clone();let step=if i.stale_evidence{"Review evidence"}else if i.waiting_ids.is_empty(){"Work"}else if i.current.signals.iter().any(|s|s.code=="waiting" && s.points>0){"Follow up"}else{"Await response"};let delta=i.previous_rank.map(|rank|{let n=rank-i.current.rank;if n>0{format!("↑{n}")}else if n<0{format!("↓{}",-n)}else{"—".into()}}).unwrap_or_else(||if i.new_since_review{"new".into()}else{"—".into()});view!{<tr class:selected=move||chosen(board,current).is_some_and(|i|i.current.task.id==id) on:click=move|_|{current.set(click.clone());importance.set(None);note.set(None);edit_version.set(None);}>
                    <td>{i.current.rank}</td><td class="orange">{delta}</td><td><button class="task-title" type="button">{i.current.task.title}</button></td><td>{i.current.score}</td><td class="muted">{step}</td><td>{i.current.task.due.unwrap_or_else(||"—".into())}</td></tr>}}).collect_view())}
            </tbody></table></div>
            <Show when=move||board.get().is_some_and(|b|b.items.is_empty())><p class="muted">"No accepted project tasks. Review candidate actions in Work first."</p></Show>
            <details class="workflow-manage"><summary>"How the score is calculated"</summary><p class="muted">"Deadline: overdue +60, today +50, within 3 days +35, within 7 days +20, later +5, no valid date +0. Importance: low +0, normal +10, high +25, critical +40. Waiting: follow-up due +20, awaiting a future follow-up −20, otherwise +0. Source changes add no points and flag evidence for review. Ties use the earliest valid due date, then the task ID. All dates use UTC."</p></details>
        </section>
        {move||chosen(board,current).map(|i|{
            let pref_version=i.current.preference.version;let default_level=i.current.preference.importance.clone();let default_note=i.current.preference.note.clone();let track=i.current.task.id.clone();let waiting=i.waiting_ids.first().cloned();
            view!{
                <div class="focus-detail-columns">
                    <section class="panel"><div class="section-heading"><h2>"Why this position"</h2><span>{format!("Rank {} · score {}",i.current.rank,i.current.score)}</span></div><h3 class="focus-task-title">{i.current.task.title.clone()}</h3>
                        <dl class="focus-signals">{i.current.signals.into_iter().map(|s|view!{<dt>{s.label}</dt><dd>{format!("{:+}",s.points)}</dd>}).collect_view()}</dl>
                        {i.stale_evidence.then(||view!{<p class="brief-stale">"The cited source has changed. Priority uses the accepted task's recorded deadline; verify its evidence."</p>})}
                        <Reference reference=i.current.task.evidence selected=selected notice=notice/>
                        <h3 class="focus-section-title">"Since your last review"</h3>
                        {i.new_since_review.then(||view!{<p class="orange">"This task became accepted work after the saved review."</p>})}
                        {i.changes.is_empty().then(||view!{<p class="muted">"No scoring input changes for this task in the current comparison."</p>})}
                        {i.changes.into_iter().map(|c|view!{<div class="focus-change"><strong>{format!("{} · {:+} points",c.code,c.points_delta)}</strong><p><span class="muted">{c.before}</span>" → "{c.after}</p></div>}).collect_view()}
                        {(!i.influences.is_empty()).then(||view!{<h3 class="focus-section-title">"Other work affecting the order"</h3>})}
                        {i.influences.into_iter().map(|other|view!{<details class="focus-influence"><summary>{other.title}" · "{other.detail}</summary><Reference reference=other.evidence selected=selected notice=notice/></details>}).collect_view()}
                        {(i.influences_omitted>0).then(||view!{<p class="muted">{format!("{} more relative-order influences omitted from this summary.",i.influences_omitted)}</p>})}
                    </section>
                    <section class="panel"><div class="section-heading"><h2>"Your preference"</h2><span>"Explicit input"</span></div><form class="workflow-form" on:submit=save_preference><fieldset class="focus-form-fields" disabled=move||busy.get()>
                        <label for="priority-importance">"Importance"</label><select id="priority-importance" prop:value=move||importance.get().unwrap_or_else(||default_level.clone()) on:change=move|ev|{if edit_version.get_untracked().is_none(){edit_version.set(Some(pref_version));}importance.set(Some(event_target_value(&ev)));}><option value="low">"Low"</option><option value="normal">"Normal"</option><option value="high">"High"</option><option value="critical">"Critical"</option></select>
                        <label for="priority-note">"Reason (optional)"</label><textarea id="priority-note" maxlength="1000" rows="4" prop:value=move||note.get().unwrap_or_else(||default_note.clone()) on:input=move|ev|{if edit_version.get_untracked().is_none(){edit_version.set(Some(pref_version));}note.set(Some(event_target_value(&ev)));} placeholder="Why this matters to your work…"></textarea>
                        <div class="workflow-actions"><button class="button secondary" type="submit" disabled=move||busy.get()>"Save preference"</button><button class="button text" type="button" on:click=move|_|{importance.set(None);note.set(None);edit_version.set(None);}>"Reset edits"</button></div>
                    </fieldset></form><div class="workflow-actions"><button class="button secondary" on:click=move|_|{initial_task.set(track.clone());waiting_id.set(String::new());mode.set("waiting".into());}>"Track a waiting request"</button>{waiting.map(|id|view!{<button class="source-link" on:click=move|_|{waiting_id.set(id.clone());initial_task.set(String::new());mode.set("waiting".into());}>"Open linked request →"</button>})}</div><p class="muted">"Preferences stay separate from source facts. Waiting requests change the next-step cue; they do not complete the task."</p>
                    </section>
                </div>
            }
        })}
        <section class="panel focus-history"><div class="section-heading"><h2>"Review history"</h2></div>
            <details class="workflow-manage"><summary>"Work that left the priority list"</summary>{move||board.get().map(|b|b.departed.into_iter().map(|d|view!{<article class="focus-change"><p>{format!("Was #{} · {} · {}",d.previous_rank,d.task.title,d.status)}</p><Reference reference=d.task.evidence selected=selected notice=notice/></article>}).collect_view())}</details>
            <button class="button text" on:click=move|_|{if let Some(p)=chosen_project(snap,project){spawn_local(async move{match api(&format!("/api/v1/projects/{}/priorities/reviews",p.id),"GET",None).await{Ok(v)=>match serde_json::from_value(v){Ok(rows)=>history.set(rows),Err(_)=>notice.set("Could not read review history".into())},Err(e)=>notice.set(e)}});}}>"Load saved reviews"</button>
            <div class="focus-review-list">{move||history.get().into_iter().map(|r|{let id=r.id.clone();view!{<button class="source-link" on:click=move|_|{if let Some(p)=chosen_project(snap,project){let path=format!("/api/v1/projects/{}/priorities/reviews/{id}",p.id);spawn_local(async move{match api(&path,"GET",None).await{Ok(v)=>match serde_json::from_value(v){Ok(r)=>past.set(Some(r)),Err(_)=>notice.set("Could not read saved review".into())},Err(e)=>notice.set(e)}});}}>{format!("{} · {} tasks",r.created_at,r.task_count)}</button>}}).collect_view()}</div>
            {move||past.get().map(|review|view!{<div class="focus-past-review"><div class="workflow-topline"><span>{format!("Saved {} · {}",review.created_at,review.as_of_date)}</span><button class="button text" on:click=move|_|past.set(None)>"Close"</button></div><p class="muted">"Historical scores and evidence; not the current priority order."</p>{review.items.into_iter().map(|i|view!{<details class="focus-influence"><summary>{format!("#{} · {} · {} points",i.rank,i.task.title,i.score)}</summary><dl class="focus-signals">{i.signals.into_iter().map(|s|view!{<dt>{s.label}</dt><dd>{s.points}</dd>}).collect_view()}</dl><p class="muted">{i.preference.note}</p><Reference reference=i.task.evidence selected=selected notice=notice/></details>}).collect_view()}</div>})}
        </section>
    }
}
