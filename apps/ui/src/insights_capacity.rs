use super::*;

#[component]
pub(super) fn Capacity(project: String, snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let project = StoredValue::new(project);
    let reload = RwSignal::new(0u64);
    let plans = RwSignal::new(Vec::<CapacityPlan>::new());
    let efforts = RwSignal::new(Vec::<TaskEffort>::new());
    let busy = RwSignal::new(false);
    let draft = RwSignal::new(CapacitySave {
        id: None,
        expected_version: 0,
        name: "Capacity review".into(),
        start_date: today(),
        end_date: today(),
        utc_offset_minutes: 0,
        selected_task_ids: vec![],
        working_windows: vec![],
        calendar: vec![],
        tradeoffs: vec![],
        idempotency_key: new_id(),
    });
    let work_start = RwSignal::new(String::new());
    let work_end = RwSignal::new(String::new());
    let event_name = RwSignal::new(String::new());
    let event_start = RwSignal::new(String::new());
    let event_end = RwSignal::new(String::new());
    let ics = RwSignal::new(String::new());
    let import_notes = RwSignal::new(Vec::<String>::new());
    let trade_task = RwSignal::new(String::new());
    let trade_action = RwSignal::new("defer".to_string());
    let trade_minutes = RwSignal::new(String::new());
    let trade_due = RwSignal::new(String::new());
    let trade_note = RwSignal::new(String::new());
    Effect::new(move |_| {
        let _ = reload.get();
        let project = project.get_value();
        spawn_local(async move {
            let response = async {
                let plans =
                    api(&format!("/api/v1/projects/{project}/capacity"), "GET", None).await?;
                let efforts =
                    api(&format!("/api/v1/projects/{project}/efforts"), "GET", None).await?;
                Ok::<_, String>((
                    serde_json::from_value::<Vec<CapacityPlan>>(plans)
                        .map_err(|_| "Invalid capacity plans".to_string())?,
                    serde_json::from_value::<Vec<TaskEffort>>(efforts)
                        .map_err(|_| "Invalid effort estimates".to_string())?,
                ))
            }
            .await;
            if plans.try_get_untracked().is_none() {
                return;
            }
            match response {
                Ok((p, e)) => {
                    plans.set(p);
                    efforts.set(e);
                }
                Err(e) => notice.set(e),
            }
        });
    });
    let import = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        let request = json!({"ics":ics.get_untracked(),"utc_offset_minutes":draft.get_untracked().utc_offset_minutes});
        spawn_local(async move {
            let response = api("/api/v1/calendar/import-preview", "POST", Some(request)).await;
            if busy.try_get_untracked().is_none() {
                return;
            }
            busy.set(false);
            match response {
                Ok(value) => match serde_json::from_value::<CalendarImportPreview>(value) {
                    Ok(result) => {
                        import_notes.set(result.notes);
                        draft.update(|d| {
                            for event in result.events {
                                if !d.calendar.iter().any(|e| e.id == event.id) {
                                    d.calendar.push(event);
                                }
                            }
                        });
                    }
                    Err(_) => notice.set("Invalid calendar preview".into()),
                },
                Err(e) => notice.set(e),
            }
        });
    };
    view! {<section class="panel"><div class="section-heading"><h2>"Effort estimates"</h2><span class="muted">"user estimates in minutes"</span></div><p class="muted">"Capacity compares estimated effort with your working intervals and selected calendar commitments. Missing estimates remain unknown."</p><EffortEditor project=project.get_value() efforts=efforts snap=snap notice=notice reload=reload/></section>
        <section class="panel insight-record"><div class="section-heading"><h2>"Capacity scenario"</h2><span class="muted">{move||draft.get().id.map(|id|format!("editing {} · v{}",id,draft.get().expected_version)).unwrap_or_else(||"new plan".into())}</span></div><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let mut request=draft.get_untracked();request.idempotency_key=new_id();submit(format!("/api/v1/projects/{}/capacity",project.get_value()),"POST",serde_json::to_value(request).unwrap(),busy,snap,notice,reload);}>
        <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Plan name"</label><input required maxlength="200" prop:value=move||draft.get().name on:input=move|ev|draft.update(|d|d.name=event_target_value(&ev))/><div class="workflow-actions"><label>"From"<input type="date" required prop:value=move||draft.get().start_date on:input=move|ev|draft.update(|d|d.start_date=event_target_value(&ev))/></label><label>"Through"<input type="date" required prop:value=move||draft.get().end_date on:input=move|ev|draft.update(|d|d.end_date=event_target_value(&ev))/></label><label>"UTC offset in minutes"<input type="number" min="-840" max="840" step="15" prop:value=move||draft.get().utc_offset_minutes.to_string() on:input=move|ev|{if let Ok(offset)=event_target_value(&ev).parse::<i32>(){draft.update(|d|d.utc_offset_minutes=offset);}}/></label></div>
        <h3 class="focus-section-title">"Work to include in personal capacity"</h3><p class="muted">"Select the accepted tasks that belong in this personal review."</p>{move||scoped_tasks(snap,&project.get_value()).into_iter().filter(|t|t.status=="accepted").map(|task|{let id=task.id.clone();let checked=id.clone();view!{<label class="focus-check"><input type="checkbox" prop:checked=move||draft.get().selected_task_ids.contains(&checked) on:change=move|ev|{let include=event_target_checked(&ev);draft.update(|d|{d.selected_task_ids.retain(|x|x!=&id);if include{d.selected_task_ids.push(id.clone());}});}/><span>{task.title}</span></label>}}).collect_view()}<h3 class="focus-section-title">"Working hours"</h3><p class="muted">"Add the actual intervals you intend to work. Overlapping working intervals count once. All timestamps include a timezone offset."</p>{move||draft.get().working_windows.into_iter().enumerate().map(|(i,w)|view!{<div class="source-gap"><span>{format!("{} → {}",w.starts_at,w.ends_at)}</span><button type="button" class="button text" on:click=move|_|draft.update(|d|{if i<d.working_windows.len(){d.working_windows.remove(i);}})>"Remove"</button></div>}).collect_view()}<label>"Work interval start"</label><input placeholder="2026-10-19T09:00:00-04:00" prop:value=move||work_start.get() on:input=move|ev|work_start.set(event_target_value(&ev))/><label>"Work interval end"</label><input placeholder="2026-10-19T17:00:00-04:00" prop:value=move||work_end.get() on:input=move|ev|work_end.set(event_target_value(&ev))/><button type="button" class="button text" on:click=move|_|{draft.update(|d|d.working_windows.push(CapacityWindow{starts_at:work_start.get_untracked(),ends_at:work_end.get_untracked()}));work_start.set(String::new());work_end.set(String::new());}>"Add working interval"</button>
        <h3 class="focus-section-title">"Selected calendar commitments"</h3><p class="muted">"Only selected events consume capacity. Calendar overlaps are counted once inside your working hours."</p>{move||draft.get().calendar.into_iter().enumerate().map(|(i,event)|view!{<div class="source-gap"><label class="focus-check"><input type="checkbox" prop:checked=event.selected on:change=move|ev|draft.update(|d|{if let Some(e)=d.calendar.get_mut(i){e.selected=event_target_checked(&ev);}})/><span>{format!("{} · {} → {}",event.title,event.starts_at,event.ends_at)}</span></label><button type="button" class="button text" on:click=move|_|draft.update(|d|{if i<d.calendar.len(){d.calendar.remove(i);}})>"Remove"</button></div>}).collect_view()}<label>"Event title"</label><input maxlength="500" prop:value=move||event_name.get() on:input=move|ev|event_name.set(event_target_value(&ev))/><label>"Event start"</label><input placeholder="2026-10-19T10:00:00-04:00" prop:value=move||event_start.get() on:input=move|ev|event_start.set(event_target_value(&ev))/><label>"Event end"</label><input placeholder="2026-10-19T11:00:00-04:00" prop:value=move||event_end.get() on:input=move|ev|event_end.set(event_target_value(&ev))/><button type="button" class="button text" on:click=move|_|{draft.update(|d|d.calendar.push(CalendarCommitment{id:new_id(),title:event_name.get_untracked(),starts_at:event_start.get_untracked(),ends_at:event_end.get_untracked(),selected:true,origin:"manual".into()}));event_name.set(String::new());event_start.set(String::new());event_end.set(String::new());}>"Add commitment"</button>
        <details><summary>"Import calendar occurrences from iCalendar"</summary><p class="muted">"Paste expanded occurrences. UTC is supported; floating times and all-day events use your selected fixed UTC offset. Named timezones and recurrence rules are rejected with an explanation. No calendar account is contacted."</p><textarea rows="6" maxlength="200000" prop:value=move||ics.get() on:input=move|ev|ics.set(event_target_value(&ev)) placeholder="BEGIN:VCALENDAR…"></textarea><button type="button" class="button secondary" on:click=import>"Preview and add occurrences"</button>{move||import_notes.get().into_iter().map(|n|view!{<p class="muted">{n}</p>}).collect_view()}</details>
        <h3 class="focus-section-title">"Editable tradeoffs"</h3><p class="muted">"These scenarios do not change task deadlines, owners or external calendars."</p>{move||draft.get().tradeoffs.into_iter().enumerate().map(|(i,t)|view!{<div class="source-gap"><span>{format!("{} · {} · {}",t.task_id,t.action,t.note)}</span><button type="button" class="button text" on:click=move|_|draft.update(|d|{if i<d.tradeoffs.len(){d.tradeoffs.remove(i);}})>"Remove"</button></div>}).collect_view()}<label>"Accepted task"<select prop:value=move||trade_task.get() on:change=move|ev|trade_task.set(event_target_value(&ev))><option value="">"Select work"</option>{move||scoped_tasks(snap,&project.get_value()).into_iter().filter(|t|t.status=="accepted").map(|t|view!{<option value=t.id>{t.title}</option>}).collect_view()}</select></label><label>"Tradeoff"<select prop:value=move||trade_action.get() on:change=move|ev|trade_action.set(event_target_value(&ev))><option value="defer">"Defer beyond this period"</option><option value="reduce">"Reduce effort / scope"</option><option value="delegate">"Delegate personal effort"</option></select></label><Show when=move||trade_action.get()=="reduce"><label>"Adjusted effort, minutes"</label><input type="number" min="1" max="100000" prop:value=move||trade_minutes.get() on:input=move|ev|trade_minutes.set(event_target_value(&ev))/></Show><Show when=move||trade_action.get()=="defer"><label>"Proposed deadline after this period"</label><input type="date" prop:value=move||trade_due.get() on:input=move|ev|trade_due.set(event_target_value(&ev))/></Show><label>"Why this tradeoff?"</label><textarea rows="2" maxlength="2000" prop:value=move||trade_note.get() on:input=move|ev|trade_note.set(event_target_value(&ev))></textarea><button type="button" class="button text" on:click=move|_|{draft.update(|d|d.tradeoffs.push(CapacityTradeoff{task_id:trade_task.get_untracked(),action:trade_action.get_untracked(),adjusted_minutes:trade_minutes.get_untracked().parse().ok(),new_due:(!trade_due.get_untracked().is_empty()).then(||trade_due.get_untracked()),note:trade_note.get_untracked()}));trade_task.set(String::new());trade_note.set(String::new());}>"Add tradeoff"</button><div class="workflow-actions"><button class="button primary" type="submit">"Save and calculate capacity"</button><button class="button text" type="button" on:click=move|_|draft.update(|d|{d.id=None;d.expected_version=0;})>"Save next as new plan"</button></div></fieldset></form></section>
        {move||plans.get().into_iter().map(|plan|{let edit=plan.clone();let suggested_plan=plan.clone();view!{<section class="panel insight-record"><div class="section-heading"><h2>{plan.name.clone()}</h2><button class="source-link" on:click=move|_|draft.set(CapacitySave{id:Some(edit.id.clone()),expected_version:edit.version,name:edit.name.clone(),start_date:edit.start_date.clone(),end_date:edit.end_date.clone(),utc_offset_minutes:edit.utc_offset_minutes,selected_task_ids:edit.selected_task_ids.clone(),working_windows:edit.working_windows.clone(),calendar:edit.calendar.clone(),tradeoffs:edit.tradeoffs.clone(),idempotency_key:new_id()})>"Edit scenario"</button></div><p>{format!("{} → {} · {} working min − {} calendar min = {} free min",plan.start_date,plan.end_date,plan.working_minutes,plan.calendar_minutes,plan.free_minutes)}</p><p class="orange">{format!("{} planned minutes · {} calendar overlap pairs · v{}",plan.planned_minutes,plan.overlapping_calendar_pairs,plan.version)}</p>{plan.notes.into_iter().map(|note|view!{<p class="muted">{note}</p>}).collect_view()}<h3 class="focus-section-title">"Suggested tradeoffs · review feasibility"</h3>{plan.suggested_tradeoffs.into_iter().map(|suggestion|{let scenario=suggested_plan.clone();view!{<div class="source-gap"><span>{suggestion.note.clone()}</span><button class="button secondary" on:click=move|_|{let mut choices=scenario.tradeoffs.clone();choices.retain(|t|t.task_id!=suggestion.task_id);choices.push(suggestion.clone());draft.set(CapacitySave{id:Some(scenario.id.clone()),expected_version:scenario.version,name:scenario.name.clone(),start_date:scenario.start_date.clone(),end_date:scenario.end_date.clone(),utc_offset_minutes:scenario.utc_offset_minutes,selected_task_ids:scenario.selected_task_ids.clone(),working_windows:scenario.working_windows.clone(),calendar:scenario.calendar.clone(),tradeoffs:choices,idempotency_key:new_id()});}>"Edit with proposal"</button></div>}}).collect_view()}{plan.tasks.into_iter().map(|row|view!{<div class="insight-capacity-task"><p>{row.task.title}</p><small class=if row.deadline_overload{"orange"}else{"muted"}>{format!("Due {} · estimate {} · planned {} · free before deadline {} · cumulative due effort {}{}",row.task.due.unwrap_or_else(||"unspecified".into()),row.estimated_minutes.map(|n|n.to_string()).unwrap_or_else(||"unknown".into()),row.planned_minutes.map(|n|n.to_string()).unwrap_or_else(||"unknown".into()),row.available_before_deadline.map(|n|n.to_string()).unwrap_or_else(||"no deadline".into()),row.cumulative_due_minutes.map(|n|n.to_string()).unwrap_or_else(||"—".into()),if row.deadline_overload{" · deadline overloaded"}else{""})}</small><EvidenceLines notice=notice evidence=vec![row.task.evidence]/></div>}).collect_view()}</section>}}).collect_view()}
    }
}
#[component]
fn EffortEditor(
    project: String,
    efforts: RwSignal<Vec<TaskEffort>>,
    snap: Snap,
    notice: RwSignal<String>,
    reload: RwSignal<u64>,
) -> impl IntoView {
    let project = StoredValue::new(project);
    let task = RwSignal::new(String::new());
    let minutes = RwSignal::new(String::new());
    let note = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    view! {<form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let Ok(value)=minutes.get_untracked().parse::<u32>()else{notice.set("Enter a whole number of effort minutes".into());return;};let version=efforts.get_untracked().iter().find(|e|e.task_id==task.get_untracked()).map(|e|e.version).unwrap_or(0);submit(format!("/api/v1/projects/{}/efforts/{}",project.get_value(),task.get_untracked()),"PUT",json!({"expected_version":version,"minutes":value,"note":note.get_untracked(),"idempotency_key":new_id()}),busy,snap,notice,reload);}>
    <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Accepted work"<select required prop:value=move||task.get() on:change=move|ev|{let id=event_target_value(&ev);let existing=efforts.get_untracked().into_iter().find(|e|e.task_id==id);minutes.set(existing.as_ref().map(|e|e.minutes.to_string()).unwrap_or_default());note.set(existing.map(|e|e.note).unwrap_or_default());task.set(id);}>
    <option value="">"Select an accepted task"</option>{move||scoped_tasks(snap,&project.get_value()).into_iter().filter(|t|t.status=="accepted").map(|t|{let existing=efforts.get().iter().find(|e|e.task_id==t.id).map(|e|e.minutes.to_string()).unwrap_or_else(||"unknown".into());view!{<option value=t.id>{format!("{} · {} minutes",t.title,existing)}</option>}}).collect_view()}</select></label><label>"Estimated personal effort in minutes"</label><input required type="number" min="1" max="100000" prop:value=move||minutes.get() on:input=move|ev|minutes.set(event_target_value(&ev))/><label>"Estimate assumptions"</label><input maxlength="2000" prop:value=move||note.get() on:input=move|ev|note.set(event_target_value(&ev))/><button class="button secondary" type="submit">"Save effort estimate"</button></fieldset></form>}
}
