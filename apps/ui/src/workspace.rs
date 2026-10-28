use super::*;

#[derive(Clone, Copy)]
pub(super) struct SettingsNavigation(pub Callback<()>);
pub(super) fn open_settings() {
    if let Some(n) = use_context::<SettingsNavigation>() {
        n.0.run(());
    }
}

#[wasm_bindgen(
    inline_js = "export function resetWorkspaceScroll(){requestAnimationFrame(()=>{document.getElementById('page-content')?.scrollTo(0,0);document.querySelector('main.settings-view')?.scrollTo(0,0);});}"
)]
extern "C" {
    #[wasm_bindgen(js_name=resetWorkspaceScroll)]
    pub(super) fn reset_scroll();
}

pub(super) fn sections(page: &str) -> &'static [(&'static str, &'static str)] {
    match page {
        "bragbook" => &[
            ("candidate", "To review"),
            ("accepted", "Accepted & highlights"),
            ("dismissed", "Dismissed"),
            ("collection", "Collection"),
        ],
        "library" => &[
            ("sources", "Sources"),
            ("retrieval", "Retrieval"),
            ("repositories", "Repositories"),
            ("capture", "Capture"),
            ("access", "Access"),
            ("context", "Context & export"),
        ],
        "memory" => &[
            ("observations", "Observations"),
            ("review", "Review & conflicts"),
        ],
        "activity" => &[
            ("runs", "Agent runs"),
            ("actions", "Action review"),
            ("notifications", "Notifications"),
            ("processing", "Processing"),
            ("completed", "Completed"),
        ],
        "connections" => &[
            ("models", "Models"),
            ("agent", "Run an agent"),
            ("usage", "Usage & limits"),
        ],
        "meetings" => &[("live", "Recording"), ("preparation", "Preparation")],
        "threads" => &[("threads", "Threads"), ("handoffs", "Handoff receipts")],
        "plugins" => &[
            ("installed", "Installed"),
            ("capabilities", "Native capabilities"),
        ],
        "settings" => &[
            ("processing", "Automation"),
            ("audio", "Audio & transcription"),
            ("models", "Models & usage"),
            ("appearance", "Appearance"),
        ],
        _ => &[],
    }
}

pub(super) fn title(page: &str) -> &'static str {
    match page {
        "library" => "Sources",
        "memory" => "Shared memory",
        "connections" => "Connections",
        "activity" => "Activity",
        "briefs" => "Project briefs",
        "threads" => "Work threads",
        "meetings" => "Meetings",
        "focus" => "Project focus",
        "recipes" => "Work recipes",
        "evaluations" => "Evaluations",
        "plugins" => "Plugins",
        "bragbook" => "Bragbook",
        "settings" => "Settings",
        _ => "Work",
    }
}

pub(super) fn description(page: &str) -> &'static str {
    match page {
        "library" => "Find the context behind your work.",
        "memory" => "What you and your agents have learned.",
        "connections" => "Your models, accounts and local runtimes.",
        "activity" => "Follow progress and review what needs you.",
        "briefs" => "A current picture of each project, with evidence.",
        "threads" => "Pick up where the work left off.",
        "meetings" => "Capture the conversation. Keep the context.",
        "focus" => "Decide what deserves your attention next.",
        "recipes" => "Useful analyses, ready to run again.",
        "evaluations" => "Check the quality of your agents’ work.",
        "plugins" => "Extend your workspace and manage its capabilities.",
        "bragbook" => "Keep a record of your contribution, with evidence.",
        "settings" => "Set your preferences once. Keep the work moving.",
        _ => "Review commitments. Choose your next action.",
    }
}

#[component]
pub(super) fn Navigation(
    view: RwSignal<String>,
    hits: RwSignal<Option<Vec<SearchHit>>>,
) -> impl IntoView {
    view! {
        <nav class="workspace-nav" aria-label="Main navigation">
            {[
                ("Daily", vec![("now", "1", "Work"), ("focus", "9", "Focus"), ("meetings", "8", "Meetings"), ("bragbook", "b", "Bragbook")]),
                ("Knowledge", vec![("library", "2", "Sources"), ("memory", "3", "Memory"), ("briefs", "6", "Briefs"), ("threads", "7", "Threads")]),
                ("Tools", vec![("activity", "4", "Activity"), ("recipes", "0", "Recipes"), ("evaluations", "e", "Evaluations"), ("connections", "5", "Connections"), ("plugins", "p", "Plugins")]),
            ].into_iter().map(|(group, items)| view! {
                <div class="nav-group"><span class="nav-group-label">{group}</span>
                    {items.into_iter().map(|(id, key, label)| view! {
                        <button class:active=move||view.get()==id on:click=move|_|{view.set(id.into());hits.set(None);} aria-current=move||if view.get()==id{"page"}else{"false"}>
                            <span>{label}</span><kbd class="nav-key">{key}</kbd>
                        </button>
                    }).collect_view()}
                </div>
            }).collect_view()}
        </nav>
    }
}

#[component]
pub(super) fn Sections(view: RwSignal<String>, section: RwSignal<String>) -> impl IntoView {
    view! {
        <Show when=move||!sections(&view.get()).is_empty()>
            <div class="page-sections" role="group" aria-label="Page sections">
                {move||sections(&view.get()).iter().map(|&(id,label)|view!{
                    <button class:active=move||section.get()==id aria-pressed=move||section.get()==id on:click=move|_|section.set(id.into())>{label}</button>
                }).collect_view()}
            </div>
        </Show>
    }
}
