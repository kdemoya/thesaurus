use super::*;
#[wasm_bindgen(
    inline_js = "export function saveContextExport(name,mime,body){const blob=new Blob([body],{type:mime+';charset=utf-8'});const url=URL.createObjectURL(blob);const a=document.createElement('a');a.href=url;a.download=name;a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);}"
)]
extern "C" {
    #[wasm_bindgen(js_name=saveContextExport)]
    pub(super) fn save_export(name: &str, mime: &str, body: &str);
}
#[component]
pub(super) fn Exports(snap: Snap, notice: RwSignal<String>) -> impl IntoView {
    let scope = RwSignal::new(Vec::<String>::new());
    let format = RwSignal::new("markdown".to_string());
    let result = RwSignal::new(None::<Value>);
    let busy = RwSignal::new(false);
    view! {<details class="panel workflow-manage"><summary>"Export portable work context"</summary><p class="muted">"Choose sources to export with immutable revisions, tasks, citations and observation history. The preview is a local snapshot. Downloaded files contain the selected work data."</p><fieldset class="focus-form-fields" disabled=move||busy.get()><fieldset class="runtime-sources"><legend>"Included sources"</legend>{move||snap.get().map(|s|s.sources.into_iter().map(|s|{let id=s.id.clone();view!{<label><input type="checkbox" prop:checked=move||scope.get().contains(&id) on:change=move|ev|{let checked=event_target_checked(&ev);scope.update(|ids|{ids.retain(|id|id!=&s.id);if checked{ids.push(s.id.clone());}});result.set(None);}/>{s.name}</label>}}).collect_view())}</fieldset><label>"Format"<select prop:value=move||format.get() on:change=move|ev|{format.set(event_target_value(&ev));result.set(None);}><option value="markdown">"Markdown"</option><option value="json">"JSON"</option></select></label><button class="button secondary" disabled=move||scope.get().is_empty() on:click=move|_|{if busy.get_untracked(){return;}busy.set(true);let request=json!({"source_ids":scope.get_untracked(),"format":format.get_untracked()});spawn_local(async move{let response=api("/api/v1/exports","POST",Some(request)).await;busy.set(false);match response{Ok(v)=>result.set(Some(v)),Err(e)=>notice.set(e)}});}>"Prepare export"</button></fieldset>{move||result.get().map(|r|{let body=r["body"].as_str().unwrap_or_default().to_string();let download=body.clone();let filename=r["filename"].as_str().unwrap_or("context.json").to_string();let mime=r["content_type"].as_str().unwrap_or("application/json").to_string();view!{<p class="muted">{format!("Snapshot {} · {} bytes",r["watermark"],body.len())}</p><button class="button secondary" on:click=move|_|save_export(&filename,&mime,&download)>"Download snapshot"</button><details><summary>"Inspect export"</summary><pre style="white-space:pre-wrap;overflow-wrap:anywhere;max-height:28rem;overflow:auto">{body}</pre></details>}})}</details>}
}
