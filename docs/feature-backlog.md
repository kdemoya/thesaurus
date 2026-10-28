# Thesaurus Feature Ideas

Research date: October 16, 2026.

Implementation and acceptance are tracked in [backlog progress](backlog-progress.md); the rows below preserve the product requirements.

This backlog contains 48 feature ideas drawn from the [similar-project research](similar-projects.md) and product feedback, including the October 18 requirements for plugins, native capability modularity and guided onboarding. It includes existing requirements, refinements, and new product opportunities. The inspiration column links to documented patterns; the proposed Thesaurus behavior is our synthesis and should not be read as a claim that the referenced project ships that exact feature.

## Priority and scope

- **P0:** Required by the current product direction or needed to make an existing requirement usable and trustworthy. Implement within its relevant phase of the [implementation plan](implementation-plan.md).
- **P1:** Strong extension to prototype after its prerequisites work. Select individually before adding to release scope.
- **P2:** Later exploration or a substantial independent workstream.

**Fit:** Existing means the plan already covers the capability; Refines specifies a stronger user experience or acceptance condition; New introduces additional scope.

**Size:** S affects one established component; M spans several components; L requires a substantial workflow or integration; XL is a separate workstream. Sizes assume prerequisites exist and are not delivery estimates.

## Meetings and capture

Prerequisites: transcript revisions, speaker identity handling, evidence locators, and the source viewer. Calendar preparation requires a calendar source that is not yet an MVP dependency. Audio/activity capture remains a later workstream.

| ID | Idea and observable outcome | Priority and size | Fit | Inspiration |
| --- | --- | --- | --- | --- |
| F01 | **Exact evidence navigation.** Open an action's cited transcript interval or document passage, including the revision used. | P0 · M | Existing | [Granola MCP][granola], [Graphiti][graphiti] |
| F02 | **Notes anchored to conversation.** Attach a manual note to the current transcript position and preserve the user's emphasis in summaries. | P1 · M | New | [Anarlog][anarlog] |
| F03 | **Recurring meeting preparation — initial implementation delivered.** Manually linked meeting series prepare unresolved commitments, task changes, accepted brief questions/blockers and source changes against the previous captured baseline. See [usage and limits](work-continuity.md). | P1 · M | New | [Granola briefs][briefs] |
| F04 | **Live question cues.** Surface an unanswered question or conflicting statement with its source; allow dismissal without interrupting the meeting. | P2 · L | Refines | [Granola meeting context][granola]; Thesaurus synthesis |
| F05 | **Speaker and terminology corrections.** Correct a speaker alias or company term and preview affected extractions before recomputing them. | P1 · M | New | [Anarlog transcription][anarlog] |
| F06 | **Independent audio and reasoning choices.** Choose local transcription and a separate approved reasoning connection; show both processing destinations. | P2 · XL | Refines | [Anarlog][anarlog] |
| F46 | **Transcription quality and provider choices.** Offer **Local · Normal** and **Local · Large** presets with visible model/download/hardware tradeoffs, plus explicitly selected cloud or company-hosted transcription. Improve speech detection, language handling and chunk continuity. Deepgram and Soniox are the first cloud adapters; keep reasoning selection independent. Integrate provider/model readiness into onboarding and compare quality through user/project evaluations. Selection, downloads and initial chunk-based adapters are implemented; live provider/quality acceptance and the remaining pipeline work are pending. Base remains the automatic offline default. See [delivery and remaining scope](transcription-options.md). | P0 · L | Refines | User requirement; builds on F06, F30, F37, F42–F45 |
| F07 | **Selected activity context.** Opt into capture for chosen apps/projects, pause visibly, and inspect what was retained. | P2 · XL | New | [Screenpipe][screenpipe] |

## Knowledge and retrieval

Prerequisites: normalized source records, permissions before retrieval, freshness metadata, and connector checkpoints. Historical queries require both observation time and validity time; semantic repository queries require an approved code intelligence integration.

