//! Shared desktop/browser management. Drafts survive background refreshes;
//! replacing a stale editor is an explicit action.
use super::workflows::{Pending, action};
use super::*;
use std::collections::BTreeMap;

type Records = RwSignal<Vec<PluginRecord>>;
type ConfigDraft = RwSignal<BTreeMap<String, String>>;

fn defaults(manifest: &PluginManifest, configuration: &Value) -> BTreeMap<String, String> {
    manifest
        .configuration
        .iter()
        .map(|f| {
            let v = configuration.get(&f.key).or(f.default.as_ref());
            (
                f.key.clone(),
                v.map(|v| {
                    v.as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v.to_string())
                })
                .unwrap_or_else(|| {
                    if f.kind == PluginFieldKind::Boolean {
                        "false".into()
                    } else {
                        String::new()
                    }
                }),
            )
        })
        .collect()
}
fn configuration(manifest: &PluginManifest, draft: ConfigDraft) -> Result<Value, String> {
    let draft = draft.get_untracked();
    let mut values = serde_json::Map::new();
    for f in &manifest.configuration {
        let value = draft.get(&f.key).cloned().unwrap_or_default();
        if value.is_empty() && !f.required {
            continue;
        }
        let value = match f.kind {
            PluginFieldKind::Boolean => json!(value == "true"),
            PluginFieldKind::Integer => json!(
                value
                    .parse::<i64>()
                    .map_err(|_| format!("{} must be a whole number", f.label))?
            ),
            _ => json!(value),
        };
        values.insert(f.key.clone(), value);
    }
    Ok(Value::Object(values))
}
fn reload(
    records: Records,
    capabilities: RwSignal<Vec<PluginCapabilityStatus>>,
    loading: RwSignal<bool>,
    error: RwSignal<String>,
) {
    if loading.get_untracked() {
        return;
    }
    loading.set(true);
    spawn_local(async move {
        let result: Result<(), String> = async {
            let plugins: Vec<PluginRecord> =
                serde_json::from_value(api("/api/v1/plugins", "GET", None).await?)
                    .map_err(|_| "Invalid plugin list")?;
            let caps: Vec<PluginCapabilityStatus> =
                serde_json::from_value(api("/api/v1/plugin-capabilities", "GET", None).await?)
                    .map_err(|_| "Invalid capability list")?;
            if records.try_get_untracked().is_none() {
                return Ok(());
            }
            records.set(plugins);
            capabilities.set(caps);
            Ok(())
        }
        .await;
        if loading.try_get_untracked().is_some() {
            loading.set(false);
            error.set(result.err().unwrap_or_default());
        }
    });
}
#[component]
pub(super) fn Plugins(
    snap: Snap,
    notice: RwSignal<String>,
    view: RwSignal<String>,
    section: RwSignal<String>,
) -> impl IntoView {
    let records = RwSignal::new(vec![]);
    let capabilities = RwSignal::new(vec![]);
    let loading = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let selected = RwSignal::new(String::new());
    let editor = RwSignal::new(0u64);
    let tick = RwSignal::new(0u64);
    Effect::new(move |_| {
        let _ = tick.get();
        let _ = snap.get().map(|s| s.watermark);
        reload(records, capabilities, loading, error);
    });
    let timer = gloo_timers::callback::Interval::new(2000, move || {
        reload(records, capabilities, loading, error)
    });
    let _timer = StoredValue::new_local(timer);
    view! {
        <div hidden=move||section.get()!="capabilities"><section class="panel plugin-capabilities"><div class="section-heading"><h2>"Native capabilities"</h2><button class="source-link" disabled=move||loading.get() on:click=move|_|tick.update(|v|*v+=1)>"Refresh"</button></div>
            <p class="muted">"The service owns recording and memory. Window and tray providers need the desktop host. Changes to providers with active resources are refused."</p>
            <Show when=move||!error.get().is_empty()><p role="alert">{move||error.get()}</p></Show>
            <For each=move||capabilities.get() key=|c|(c.capability.clone(),c.selection_version) children={move|c|view!{<CapabilityRow capability=c capabilities=capabilities records=records snap=snap notice=notice tick=tick/>}}/>
            <crate::native_notifications::NativeNotifications capabilities=capabilities notice=notice/>
        </section>
        </div>
        <div hidden=move||section.get()!="installed"><div class="plugin-layout"><section class="panel"><h2>"Installed plugins"</h2>
            <For each=move||records.get() key=|p|(p.id.clone(),p.version) children={move|p|{let id=p.id.clone();let key=id.clone();view!{<button class="plugin-list-row" class:chosen=move||selected.get()==key on:click=move|_|selected.set(id.clone())><span>{p.manifest.name}</span><small>{format!("{} · {} · {}",p.manifest.version,if p.builtin {"built in"}else{"local"},if p.enabled {"enabled"}else{"disabled"})}</small></button>}}}/>
        </section><section class="panel plugin-editor">
            <Show when=move||selected.get().is_empty()><h2>"Plugin settings"</h2><p class="muted">"Select a plugin to configure permissions, inspect deliveries or change its package."</p></Show>
            <For each={move||records.get().into_iter().find(|p|p.id==selected.get()).map(|p|(p,editor.get())).into_iter().collect::<Vec<_>>()} key=|(p,generation)|(p.id.clone(),*generation) children={move|(plugin,_)|view!{<PluginEditor plugin=plugin records=records editor=editor selected=selected tick=tick snap=snap notice=notice view=view/>}}/>
        </section></div>
        <PluginInstall snap=snap notice=notice records=records selected=selected tick=tick/></div>
    }
}
#[component]
fn CapabilityRow(
    capability: PluginCapabilityStatus,
    capabilities: RwSignal<Vec<PluginCapabilityStatus>>,
    records: Records,
    snap: Snap,
    notice: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    let selected = RwSignal::new(capability.provider_id.clone());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let cap = capability.capability.clone();
    let status_id = cap.clone();
    let label = format!("Provider for {cap}");
    let current = capability.provider_id.clone();
    view! {<div class="plugin-capability-row"><div><strong>{capability.capability.clone()}</strong><small>{move||capabilities.get().into_iter().find(|c|c.capability==status_id).map(|c|format!("{} · {}",if c.available {"ready"}else{"unavailable"},c.detail)).unwrap_or_else(||"Unavailable".into())}</small></div>
        <select aria-label=label prop:value=move||selected.get() on:change=move|ev|selected.set(event_target_value(&ev))><For each={move||records.get().into_iter().filter(|p|p.manifest.capabilities.iter().any(|c|c.id==cap)).collect::<Vec<_>>()} key=|p|(p.id.clone(),p.enabled,p.manifest.name.clone()) children={move|p|view!{<option value=p.id.clone() disabled=!p.enabled>{p.manifest.name}</option>}}/></select>
        <button class="button secondary" disabled=move||busy.get()||selected.get()==current on:click=move|_|action(format!("/api/v1/plugin-capabilities/{}",capability.capability),"PATCH",json!({"provider_id":selected.get_untracked(),"expected_version":capability.selection_version}),snap,notice,busy,pending,move|_|tick.update(|v|*v+=1))>"Select"</button>
    </div>}
}
#[component]
fn ConfigFields(manifest: PluginManifest, draft: ConfigDraft) -> impl IntoView {
    view! {<div class="plugin-config-fields">{manifest.configuration.into_iter().map(|f| {
        let key=f.key.clone();let read=key.clone();let required=f.required;
        let field=match f.kind {
            PluginFieldKind::Boolean=>view!{<input type="checkbox" prop:checked=move||draft.get().get(&read).is_some_and(|v|v=="true") on:change=move|ev|draft.update(|d|{d.insert(key.clone(),event_target_checked(&ev).to_string());})/>}.into_any(),
            PluginFieldKind::Choice=>view!{<select required=required prop:value=move||draft.get().get(&read).cloned().unwrap_or_default() on:change=move|ev|draft.update(|d|{d.insert(key.clone(),event_target_value(&ev));})><option value="">"Choose…"</option>{f.choices.into_iter().map(|v|view!{<option value=v.clone()>{v.clone()}</option>}).collect_view()}</select>}.into_any(),
            kind=>view!{<input type=if kind==PluginFieldKind::Integer {"number"}else{"text"} required=required placeholder=if kind==PluginFieldKind::CredentialReference {"env:NAME or keychain:reference"}else{""} prop:value=move||draft.get().get(&read).cloned().unwrap_or_default() on:input=move|ev|draft.update(|d|{d.insert(key.clone(),event_target_value(&ev));})/>}.into_any(),
        };
        view!{<label data-plugin-setting=f.key><span>{f.label}{if required {" *"}else{""}}</span>{field}<small class="muted">{f.description}</small></label>}
    }).collect_view()}</div>}
}
fn toggle(items: &mut Vec<String>, id: &str, checked: bool) {
    items.retain(|v| v != id);
    if checked {
        items.push(id.into());
    }
}
#[component]
fn Grants(
    manifest: PluginManifest,
    grant: RwSignal<PluginPermissions>,
    snap: Snap,
) -> impl IntoView {
    view! {<details class="workflow-manage" data-plugin-grants="true"><summary>"Permissions and data scope"</summary><p class="muted">"Only selected operations and sources are supplied by the host. Local executables still run with your OS account's privileges. Credential settings contain references, never resolved secret values."</p>
        <fieldset class="runtime-sources"><legend>"Requested operations"</legend>{manifest.requests.into_iter().map(|op|{let read=op.clone();let label=op.clone();view!{<label><input type="checkbox" prop:checked=move||grant.get().operations.contains(&read) on:change=move|ev|grant.update(|g|toggle(&mut g.operations,&op,event_target_checked(&ev)))/>{label}</label>}}).collect_view()}</fieldset>
        <fieldset class="runtime-sources"><legend>"Sources"</legend>{move||snap.get().map(|s|s.sources.into_iter().map(|s|{let read=s.id.clone();let id=s.id;view!{<label><input type="checkbox" prop:checked=move||grant.get().source_ids.contains(&read) on:change=move|ev|grant.update(|g|toggle(&mut g.source_ids,&id,event_target_checked(&ev)))/>{s.name}</label>}}).collect_view())}</fieldset>
        <fieldset class="runtime-sources"><legend>"Projects (current source membership)"</legend>{move||snap.get().map(|s|s.briefs.into_iter().map(|p|{let read=p.id.clone();let id=p.id;view!{<label><input type="checkbox" prop:checked=move||grant.get().project_ids.contains(&read) on:change=move|ev|grant.update(|g|toggle(&mut g.project_ids,&id,event_target_checked(&ev)))/>{p.name}</label>}}).collect_view())}</fieldset>
        <fieldset class="runtime-sources"><legend>"Model connections"</legend>{move||snap.get().map(|s|s.connections.into_iter().map(|c|{let read=c.id.clone();let id=c.id.clone();view!{<label><input type="checkbox" prop:checked=move||grant.get().connection_ids.contains(&read) on:change=move|ev|grant.update(|g|toggle(&mut g.connection_ids,&id,event_target_checked(&ev)))/>{c.id}</label>}}).collect_view())}</fieldset>
        <label><input type="checkbox" prop:checked=move||grant.get().external on:change=move|ev|grant.update(|g|g.external=event_target_checked(&ev))/>"Apply external-destination source policy to data supplied to this plugin"</label>
    </details>}
}

