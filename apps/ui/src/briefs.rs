use super::*;

fn project(snap: Snap, id: RwSignal<String>) -> Option<ProjectBrief> {
    let briefs = snap.get()?.briefs;
    briefs
        .iter()
        .find(|b| b.id == id.get())
        .cloned()
        .or_else(|| briefs.first().cloned())
}
fn revision(brief: &ProjectBrief, selected: Option<i64>) -> Option<BriefRevision> {
    brief
        .revisions
        .iter()
        .find(|r| Some(r.revision) == selected)
        .or_else(|| brief.revisions.first())
        .cloned()
}
fn heading(kind: &str) -> &'static str {
    match kind {
        "goals" => "Goals",
        "decisions" => "Decisions",
        "blockers" => "Blockers",
        _ => "Open questions",
    }
}
pub(super) fn open_evidence(
    e: Evidence,
    selected: RwSignal<Option<Document>>,
    notice: RwSignal<String>,
) {
    let ticket = super::document_request();
    spawn_local(async move {
        match api(
            &format!(
                "/api/v1/documents/{}?revision={}",
                e.document_id, e.revision
            ),
            "GET",
            None,
        )
        .await
        {
            Ok(value) => match serde_json::from_value(value) {
                Ok(doc) => {
                    let doc: Document = doc;
                    if !super::current_document_request(ticket) {
                        return;
                    }
                    if e.line < 1 || e.line as usize > doc.body.lines().count() {
                        notice.set("The cited line is unavailable in this source revision".into());
                        return;
                    }
                    super::evidence_target(Some(super::EvidenceTarget {
                        document_id: doc.id.clone(),
                        revision: doc.revision,
                        line: e.line,
                    }));
                    selected.set(Some(doc));
                }
                Err(_) => notice.set("Could not read source evidence".into()),
            },
            Err(error) => notice.set(error),
        }
    });
}

