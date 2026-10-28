# F45: Guided onboarding and dot typography

Implemented October 26, 2026. This is the initial guided setup path for the local desktop/browser app. Runtime interaction and real-provider acceptance remain pending; successful compilation is not proof of account entitlement, recording quality or native rendering.

## Entry and navigation

Use **Settings → Automation → Revisit onboarding**. A new empty workspace opens setup automatically; an explicitly active setup resumes on next launch. Existing workspaces are discovered before any configuration is requested and keep their ordinary workspace entry. **Set up later** remembers the selected goal/step and returns to Work. **Settings → Audio & transcription** contains saved recording preferences, and **Set up or repair a model** in Connections opens the connection step.

Five steps:

1. **HELLO / Start:** choose meeting recording, understanding work context, or sharing memory with an existing agent.
2. **LINK / Connect:** select the primary connection, retain local rules, or explicitly add an installed agent, compatible API or local model endpoint. Discovery only checks known executable locations; it does not run or install those programs. The selected model also supplies automatic meeting summaries; local mode supplies an extractive digest.
3. **TOOLS / Prepare:** configure the selected goal. Audio uses the existing recording settings, microphone/system selection and F46 transcription presets/downloads. Shared memory selects sources, local/external processing and explicitly creates read-only MCP access. Work context needs no model or microphone. Installed plugin readiness is shown with links to the existing plugin setup renderer.
4. **PROOF / Try:** explicitly import one small synthetic example or a personal note, inspect local extraction candidates and their exact quoted source lines, or open the recording companion. Opening the companion does not record. Memory setup can copy the agent configuration; preparing access does not claim an agent has connected.
5. **READY / Finish:** show current selected-capability readiness and optional next steps. Missing prerequisites display CHECK instead of READY. Completion validates the selected goal on the host. A configuration can be prepared while first-use OS authorization or cloud access remains unverified; those distinctions stay visible.

Step, goal, primary connection, completion/defer state and selected memory grant are persisted in the workspace service with version checks and idempotency. Desktop and browser share these values. A concurrent change leaves the current draft intact and offers **Reload saved progress**. Ordinary status refreshes do not replace form inputs. Dirty connection/audio/memory edits must be saved or explicitly canceled before changing steps. Unsaved form fields are intentionally not persisted across a process restart; API keys are never retained as onboarding drafts. Saved settings and completed steps are resumable.

## Model/account setup

New managed connections are private SQLite records containing credential references. Existing `config.toml` connections are preserved. Managed connections become available immediately to snapshots, disclosure previews, runtime/summary/recipe/evaluation routing and queued extraction; the service does not need a restart. Config-file edits still require a restart. IDs cannot collide, and managed connections are append-only: add another ID and select it to change account/provider settings without mutating an in-flight route.

Selecting the primary connection sorts it first in shared connection choices and supplies the default for automatic meeting summaries. Finishing setup explains that the saved model receives finished meeting transcripts and notes without another approval for each recording. Settings can disable automation or select a different summary model. Each new recording pins its destination; changing the primary does not redirect an active meeting or run. Recording/transcription inputs and external agent memory grants remain separate choices. Other explicitly initiated workflows retain their own scope and budget controls.

API keys entered during setup are written through the host credential provider to a private Keychain reference. Advanced environment/Keychain references are supported. Saving a connection makes no model call; status discovery does not read Keychain secrets or prompt for OS permissions. Runtime accounts manage their own login and billing. Presence of a binary/reference is labeled configured/unverified, not authenticated/callable.

An optional ACP initialize probe is explicit and sends no model prompt. A separate **Send synthetic check** needs its own usage approval and reserves one ordinary budgeted runtime request. Its prompt is fixed, source list empty, output bounded, and host tools/MCP are not exposed. Checks have configuration-bound receipts linked atomically with the runtime reservation; cancellation, recovery and uncertain-result behavior use the existing runtime. A completed check establishes that this configuration responded at that time, not future availability or model accuracy. It never grants future usage.

## Audio and memory boundaries

Audio readiness is evaluated on the connected service host. Executable/model availability, selected device, plugin availability and passive microphone/system permission state are checked separately. No microphone is opened by setup or readiness. First-use permission is requested only through explicit Start; an OS denial has an actionable repair message. Cloud routes remain configured until their credentials/provider are used explicitly. Existing recordings retain their session choices.

Memory access is separately approved, read-only, and scoped to chosen sources. External agents require source sharing to be enabled through ordinary source policy. Revoked/destination-denied sources are rechecked. The private 0600 grant file is written by the host; setup returns an MCP configuration referring to it, not a raw token. Copying this snippet is explicit. Saved access/configuration can be reopened later; revoked, missing, nonprivate or denied grants require repair. Agent configuration files are not edited automatically.

Plugin steps are declarative F44 setup statuses. Setup shows provider/configuration/host-bound readiness and navigates to the existing explicit checks/repairs. Optional plugin failures do not block unrelated goals. Connector-specific OAuth flows and a complete inline plugin-configuration wizard remain follow-up work; this delivery provides source imports and scoped agent setup.

## Dot typography

The user's updated direction is **large dotted numbers and letters**, with the terminal interface and monospaced controls retained. The reusable `DotText` primitive in `apps/ui/src/dot_type.rs` supplies a 5×7 SVG alphabet, digits and basic punctuation. Step numbers have a faint inactive grid; headings have lit dots only. The numbered rail, generous spacing and short words provide hierarchy. Forms, instructions, status and actions remain ordinary HTML text.

- Semantic theme tokens supply color in all existing palettes.
- Dot graphics are decorative (`aria-hidden`); HTML supplies the full heading and step labels.
- Settling is a finite CSS transition when the actual step/readiness changes. Memoized typography does not restart when form fields or polling update. No canvas, animation loop, font download or continuous particle simulation is required.
- Reduced motion disables the effect. Navigation remains keyboard accessible; step transitions focus the heading without stealing focus while editing.
- Styling currently applies to onboarding. The component can later support section headers, review counts or a small wordmark; it does not automatically restyle the rest of the app.

[Typography specimen](design/onboarding-dot-type.svg) is generated from the same glyph definitions as the application. It is a design specimen, not a captured running-app screenshot. The original inspiration is the user's [personal site](https://kdemoya.github.io/); the current implementation follows the supplied dot/particle direction and the repository's earlier recorded reference, as the site could not be fetched during this turn.

## Implementation and delivery

- Contracts: `crates/contracts/src/onboarding.rs`.
- State, managed connections, readiness and scoped grant preparation: `crates/core/src/onboarding.rs`.
- Tool-free synthetic-check reservation/authority: `runtime_store.rs`, `apps/cli/src/runtime.rs`, `plugin_models.rs`.
- Admin endpoints: `/api/v1/onboarding`, `/connections`, `/check`, `/example`, `/memory` and `/memory/{id}` under that prefix.
- Shared UI: `apps/ui/src/onboarding.rs`, `onboarding/{model,audio,memory}.rs`, `dot_type.rs`, and semantic CSS.

Rust service, desktop and Wasm compilation, the optimized web build, and formatting passed. The updated native bundle was staged and signed/verified using the existing persistent identity at `.local/f45-onboarding-20261026-222241/Thesaurus.app`; `.local/f45-latest.json` points to the bundle and its build/signature/hash report. The normal app and service were not replaced or restarted. No automated tests, live model request, credential write, source import, grant creation, recording or normal-service restart was performed during implementation. Browser/native interaction, keyboard/reduced-motion behavior and real host/account failure scenarios still need explicit runtime verification.