#[component]
fn PluginEditor(
    plugin: PluginRecord,
    records: Records,
    editor: RwSignal<u64>,
    selected: RwSignal<String>,
    tick: RwSignal<u64>,
    snap: Snap,
    notice: RwSignal<String>,
    view: RwSignal<String>,
) -> impl IntoView {
    let id = plugin.id.clone();
    let stale_id = id.clone();
    let reload_id = id.clone();
    let remove_id = id.clone();
    let version = plugin.version;
    let enabled = RwSignal::new(plugin.enabled);
    let grant = RwSignal::new(plugin.grant.clone());
    let draft = RwSignal::new(defaults(&plugin.manifest, &plugin.configuration));
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let confirm = RwSignal::new(false);
    let m = plugin.manifest.clone();
    let save_id = id.clone();
    view! {<h2>{plugin.manifest.name.clone()}</h2><p class="muted">{format!("{} · package {} · configuration v{} · {:?} host",plugin.id,plugin.manifest.version,version,plugin.manifest.host)}</p><p>{plugin.manifest.description.clone()}</p>
        <Show when=move||records.get().iter().find(|p|p.id==stale_id).is_none_or(|p|p.version!=version)>{let reload_id=reload_id.clone();view!{<div class="notice"><span>"This plugin changed. Your draft has been kept; reload before saving."</span><button on:click=move|_|{selected.set(reload_id.clone());editor.update(|v|*v+=1);}>"Reload settings"</button></div>}}</Show>
        <form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let configuration=match configuration(&m,draft){Ok(v)=>v,Err(e)=>{notice.set(e);return;}};action(format!("/api/v1/plugins/{save_id}"),"PATCH",json!({"expected_version":version,"enabled":enabled.get_untracked(),"grant":grant.get_untracked(),"configuration":configuration}),snap,notice,busy,pending,move|v|{if let Ok(p)=serde_json::from_value::<PluginRecord>(v){records.update(|items|{items.retain(|old|old.id!=p.id);items.push(p);});editor.update(|v|*v+=1);}tick.update(|v|*v+=1);});}>
            <fieldset disabled=move||busy.get()><label><input type="checkbox" prop:checked=move||enabled.get() on:change=move|ev|enabled.set(event_target_checked(&ev))/>"Enabled"</label><ConfigFields manifest=plugin.manifest.clone() draft=draft/>
                {(!plugin.builtin || plugin.id=="thesaurus.bragbook").then(||view!{<Grants manifest=plugin.manifest.clone() grant=grant snap=snap/>})}
                <button class="button primary" type="submit">"Save configuration and permissions"</button>
            </fieldset>
        </form>
        <super::plugin_setup::PluginSetup plugin=plugin.clone() snap=snap notice=notice view=view/>
        <details class="workflow-manage"><summary>"Manifest and dependencies"</summary><pre>{serde_json::to_string_pretty(&plugin.manifest).unwrap_or_default()}</pre></details>
        <Deliveries id=id.clone() notice=notice/>
        <NativeRequests id=id.clone() notice=notice/>
        {(!plugin.builtin && (!plugin.manifest.contributions.is_empty()||plugin.manifest.requests.iter().any(|op|op=="models.request"))).then(||view!{<super::plugin_runs::PluginRuns plugin=plugin.clone() snap=snap notice=notice/>})}
        {(!plugin.builtin).then(||view!{<PluginUpgrade plugin=plugin.clone() records=records editor=editor tick=tick snap=snap notice=notice/><details class="workflow-manage"><summary>"Uninstall local plugin"</summary><label><input type="checkbox" prop:checked=move||confirm.get() on:change=move|ev|confirm.set(event_target_checked(&ev))/>"Remove this disabled plugin from the workspace"</label><button class="button secondary" disabled=move||busy.get()||!confirm.get()||plugin.enabled on:click=move|_|action(format!("/api/v1/plugins/{remove_id}"),"DELETE",json!({"expected_version":version}),snap,notice,busy,pending,move|_|{selected.set(String::new());tick.update(|v|*v+=1);})>"Uninstall"</button></details>})}
    }
}