| ID | Idea and observable outcome | Priority and size | Fit | Inspiration |
| --- | --- | --- | --- | --- |
| F08 | **Indexed and live retrieval policy.** Choose index, fetch-on-demand, or hybrid per source and identify the path used for each result. | P1 · L | New | [Glean connectors][glean] |
| F09 | **Answer evidence panel.** Show supporting passages, source dates, unresolved contradictions, and missing evidence beside an answer. | P0 · M | Refines | [Onyx][onyx] |
| F10 | **Knowledge as of a date.** Answer historical ownership or decision questions and compare the supporting facts with today's state. | P1 · M | Refines | [Graphiti][graphiti] |
| F11 | **People and expertise lookup.** Find relevant people through cited authored work and explicit roles; show uncertainty in identity matches. | P2 · M | New | [Glean context][glean]; Thesaurus synthesis |
| F12 | **Repository changes relevant to work.** Summarize selected changes against active tasks and show exact commits/files. | P1 · M | Refines | [Serena][serena], [Threadline][threadline] |
| F13 | **Symbol-aware repository questions.** Retrieve definitions and references through an approved service without reading entire repositories. | P2 · L | New | [Serena][serena] |
| F14 | **Knowledge gap inbox.** Save questions that lack enough evidence, identify the missing source, and revisit after new information arrives. | P1 · M | New | [Onyx retrieval][onyx]; Thesaurus synthesis |

## Shared memory and continuity

Prerequisites: scoped agent identity, versioned records, evidence lineage, and user review of contributions. A living brief is a derived record with source watermarks, not another independent source of truth.

| ID | Idea and observable outcome | Priority and size | Fit | Inspiration |
| --- | --- | --- | --- | --- |
| F15 | **Shared memory through MCP and API.** Two separately authorized agents retrieve the same permitted context and contribute attributed observations. | P0 · L | Existing | [Mem0][mem0], [Onyx MCP][onyx] |
| F16 | **Memory inspector and undo.** Inspect why a memory exists, who contributed it, its sources and scope; correct or supersede it with history. | P0 · M | Refines | [Letta MemFS][letta] |
| F17 | **Living project briefs — initial implementation delivered.** Source-scoped projects generate cited goals, decisions, blockers, and questions through ACP. Review drafts, inspect revision differences, detect stale sections, and share accepted revisions through MCP. See [usage and current limits](living-briefs.md). | P1 · M | New | [Hindsight knowledge pages][hindsight] |
| F18 | **Context bundles with a budget.** Produce a compact task context with citations, scope, token budget, omissions, and snapshot watermark. | P0 · M | Refines | [Hindsight recall][hindsight] |
| F19 | **Work threads and resume — initial implementation delivered.** Named project threads retain attributed checkpoints and linked records; users can split/merge, preview bounded context, and resume through a chosen ACP agent with durable delivery receipts. See [usage and limits](work-continuity.md). | P1 · L | New | [Threadline][threadline] |
| F20 | **Conflicting memory review.** Present incompatible claims with dates and sources; preserve uncertainty until a source or user resolves it. | P1 · M | Refines | [Graphiti][graphiti] |
| F21 | **Handoff receipts.** Record the context revision delivered to an agent and distinguish delivery, acknowledgement, and completed work. | P1 · M | Refines | [Letta agent continuity][letta]; Thesaurus synthesis |

## Actions and discovery

Prerequisites: accepted tasks, explicit identity mappings, links between source claims and tasks, and durable feedback. Evidence of implementation can propose completion; the user or a trusted external task state confirms it.

| ID | Idea and observable outcome | Priority and size | Fit | Inspiration |
| --- | --- | --- | --- | --- |
| F22 | **Commitment ledger.** Track what was promised, by whom, when, and how the promise changed, with supporting evidence. | P0 · L | Refines | [Threadline commitments][threadline] |
| F23 | **Priority changes explained — initial implementation delivered.** Project priorities show deadline, user-importance and waiting signals, source freshness, saved review baselines and changes in relative order with cited influences. See [usage and limits](project-focus.md). | P1 · M | Refines | [Khoj scheduled insights][khoj]; Thesaurus synthesis |
| F24 | **Waiting-for queue — initial implementation delivered.** Track people, requests, exact evidence and chosen follow-up dates; review local follow-up drafts, snooze or record responses with attributed history. See [usage and limits](project-focus.md). | P1 · M | New | [Granola open threads][briefs]; Thesaurus synthesis |
| F25 | **Meeting-to-delivery trace.** Link a commitment to later discussion and implementation evidence; propose completion for review. | P1 · L | New | [Threadline][threadline], [Serena][serena] |
| F26 | **Repeated pain-point discovery.** Group independent evidence of recurring friction and propose an opportunity with affected work and counterevidence. | P1 · L | Refines | [Hindsight reflection][hindsight]; Thesaurus synthesis |
| F27 | **Weekly outcomes and loose ends.** Draft a cited account of completed work, changed decisions, blockers, and unresolved commitments. | P1 · M | New | [Khoj automations][khoj], [Hermes scheduling][hermes] |
| F28 | **Personal capacity review.** Compare accepted deadlines, estimated effort, and selected calendar commitments; suggest tradeoffs the user can edit. | P2 · L | New | [Granola calendar preparation][briefs]; Thesaurus synthesis |

