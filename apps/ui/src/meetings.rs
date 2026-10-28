use super::workflows::{Citations, Pending, ProjectSources, action, chosen_project};
use super::*;

fn series(snap: Snap, id: RwSignal<String>) -> Option<MeetingSeries> {
    let items = snap.get()?.meeting_series;
    items
        .iter()
        .find(|s| s.id == id.get())
        .or_else(|| items.first())
        .cloned()
}
#[component]
pub(super) fn Meetings(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    let current = RwSignal::new(String::new());
    let project = RwSignal::new(String::new());
    let name = RwSignal::new(String::new());
    let cadence = RwSignal::new(String::new());
    let source = RwSignal::new(String::new());
    let held_on = RwSignal::new(
        js_sys::Date::new_0()
            .to_iso_string()
            .as_string()
            .unwrap_or_default()
            .chars()
            .take(10)
            .collect::<String>(),
    );
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let prep = RwSignal::new(None::<MeetingPreparation>);
    let loading = RwSignal::new(false);
    let sequence = RwSignal::new(0u64);
    let load = move || {
        let Some(s) = series(snap, current) else {
            prep.set(None);
            return;
        };
        let next = sequence.get_untracked() + 1;
        sequence.set(next);
        loading.set(true);
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/meetings/{}/preparation", s.id),
                "GET",
                None,
            )
            .await;
            if sequence.get_untracked() != next {
                return;
            }
            loading.set(false);
            if !series(snap, current).is_some_and(|now| now.id == s.id) {
                return;
            }
            match result {
                Ok(value) => match serde_json::from_value(value) {
                    Ok(data) => {
                        prep.set(Some(data));
                    }
                    Err(_) => notice.set("Could not read meeting preparation".into()),
                },
                Err(e) => notice.set(e),
            }
        });
    };
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = current.get();
        load();
    });
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            notice.set("Create a project in Briefs first.".into());
            return;
        };
        action(
            "/api/v1/meetings".into(),
            "POST",
            json!({"project_id":p.id,"name":name.get_untracked(),"cadence":cadence.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |value| {
                current.set(value["id"].as_str().unwrap_or_default().into());
                name.set(String::new());
                cadence.set(String::new());
                source.set(String::new());
                prep.set(None);
            },
        );
    };
    let link = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(s) = series(snap, current) else {
            return;
        };
        let Some(doc) = snap.get_untracked().and_then(|data| {
            data.documents
                .into_iter()
                .find(|d| d.source_id == source.get_untracked())
        }) else {
            notice.set("Choose a transcript or meeting note.".into());
            return;
        };
        action(
            format!("/api/v1/meetings/{}/occurrences", s.id),
            "POST",
            json!({"expected_version":s.version,"source_id":doc.source_id,"revision":doc.revision,"held_on":held_on.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |_| source.set(String::new()),
        );
    };
    view! {
        <div class="brief-layout workflow-layout">
            <aside class="brief-sidebar">
                <section class="panel"><div class="section-heading"><h2>"Meeting series"</h2></div>{move||snap.get().map(|s|s.meeting_series.into_iter().map(|s|{let id=s.id.clone();let active=s.id;view!{<button class="brief-project" class:active=move||series(snap,current).is_some_and(|s|s.id==active) on:click=move|_|{current.set(id.clone());source.set(String::new());prep.set(None);}>
                    <span>{s.name}</span><small>{s.cadence}</small></button>}}).collect_view())}</section>
                <section class="panel brief-create"><div class="section-heading"><h2>"New series"</h2></div><form on:submit=create><label for="meeting-project">"Project"</label><select id="meeting-project" prop:value=move||chosen_project(snap,project).map(|p|p.id).unwrap_or_default() on:change=move|ev|project.set(event_target_value(&ev))>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select>
                    <label for="meeting-name">"Name"</label><input id="meeting-name" required maxlength="200" prop:value=move||name.get() on:input=move|ev|name.set(event_target_value(&ev)) placeholder="Release review"/>
                    <label for="meeting-cadence">"Cadence (optional)"</label><input id="meeting-cadence" maxlength="200" prop:value=move||cadence.get() on:input=move|ev|cadence.set(event_target_value(&ev)) placeholder="Weekly · Fridays"/><small class="muted">"A label for your series. Occurrences are linked manually."</small><button class="button secondary" disabled=move||busy.get() type="submit">"Create meeting series"</button>
                </form></section>
            </aside>
            <div class="brief-main">
                {move||series(snap,current).map(|s|{
                    let id=s.id.clone();let project=snap.get().and_then(|data|data.briefs.into_iter().find(|p|p.id==s.project_id));let scope=project.as_ref().map(|p|p.source_ids.clone()).unwrap_or_default();
                    view!{
                        <section class="panel"><div class="section-heading"><h2>{s.name.clone()}</h2><span>{s.cadence.clone()}</span></div>
                            <p class="muted">"Link a completed meeting transcript or note. Its revision and the app's current project state become the baseline for preparation before the next occurrence."</p>
                            <form class="workflow-form" on:submit=link><div class="workflow-actions"><label for="meeting-source">"Transcript / notes"</label><select id="meeting-source" required prop:value=move||source.get() on:change=move|ev|source.set(event_target_value(&ev))><option value="">"Choose source…"</option>{snap.get().map(|data|data.documents.into_iter().filter(|d|scope.contains(&d.source_id) && data.sources.iter().any(|s|s.id==d.source_id && ["transcript","markdown"].contains(&s.kind.as_str()))).map(|d|view!{<option value=d.source_id>{format!("{} · r{}",d.title,d.revision)}</option>}).collect_view())}</select></div>
                                <div class="workflow-actions"><label for="meeting-date">"Held on"</label><input id="meeting-date" type="date" required prop:value=move||held_on.get() on:input=move|ev|held_on.set(event_target_value(&ev))/><button class="button secondary" type="submit" disabled=move||busy.get()>"Link occurrence"</button></div>
                            </form>
                            {project.map(|project|view!{<ProjectSources project=project snap=snap notice=notice/>})}
                        </section>
                        <section class="panel"><div class="section-heading"><h2>"Previous occurrences"</h2><span>{s.occurrences.iter().filter(|o|o.status=="active").count()}</span></div>
                            {s.occurrences.into_iter().map(|o|{let series_id=id.clone();let ev=Evidence{document_id:o.document_id.clone(),revision:o.revision,line:1,quote:String::new()};let occurrence_id=o.id;let next=if o.status=="active"{"removed"}else{"active"};view!{<div class="meeting-occurrence"><span>{o.held_on}</span><button class="source-link" on:click=move|_|super::briefs::open_evidence(ev.clone(),selected,notice)>{format!("{} · r{}",o.title,o.revision)}</button><small>{o.status}</small><button class="button text" disabled=move||busy.get() on:click=move|_|action(format!("/api/v1/meetings/{series_id}/occurrences/{occurrence_id}"),"PATCH",json!({"expected_version":s.version,"status":next}),snap,notice,busy,pending,move|_|{})>{if next=="removed"{"Unlink"}else{"Restore"}}</button></div>}}).collect_view()}
                        </section>
                    }
                })}
                <section class="panel"><div class="section-heading"><h2>"Before the next meeting"</h2><button class="source-link" disabled=move||loading.get() on:click=move|_|load()>"Refresh · local"</button></div>
                    <Show when=move||loading.get()><p class="muted" role="status">"Refreshing preparation…"</p></Show>
                    {move||prep.get().filter(|p|series(snap,current).is_some_and(|s|s.id==p.series_id)).map(|p|{
                        let names=snap.get().map(|s|s.meeting_series.into_iter().flat_map(|s|s.occurrences).map(|o|(o.id,o.title)).collect::<std::collections::HashMap<_,_>>()).unwrap_or_default();
                        view!{
                            {p.baseline.clone().map(|b|view!{<p class="meeting-baseline">{format!("After {} · {}",b.held_on,b.title)}<small>{format!("Baseline saved {}. Changes below compare captured app state, not historical knowledge as of the meeting date.",b.created_at)}</small></p>}.into_any()).unwrap_or_else(||view!{<p class="muted">"No previous occurrence linked. Open commitments and the accepted project brief are shown; source comparisons begin after you link a baseline."</p>}.into_any())}
                            {p.baseline_transcript_changed.then(||view!{<p class="brief-stale">"The linked transcript has a newer revision. The original occurrence remains pinned to its saved evidence."</p>})}
                            <section class="prep-section"><h3>"Unresolved commitments"</h3>{p.commitments.is_empty().then(||view!{<p class="muted">"No accepted or candidate commitments found in project sources."</p>})}
                                {p.commitments.into_iter().map(|c|{let owner=c.task.owner.clone().unwrap_or_else(||"Unassigned".into());let due=c.task.due.clone().unwrap_or_else(||"No due date".into());let origin=c.meeting_id.and_then(|id|names.get(&id).cloned()).unwrap_or_else(||"Project context".into());let complete=c.task.id.clone();let dismiss=c.task.id.clone();let candidate=c.task.status=="candidate";let version=c.task.version;view!{<article class="prep-commitment"><div class="workflow-topline"><span class="orange">{if candidate{"? candidate · review"}else{"+ accepted"}}</span><small>{c.change}</small></div><p>{c.task.title}</p><small>{format!("{owner} · {due} · {origin}")}</small>{c.stale_evidence.then(||view!{<p class="orange">"Source evidence changed · verify the current source."</p>})}<Citations evidence=vec![c.task.evidence] selected=selected notice=notice/>
                                    <div class="workflow-actions"><button class="button secondary" disabled=move||busy.get() on:click=move|_|{if busy.get_untracked(){return;}let complete=complete.clone();busy.set(true);spawn_local(async move{let result=api(&format!("/api/v1/tasks/{complete}"),"PATCH",Some(json!({"status":if candidate{"accepted"}else{"completed"},"expected_version":version}))).await;busy.set(false);match result{Ok(_)=>{notice.set(String::new());refresh(snap,notice);},Err(e)=>notice.set(e)}});}>{if candidate{"Accept action"}else{"Mark complete"}}</button>{candidate.then(||view!{<button class="button text" disabled=move||busy.get() on:click=move|_|mutate(format!("/api/v1/tasks/{dismiss}"),"PATCH",json!({"status":"dismissed","expected_version":version}),snap,notice)>"Dismiss"</button>})}</div>
                                </article>}}).collect_view()}
                            </section>
                            <section class="prep-section"><h3>"Task changes since baseline"</h3>{p.task_changes.is_empty().then(||view!{<p class="muted">"No other task status changes recorded."</p>})}{p.task_changes.into_iter().map(|c|view!{<article class="prep-commitment"><p>{format!("{} · {}",c.task.status,c.task.title)}</p><small>{c.change}</small><Citations evidence=vec![c.task.evidence] selected=selected notice=notice/></article>}).collect_view()}</section>
                            <section class="prep-section"><h3>"Waiting for responses"</h3>{p.waiting_for.is_empty().then(||view!{<p class="muted">"No active waiting requests recorded for this project."</p>})}{p.waiting_for.into_iter().map(|w|view!{<article class="prep-commitment"><p>{format!("{} · {}",w.person,w.title)}</p><small class:orange=w.follow_up_due>{format!("Follow-up {}{}",w.follow_up_on,if w.follow_up_due{" · due"}else{""})}</small>{w.stale_evidence.then(||view!{<p class="orange">"Original request evidence changed; verify it before following up."</p>})}<Citations evidence=w.evidence selected=selected notice=notice/></article>}).collect_view()}</section>
                            <section class="prep-section"><h3>"Questions and blockers"</h3>{p.brief.map(|b|view!{
                                <p class="muted">{format!("Accepted project brief r{}{}",b.revision,if b.stale{" · STALE · refresh in Briefs"}else{""})}</p>
                                {b.sections.into_iter().filter(|s|["questions","blockers"].contains(&s.kind.as_str())).map(|s|view!{<h4>{if s.kind=="questions"{"Open questions"}else{"Blockers"}}</h4>{s.items.is_empty().then(||view!{<p class="muted">"No supported items in the reviewed brief."</p>})}{s.items.into_iter().map(|i|view!{<article class="prep-commitment"><p>{i.text}</p><Citations evidence=i.evidence selected=selected notice=notice/></article>}).collect_view()}}).collect_view()}
                            }.into_any()).unwrap_or_else(||view!{<p class="muted">"No accepted project brief. Generate and review one in Briefs to carry its cited questions and blockers here."</p>}.into_any())}</section>
                            <section class="prep-section"><h3>"Source changes since baseline"</h3>{p.source_changes.is_empty().then(||view!{<p class="muted">"No changed sources in the captured comparison."</p>})}{p.source_changes.into_iter().map(|c|{let old=c.from_revision;let read=c.document_id.clone();let latest=c.to_revision;view!{<article class="prep-commitment"><h4>{c.title}</h4><p class="muted">{format!("{} → {}",old.map(|r|format!("r{r}")).unwrap_or_else(||"New source".into()),latest.map(|r|format!("r{r}")).unwrap_or_else(||"Unavailable".into()))}</p><Citations evidence=c.excerpts selected=selected notice=notice/>{c.excerpts_omitted.then(||view!{<small class="orange">"Showing a sample of added lines. More changes may exist."</small>})}<div class="workflow-actions">{old.map(|r|{let id=read.clone();view!{<button class="source-link" on:click=move|_|super::briefs::open_evidence(Evidence{document_id:id.clone(),revision:r,line:1,quote:String::new()},selected,notice)>"Baseline source"</button>}})}{latest.map(|r|view!{<button class="source-link" on:click=move|_|super::briefs::open_evidence(Evidence{document_id:read.clone(),revision:r,line:1,quote:String::new()},selected,notice)>"Current source"</button>})}</div></article>}}).collect_view()}</section>
                            {p.omissions.into_iter().map(|text|view!{<p class="orange">{text}</p>}).collect_view()}<small class="muted">{format!("Prepared locally {} · watermark {}",p.created_at,p.watermark)}</small>
                        }
                    })}
                    <Show when=move||series(snap,current).is_none()><p class="muted">"Create a meeting series linked to a project to prepare for its next occurrence."</p></Show>
                </section>
            </div>
        </div>
    }
}
