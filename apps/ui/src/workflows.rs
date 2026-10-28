use super::*;

#[derive(Clone)]
pub(super) struct Pending {
    path: String,
    method: &'static str,
    intent: Value,
    body: Value,
}
pub(super) fn action(
    path: String,
    method: &'static str,
    mut body: Value,
    snap: Snap,
    notice: RwSignal<String>,
    busy: RwSignal<bool>,
    pending: RwSignal<Option<Pending>>,
    after: impl FnOnce(Value) + 'static,
) {
    if busy.get_untracked() {
        return;
    }
    let mut intent = body.clone();
    if let Some(o) = intent.as_object_mut() {
        o.remove("expected_version");
        o.remove("other_expected_version");
    }
    let request = pending
        .get_untracked()
        .filter(|p| p.path == path && p.method == method && p.intent == intent)
        .unwrap_or_else(|| {
            body["idempotency_key"] = json!(new_id());
            Pending {
                path,
                method,
                intent,
                body,
            }
        });
    pending.set(Some(request.clone()));
    busy.set(true);
    spawn_local(async move {
        let result = api(&request.path, request.method, Some(request.body)).await;
        if busy.try_get_untracked().is_none() {
            return;
        }
        busy.set(false);
        match result {
            Ok(value) => {
                pending.set(None);
                notice.set(String::new());
                after(value);
                refresh(snap, notice);
            }
            Err(error) => {
                if ["invalid", "conflict", "forbidden", "unknown"]
                    .iter()
                    .any(|p| error.starts_with(p))
                {
                    pending.set(None);
                }
                notice.set(error);
                refresh(snap, notice);
            }
        }
    });
}
pub(super) fn chosen_project(snap: Snap, id: RwSignal<String>) -> Option<ProjectBrief> {
    let projects = snap.get()?.briefs;
    projects
        .iter()
        .find(|p| p.id == id.get())
        .cloned()
        .or_else(|| projects.first().cloned())
}
#[component]
pub(super) fn Citations(
    evidence: Vec<Evidence>,
    selected: RwSignal<Option<Document>>,
    notice: RwSignal<String>,
) -> impl IntoView {
    view! {<div class="brief-evidence">{evidence.into_iter().map(|e|view!{<details><summary>{format!("{} · r{}:{}",e.document_id,e.revision,e.line)}</summary><blockquote>{e.quote.clone()}</blockquote><button class="source-link" on:click=move|_|super::briefs::open_evidence(e.clone(),selected,notice)>"Open source →"</button></details>}).collect_view()}</div>}
}
#[component]
pub(super) fn ProjectSources(
    project: ProjectBrief,
    snap: Snap,
    notice: RwSignal<String>,
) -> impl IntoView {
    let additions = RwSignal::new(Vec::<String>::new());
    let busy = RwSignal::new(false);
    let pending = RwSignal::new(None::<Pending>);
    let scope = project.source_ids.clone();
    let id = project.id;
    let version = project.version;
    view! {<details class="workflow-manage"><summary>"Add project sources"</summary><p class="muted">"New sources will mark existing briefs stale. Existing source history is retained."</p>
        <fieldset class="runtime-sources"><legend>"Available sources"</legend>{move||snap.get().map(|s|s.sources.into_iter().filter(|s|!scope.contains(&s.id)).map(|s|{let id=s.id.clone();let check=s.id;view!{<label><input type="checkbox" prop:checked=move||additions.get().contains(&check) on:change=move|ev|{let checked=event_target_checked(&ev);additions.update(|ids|{ids.retain(|i|i!=&id);if checked{ids.push(id.clone());}});}/><span>{s.name}</span></label>}}).collect_view())}</fieldset>
        <button class="button secondary" disabled=move||busy.get() || additions.get().is_empty() on:click=move|_|{let mut ids=project.source_ids.clone();ids.extend(additions.get_untracked());action(format!("/api/v1/projects/{id}/sources"),"PATCH",json!({"expected_version":version,"source_ids":ids}),snap,notice,busy,pending,move|_|additions.set(vec![]));}>"Add selected sources"</button>
    </details>}
}