## Existing agents and reusable workflows

Prerequisites: documented runtime adapters, source destination policy, structured outputs, and account/billing identification. ACP compatibility is a foundation experiment; adopting an adapter remains a separate decision.

| ID | Idea and observable outcome | Priority and size | Fit | Inspiration |
| --- | --- | --- | --- | --- |
| F29 | **Use existing agent/model connections.** Choose supported installed-agent, subscription, API, or local access while retaining the user's other agent tools. | P0 · L | Existing | [Goose providers][goose-acp], [OpenClaw harnesses][openclaw] |
| F30 | **Quota and billing controls.** Reserve capacity for interactive work, pause background jobs, and require an explicit rule for paid fallback. | P0 · M | Existing | [Goose provider separation][goose-acp]; Thesaurus synthesis |
| F31 | **ACP runtime adapter — initial implementation delivered.** Codex ACP v1 runs support scoped MCP memory, streamed output, one-time permission controls, cancellation, and durable receipts. One live synthetic Codex request completed through the existing ChatGPT login. See [integration status and comparison](acp-integration.md) for verification limits and remaining compatibility work. | P1 · M | New | [ACP][acp], [Goose ACP][goose-acp] |
| F32 | **Saved work recipes.** Save a useful analysis with editable inputs, source scope, model route, output format, and budget. | P1 · M | New | [Granola recipes][recipes] |
| F33 | **Agent run progress and reconnect.** Show task/session progress and resume observation after client closure without duplicating the task. | P1 · L | Refines | [Goose sessions][goose], [Letta][letta] |
| F34 | **Suggested reusable procedures.** Propose a recipe or skill from repeated successful work, show the supporting runs, and require review before installing it. | P2 · L | New | [Hermes skills][hermes] |

## Control and reliability

Prerequisites: shared authorization services, safe telemetry, lineage, version checks, and the durable job/event system. These capabilities should appear in the user interface as sources, explanations, and controls rather than implementation diagnostics.

| ID | Idea and observable outcome | Priority and size | Fit | Inspiration |
| --- | --- | --- | --- | --- |
| F35 | **Explain notifications.** Show why a suggestion appeared now, which evidence is new, and controls to mute similar notifications. | P0 · M | Refines | [Khoj notifications][khoj]; Thesaurus synthesis |
| F36 | **Source freshness and coverage.** Report last sync, excluded content, capture gaps, and the effect on answers; provide a targeted retry. | P0 · M | Refines | [Screenpipe capture status][screenpipe], [Glean freshness][glean] |
| F37 | **Data destination controls.** Preview what a source may disclose to models or agents and revoke that access independently of ingestion. | P0 · M | Existing | [Anarlog provider boundaries][anarlog], [Onyx permissions][onyx] |
| F38 | **Memory quality evaluation.** Replay selected cases after a model/prompt change; compare citation accuracy, stale answers, duplication, and cost. Supplies the evaluation runner and comparison history for F42. | P1 · M | Refines | [Mem0 evaluation distinctions][mem0]; Thesaurus synthesis |
| F39 | **Preview actions and recover changes.** Show exact proposed effects, retain execution receipts, and offer undo only where the provider supports it. | P1 · L | Refines | [Threadline reversible operations][threadline]; Thesaurus synthesis |
| F40 | **Portable work context exports.** Export selected context, citations, and observation history to Markdown/JSON with scope and snapshot date. | P1 · M | Refines | [Anarlog export][anarlog], [Letta versioned context][letta] |
| F42 | **Project-level and user-defined evaluations.** Create evaluation suites for a project and reusable personal suites. Users define cases, expected answers or outcomes, required evidence, scoring rubrics, and pass/fail thresholds for retrieval, memory, briefs, priorities, and agent workflows. Support deterministic checks, human review, and explicitly selected model judges. Version cases and criteria; pin source snapshots and record agent/model/prompt versions so runs can be compared against a chosen baseline with per-case results and regression explanations. Project suites inherit project source permissions; applying a personal suite to a project grants no extra data access. Preview model usage and enforce the user's run budget. | P1 · L | New | User requirement; builds on F38 |