#[component]
pub(super) fn Briefs(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
    view: RwSignal<String>,
) -> impl IntoView {
    let current = RwSignal::new(String::new());
    let selected_revision = RwSignal::new(None::<i64>);
    let name = RwSignal::new(String::new());
    let source_ids = RwSignal::new(Vec::<String>::new());
    let connection = RwSignal::new(String::new());
    let approved = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let last_create = RwSignal::new(None::<BriefCreateRequest>);
    let last_generate = RwSignal::new(None::<(String, BriefGenerateRequest)>);
    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() {
            return;
        }
        if source_ids.get_untracked().is_empty() {
            notice.set("Select at least one source for the project.".into());
            return;
        }
        let req = last_create
            .get_untracked()
            .filter(|r| {
                r.name == name.get_untracked() && r.source_ids == source_ids.get_untracked()
            })
            .unwrap_or_else(|| BriefCreateRequest {
                name: name.get_untracked(),
                source_ids: source_ids.get_untracked(),
                idempotency_key: new_id(),
            });
        last_create.set(Some(req.clone()));
        busy.set(true);
        spawn_local(async move {
            let result = api(
                "/api/v1/briefs",
                "POST",
                Some(serde_json::to_value(req).unwrap()),
            )
            .await;
            busy.set(false);
            match result {
                Ok(value) => {
                    current.set(value["id"].as_str().unwrap_or_default().into());
                    selected_revision.set(None);
                    last_create.set(None);
                    name.set(String::new());
                    source_ids.set(vec![]);
                    notice.set(String::new());
                    refresh(snap, notice);
                }
                Err(error) => notice.set(error),
            }
        });
    };
    let generate = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if busy.get_untracked() || !approved.get_untracked() {
            return;
        }
        let Some(brief) = project(snap, current) else {
            return;
        };
        let chosen = if connection.get_untracked().is_empty() {
            snap.get_untracked()
                .and_then(|s| {
                    s.connections
                        .into_iter()
                        .find(|c| c.kind == "acp" && c.available)
                })
                .map(|c| c.id)
                .unwrap_or_default()
        } else {
            connection.get_untracked()
        };
        if chosen.is_empty() {
            notice.set("Configure an ACP connection in Connections first.".into());
            return;
        }
        let req = last_generate
            .get_untracked()
            .filter(|(id, r)| id == &brief.id && r.connection == chosen)
            .map(|(_, r)| r)
            .unwrap_or_else(|| BriefGenerateRequest {
                connection: chosen,
                expected_version: brief.version,
                approve_usage: true,
                idempotency_key: new_id(),
            });
        last_generate.set(Some((brief.id.clone(), req.clone())));
        busy.set(true);
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/briefs/{}/generate", brief.id),
                "POST",
                Some(serde_json::to_value(req).unwrap()),
            )
            .await;
            busy.set(false);
            match result {
                Ok(_) => {
                    last_generate.set(None);
                    approved.set(false);
                    selected_revision.set(None);
                    notice.set(String::new());
                    refresh(snap, notice);
                }
                Err(error) => {
                    // A definite rejection made no reservation; an uncertain connection error must reuse its key.
                    if ["invalid", "conflict", "forbidden", "unknown"]
                        .iter()
                        .any(|prefix| error.starts_with(prefix))
                    {
                        last_generate.set(None);
                    }
                    notice.set(error);
                    refresh(snap, notice);
                }
            }
        });
    };
    let review = move |id: String, rev: i64, version: i64, decision: &'static str| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        spawn_local(async move {
            let result = api(
                &format!("/api/v1/briefs/{id}/review"),
                "POST",
                Some(json!({"revision":rev,"expected_version":version,"decision":decision})),
            )
            .await;
            busy.set(false);
            match result {
                Ok(_) => {
                    notice.set(String::new());
                    refresh(snap, notice);
                }
                Err(error) => {
                    notice.set(error);
                    refresh(snap, notice);
                }
            }
        });
    };
    view! {
        <div class="brief-layout">
            <aside class="brief-sidebar">
                <section class="panel"><div class="section-heading"><h2>"Projects"</h2><span>{move||snap.get().map(|s|s.briefs.len()).unwrap_or(0)}</span></div>
                    {move||snap.get().map(|s|s.briefs.into_iter().map(|b|{let id=b.id.clone();let active=b.id.clone();let accepted=b.revisions.iter().find(|r|Some(r.revision)==b.accepted_revision);let state=if accepted.is_some_and(|r|r.stale){"stale"}else if accepted.is_some(){"current"}else{"unreviewed"};view!{<button class="brief-project" class:active=move||project(snap,current).is_some_and(|p|p.id==active) on:click=move|_|{current.set(id.clone());selected_revision.set(None);approved.set(false);}>
                        <span>{b.name}</span><small>{state}</small></button>}}).collect_view())}
                    <Show when=move||snap.get().is_some_and(|s|s.briefs.is_empty())><p class="muted">"No projects yet."</p></Show>
                </section>
                <section class="panel brief-create"><div class="section-heading"><h2>"New project"</h2></div>
                    <form on:submit=create><label for="brief-name">"Name"</label><input id="brief-name" required maxlength="200" prop:value=move||name.get() on:input=move|ev|name.set(event_target_value(&ev)) placeholder="Project name"/>
                        <fieldset class="runtime-sources"><legend>"Sources"</legend>{move||snap.get().map(|s|s.sources.into_iter().map(|s|{let id=s.id.clone();let checked=s.id;view!{<label><input type="checkbox" prop:checked=move||source_ids.get().contains(&checked) on:change=move|ev|{let value=event_target_checked(&ev);source_ids.update(|ids|{ids.retain(|s|s!=&id);if value{ids.push(id.clone());}});}/><span>{s.name}</span></label>}}).collect_view())}</fieldset>
                        <p class="muted">"Choose the complete source scope for this brief. Creating a project uses no model allowance."</p><button class="button secondary" type="submit" disabled=move||busy.get()>"Create project"</button>
                    </form>
                </section>
            </aside>
            <div class="brief-main">
                {move||project(snap,current).map(|b|{
                    let brief_id=b.id.clone();let gen_active=b.revisions.iter().any(|r|r.status=="generating");
                    let has_agent=snap.get().is_some_and(|s|s.connections.iter().any(|c|c.kind=="acp" && c.available));
                    let source_names=snap.get().map(|s|b.source_ids.iter().filter_map(|id|s.sources.iter().find(|s|&s.id==id).map(|s|s.name.clone())).collect::<Vec<_>>().join(", ")).unwrap_or_default();
                    view!{
                        <section class="panel brief-controls"><div class="section-heading"><h2>{b.name.clone()}</h2><span>{b.accepted_revision.map(|n|format!("accepted r{n}")).unwrap_or_else(||"No accepted brief".into())}</span></div>
                            <p class="brief-scope">{format!("Sources: {source_names}")}</p>
                            <super::workflows::ProjectSources project=b.clone() snap=snap notice=notice/>
                            <p class="muted">"Live-only sources and their cached annotations are excluded. Every included source must permit the selected agent; destination checks run before any model request."</p>
                            <Show when=move||!has_agent><p class="muted">"Set up an ACP agent in Connections to generate a brief."</p></Show>
                            <form on:submit=generate><div class="brief-generate-row"><label for="brief-agent">"Agent"</label><select id="brief-agent" disabled=gen_active prop:value=move||{
                                let value=connection.get();if !value.is_empty(){value}else{snap.get().and_then(|s|s.connections.into_iter().find(|c|c.kind=="acp" && c.available)).map(|c|c.id).unwrap_or_default()}
                            } on:change=move|ev|{connection.set(event_target_value(&ev));approved.set(false);}>
                                {move||snap.get().map(|s|s.connections.into_iter().filter(|c|c.kind=="acp" && c.available).map(|c|view!{<option value=c.id.clone()>{c.id.clone()}</option>}).collect_view())}
                            </select><button class="button primary" type="submit" disabled=move||busy.get() || gen_active || !has_agent || !approved.get()>{if gen_active{"Generating…"}else if b.revisions.is_empty(){"Generate draft"}else{"Refresh draft"}}</button><button class="button text" type="button" on:click=move|_|view.set("activity".into())>"Agent activity →"</button></div>
                                <label class="checkbox-label"><input type="checkbox" required disabled=gen_active prop:checked=move||approved.get() on:change=move|ev|approved.set(event_target_checked(&ev))/><span>"Use this agent's account allowance and send the included indexed source snapshot for this generation."</span></label>
                            </form>
                            <small class="muted">"Up to 16 KB of input. Indexed and hybrid snapshots are included; live-only sources are omitted with an explanation in the draft. Larger inputs are rejected. Refresh is explicit."</small>
                        </section>
                        <section class="panel brief-content"><div class="section-heading"><h2>"Brief"</h2><span>"Review before sharing"</span></div>
                            <div class="brief-version-row"><label for="brief-revision">"Revision"</label><select id="brief-revision" prop:value=move||selected_revision.get().map(|r|r.to_string()).unwrap_or_default() on:change=move|ev|selected_revision.set(event_target_value(&ev).parse().ok())>
                                <option value="">"Latest"</option>{b.revisions.iter().map(|r|view!{<option value=r.revision.to_string()>{format!("r{} · {}{}",r.revision,r.status,if r.stale{" · stale"}else{""})}</option>}).collect_view()}
                            </select></div>
                            {revision(&b,selected_revision.get()).map(|r|{
                                let review_id=brief_id.clone();let reject_id=brief_id.clone();let version=b.version;let rev=r.revision;
                                let can_accept=r.status=="draft" && !r.stale && b.revisions.first().is_some_and(|latest|latest.revision==rev);
                                let changed=r.changed_sources.iter().map(|id|snap.get().and_then(|s|s.sources.into_iter().find(|s|&s.id==id)).map(|s|s.name).unwrap_or_else(||id.clone())).collect::<Vec<_>>().join(", ");
                                let sources=r.sources.clone();
                                view!{
                                    <div class="brief-meta"><span>{format!("r{} · {}",r.revision,r.status)}</span><small>{format!("{} · {}",r.connection,r.created_at)}</small></div>
                                    {r.stale.then(||view!{<p class="brief-stale" role="status">"STALE · "{changed}" evidence or retrieval policy changed. All sections need a fresh review, including empty sections. Historical evidence remains available."</p>})}
                                    {r.omissions.iter().map(|note|view!{<p class="muted">{note.clone()}</p>}).collect_view()}<p class="muted">{format!("{} selected sources · {} included revision pins · {} live-only exclusions",r.selected_source_ids.len(),r.sources.len(),r.omitted_source_ids.len())}</p>{r.error.map(|e|view!{<p class="runtime-error" role="alert">{e}</p>})}
                                    {(r.status=="generating").then(||{
                                        let run=r.run_id.clone();view!{<p class="muted">"Generating a draft. Watch progress or respond to permissions in Activity. The accepted brief stays available."</p>{run.map(|id|view!{<button class="button text" on:click=move|_|mutate(format!("/api/v1/runtime/runs/{id}/cancel"),"POST",json!({}),snap,notice)>"Cancel generation"</button>})}}
                                    })}
                                    {r.sections.into_iter().map(|s|view!{<section class="brief-section"><h3>{heading(&s.kind)}{s.stale.then(||view!{<small>"needs refresh"</small>})}</h3>
                                        {s.items.is_empty().then(||view!{<p class="muted">"No supported information in the included indexed sources. Review any exclusions above."</p>})}
                                        <ul>{s.items.into_iter().map(|item|view!{<li><p>{item.text}</p><div class="brief-evidence">{item.evidence.into_iter().map(|e|{let label=sources.iter().find(|s|s.document_id==e.document_id).map(|s|s.title.clone()).unwrap_or_else(||e.document_id.clone());view!{<details><summary>{format!("{} · r{}:{}",label,e.revision,e.line)}</summary><blockquote>{e.quote.clone()}</blockquote><button class="source-link" on:click=move|_|open_evidence(e.clone(),selected,notice)>"Open immutable source →"</button></details>}}).collect_view()}</div></li>}).collect_view()}</ul>
                                    </section>}).collect_view()}
                                    {(r.status=="draft").then(||view!{<div class="brief-review"><button class="button primary" disabled=move||busy.get() || !can_accept on:click=move|_|review(review_id.clone(),rev,version,"accept")>"Accept brief"</button><button class="button text" disabled=move||busy.get() on:click=move|_|review(reject_id.clone(),rev,version,"reject")>"Reject draft"</button><small class="muted">"Accepted briefs are available to agents with access to every selected source. Check that the cited passages support each claim."</small></div>})}
                                    <details class="brief-changes"><summary>{r.base_revision.map(|n|format!("Changes since accepted r{n}")).unwrap_or_else(||"Initial brief additions".into())}</summary>
                                        <p class="muted">"Changes include wording and citation revisions. Inspect the source snapshot for exact evidence."</p>
                                    {r.changes.into_iter().map(|c|view!{<section><h3>{heading(&c.kind)}</h3>{(c.added.is_empty() && c.removed.is_empty()).then(||view!{<p class="muted">"No changes."</p>})}{c.removed.into_iter().map(|i|view!{<p class="brief-removed">"− "{i.text}<small>{i.evidence.iter().map(|e|format!("{} r{}:{}",e.document_id,e.revision,e.line)).collect::<Vec<_>>().join(" · ")}</small></p>}).collect_view()}{c.added.into_iter().map(|i|view!{<p class="brief-added">"+ "{i.text}<small>{i.evidence.iter().map(|e|format!("{} r{}:{}",e.document_id,e.revision,e.line)).collect::<Vec<_>>().join(" · ")}</small></p>}).collect_view()}</section>}).collect_view()}
                                    </details>
                                    <details class="brief-watermarks"><summary>"Source snapshot"</summary><small>{format!("Event watermark {} · {}",r.watermark,r.reviewed_at.map(|t|format!("Reviewed {t}")).unwrap_or_else(||"Awaiting review".into()))}</small>
                                        {r.sources.into_iter().map(|s|{let e=Evidence{document_id:s.document_id.clone(),revision:s.revision,line:1,quote:String::new()};view!{<div><button class="source-link" on:click=move|_|open_evidence(e.clone(),selected,notice)>{format!("{} · r{}",s.title,s.revision)}</button><code>{s.content_hash}</code></div>}}).collect_view()}
                                    </details>
                                }
                            })}
                            {b.revisions.is_empty().then(||view!{<p class="muted">"Generate the first draft to assemble goals, decisions, blockers, and open questions with source citations."</p>})}
                        </section>
                    }
                })}
                <Show when=move||project(snap,current).is_none()><section class="panel"><div class="section-heading"><h2>"Living project briefs"</h2></div><p class="muted">"Create a project and choose its sources. Your agent drafts a brief, you review its evidence, and source changes mark it stale. Accepted revisions are shared with authorized agents."</p></section></Show>
            </div>
        </div>
    }
}