#[component]
fn Deliveries(id: String, notice: RwSignal<String>) -> impl IntoView {
    let rows = RwSignal::new(Vec::<PluginDelivery>::new());
    let error = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let refresh = RwSignal::new(0u64);
    let request_id = id.clone();
    Effect::new(move |_| {
        let _ = refresh.get();
        let id = request_id.clone();
        spawn_local(async move {
            let result = api(&format!("/api/v1/plugins/{id}/deliveries"), "GET", None)
                .await
                .and_then(|v| {
                    serde_json::from_value::<Vec<PluginDelivery>>(v)
                        .map_err(|_| "Invalid delivery response".into())
                });
            if rows.try_get_untracked().is_none() {
                return;
            }
            match result {
                Ok(v) => {
                    rows.set(v);
                    error.set(String::new());
                }
                Err(e) => error.set(e),
            }
        });
    });
    view! {<details class="workflow-manage"><summary>"Lifecycle deliveries"</summary><button class="source-link" on:click=move|_|refresh.update(|v|*v+=1)>"Refresh deliveries"</button><p class="muted">"Ordered per plugin, with bounded automatic retries. Explicit retry may revisit an older event after later work has completed."</p>
        <Show when=move||!error.get().is_empty()><p role="alert">{move||error.get()}</p></Show>
        <Show when=move||rows.get().is_empty()><p class="muted">"No deliveries recorded."</p></Show>
        {move||rows.get().into_iter().map(|d|{let plugin=id.clone();let history_plugin=plugin.clone();let history_delivery=d.id.clone();let retry=d.id.clone();let allowed=matches!(d.status.as_str(),"failed"|"rejected"|"cancelled");view!{<article class="plugin-delivery"><div>{format!("#{} · {} · {} attempts",d.event_sequence,d.status,d.attempts)}</div><small>{format!("{} · next check {}",d.updated_at,d.next_attempt_at)}</small><code>{d.error.unwrap_or_default()}</code><DeliveryAttempts plugin=history_plugin delivery=history_delivery notice=notice/><button class="source-link" disabled=move||busy.get()||!allowed on:click=move|_|{busy.set(true);let plugin=plugin.clone();let retry=retry.clone();spawn_local(async move{let result=api(&format!("/api/v1/plugins/{plugin}/deliveries/{retry}/retry"),"POST",Some(json!({}))).await;if busy.try_get_untracked().is_some(){busy.set(false);match result{Ok(_)=>refresh.update(|v|*v+=1),Err(e)=>notice.set(e)}}});}>"Retry delivery"</button></article>}}).collect_view()}
    </details>}
}