## Interface and preferences

Prerequisites: shared semantic color tokens and persisted user preferences. Built-in theme support is implemented; the original TUI palette remains the default. See [theme usage](themes.md) and [acceptance status](backlog-progress.md).

| ID | Idea and observable outcome | Priority and size | Fit | Inspiration |
| --- | --- | --- | --- | --- |
| F41 | **Theme support.** Preview and switch between built-in terminal palettes and light/dark themes, optionally follow system appearance, and retain the selection across restarts in both desktop and browser clients. Apply the theme consistently to panels, tables, dialogs, inputs, source views, focus indicators, and status colors while preserving readable contrast and distinct selection states. | P1 · M | New | User-provided TUI references and product feedback |
| F43 | **Meeting companion widget.** Start, pause, resume and stop meeting recording without opening the dashboard; see recording status, elapsed time and real-time meeting context, capture notes/bookmarks, and review suggested actions. Experiment with a menu bar/system tray icon and an optional floating widget anchored to a screen border. Share meeting and recording state with the main app, support keyboard shortcuts and existing themes, and expand into the full meeting workspace. Initial implementation includes live audio capture, incremental local transcription, shared controls and native companion surfaces. Explicit post-recording processing now saves cited key points, decisions, action candidates, questions and risks through a selected agent/model, with history and candidate review; activation and model-quality acceptance are tracked separately. See [usage and acceptance limits](meeting-companion.md). | P1 · L | New | User requirement; builds on F01–F06, F18 and F35 |
| F45 | **Guided onboarding and configuration readiness.** Help a new user connect a primary model/installed agent, configure chosen capture and data capabilities, and reach a first useful result. Show required setup where a feature is invoked, preserve progress, and offer targeted repair when configuration stops working. Give the shared desktop/browser onboarding a pixel-dot visual language inspired by the user's [personal site](https://kdemoya.github.io/), with accessible forms and optional particle motion. | P0 · L | Refines | User requirement; builds on F06, F15, F29–F31, F37, F41 and F43; later integrates with F44 |

### F43 scope and experiments

- Compare a compact menu bar/system tray panel with a collapsible floating widget docked to a chosen screen edge. Prototype the icon first; evaluate whether persistent context in the floating version is useful without obstructing work. Preserve its chosen edge and position.
- Start recording immediately with saved microphone/system-audio settings; reconcile the title, project and meeting series afterward. Audio configuration is a separate one-time setup. Keep recording, paused and stopped states unmistakable, expose pause/stop immediately, and show capture gaps or processing delays.
- Present cited context such as relevant people, previous decisions, open commitments and unanswered questions. Keep updates quiet; let the user expand supporting evidence, add a quick note, bookmark a moment or review an action item.
- Use the same Rust background recording session as the main interface. Switching surfaces or reopening the widget must not duplicate recording; all controls must reflect the same state.
- **Capture foundation implemented:** the service now owns explicit microphone/system-audio recording, permission-triggered startup, bounded chunked local transcription and shared session controls. Browser/native generated-audio controls and full-screen tray checks pass; real-device input and speech quality remain unverified. See [meeting companion](meeting-companion.md) and [plugin acceptance](plugin-acceptance-audit.md).

### F45 onboarding flow and visual direction

**Goal:** a new user can tell what is ready, what needs configuration, and how to get one useful outcome. Configuration errors should lead to the relevant setup step rather than require guessing an executable, credential reference, model path or OS permission from a generic error message. An initial guided flow is implemented; see [onboarding behavior and delivery limits](onboarding.md). The detailed requirements below remain the target, with runtime acceptance and follow-up work tracked separately.

