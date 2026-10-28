use super::workflows::{Citations, Pending, action, chosen_project};
use super::*;
#[component]
pub(super) fn Retrieval(
    snap: Snap,
    notice: RwSignal<String>,
    selected: RwSignal<Option<Document>>,
) -> impl IntoView {
    let project = RwSignal::new(String::new());
    let connectors = RwSignal::new(Vec::<UpstreamConnection>::new());
    let current = RwSignal::new(None::<UpstreamConnection>);
    let editor=RwSignal::new(r#"{"name":"Company knowledge","transport":"stdio","command":"/absolute/path/to/mcp-server","args":[],"endpoint":null,"token_env":null,"resource_uri":"docs://selected/resource","read_tool":null,"tool_arguments":{},"policy":"hybrid","allow_query_transmission":false,"allow_agent_live":false,"daily_limit":20}"#.to_string());
    let configured_approved = RwSignal::new(false);
    let query = RwSignal::new(String::new());
    let scope = RwSignal::new(Vec::<String>::new());
    let live_approved = RwSignal::new(false);
    let result = RwSignal::new(None::<FederatedResponse>);
    let refresh_result = RwSignal::new(None::<Value>);
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let epoch = RwSignal::new(0u64);
    Effect::new(move |_| {
        let _ = snap.get().map(|s| s.watermark);
        epoch.update(|n| *n += 1);
        let ticket = epoch.get_untracked();
        spawn_local(async move {
            let response = api("/api/v1/upstreams", "GET", None).await;
            if epoch.try_get_untracked() != Some(ticket) {
                return;
            }
            match response {
                Ok(v) => {
                    if let Ok(rows) = serde_json::from_value(v) {
                        connectors.set(rows);
                    }
                }
                Err(e) => notice.set(e),
            }
        });
    });
    view! {<div class="focus-toolbar"><label>"Project"</label><select prop:value=move||chosen_project(snap,project).map(|p|p.id).unwrap_or_default() on:change=move|ev|{project.set(event_target_value(&ev));current.set(None);scope.set(vec![]);result.set(None);refresh_result.set(None);}>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|view!{<option value=p.id>{p.name}</option>}).collect_view())}</select><span class="muted">"Existing MCP servers/accounts · explicit read mappings"</span></div><div class="focus-detail-columns">
    <section class="panel"><div class="section-heading"><h2>"Upstream connections"</h2><span>"Index / live / hybrid"</span></div><p class="muted">"Choose an exact resource URI or a trusted read-only tool and fixed arguments. {query} in tool argument strings inserts your query only when query transmission is enabled. Saving does not connect. Sources default to local-only; Source controls manage agent sharing independently."</p><button class="button text" on:click=move|_|{current.set(None);configured_approved.set(false);}>"+ New upstream source"</button>
    {move||connectors.get().into_iter().filter(|c|chosen_project(snap,project).is_some_and(|p|p.id==c.project_id)).map(|c|{let label=format!("{} · {} · v{}",c.name,c.policy,c.version);view!{<button class="source-link" on:click=move|_|{editor.set(serde_json::to_string_pretty(&json!({"name":c.name,"transport":c.transport,"command":c.command,"args":c.args,"endpoint":c.endpoint,"token_env":c.token_env,"resource_uri":c.resource_uri,"read_tool":c.read_tool,"tool_arguments":c.tool_arguments,"policy":c.policy,"allow_query_transmission":c.allow_query_transmission,"allow_agent_live":c.allow_agent_live,"daily_limit":c.daily_limit})).unwrap_or_default());current.set(Some(c.clone()));configured_approved.set(false);}>{label}</button>}}).collect_view()}
    <form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let Some(p)=chosen_project(snap,project)else{return;};let mut body=match serde_json::from_str::<Value>(&editor.get_untracked()){Ok(v)if v.is_object()=>v,_=>{notice.set("Invalid upstream JSON object".into());return;}};let original_project=project.get_untracked();let c=current.get_untracked();let original_id=c.as_ref().map(|c|c.id.clone());body["project_id"]=json!(p.id);body["expected_version"]=json!(c.as_ref().map(|c|c.version).unwrap_or(0));body["approve_upstream"]=json!(configured_approved.get_untracked());action(c.map(|c|format!("/api/v1/upstreams/{}",c.id)).unwrap_or("/api/v1/upstreams".into()),if current.get_untracked().is_some(){"PATCH"}else{"POST"},body,snap,notice,busy,pending,move|v|{if project.try_get_untracked().as_ref()!=Some(&original_project)||current.try_get_untracked().flatten().map(|c|c.id)!=original_id{return;}if let Ok(c)=serde_json::from_value(v){current.set(Some(c));configured_approved.set(false);}});}>
        <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Destination, operation and policy · JSON"</label><textarea rows="18" spellcheck="false" prop:value=move||editor.get() on:input=move|ev|{editor.set(event_target_value(&ev));configured_approved.set(false);}></textarea><label><input type="checkbox" prop:checked=move||configured_approved.get() on:change=move|ev|configured_approved.set(event_target_checked(&ev))/>" Approve this destination and exact read mapping; stdio executes trusted local software, HTTP uses only its named token environment variable"</label><button class="button primary" type="submit" disabled=move||!configured_approved.get()>"Save approved mapping"</button></fieldset></form>
    {move||current.get().map(|c|view!{<p class="muted">{format!("Source {} · fetched {:?} · document {:?} r{:?}",c.source_id,c.last_fetched_at,c.last_document_id,c.last_revision)}</p>{c.last_error.map(|e|view!{<p class="runtime-error">{e}</p>})}})}
    </section><section class="panel"><div class="section-heading"><h2>"Federated retrieval"</h2><span>"Visible paths and checkpoints"</span></div><form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let original_query=query.get_untracked();let original_scope=scope.get_untracked();let original_project=project.get_untracked();action("/api/v1/retrieval".into(),"POST",json!({"query":query.get_untracked(),"source_ids":scope.get_untracked(),"approve_upstream":live_approved.get_untracked(),"limit":20}),snap,notice,busy,pending,move|v|{if query.try_get_untracked().as_ref()!=Some(&original_query)||scope.try_get_untracked().as_ref()!=Some(&original_scope)||project.try_get_untracked().as_ref()!=Some(&original_project){return;}if let Ok(r)=serde_json::from_value(v){result.set(Some(r));live_approved.set(false);}});}>
        <fieldset class="focus-form-fields" disabled=move||busy.get()><label>"Query"</label><input required maxlength="1024" prop:value=move||query.get() on:input=move|ev|{query.set(event_target_value(&ev));live_approved.set(false);}/><fieldset class="runtime-sources"><legend>"Choose source scope"</legend>{move||snap.get().map(|s|s.sources.into_iter().filter(|s|chosen_project(snap,project).is_some_and(|p|p.source_ids.contains(&s.id))).map(|s|{let id=s.id.clone();view!{<label><input type="checkbox" prop:checked=move||scope.get().contains(&id) on:change=move|ev|{let checked=event_target_checked(&ev);scope.update(|ids|{ids.retain(|id|id!=&s.id);if checked{ids.push(s.id.clone());}});live_approved.set(false);}/>{s.name}</label>}}).collect_view())}</fieldset><label><input type="checkbox" prop:checked=move||live_approved.get() on:change=move|ev|live_approved.set(event_target_checked(&ev))/>" Approve configured upstream reads for this query (index policy still stays local)"</label><button class="button primary" type="submit">"Retrieve using source policies"</button><button class="button secondary" type="button" disabled=move||!live_approved.get() || current.get().is_none() on:click=move|_|{if let Some(c)=current.get_untracked(){let original_id=c.id.clone();let original_query=query.get_untracked();action(format!("/api/v1/upstreams/{}/refresh",c.id),"POST",json!({"query":query.get_untracked(),"approve_upstream":live_approved.get_untracked()}),snap,notice,busy,pending,move|v|{if current.try_get_untracked().flatten().is_none_or(|c|c.id!=original_id)||query.try_get_untracked().as_ref()!=Some(&original_query){return;}refresh_result.set(Some(v));live_approved.set(false);});}}>"Explicitly refresh selected index"</button></fieldset></form>
        {move||refresh_result.get().map(|v|view!{<details><summary>"Refresh receipt"</summary><pre style="white-space:pre-wrap;overflow-wrap:anywhere">{serde_json::to_string_pretty(&v).unwrap_or_default()}</pre></details>})}
        {move||result.get().map(|r|view!{<p class="muted">{format!("Snapshot watermark {} · {} results",r.watermark,r.results.len())}</p>{r.sources.into_iter().map(|s|view!{<article class="focus-change"><strong>{format!("{} · {} → {}",s.source_id,s.policy,s.path)}</strong>{s.error.map(|e|view!{<p class="runtime-error">{e}</p>})}{s.checkpoint.map(|c|view!{<code>{format!("{} r{} · {}",c.document_id,c.revision,c.content_hash)}</code>})}</article>}).collect_view()}{r.omissions.into_iter().map(|o|view!{<p class="orange">{o}</p>}).collect_view()}{r.results.into_iter().map(|item|view!{<article class="focus-change"><strong>{item.hit.title}</strong><p class="muted">{format!("{} · policy {} · fetched {:?} · {}",item.path,item.policy,item.fetched_at,if item.stale{"cached freshness unverified"}else if item.fetched_at.is_some(){"fetched in this request"}else{"local indexed source"})}</p><p>{item.hit.excerpt}</p><Citations evidence=item.evidence selected=selected notice=notice/></article>}).collect_view()}})}
    </section></div>}
}