#[component]
fn DeliveryAttempts(plugin: String, delivery: String, notice: RwSignal<String>) -> impl IntoView {
    let rows = RwSignal::new(Vec::<PluginDeliveryAttempt>::new());
    let busy = RwSignal::new(false);
    view! {<details><summary>"Attempt history"</summary><button class="source-link" disabled=move||busy.get() on:click=move|_|{busy.set(true);let plugin=plugin.clone();let delivery=delivery.clone();spawn_local(async move{let result=api(&format!("/api/v1/plugins/{plugin}/deliveries/{delivery}/attempts"),"GET",None).await.and_then(|v|serde_json::from_value(v).map_err(|_|"Invalid attempt history".into()));if busy.try_get_untracked().is_some(){busy.set(false);match result{Ok(v)=>rows.set(v),Err(e)=>notice.set(e)}}});}>"Load attempt history"</button>
        {move||rows.get().into_iter().map(|a|view!{<article><p>{format!("{} · package {} · configuration v{} · API {}",a.status,a.package_version,a.configuration_version,a.api_version)}</p><small class="muted">{format!("{} → {}",a.started_at,a.finished_at.unwrap_or_else(||"running".into()))}</small><p>{a.error.unwrap_or_default()}</p><details><summary>"Recorded scope and fingerprints"</summary><pre>{serde_json::to_string_pretty(&json!({"grant":a.grant,"configuration_hash":a.configuration_hash,"executable_definition_hash":a.executable_definition_hash})).unwrap_or_default()}</pre></details></article>}).collect_view()}
    </details>}
}