| Step | User experience | Completion evidence |
| --- | --- | --- |
| Choose a starting goal | Offer recording/transcription, work-context analysis, or connecting an existing agent to shared memory; allow exploration and changing the choice later. | Only the selected goal's prerequisites are required. |
| Connect the main model/agent | Show supported detected installations and choices for an existing agent/account, API connection or local model. Choose the primary connection and show its model/account/billing route where known. Reference existing credentials through supported setup paths. | Separate installed, configured, authenticated and callable states; distinguish unknown capabilities from confirmed ones. No silent paid fallback. |
| Check the connection | Validate configuration and supported authentication/capabilities first. Offer a short synthetic request with an explicit usage preview when a real invocation is needed. | Report the actual check result and what remains unverified; saving a connection or probing a binary is not proof of a successful model response. |
| Prepare selected capabilities | For recording, offer Local Normal/Large presets, supported cloud providers or a company endpoint; show model download/hardware needs or credential/usage setup, then guide input selection and platform permissions. Validate the selected transcription path. For knowledge/agents, guide source selection, connector authentication and scoped memory setup. Explain missing prerequisites and provide direct repair actions. | Each selected capability reports ready, needs setup, checking, unavailable, or repair needed, with a concrete reason. Downloads/installation, recording, source sync and configuration writes are explicit actions. |
| Get a first useful result | Offer a small sample transcript or user-selected note for a cited action preview; alternatively record explicitly or connect a chosen agent to scoped memory. Explain the next available action. | Preview-only examples are labeled; a real invocation/capture/sync is reported complete only when it actually succeeds. |
| Return or repair later | Resume saved progress, reopen any step from Settings, and take the user directly to the missing setup when a feature is invoked. Preserve configured connections and unrelated progress. | Recheck the affected prerequisite when a token expires, executable/model disappears, microphone changes or plugin/provider becomes unavailable. |

- **Progressive requirements.** Recording with local transcription does not require a reasoning model, and text capture/inspection does not require microphone access. Allow skipping optional connections and sources. Once audio is ready, **Start recording** stays immediate; meeting title/project/series are still reconciled afterward. First-use setup is an explicit readiness step, not a form repeated for every recording.
- **Entry and completion.** Open the welcome flow for a new workspace. For an existing workspace, discover configured connections and capabilities before asking the user to set them up again; offer a resumable setup checklist from Settings. Completion means the selected starting goal is usable, with skipped capabilities listed as optional next steps. Invoking an unconfigured feature opens its specific setup step and returns to the original action afterward; recording still requires an explicit start action after permissions/setup are finished.
- **Primary connection semantics.** Persist the selected primary model/agent separately from memory-client grants, transcription settings and source credentials. Let users replace it later. Background processing uses the existing budget/approval rules; the onboarding connection check does not authorize unrestricted future requests. Missing provider/model information remains labeled unknown.
- **Persist real state.** Keep onboarding progress in the workspace's service-backed configuration, shared across desktop and browser. Track readiness per capability/connection and per local host/device for OS permissions. Retain check timestamps and relevant configuration/provider versions; revalidate stale checks rather than treating a remembered checkmark as proof. Keep secrets in existing credential references. Prevent unrelated refreshes from replacing unsaved form inputs.
- **Plugin contributions.** F44 manifests can declare configuration schema, prerequisites, configuration versions, readiness/health checks and setup contributions. Render these through the shared onboarding UI; platform providers own their OS permission requests. Installing or upgrading a plugin can add a targeted setup step for its new requirements, preserving the rest of the workspace's progress. Show the missing prerequisite alongside the affected feature and offer a direct setup action. Implement the first onboarding flow against current configuration services without waiting for the entire plugin runtime, then adapt it to the same contracts. A failed optional plugin does not reset completed setup.
- **Pixel-dot direction.** The user's reference uses a subtle dot grid, warm charcoal/cream, a rose accent, monospaced status labels and particle forms that scatter/reassemble. Source reference reviewed October 19: [site](https://kdemoya.github.io/), [styles](https://kdemoya.github.io/assets/css/site.css), [particle behavior](https://kdemoya.github.io/assets/js/particles.js). Translate this into Thesaurus-specific dot forms: loose points gather into a connection as setup becomes ready; linked dots show the selected configuration steps; a short settling transition acknowledges a completed step. Keep one clear task and primary action on screen, with compact programming-font controls and ample room for the form. Use semantic theme tokens so the result fits existing palettes.
- **Useful motion and resilient forms.** Animation reflects real state, with textual status and step counts. It never supplies fake progress or delays navigation. Provide reduced-motion/static rendering, keyboard access, readable contrast, focus/validation feedback, and accessible status announcements. Keep decorative dots separate from form text, hide them from assistive technology, and preserve usable HTML forms if canvas/animation fails. Stop animation when idle or hidden and bound particle work so onboarding does not compete with recording.

**Visual sequence:** welcome with a small dot-built Thesaurus mark; connection setup with two dot clusters joining after a successful check; capability setup with a compact path of ready/pending steps; completion with a settled constellation and the selected goal's next action. Failed checks keep the form and its values visible with an inline repair action. Use static equivalents for every state. The illustration supports the task without occupying the form or requiring a full-screen animation.

