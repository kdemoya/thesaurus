use super::workflows::{Citations, Pending, action, chosen_project};
use super::*;
fn prefixes(s: String) -> Vec<String> {
    s.lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
#[component]
pub(super) fn Repositories(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    let project = RwSignal::new(String::new());
    let list = RwSignal::new(Vec::<RepositoryConnection>::new());
    let repo = RwSignal::new(None::<RepositoryConnection>);
    let name = RwSignal::new(String::new());
    let path = RwSignal::new(String::new());
    let initial_ref = RwSignal::new("HEAD".to_string());
    let include = RwSignal::new(String::new());
    let exclude = RwSignal::new(String::new());
    let external = RwSignal::new(false);
    let service = RwSignal::new(String::new());
    let args = RwSignal::new("[]".to_string());
    let language = RwSignal::new("rust".to_string());
    let approve = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let reports = RwSignal::new(Vec::<RepositoryReport>::new());
    let symbols = RwSignal::new(Vec::<SymbolQueryResult>::new());
    let to_ref = RwSignal::new("HEAD".to_string());
    let advance = RwSignal::new(false);
    let file = RwSignal::new(String::new());
    let line = RwSignal::new("0".to_string());
    let character = RwSignal::new("0".to_string());
    let method = RwSignal::new("definition".to_string());
    let approve_query = RwSignal::new(false);
    let epoch = RwSignal::new(0u64);
    let load_epoch = RwSignal::new(0u64);
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        epoch.update(|n| *n += 1);
        let ticket = epoch.get_untracked();
        spawn_local(async move {
            let response = api("/api/v1/repositories", "GET", None).await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match response {
                Ok(v) => {
                    if let Ok(rows) = serde_json::from_value::<Vec<RepositoryConnection>>(v) {
                        if let Some(old) = repo.get_untracked() {
                            if let Some(new) = rows.iter().find(|r| r.id == old.id) {
                                repo.set(Some(new.clone()));
                            }
                        }
                        list.set(rows);
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    });
    let load = move || {
        if let Some(r) = repo.try_get_untracked().flatten() {
            load_epoch.update(|n| *n += 1);
            let ticket = load_epoch.get_untracked();
            spawn_local(async move {
                let data = api(
                    &format!("/api/v1/repositories/{}/reports", r.id),
                    "GET",
                    None,
                )
                .await;
                if load_epoch.try_get_untracked() != Some(ticket)
                    || repo
                        .try_get_untracked()
                        .flatten()
                        .is_none_or(|x| x.id != r.id)
                {
                    return;
                }
                match data {
                    Ok(v) => {
                        if repo
                            .try_get_untracked()
                            .flatten()
                            .is_some_and(|x| x.id == r.id)
                        {
                            if let Ok(rows) = serde_json::from_value(v) {
                                reports.set(rows);
                            }
                        }
                    }
                    Err(e) => notice.set(e),
                }
                let data = api(
                    &format!("/api/v1/repositories/{}/symbols", r.id),
                    "GET",
                    None,
                )
                .await;
                if load_epoch.try_get_untracked() != Some(ticket)
                    || repo
                        .try_get_untracked()
                        .flatten()
                        .is_none_or(|x| x.id != r.id)
                {
                    return;
                }
                match data {
                    Ok(v) => {
                        if repo
                            .try_get_untracked()
                            .flatten()
                            .is_some_and(|x| x.id == r.id)
                        {
                            if let Ok(rows) = serde_json::from_value(v) {
                                symbols.set(rows);
                            }
                        }
                    }
                    Err(e) => notice.set(e),
                }
            });
        }
    };
    view! {<div class="focus-toolbar"><label>"Project"</label><select prop:value=move||chosen_project(snap,project).map(|p|p.id).unwrap_or_default() on:change=move|ev|{project.set(event_target_value(&ev));repo.set(None);reports.set(vec![]);symbols.set(vec![]);}><option value="">"Choose project"</option>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select><span class="muted">"Explicit local repository access · read-only Git and approved LSP"</span></div><div class="focus-detail-columns"><section class="panel"><div class="section-heading"><h2>"Repository selection"</h2></div><button class="button text" on:click=move|_|{repo.set(None);name.set(String::new());path.set(String::new());include.set(String::new());exclude.set(String::new());service.set(String::new());approve.set(false);}>"+ Register repository"</button>
        {move||list.get().into_iter().filter(|r|chosen_project(snap,project).is_some_and(|p|p.id==r.project_id)).map(|r|{let label=format!("{} · v{}",r.name,r.version);view!{<button class="source-link" on:click=move|_|{name.set(r.name.clone());path.set(r.path.clone());include.set(r.include.join("\n"));exclude.set(r.exclude.join("\n"));external.set(r.allow_external);service.set(r.symbol_service.as_ref().map(|s|s.command.clone()).unwrap_or_default());args.set(r.symbol_service.as_ref().map(|s|serde_json::to_string(&s.args).unwrap_or("[]".into())).unwrap_or("[]".into()));language.set(r.symbol_service.as_ref().map(|s|s.language_id.clone()).unwrap_or("rust".into()));repo.set(Some(r.clone()));approve.set(false);reports.set(vec![]);symbols.set(vec![]);load();}>{label}</button>}}).collect_view()}
        <form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let Some(p)=chosen_project(snap,project) else{notice.set("Create a project first".into());return;};let original_project=project.get_untracked();let old=repo.get_untracked();let old_id=old.as_ref().map(|r|r.id.clone());let svc=service.get_untracked();let options=match serde_json::from_str::<Vec<String>>(&args.get_untracked()){Ok(v)=>v,Err(e)=>{notice.set(format!("Invalid service args JSON: {e}"));return;}};action(old.as_ref().map(|r|format!("/api/v1/repositories/{}",r.id)).unwrap_or("/api/v1/repositories".into()),if old.is_some(){"PATCH"}else{"POST"},json!({"expected_version":old.map(|r|r.version).unwrap_or(0),"project_id":p.id,"name":name.get_untracked(),"path":path.get_untracked(),"initial_ref":initial_ref.get_untracked(),"include":prefixes(include.get_untracked()),"exclude":prefixes(exclude.get_untracked()),"allow_external":external.get_untracked(),"symbol_service":if svc.is_empty(){None}else{Some(SymbolService{command:svc,args:options,language_id:language.get_untracked()})},"approve_service":approve.get_untracked()}),snap,notice,busy,pending,move|v|{if project.try_get_untracked().as_ref()!=Some(&original_project)||repo.try_get_untracked().flatten().map(|r|r.id)!=old_id{return;}if let Ok(r)=serde_json::from_value(v){repo.set(Some(r));approve.set(false);}});}>
            <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Name"</label><input required maxlength="200" prop:value=move||name.get() on:input=move|ev|name.set(event_target_value(&ev))/><label>"Absolute local repository root"</label><input required maxlength="4096" prop:value=move||path.get() on:input=move|ev|path.set(event_target_value(&ev))/><label>"Initial checkpoint reference (new registrations only)"</label><input prop:value=move||initial_ref.get() on:input=move|ev|initial_ref.set(event_target_value(&ev))/><label>"Include relative file/directory prefixes · one per line; empty means all eligible"</label><textarea rows="3" prop:value=move||include.get() on:input=move|ev|include.set(event_target_value(&ev))></textarea><label>"Exclude prefixes · one per line"</label><textarea rows="3" prop:value=move||exclude.get() on:input=move|ev|exclude.set(event_target_value(&ev))></textarea><label><input type="checkbox" prop:checked=move||external.get() on:change=move|ev|external.set(event_target_checked(&ev))/>" Allow project agents to read saved repository results"</label><details><summary>"Optional symbol service · LSP over stdio"</summary><label>"Absolute service executable"</label><input prop:value=move||service.get() on:input=move|ev|{service.set(event_target_value(&ev));approve.set(false);}/><label>"Arguments · JSON array"</label><input prop:value=move||args.get() on:input=move|ev|{args.set(event_target_value(&ev));approve.set(false);}/><label>"Language ID"</label><input prop:value=move||language.get() on:input=move|ev|language.set(event_target_value(&ev))/><label><input type="checkbox" prop:checked=move||approve.get() on:change=move|ev|approve.set(event_target_checked(&ev))/>" Approve this executable; the service runs with your user privileges and can index this repository"</label></details><button class="button primary" type="submit">"Save repository selection"</button></fieldset></form>
    </section><section class="panel"><div class="section-heading"><h2>"Selected changes"</h2><span>"Pinned commits · accepted work"</span></div>{move||repo.get().map(|r|view!{<p>{format!("{} · checkpoint {}",r.name,r.checkpoint)}</p>})}<div class="workflow-form"><fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Target commit/reference"</label><input prop:value=move||to_ref.get() on:input=move|ev|to_ref.set(event_target_value(&ev))/><label><input type="checkbox" prop:checked=move||advance.get() on:change=move|ev|advance.set(event_target_checked(&ev))/>" Advance checkpoint after this report"</label><button class="button secondary" disabled=move||repo.get().is_none() on:click=move|_|{if let Some(r)=repo.get_untracked(){action(format!("/api/v1/repositories/{}/reports",r.id),"POST",json!({"expected_version":r.version,"to_ref":to_ref.get_untracked(),"advance_checkpoint":advance.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(r)=serde_json::from_value::<RepositoryReport>(v){if repo.try_get_untracked().flatten().is_none_or(|current|current.id!=r.repository_id){return;}load_epoch.update(|n|*n+=1);reports.update(|rows|rows.insert(0,r));}});}}>"Summarize selected changes"</button><button class="button text" on:click=move|_|load()>"Load saved reports and symbol results"</button></fieldset></div><p class="muted">"Git reads committed history only. File prefixes and sensitive-path exclusions filter results. Task relevance uses explicit keyword matches and does not claim completion."</p>
        {move||reports.get().into_iter().map(|r|view!{<details class="focus-influence"><summary>{format!("{} · {} commits · {} files · {} task links",r.created_at,r.commits.len(),r.files.len(),r.task_matches.len())}</summary><code>{format!("{} → {}",r.from_commit,r.to_commit)}</code>{r.omissions.into_iter().map(|o|view!{<p class="orange">{o}</p>}).collect_view()}{r.commits.into_iter().map(|c|view!{<details><summary>{format!("{} · {}",c.id,c.subject)}</summary>{c.files.into_iter().map(|p|view!{<code>{p}</code>}).collect_view()}</details>}).collect_view()}{r.files.into_iter().map(|f|view!{<p>{format!("{} {}",f.status,f.path)}</p>}).collect_view()}{r.task_matches.into_iter().map(|t|view!{<article class="focus-change"><strong>{t.title}</strong><p class="muted">{t.reason}</p><p>{t.matched_paths.join(", ")}</p><Citations evidence=vec![t.evidence] selected=selected notice=notice/></article>}).collect_view()}</details>}).collect_view()}
    </section></div>
    <section class="panel"><div class="section-heading"><h2>"Symbol questions"</h2><span>"Actual definitions and references · approved LSP"</span></div><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let Some(r)=repo.get_untracked()else{return;};let(l,c)=match(line.get_untracked().parse::<u32>(),character.get_untracked().parse::<u32>()){(Ok(l),Ok(c))=>(l,c),_=>{notice.set("Invalid UTF-16 position".into());return;}};action(format!("/api/v1/repositories/{}/symbols",r.id),"POST",json!({"expected_version":r.version,"file":file.get_untracked(),"line":l,"utf16_character":c,"method":method.get_untracked(),"approve_service":approve_query.get_untracked()}),snap,notice,busy,pending,move|v|{if let Ok(r)=serde_json::from_value::<SymbolQueryResult>(v){if repo.try_get_untracked().flatten().is_none_or(|current|current.id!=r.repository_id){return;}load_epoch.update(|n|*n+=1);symbols.update(|rows|rows.insert(0,r));approve_query.set(false);}});}>
        <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Relative selected file"</label><input required maxlength="500" prop:value=move||file.get() on:input=move|ev|file.set(event_target_value(&ev))/><label>"Zero-based line"</label><input type="number" min="0" prop:value=move||line.get() on:input=move|ev|line.set(event_target_value(&ev))/><label>"Zero-based UTF-16 character"</label><input type="number" min="0" prop:value=move||character.get() on:input=move|ev|character.set(event_target_value(&ev))/><select prop:value=move||method.get() on:change=move|ev|method.set(event_target_value(&ev))><option value="definition">"Definition"</option><option value="references">"References"</option></select><label><input type="checkbox" prop:checked=move||approve_query.get() on:change=move|ev|approve_query.set(event_target_checked(&ev))/>" Approve this one service invocation; it can index files using its own local process"</label><button class="button primary" type="submit" disabled=move||!approve_query.get() || repo.get().is_none()>"Ask selected symbol service"</button></fieldset></form>
        {move||symbols.get().into_iter().map(|r|view!{<details class="focus-influence"><summary>{format!("{} · {} · {} locations",r.created_at,r.method,r.locations.len())}</summary><code>{format!("{}:{}:{} · HEAD {} · file hash {}",r.file,r.line,r.utf16_character,r.commit,r.file_hash)}</code>{r.notes.into_iter().map(|n|view!{<p class="muted">{n}</p>}).collect_view()}<p class="muted">{format!("{} locations omitted by scope, size or count",r.omitted_locations)}</p>{r.locations.into_iter().map(|l|view!{<article class="focus-change"><code>{format!("{}:{}:{}–{}:{}",l.file,l.line,l.utf16_character,l.end_line,l.end_utf16_character)}</code><pre style="white-space:pre-wrap">{l.excerpt}</pre></article>}).collect_view()}</details>}).collect_view()}
    </section>}
}