#[component]
fn PluginInstall(
    snap: Snap,
    notice: RwSignal<String>,
    records: Records,
    selected: RwSignal<String>,
    tick: RwSignal<u64>,
) -> impl IntoView {
    view! {<section class="panel"><details><summary>"Install a local plugin"</summary><p class="muted">"Choose an executable already present on this computer and review its manifest, settings and permissions. Installation leaves it disabled. The executable runs with your OS account's privileges when enabled or invoked."</p><PackageForm existing=None snap=snap notice=notice records=records selected=selected tick=tick editor=RwSignal::new(0)/></details></section>}
}
#[component]
fn PluginUpgrade(
    plugin: PluginRecord,
    records: Records,
    editor: RwSignal<u64>,
    tick: RwSignal<u64>,
    snap: Snap,
    notice: RwSignal<String>,
) -> impl IntoView {
    let disabled = !plugin.enabled;
    view! {<details class="workflow-manage"><summary>"Upgrade package"</summary><p class="muted">"Disable the plugin and wait for active work to finish first. Upgrades preserve identity and require compatible configuration and grants."</p><Show when=move||disabled><PackageForm existing=Some(plugin.clone()) snap=snap notice=notice records=records selected=RwSignal::new(plugin.id.clone()) tick=tick editor=editor/></Show></details>}
}
#[component]
fn PackageForm(
    existing: Option<PluginRecord>,
    snap: Snap,
    notice: RwSignal<String>,
    records: Records,
    selected: RwSignal<String>,
    tick: RwSignal<u64>,
    editor: RwSignal<u64>,
) -> impl IntoView {
    let text = RwSignal::new(
        existing
            .as_ref()
            .map(|p| serde_json::to_string_pretty(&p.manifest).unwrap_or_default())
            .unwrap_or_default(),
    );
    let program = RwSignal::new(
        existing
            .as_ref()
            .and_then(|p| p.executable.as_ref())
            .map(|e| e.program.clone())
            .unwrap_or_default(),
    );
    let args = RwSignal::new(
        existing
            .as_ref()
            .and_then(|p| p.executable.as_ref())
            .map(|e| serde_json::to_string_pretty(&e.args).unwrap_or_else(|_| "[]".into()))
            .unwrap_or_else(|| "[]".into()),
    );
    let manifest = RwSignal::new(None::<PluginManifest>);
    let draft = RwSignal::new(BTreeMap::new());
    let grant = RwSignal::new(
        existing
            .as_ref()
            .map(|p| p.grant.clone())
            .unwrap_or_default(),
    );
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let checked = RwSignal::new(false);
    let configuration = existing
        .as_ref()
        .map(|p| p.configuration.clone())
        .unwrap_or(json!({}));
    let upgrading = existing.is_some();
    view! {<form class="workflow-form" on:submit=move|ev|{ev.prevent_default();let Some(m)=manifest.get_untracked()else{return;};let cfg=match self::configuration(&m,draft){Ok(v)=>v,Err(e)=>{notice.set(e);return;}};let args=match serde_json::from_str::<Vec<String>>(&args.get_untracked()){Ok(v)=>v,Err(e)=>{notice.set(format!("Arguments must be a JSON string array: {e}"));return;}};let executable=json!({"program":program.get_untracked(),"args":args});let(path,method,body)=if let Some(p)=&existing {(format!("/api/v1/plugins/{}",p.id),"PATCH",json!({"expected_version":p.version,"enabled":false,"manifest":m,"executable":executable,"configuration":cfg,"grant":grant.get_untracked()}))}else{("/api/v1/plugins".into(),"POST",json!({"manifest":m,"executable":executable,"configuration":cfg,"grant":grant.get_untracked()}))};action(path,method,body,snap,notice,busy,pending,move|value|{if let Ok(p)=serde_json::from_value::<PluginRecord>(value){selected.set(p.id.clone());records.update(|items|{items.retain(|old|old.id!=p.id);items.push(p);});editor.update(|v|*v+=1);}checked.set(false);tick.update(|v|*v+=1);});}>
        <fieldset disabled=move||busy.get()>
            <label>"Manifest JSON"<textarea rows="10" required prop:value=move||text.get() on:input=move|ev|{text.set(event_target_value(&ev));manifest.set(None);checked.set(false);}/></label>
            <button class="button secondary" type="button" on:click=move|_|{match serde_json::from_str::<PluginManifest>(&text.get_untracked()){Ok(m)=>{if m.host!=PluginHost::Service||m.id.starts_with("thesaurus."){notice.set("Local extensions must target the service and use their own namespace".into());return;}draft.set(defaults(&m,&configuration));grant.update(|g|g.operations.retain(|op|m.requests.contains(op)));manifest.set(Some(m));checked.set(false);},Err(e)=>notice.set(format!("Invalid manifest: {e}"))}}>"Read manifest"</button>
            <label>"Executable · absolute path on the service computer"<input required prop:value=move||program.get() on:input=move|ev|{program.set(event_target_value(&ev));checked.set(false);}/></label>
            <label>"Arguments · JSON string array, passed without a shell"<textarea rows="3" prop:value=move||args.get() on:input=move|ev|{args.set(event_target_value(&ev));checked.set(false);}/></label>
            {move||manifest.get().map(|m|view!{<h3>{format!("{} · {}",m.name,m.version)}</h3><p>{m.description.clone()}</p><ConfigFields manifest=m.clone() draft=draft/><Grants manifest=m grant=grant snap=snap/>})}
            <label><input type="checkbox" prop:checked=move||checked.get() on:change=move|ev|checked.set(event_target_checked(&ev))/>"I reviewed this local executable, its configuration and requested access"</label>
            <button class="button primary" type="submit" disabled=move||manifest.get().is_none()||!checked.get()>{if upgrading {"Save disabled package upgrade"}else{"Install disabled"}}</button>
        </fieldset>
    </form>}
}