**Screen composition:** retain the terminal-inspired interface: bundled programming font, short step labels, restrained borders and a persistent `back` / `continue` / `set up later` row where applicable. Place a small dot illustration alongside one focused setup form rather than above a dashboard of options. Use plain readiness labels such as `not connected`, `checking`, `ready` and `needs attention`; the connecting dots settle only after the corresponding check succeeds. Keep the current theme's palette, with the site's charcoal, cream and rose serving as a visual reference rather than a forced onboarding-only theme.

**Plugin setup boundary:** first-party native capabilities and user plugins describe prerequisites through the same versioned host contracts. A setup contribution identifies the affected feature, required configuration, supported host, readiness check and repair action; it does not supply arbitrary executable UI. The host renders configuration fields and invokes explicitly selected checks or native permission requests. Keep capability readiness separate from the user's choice to skip an optional step, so dismissing onboarding cannot make an unconfigured feature appear ready.

**Current implementation:** a workspace-backed five-step flow, live primary connection setup/selection, guided audio/transcription settings, explicit source/example import, scoped MCP agent preparation and plugin readiness links. Large dotted step numbers and short lettered headings establish the visual direction through a reusable SVG type component. See [implementation and remaining scope](onboarding.md).

**Future acceptance:** complete and resume setup in desktop and browser; use local capture without a reasoning connection; connect an existing installed agent or API/local route; distinguish a failed authentication check from saved configuration; handle a missing executable, model, microphone and expired token with targeted repair; retain drafts/progress across refresh/restart; and complete the entire flow using keyboard and reduced-motion/static rendering. These are acceptance criteria for future implementation, not completed checks.

## Plugins and native capabilities

Implemented with recorded acceptance for the local macOS/browser target: versioned service/desktop hosts, first-party native providers, lifecycle subscriptions, six contribution kinds, reviewed model routing, scoped native requests, shared management, setup/readiness and a Rust SDK. See [authoring and contracts](plugins.md) and the [requirement audit](plugin-acceptance-audit.md). F45 now supplies an initial guided onboarding flow; its runtime acceptance and follow-up scope are tracked in [onboarding](onboarding.md).

| ID | Idea and observable outcome | Priority and size | Fit | Inspiration |
| --- | --- | --- | --- | --- |
| F44 | **Plugin architecture and lifecycle hooks.** Let users install, configure, enable and disable extensions that subscribe to application lifecycle events and contribute connectors, processors, commands, actions, evaluation checks and interface contributions. Make all native product functionality first-party plugins behind the same capability model: window management, tray/popover, microphone/system audio, screen capture/OCR, notifications, shortcuts and OS credential storage. Keep the Rust work/memory service usable without the desktop host. Document manifests, version compatibility, grants, hook semantics and a Rust authoring SDK. | P1 · XL | New | User requirement; builds on F15, F29, F37, F39, F42 and F43 |

### F44 scope and architecture

