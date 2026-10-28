use super::workflows::{Citations, Pending, action, chosen_project};
use super::*;
use std::{cell::Cell, rc::Rc};
fn picked(
    snap: Snap,
    project: RwSignal<String>,
    doc: &str,
    line: &str,
) -> Result<Vec<Evidence>, String> {
    if doc.is_empty() {
        return Ok(vec![]);
    }
    let p = chosen_project(snap, project).ok_or("Choose a project")?;
    let d = snap
        .get_untracked()
        .and_then(|s| {
            s.documents
                .into_iter()
                .find(|d| d.id == doc && p.source_ids.contains(&d.source_id))
        })
        .ok_or("Choose a project source")?;
    let n = line
        .parse::<i64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("Choose a positive source line")?;
    let q = d
        .body
        .lines()
        .nth(n as usize - 1)
        .ok_or("Source line does not exist")?;
    if q.trim().is_empty() || q.len() > 2000 {
        return Err("Choose a nonempty line up to 2,000 bytes".into());
    }
    Ok(vec![Evidence {
        document_id: d.id,
        revision: d.revision,
        line: n,
        quote: q.into(),
    }])
}
#[component]
fn EvidencePicker(
    snap: Snap,
    project: RwSignal<String>,
    doc: RwSignal<String>,
    line: RwSignal<String>,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    view! {<div class="waiting-evidence-picker"><label>"Source"<select prop:value=move||doc.get() on:change=move|ev|{doc.set(event_target_value(&ev));line.set("1".into());}><option value="">"Choose source…"</option>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.documents.into_iter().filter(|d|p.source_ids.contains(&d.source_id)).map(|d|view!{<option value=d.id>{format!("{} · r{}",d.title,d.revision)}</option>}).collect_view())}</select></label><label>"Line"<input type="number" min="1" prop:value=move||line.get() on:input=move|ev|line.set(event_target_value(&ev))/></label>{move||match picked(snap,project,&doc.get(),&line.get()){Ok(e)=>view!{<Citations evidence=e selected=selected notice=notice/>}.into_any(),Err(e)=>view!{<small class="muted">{e}</small>}.into_any()}}</div>}
}
#[component]
pub(super) fn Discovery(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    project: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let cues = RwSignal::new(None::<CuePage>);
    let gaps = RwSignal::new(None::<GapPage>);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let loading = RwSignal::new(false);
    let active = RwSignal::new(String::new());
    let cue_filter = RwSignal::new("active".to_string());
    let gap_filter = RwSignal::new("active".to_string());
    let cue_offset = RwSignal::new(0usize);
    let gap_offset = RwSignal::new(0usize);
    let current_cue = RwSignal::new(String::new());
    let current_gap = RwSignal::new(String::new());
    let cue_text = RwSignal::new(String::new());
    let cue_kind = RwSignal::new("question".to_string());
    let cue_note = RwSignal::new(String::new());
    let cue_doc = RwSignal::new(String::new());
    let cue_line = RwSignal::new("1".to_string());
    let conflict_doc = RwSignal::new(String::new());
    let conflict_line = RwSignal::new("1".to_string());
    let gap_question = RwSignal::new(String::new());
    let missing = RwSignal::new(String::new());
    let required = RwSignal::new(Vec::<String>::new());
    let answer = RwSignal::new(String::new());
    let answer_doc = RwSignal::new(String::new());
    let answer_line = RwSignal::new("1".to_string());
    let answer_version = RwSignal::new(None::<i64>);
    let answer_gap = RwSignal::new(String::new());
    let answer_pinned = RwSignal::new(None::<Evidence>);
    let resolution = RwSignal::new(String::new());
    let resolution_doc = RwSignal::new(String::new());
    let resolution_line = RwSignal::new("1".to_string());
    let resolution_version = RwSignal::new(None::<i64>);
    let resolution_cue = RwSignal::new(String::new());
    let history_key = RwSignal::new(None::<(String, String)>);
    let history_offset = RwSignal::new(0usize);
    let history = RwSignal::new(None::<DiscoveryHistoryPage>);
    let serial = Rc::new(Cell::new(0u64));
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let clean = alive.clone();
    on_cleanup(move || clean.store(false, std::sync::atomic::Ordering::Relaxed));
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        let _ = tick.get();
        let p = chosen_project(snap, project);
        let cf = cue_filter.get();
        let gf = gap_filter.get();
        let co = cue_offset.get();
        let go = gap_offset.get();
        let hk = history_key.get();
        let ho = history_offset.get();
        let Some(p) = p else {
            return;
        };
        let changed = active.get_untracked() != p.id;
        if changed {
            active.set(p.id.clone());
            cues.set(None);
            gaps.set(None);
            current_cue.set(String::new());
            current_gap.set(String::new());
            cue_offset.set(0);
            gap_offset.set(0);
            cue_text.set(String::new());
            cue_note.set(String::new());
            cue_doc.set(String::new());
            conflict_doc.set(String::new());
            gap_question.set(String::new());
            missing.set(String::new());
            required.set(p.source_ids.clone());
            answer.set(String::new());
            answer_doc.set(String::new());
            answer_version.set(None);
            answer_pinned.set(None);
            resolution.set(String::new());
            resolution_doc.set(String::new());
            resolution_version.set(None);
            history_key.set(None);
            history.set(None);
            pending.set(None);
        }
        let n = serial.get() + 1;
        serial.set(n);
        let serial = serial.clone();
        let alive = alive.clone();
        loading.set(true);
        spawn_local(async move {
            if changed {
                let _ = api(
                    &format!("/api/v1/projects/{}/discovery/refresh", p.id),
                    "POST",
                    Some(json!({})),
                )
                .await;
            }
            let a = api(
                &format!(
                    "/api/v1/projects/{}/cues?status={cf}&offset={}&limit=25",
                    p.id,
                    if changed { 0 } else { co }
                ),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<CuePage>(v)
                    .map_err(|_| "Could not read question cues".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match a {
                Ok(v) => cues.set(Some(v)),
                Err(e) => notice.set(e),
            }
            let a = api(
                &format!(
                    "/api/v1/projects/{}/gaps?status={gf}&offset={}&limit=10",
                    p.id,
                    if changed { 0 } else { go }
                ),
                "GET",
                None,
            )
            .await
            .and_then(|v| {
                serde_json::from_value::<GapPage>(v)
                    .map_err(|_| "Could not read knowledge gaps".into())
            });
            if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                return;
            }
            match a {
                Ok(v) => gaps.set(Some(v)),
                Err(e) => notice.set(e),
            }
            if let Some((kind, id)) = hk {
                let a = api(
                    &format!("/api/v1/discovery/{kind}/{id}/history?offset={ho}&limit=5"),
                    "GET",
                    None,
                )
                .await
                .and_then(|v| {
                    serde_json::from_value::<DiscoveryHistoryPage>(v)
                        .map_err(|_| "Could not read review history".into())
                });
                if !alive.load(std::sync::atomic::Ordering::Relaxed) || serial.get() != n {
                    return;
                }
                match a {
                    Ok(v) => history.set(Some(v)),
                    Err(e) => notice.set(e),
                }
            }
            loading.set(false);
        });
    });
    let chosen_cue = move || {
        cues.get().and_then(|p| {
            p.items
                .iter()
                .find(|c| c.id == current_cue.get())
                .or_else(|| p.items.first())
                .cloned()
        })
    };
    let chosen_gap = move || {
        gaps.get().and_then(|p| {
            p.items
                .iter()
                .find(|g| g.id == current_gap.get())
                .or_else(|| p.items.first())
                .cloned()
        })
    };
    Effect::new(move |_| {
        if let Some(g) = chosen_gap() {
            if answer_gap.get_untracked() != g.id {
                answer_gap.set(g.id);
                answer.set(String::new());
                answer_doc.set(String::new());
                answer_version.set(None);
                answer_pinned.set(None);
            }
        }
        if let Some(c) = chosen_cue() {
            if resolution_cue.get_untracked() != c.id {
                resolution_cue.set(c.id);
                resolution.set(String::new());
                resolution_doc.set(String::new());
                resolution_version.set(None);
            }
        }
    });
    let create_cue = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let mut e = match picked(
            snap,
            project,
            &cue_doc.get_untracked(),
            &cue_line.get_untracked(),
        ) {
            Ok(e) => e,
            Err(e) => {
                notice.set(e);
                return;
            }
        };
        if cue_kind.get_untracked() == "conflict" {
            match picked(
                snap,
                project,
                &conflict_doc.get_untracked(),
                &conflict_line.get_untracked(),
            ) {
                Ok(other) => e.extend(other),
                Err(e) => {
                    notice.set(e);
                    return;
                }
            }
        }
        let id = p.id.clone();
        action(
            "/api/v1/cues".into(),
            "POST",
            json!({"project_id":p.id,"kind":cue_kind.get_untracked(),"text":cue_text.get_untracked(),"note":cue_note.get_untracked(),"evidence":e}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if !chosen_project(snap, project).is_some_and(|p| p.id == id) {
                    return;
                }
                current_cue.set(v["id"].as_str().unwrap_or_default().into());
                cue_text.set(String::new());
                cue_note.set(String::new());
                cue_doc.set(String::new());
                conflict_doc.set(String::new());
                cue_filter.set("active".into());
                cue_offset.set(0);
                tick.update(|n| *n += 1);
            },
        );
    };
    let create_gap = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(p) = chosen_project(snap, project) else {
            return;
        };
        let id = p.id.clone();
        action(
            "/api/v1/gaps".into(),
            "POST",
            json!({"project_id":p.id,"question":gap_question.get_untracked(),"missing_source":missing.get_untracked(),"required_source_ids":required.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |v| {
                if !chosen_project(snap, project).is_some_and(|p| p.id == id) {
                    return;
                }
                current_gap.set(v["id"].as_str().unwrap_or_default().into());
                gap_question.set(String::new());
                missing.set(String::new());
                gap_filter.set("active".into());
                gap_offset.set(0);
                tick.update(|n| *n += 1);
            },
        );
    };
    let review_cue = move |cue: QuestionCue, status: &'static str| {
        action(
            format!("/api/v1/cues/{}", cue.id),
            "PATCH",
            json!({"expected_version":cue.version,"status":status,"note":"User review","evidence":[]}),
            snap,
            notice,
            busy,
            pending,
            move |_| tick.update(|n| *n += 1),
        );
    };
    let resolve = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(c) = chosen_cue() else {
            return;
        };
        let e = match picked(
            snap,
            project,
            &resolution_doc.get_untracked(),
            &resolution_line.get_untracked(),
        ) {
            Ok(e) => e,
            Err(e) => {
                notice.set(e);
                return;
            }
        };
        action(
            format!("/api/v1/cues/{}", c.id),
            "PATCH",
            json!({"expected_version":resolution_version.get_untracked().unwrap_or(c.version),"status":"resolved","note":resolution.get_untracked(),"evidence":e}),
            snap,
            notice,
            busy,
            pending,
            move |_| {
                resolution.set(String::new());
                resolution_doc.set(String::new());
                resolution_version.set(None);
                tick.update(|n| *n += 1);
            },
        );
    };
    let answer_gap_form = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(g) = chosen_gap() else {
            return;
        };
        let e = if let Some(e) = answer_pinned.get_untracked() {
            vec![e]
        } else {
            match picked(
                snap,
                project,
                &answer_doc.get_untracked(),
                &answer_line.get_untracked(),
            ) {
                Ok(e) => e,
                Err(e) => {
                    notice.set(e);
                    return;
                }
            }
        };
        action(
            format!("/api/v1/gaps/{}", g.id),
            "PATCH",
            json!({"expected_version":answer_version.get_untracked().unwrap_or(g.version),"action":"answer","answer":answer.get_untracked(),"evidence":e,"note":"User-reviewed answer"}),
            snap,
            notice,
            busy,
            pending,
            move |_| {
                answer.set(String::new());
                answer_doc.set(String::new());
                answer_version.set(None);
                answer_pinned.set(None);
                tick.update(|n| *n += 1);
            },
        );
    };
    view! {<div class="brief-layout waiting-layout"><aside class="brief-sidebar"><section class="panel brief-create"><div class="section-heading"><h2>"Capture a question"</h2></div><details><summary>"Question / conflicting statements"</summary><form on:submit=create_cue><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Kind"<select prop:value=move||cue_kind.get() on:change=move|ev|cue_kind.set(event_target_value(&ev))><option value="question">"Unanswered question"</option><option value="conflict">"Conflicting statements (your assessment)"</option></select></label><label>"Question / conflict"<textarea required maxlength="1000" prop:value=move||cue_text.get() on:input=move|ev|cue_text.set(event_target_value(&ev))></textarea></label><EvidencePicker snap=snap project=project doc=cue_doc line=cue_line notice=notice selected=selected/><Show when=move||cue_kind.get()=="conflict"><p class="muted">"Second statement:"</p><EvidencePicker snap=snap project=project doc=conflict_doc line=conflict_line notice=notice selected=selected/></Show><label>"Context"<textarea maxlength="2000" prop:value=move||cue_note.get() on:input=move|ev|cue_note.set(event_target_value(&ev))></textarea></label><button type="submit" class="button primary">"Save question cue"</button></fieldset></form></details>
    <details open><summary>"Save a knowledge gap"</summary><form on:submit=create_gap><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Question without enough evidence"<textarea required maxlength="1000" prop:value=move||gap_question.get() on:input=move|ev|gap_question.set(event_target_value(&ev))></textarea></label><label>"What source or information is missing?"<textarea required maxlength="1000" prop:value=move||missing.get() on:input=move|ev|missing.set(event_target_value(&ev)) placeholder="E.g. rollout decision from the platform meeting"></textarea></label><fieldset class="runtime-sources"><legend>"Revisit when these sources change"</legend>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.sources.into_iter().filter(|s|p.source_ids.contains(&s.id)).map(|s|{let id=s.id.clone();let check=s.id;view!{<label><input type="checkbox" prop:checked=move||required.get().contains(&check) on:change=move|ev|{let checked=event_target_checked(&ev);required.update(|ids|{ids.retain(|s|s!=&id);if checked{ids.push(id.clone());}});}/>{s.name}</label>}}).collect_view())}</fieldset><button type="submit" class="button primary">"Save gap"</button></fieldset></form></details></section></aside>
    <div class="brief-main"><section class="panel"><div class="section-heading"><h2>"Questions & gaps"</h2><span>{move||if loading.get(){"Refreshing…"}else{"Local rules · no model usage"}}</span></div><p class="muted">"Explicit question lines become candidates; whether they are unanswered needs your review. Conflicts are linked by you. Possible gap evidence is matched by words and requires review."</p><div class="workflow-actions"><button class="button secondary" disabled=move||busy.get() on:click=move|_|{let Some(p)=chosen_project(snap,project)else{return;};busy.set(true);spawn_local(async move{let result=api(&format!("/api/v1/projects/{}/discovery/refresh",p.id),"POST",Some(json!({}))).await;if busy.try_get_untracked().is_none(){return;}if active.try_get_untracked().as_deref()!=Some(p.id.as_str()){busy.set(false);return;}busy.set(false);match result{Ok(v)=>{notice.set(format!("{} question cues, {} possible gap passages; {} question lines / {} passages omitted",v["cues_created"],v["suggestions_created"],v["cue_lines_omitted"],v["suggestions_omitted"]));refresh(snap,notice);tick.update(|n|*n+=1);},Err(e)=>notice.set(e)}});}>"Revisit changed sources"</button><button class="button text" disabled=move||busy.get() on:click=move|_|{let Some(p)=chosen_project(snap,project)else{return;};let muted=cues.get_untracked().is_some_and(|c|c.muted);action(format!("/api/v1/projects/{}/discovery/mute",p.id),"PUT",json!({"muted":!muted}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}>{move||if cues.get().is_some_and(|c|c.muted){"Unmute cues"}else{"Mute cues"}}</button><span class="muted">{move||if cues.get().is_some_and(|c|c.muted){"Muted · new cues retained quietly"}else{"Quiet inbox · no meeting interruptions"}}</span></div></section>
    <section class="panel"><div class="section-heading"><h2>"Question cues"</h2><span>{move||cues.get().map(|c|c.total.to_string()).unwrap_or_default()}</span></div><label>"Status"<select prop:value=move||cue_filter.get() on:change=move|ev|{cue_filter.set(event_target_value(&ev));cue_offset.set(0);current_cue.set(String::new());}><option value="active">"Needs review / answer"</option><option value="all">"All"</option><option value="dismissed">"Dismissed"</option><option value="resolved">"Resolved"</option></select></label>
    {move||cues.get().map(|c|c.items.into_iter().map(|q|{let id=q.id;view!{<div class="memory-line"><button class="task-title" on:click=move|_|current_cue.set(id.clone())>{q.text}</button><small>{format!("{} · {}",q.kind,q.status)}</small></div>}}).collect_view())}
    <div class="workflow-actions"><button class="button text" disabled=move||cue_offset.get()==0 on:click=move|_|cue_offset.update(|n|*n=n.saturating_sub(25))>"← Previous"</button><button class="button text" disabled=move||cues.get().is_none_or(|c|c.next_offset.is_none()) on:click=move|_|{if let Some(n)=cues.get_untracked().and_then(|c|c.next_offset){cue_offset.set(n);}}>"Next →"</button></div>
    {move||chosen_cue().map(|c|{let mark=c.clone();let dismiss=c.clone();let reopen=c.clone();let id=c.id;view!{<div class="thread-context-section"><h3>{c.text}</h3><p class="muted">{format!("{} · {} · v{}",c.contributor,c.status,c.version)}</p><Citations evidence=c.original_evidence.clone() selected=selected notice=notice/>{(c.evidence!=c.original_evidence).then(||view!{<Citations evidence=c.evidence selected=selected notice=notice/>})}<p>{c.note}</p><div class="workflow-actions"><button class="button secondary" disabled=move||busy.get() on:click=move|_|review_cue(mark.clone(),"needs_answer")>"Confirm needs an answer"</button><button class="button text" disabled=move||busy.get() on:click=move|_|review_cue(dismiss.clone(),"dismissed")>"Dismiss quietly"</button><button class="button text" disabled=move||busy.get() on:click=move|_|review_cue(reopen.clone(),"unreviewed")>"Reopen for review"</button><button class="button text" on:click=move|_|{history_key.set(Some(("cue".into(),id.clone())));history_offset.set(0);}>"History"</button></div><details><summary>"Record an evidence-backed resolution"</summary><form on:submit=resolve><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Resolution / answer"<textarea required maxlength="2000" prop:value=move||resolution.get() on:input=move|ev|{if resolution_version.get_untracked().is_none(){resolution_version.set(chosen_cue().map(|c|c.version));}resolution.set(event_target_value(&ev));}></textarea></label><EvidencePicker snap=snap project=project doc=resolution_doc line=resolution_line notice=notice selected=selected/><button type="submit" class="button primary">"Mark resolved with this evidence"</button></fieldset></form></details></div>}})}
    </section>
    <section class="panel"><div class="section-heading"><h2>"Knowledge gap inbox"</h2><span>{move||gaps.get().map(|g|g.total.to_string()).unwrap_or_default()}</span></div><label>"Status"<select prop:value=move||gap_filter.get() on:change=move|ev|{gap_filter.set(event_target_value(&ev));gap_offset.set(0);current_gap.set(String::new());}><option value="active">"Open / new evidence"</option><option value="all">"All"</option><option value="answered">"Answered"</option><option value="archived">"Archived"</option></select></label>
    {move||gaps.get().map(|p|p.items.into_iter().map(|g|{let id=g.id;view!{<div class="memory-line"><button class="task-title" on:click=move|_|current_gap.set(id.clone())>{g.question}</button><small>{g.status}</small></div>}}).collect_view())}
    <div class="workflow-actions"><button class="button text" disabled=move||gap_offset.get()==0 on:click=move|_|gap_offset.update(|n|*n=n.saturating_sub(10))>"← Previous"</button><button class="button text" disabled=move||gaps.get().is_none_or(|g|g.next_offset.is_none()) on:click=move|_|{if let Some(n)=gaps.get_untracked().and_then(|g|g.next_offset){gap_offset.set(n);}}>"Next →"</button></div>
    {move||chosen_gap().map(|g|{let id=g.id.clone();let archive=g.clone();let reopen=g.clone();let gap_id=g.id.clone();let gap_version=g.version;let is_active=["open","needs_review"].contains(&g.status.as_str());view!{<div class="thread-context-section"><h3>{g.question}</h3><p>{format!("Missing: {}",g.missing_source)}</p><p class="muted">{format!("{} · v{} · {} watched source(s) · {} omitted passages",g.status,g.version,g.required_source_ids.len(),g.suggestions_omitted)}</p>{g.answer.map(|a|view!{<p>{a}</p><Citations evidence=g.answer_evidence selected=selected notice=notice/>})}
    <div class="workflow-actions"><button class="button text" disabled=move||busy.get()||!is_active on:click=move|_|{action(format!("/api/v1/gaps/{}",archive.id),"PATCH",json!({"expected_version":archive.version,"action":"archive","note":"User archived this question"}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}>"Archive"</button><button class="button text" disabled=move||busy.get()||is_active on:click=move|_|{action(format!("/api/v1/gaps/{}",reopen.id),"PATCH",json!({"expected_version":reopen.version,"action":"reopen","note":"User reopened this question"}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}>"Reopen"</button><button class="button text" on:click=move|_|{history_key.set(Some(("gap".into(),id.clone())));history_offset.set(0);}>"History"</button></div>
    <h4>"Possible relevant passages · review required"</h4>{g.suggestions.into_iter().map(|s|{let gid=gap_id.clone();let sid=s.id.clone();let gid2=gap_id.clone();let sid2=s.id;let ev=s.evidence.clone();let use_evidence=s.evidence.clone();view!{<article class="observation"><small>{format!("{} · matched: {}",s.status,s.matching_terms.join(", "))}</small><Citations evidence=vec![ev] selected=selected notice=notice/><div class="workflow-actions"><button class="button text" disabled=move||busy.get()||!is_active on:click=move|_|{action(format!("/api/v1/gaps/{gid}"),"PATCH",json!({"expected_version":gap_version,"action":"accept_suggestion","suggestion_id":sid,"note":"User marked passage relevant; answer still requires review"}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}>"Relevant"</button><button class="button text" disabled=move||busy.get()||!is_active on:click=move|_|{action(format!("/api/v1/gaps/{gid2}"),"PATCH",json!({"expected_version":gap_version,"action":"dismiss_suggestion","suggestion_id":sid2,"note":"User dismissed candidate passage"}),snap,notice,busy,pending,move|_|tick.update(|n|*n+=1));}>"Dismiss"</button><button class="button text" disabled=!is_active on:click=move|_|{answer_doc.set(use_evidence.document_id.clone());answer_line.set(use_evidence.line.to_string());answer_version.set(Some(gap_version));answer_pinned.set(Some(use_evidence.clone()));}>"Use for answer"</button></div></article>}}).collect_view()}
    <Show when=move||is_active><details><summary>"Record a reviewed answer"</summary><form on:submit=answer_gap_form><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Answer"<textarea required maxlength="4000" prop:value=move||answer.get() on:input=move|ev|{if answer_version.get_untracked().is_none(){answer_version.set(chosen_gap().map(|g|g.version));}answer.set(event_target_value(&ev));}></textarea></label>{move||if let Some(e)=answer_pinned.get(){view!{<Citations evidence=vec![e] selected=selected notice=notice/><button class="button text" type="button" on:click=move|_|answer_pinned.set(None)>"Choose different evidence"</button>}.into_any()}else{view!{<EvidencePicker snap=snap project=project doc=answer_doc line=answer_line notice=notice selected=selected/>}.into_any()}}<p class="muted">"Relevance does not establish sufficiency. Only your reviewed answer closes the gap."</p><button type="submit" class="button primary">"Save answer with evidence"</button></fieldset></form></details></Show>
    </div>}})}
    <GapScopeEditor gap=Signal::derive(chosen_gap) snap=snap project=project notice=notice tick=tick/></section>
    {move||history.get().map(|h|view!{<section class="panel"><div class="section-heading"><h2>"Question review history"</h2><span>{h.total}</span></div>{h.items.into_iter().map(|e|view!{<details><summary>{format!("{} · {} · {}",e.action,e.contributor,e.created_at)}</summary><p>{e.note}</p><pre>{serde_json::to_string_pretty(&e.snapshot).unwrap_or_default()}</pre></details>}).collect_view()}<div class="workflow-actions"><button class="button text" disabled=move||history_offset.get()==0 on:click=move|_|history_offset.update(|n|*n=n.saturating_sub(5))>"← Newer"</button><button class="button text" disabled=h.next_offset.is_none() on:click=move|_|{if let Some(n)=h.next_offset{history_offset.set(n);}}>"Older →"</button></div></section>})}
    </div></div>}
}

#[component]
fn GapScopeEditor(
    gap: Signal<Option<KnowledgeGap>>,
    snap: Snap,
    project: RwSignal<String>,
    notice: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let question = RwSignal::new(None::<String>);
    let missing = RwSignal::new(None::<String>);
    let scope = RwSignal::new(None::<Vec<String>>);
    let note = RwSignal::new(String::new());
    let version = RwSignal::new(None::<i64>);
    let editing = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let reset = move || {
        question.set(None);
        missing.set(None);
        scope.set(None);
        note.set(String::new());
        version.set(None);
        pending.set(None);
    };
    Effect::new(move |_| {
        let id = gap.get().map(|g| g.id).unwrap_or_default();
        if editing.get_untracked() != id {
            editing.set(id);
            reset();
        }
    });
    let capture = move || {
        if version.get_untracked().is_none() {
            version.set(gap.get_untracked().map(|g| g.version));
        }
    };
    let sources = move || {
        scope.get().unwrap_or_else(|| {
            gap.get()
                .zip(chosen_project(snap, project))
                .map(|(g, p)| {
                    g.required_source_ids
                        .into_iter()
                        .filter(|id| p.source_ids.contains(id))
                        .collect()
                })
                .unwrap_or_default()
        })
    };
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(g) = gap.get_untracked() else {
            return;
        };
        let id = g.id.clone();
        action(
            format!("/api/v1/gaps/{}", g.id),
            "PATCH",
            json!({"expected_version":version.get_untracked().unwrap_or(g.version),"action":"edit","question":question.get_untracked().unwrap_or(g.question),"missing_source":missing.get_untracked().unwrap_or(g.missing_source),"required_source_ids":sources(),"note":note.get_untracked()}),
            snap,
            notice,
            busy,
            pending,
            move |_| {
                if gap.get_untracked().is_some_and(|g| g.id == id) {
                    reset();
                }
                tick.update(|n| *n += 1);
            },
        );
    };
    view! {<Show when=move||gap.get().is_some()><details class="workflow-manage"><summary>"Edit question, missing information & watched sources"</summary><p class="muted">"Select current project sources to revisit. Sources removed from the project leave this selection; their historical evidence permissions remain retained. Saving schedules current selected revisions for the next local refresh."</p><form on:submit=submit><fieldset class="focus-form-fields" disabled=move||busy.get()||gap.get().is_none_or(|g|!["open","needs_review"].contains(&g.status.as_str()))><label>"Question"<textarea required maxlength="1000" prop:value=move||question.get().unwrap_or_else(||gap.get().map(|g|g.question).unwrap_or_default()) on:input=move|ev|{capture();question.set(Some(event_target_value(&ev)));}></textarea></label><label>"Missing source or information"<textarea required maxlength="1000" prop:value=move||missing.get().unwrap_or_else(||gap.get().map(|g|g.missing_source).unwrap_or_default()) on:input=move|ev|{capture();missing.set(Some(event_target_value(&ev)));}></textarea></label><fieldset class="runtime-sources"><legend>"Revisit when selected sources change"</legend>{move||snap.get().zip(chosen_project(snap,project)).map(|(s,p)|s.sources.into_iter().filter(|s|p.source_ids.contains(&s.id)).map(|s|{let id=s.id.clone();let check=s.id;view!{<label><input type="checkbox" prop:checked=move||sources().contains(&check) on:change=move|ev|{capture();let yes=event_target_checked(&ev);let mut ids=sources();ids.retain(|s|s!=&id);if yes{ids.push(id.clone());}scope.set(Some(ids));}/>{s.name}</label>}}).collect_view())}</fieldset><label>"Change reason"<textarea required maxlength="2000" prop:value=move||note.get() on:input=move|ev|{capture();note.set(event_target_value(&ev));}></textarea></label><button type="submit" class="button primary">"Save watched scope and question"</button><button type="button" class="button text" on:click=move|_|reset()>"Discard edits / use current version"</button></fieldset></form></details></Show>}
}