#[component]
fn NativeRequests(id: String, notice: RwSignal<String>) -> impl IntoView {
    let rows = RwSignal::new(Vec::<PluginNativeReceipt>::new());
    let busy = RwSignal::new(false);
    let loaded = RwSignal::new(false);
    view! {<details class="workflow-manage"><summary>"Native requests"</summary>
      <p class="muted">"Explicit command grants allow this plugin to request local window, tray or notification actions. Accepted requests are queued; inspect the receipt for OS/provider completion. Expired or uncertain requests are never replayed automatically."</p>
      <button class="source-link" disabled=move||busy.get() on:click=move|_|{busy.set(true);let id=id.clone();spawn_local(async move{
        let result=api(&format!("/api/v1/plugins/{id}/native-requests"),"GET",None).await.and_then(|v|serde_json::from_value(v).map_err(|_|"Invalid native request history".into()));
        if busy.try_get_untracked().is_some(){busy.set(false);match result{Ok(v)=>{rows.set(v);loaded.set(true);},Err(e)=>notice.set(e)}}
      });}>"Load native requests"</button>
      <Show when=move||loaded.get()&&rows.get().is_empty()><p class="muted">"No native requests recorded for this installation."</p></Show>
      {move||rows.get().into_iter().map(|r|view!{<article class="plugin-delivery">
        <h3>{format!("{}.{} · {}",r.capability,r.command,r.status)}</h3>
        <p class="muted">{format!("{} · {} {} · package {} · configuration v{}",r.created_at,r.origin_kind,r.origin_id,r.package_version,r.configuration_version)}</p>
        <p role="status">{if r.status=="completed"&&r.error.is_none(){if r.capability=="native.notifications"{"Provider completed; submission does not confirm banner display"}else{"Provider completed"}}else if r.error.is_some(){"Provider reported failure; the action was not confirmed"}else if r.status=="rejected"{"Provider selection or plugin authority changed before dispatch"}else if r.status=="expired"{"Request expired; outcome may be unknown and was not replayed"}else{"Request accepted; completion is not confirmed"}}</p>
        <p>{r.error.unwrap_or_default()}</p>
        <details><summary>"Recorded scope and result"</summary><pre>{serde_json::to_string_pretty(&json!({"request_id":r.id,"grant":r.grant,"source_ids":r.source_ids,"project_ids":r.project_ids,"result":r.result})).unwrap_or_default()}</pre></details>
      </article>}).collect_view()}
    </details>}
}