- **Small hosts, explicit providers.** The Rust service hosts work/data plugins and headless-capable platform providers; the desktop shell hosts UI-bound native providers. Share plugin manifests, capability identifiers and versioned contracts across hosts. The desktop bootstrap retains only the event loop, IPC/broker and plugin loading. Product-level window/tray, capture and OS integrations belong to plugins, including the first-party implementations.
- **Providers and subscribers have different roles.** A capability provider implements a command such as opening a tray panel or starting an audio input and returns readiness/failure. Lifecycle subscribers react to committed outcomes asynchronously. Starting recording can depend on the selected audio provider becoming ready; it must not wait for optional analysis/notification hooks.
- **Execution boundary.** Prefer a versioned subprocess protocol for user-authored service extensions, with a Rust SDK and a language-neutral wire format. Initially package native providers as statically linked Rust modules registered through host traits in the appropriate service/desktop host; AppKit/UI operations run on the desktop main thread. Avoid a public Rust dynamic-library ABI. External plugins invoke native capabilities through the broker rather than receiving raw window pointers. A sandboxed Wasm option can be evaluated later. Subprocess separation alone does not sandbox an executable's filesystem/network access.
- **Manifest and authoring.** Declare plugin ID/version, supported host API versions, service/desktop host target, contributed/provided capabilities, subscribed hooks, dependencies, configuration schema, onboarding/readiness contributions (F45) and requested grants. Package configuration and diagnostics with enable/disable/update/uninstall controls. First-party plugins use these contracts too; users can replace a provider through an explicit selection.
- **Access and model use.** Scope subscriptions and host commands to the approved workspace/project/sources. Events carry references and permitted metadata; fetching content checks current policy. Plugins request existing model/agent connections through the host's permissions and budget controls, retaining supported subscriptions/API token references without receiving blanket credentials. Mutations and outbound actions use the ordinary command/review/receipt paths.
- **Delivery and recovery.** Persist event delivery work with the originating transaction. Give events stable IDs, schema versions, record revisions and correlation/causation IDs. Provide per-plugin cursors, at-least-once delivery, bounded retries/timeouts and idempotent host commands. Preserve order per recording/record where required; do not promise global completion order or exactly-once external effects. Tag plugin-originated effects and bound re-entry to prevent hook loops.
- **Native resource lifecycle.** Capability discovery tells both clients what the connected host supports. Disabling/updating a provider with an active recording or capture requires stopping and flushing that resource first; lifecycle cleanup must not silently kill capture or swap inputs. Missing/failed optional providers leave the core service and unrelated capabilities available, with a clear unavailable state in both interfaces. Shutdown hooks are best effort; durable recovery cannot depend on their delivery.
- **Shared interface.** Start with declarative commands, settings and panels rendered by the shared UI, using its theme and keyboard conventions. Browser clients call capabilities of the connected local host; they do not acquire desktop permissions themselves. Audio/OCR/credential providers can run in the service or a platform worker when supported, so browser recording need not depend on an open desktop window. Window/tray providers require the desktop host. Native providers still own OS permission prompts and platform handles.

### Proposed lifecycle hook families

The catalog below is the complete target; the currently emitted subset and remaining bindings are recorded in [plugin implementation status](plugins.md). Recording events describe actual capture transitions; a start request is not evidence that an input became ready.

| Lifecycle | Example hooks | Useful extension |
| --- | --- | --- |
| Host/plugin | `service.ready`, `service.stopping`, `plugin.enabled`, `plugin.disabled` | Initialize a connector or release resources |
| Recording | `recording.started`, `recording.paused`, `recording.resumed`, `recording.stopped`, `recording.failed` | Update an indicator or schedule follow-up work |
| Transcript/source | `transcript.chunk.committed`, `document.revision.created`, `source.sync.completed`, `source.sync.failed`, `source.policy.changed` | Extract domain-specific concepts or refresh an index |
| Memory/work | `memory.observation.created`, `memory.observation.superseded`, `task.accepted`, `task.completed`, `priority.changed` | Maintain a project-specific workflow |
| Agent/context | `context.bundle.created`, `agent.run.started`, `agent.run.completed`, `agent.run.failed`, `agent.run.cancelled`, `handoff.acknowledged` | Add an attributed follow-up or retain run metadata |
| Evaluation | `eval.run.completed` | Report regressions using the user's approved destination |

### First implementation slice and acceptance

1. Define the host/capability registry, manifests and event envelope. Extract window/tray management into desktop providers and the audio bridge into a service-owned platform provider, retaining browser-controlled recording and current behavior. Keep recording state, transcript storage and processing ownership in the Rust service.
2. Add durable subscriptions and one sample service plugin that reacts to a committed transcript revision and proposes an attributed observation through the scoped API. Add install/configuration, enable/disable, delivery history and compatibility errors.
3. Migrate remaining native capture/OCR, notification, shortcut and credential-storage implementations to providers. Broaden connector/processor/command/evaluation extension points through concrete use cases before publishing a stable SDK.

Acceptance should demonstrate the same recording flow through both clients, a headless service with no desktop plugins (including browser recording through its platform audio provider), alternate/mock providers, restart/replay without duplicate mutations, revoked-scope delivery rejection, bounded slow/crashing hooks, and safe refusal to unload an active capture provider. The recorded synthetic acceptance for these cases is summarized in the [requirement audit](plugin-acceptance-audit.md); real-device speech/model quality, other desktop platforms and release distribution are not implied. A public marketplace remains separate from local plugin support.

## Recommended prototype sequence

The existing plan supplies the P0 foundation. Select extensions through these small product experiments before expanding production scope:

F44 is implemented with recorded acceptance. It supplies declarative setup/readiness contracts and a shared plugin-settings renderer. F45 now implements the guided setup flow, primary-model selection and large dot-matrix number/letter direction separately from the plugin architecture; runtime acceptance and further connector/plugin setup remain open. See [onboarding](onboarding.md).

| Order | Experiment | Dependency | Decision evidence |
| --- | --- | --- | --- |
| 1 | F31 ACP compatibility | One connected agent and memory MCP | Does the adapter expose the permissions, cancellation, and tools we need with less custom integration? |
| 2 | F17 Living project brief | Evidence search and source watermarks | Can a user catch up accurately without repeating retrieval or reading stale conclusions? |
| 3 | F19 Resume a work thread | Session attribution and project mappings | Can a different agent continue using a bounded bundle without losing the unresolved work? |
| 4 | F03 Recurring meeting preparation | Meeting links; optional calendar import | Does the brief recall the right open questions and changes without filler? |
| 5 | F23 Priority changes and F24 Waiting-for | Accepted task history and new events | Do the recommendations explain a useful action the user would otherwise miss? |
| 6 | F25 Meeting-to-delivery trace | Stable task/source/repository links | Can the user verify progress without false completion? |
| 7 | F32 Saved work recipes | A useful repeated analysis and model budgets | Does reuse save setup effort while producing consistent, cited outputs? |
| 8 | F43 Meeting companion | Live audio capture, incremental transcription and scoped meeting context | Which surface makes recording controls and useful context quickest to access with the least interruption: an icon panel or an edge-docked widget? |
| 9 | F44 Plugin hosts and native providers | Versioned events, scoped commands and existing native capture/window behavior | Can a user extend transcript processing and replace a native provider while the core remains usable and recording stays immediate? |

## Three product scenarios

### Prepare for a recurring project meeting

F17 and F03 assemble the current project brief, open commitments, and relevant changes since last time. F01 lets the user inspect disputed details. The output remains useful when the available evidence supports only a short brief.

### Switch agents during implementation

F19 captures the active work thread. F18 builds a bounded bundle of goals, accepted decisions, repository references, and open questions. F21 records which context the receiving agent received. F15 supplies additional authorized evidence on demand.

### Discover an improvement worth pursuing

F26 connects repeated friction from independent meetings, Slack discussions, and repository evidence. F14 exposes missing information; F23 explains why it matters now. The user can save a research recipe with F32 or dismiss the opportunity durably.

## Recently implemented

F48 **Bragbook** has a first implementation: a built-in local plugin with scoped collection, persistent cited candidates, personal review/edit/dismiss decisions and selected accepted highlights in Markdown or JSON. See [Bragbook](bragbook.md) for supported collection signals and bounds. Broader accomplishment inference and model-written narratives remain follow-up work.

## Future ideas — not scheduled

| ID | Idea and observable outcome | Status |
| --- | --- | --- |
| F47 | **Calendar integration.** Connect selected calendars to show upcoming meetings, prepare relevant context, and link recordings to events and recurring series automatically after setup. Keep calendar selection and behavior in Settings. Builds on F03 meeting preparation and F28 capacity review. | Future idea · priority and scope TBD |

## Features to postpone

Keep continuous desktop recording, automatic skill installation, autonomous publishing, a graph database migration, and a public plugin marketplace outside the initial release. Expand these only after a specific use case or measured limitation justifies the additional surface.

[anarlog]: https://github.com/fastrepl/anarlog
[screenpipe]: https://github.com/screenpipe/screenpipe
[granola]: https://docs.granola.ai/help-center/sharing/integrations/mcp
[briefs]: https://docs.granola.ai/help-center/taking-notes/pre-meeting-briefs
[recipes]: https://docs.granola.ai/help-center/getting-more-from-your-notes/recipes
[onyx]: https://github.com/onyx-dot-app/onyx
[glean]: https://docs.glean.com/connectors/connectors-power-glean
[khoj]: https://github.com/khoj-ai/khoj
[threadline]: https://github.com/ADIITJ/threadline
[mem0]: https://github.com/mem0ai/mem0
[letta]: https://github.com/letta-ai/letta-code
[graphiti]: https://github.com/getzep/graphiti
[hindsight]: https://github.com/vectorize-io/hindsight
[goose]: https://github.com/aaif-goose/goose
[goose-acp]: https://goose-docs.ai/docs/guides/acp-providers/
[serena]: https://github.com/oraios/serena
[hermes]: https://github.com/NousResearch/hermes-agent
[openclaw]: https://github.com/openclaw/openclaw
[acp]: https://agentclientprotocol.com/get-started/introduction
