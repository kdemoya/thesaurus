# F44 plugin architecture

The goal is the complete [F44 architecture](feature-backlog.md#plugins-and-native-capabilities), including native providers and user extension points. Service providers, desktop window/tray/shortcut and notification modules, OS credential storage, lifecycle subscriptions and the shared management screen are implemented. API 2 executes all six declared contribution kinds: commands, processors, declarative panels, connector imports, reviewed local task actions and saved-evaluation checks. Plugins can also propose model work for explicit host approval through a selected granted API or ACP connection. The requirement audit is recorded in [F44 acceptance](plugin-acceptance-audit.md). The implementation and recorded F44 acceptance gates are complete for the local macOS/browser target.

## Current components

The built-in [Bragbook plugin](bragbook.md) uses the same settings and source grants. Its local event consumer collects accomplishment candidates into a dedicated personal review record. It starts disabled; open **Bragbook → Collection** to choose its sources and enable collection.

| Component | Location | Current behavior |
| --- | --- | --- |
| Shared protocol | `crates/contracts/src/plugins.rs` | API/event versions, manifests, configuration schema, grants, contributions, events, deliveries and provider selections |
| Rust authoring SDK | `crates/plugin-sdk` | Bounded single-invocation JSON-line protocol and an executable transcript-review example |
| Provider host | `crates/core/src/plugins/registry.rs` | Compiled capability providers, readiness, explicit selection and leases for streaming/native resources |
| First-party platform providers | `crates/core/src/plugins/native` | Microphone/system-audio helper construction, input discovery, selected-window permission/enumeration/capture, Vision OCR, first-party `native.transcription` local execution and credential references backed by environment/macOS Keychain |
| Browser launch provider | `crates/core/src/plugins/native/browser.rs` | Selected service provider for CLI browser pairing links, bounded OS launch, redacted durable receipts and no uncertain replay |
| Desktop host/providers | `apps/desktop/src-tauri/src/plugins`, `crates/core/src/plugins/desktop.rs` | Window/companion, tray/NSPanel popover, menu shortcuts and macOS notifications, main-thread dispatch, leased registration, applied-version health and durable command receipts |
| Durable subscriptions | `crates/core/src/plugins/schema.sql`, `worker.rs` | Transactional outbox, per-plugin cursors, queued delivery, retries, cancellation and scoped, attributed observation effects |
| Contribution execution | `crates/core/src/plugins/contributions.rs`, `contributions/effects.rs`, `contributions/evaluation.rs`, `apps/ui/src/plugin_runs.rs`, `plugin_action_review.rs`, `plugin_evaluation_results.rs` | Durable execution of six contribution kinds, pinned inputs, scoped cancellation, atomic host effects/results, escaped panels, review/receipt/undo and evaluation check receipts |
| Management API/CLI | `apps/cli/src/plugins.rs`, `plugins_cli.rs` | Install disabled, configure, enable/disable, upgrade a disabled package, uninstall, inspect/retry deliveries and select a registered provider |
| Shared management screen | `apps/ui/src/plugins.rs` | Plugins navigation (`p`), capability readiness/selection, typed configuration, scoped grants, package install/upgrade/uninstall and delivery inspection/retry |
| Setup/readiness | `crates/core/src/plugins/setup.rs`, `apps/ui/src/plugin_setup.rs` | Declarative prerequisites and repairs, explicit durable checks, stale-check invalidation and contribution setup gates |

Recording state and transcript ownership remain in the Rust service. The audio provider opens the helper and owns an active-resource lease; the service's recorder consumes readiness/chunk events and controls stop/flush. Browser and desktop use the same service endpoints. Neither optional plugin execution nor a desktop window is required to start recording.

The local transcription execution provider participates in the same registry and resource/queue idle guards. Host audio grants, model downloads, session snapshots and transcript publication are not delegated to user extensions. Cloud adapters resolve host credential references after their audio authorization/budget checks; installation alone grants no audio/credential access.

The provider registry supports alternate compiled providers. The service seeds the audio, screen-capture and OCR implementations as first-party plugins and persists explicit capability selections. A disabled provider is unavailable, with no silent input/provider fallback. A streaming resource lease blocks disable/replacement; the persisted session/queue checks additionally reject changes during recording or activity capture.

The desktop bootstrap retains the event loop, service connection and plugin host. Its main window is created by the window provider. The tray provider owns the compact webview and macOS nonactivating NSPanel, including restoring/releasing its content on disable. The shortcut provider owns application-menu accelerators; its dependencies require the tray and window providers. Provider changes cannot remove desktop recording controls during an active capture.

The service gives one connected desktop host a ten-second renewable lease. Registration alone does not imply readiness: the host reports the actual installed configuration version and errors. Browser and native commands enter the same bounded broker queue, and native effects execute on the desktop main thread. Duplicate command keys return the original receipt. A lost reply, host reconnection, expired deadline or service restart never automatically replays a UI command with an uncertain outcome. Selecting a different registered desktop provider allows it to initialize before it becomes ready.

### Desktop startup and connection recovery

The window provider owns a small recovery window that works before the service or shared Wasm interface loads. It distinguishes an unavailable service, an incompatible protocol/plugin interface, another desktop host, a disabled window provider and a failed provider. Retry reconnects without replacing a process. Start local service is offered only when the workspace lock has no owner; enabling the window provider preserves its saved grants/configuration and uses an expected-version check. Recovery commands are restricted to that window.

Service probes, descriptor sizes and response bodies are bounded. A stale descriptor does not imply a running process; ownership uses the operating system's workspace lock, without truncating it. The desktop never kills an existing service. A newly launched service has its own process group and survives quitting the desktop. Reconnection closes the recovery window and clears only the matching connection notice, preserving unrelated messages.

Actual isolated native checks covered incompatible/occupied startup, explicit start, a disabled provider, stale-descriptor bootstrap, service loss with an existing workspace window and successful recovery. The final reconnect cleared its notice. The shared plugin/settings/readiness screen, a compiled SDK panel run, exact revision citations and delivery-attempt grants/fingerprints rendered in the native app. A separate synthetic app remained full screen while the tray reported visible on the active Space and its controls rendered; the fixture was restored afterward.

Use **Plugins** to configure providers and local extensions. Settings drafts survive background refreshes; a concurrent change shows an explicit reload action and saves retain their expected-version check. Package installation requires reading the manifest and reviewing its executable/access. Arguments are a JSON string array, passed directly without a shell. Credential fields hold references. Installation and package upgrades leave user plugins disabled. Recorded acceptance and verification limits are listed below.

## Author a local service plugin

Build the example and the CLI:

```sh
./scripts/cargo.sh build -p thesaurus-plugin-sdk --example transcript-review
./scripts/cargo.sh build -p thesaurus
```

With a running service and a deliberately selected source, install the example using its manifest and absolute executable path:

```sh
target/debug/thesaurus plugin install \
  --manifest crates/plugin-sdk/examples/transcript-review.manifest.json \
  --program "$PWD/target/debug/examples/transcript-review" \
  --source SOURCE_ID \
  --operation sources.read \
  --operation memory.propose \
  --operation commands.invoke
target/debug/thesaurus plugin enable example.transcript-review
target/debug/thesaurus plugin deliveries example.transcript-review
```

Use `--data-dir` before the subcommand when targeting a nondefault workspace. `--project` grants the project's current sources; explicit source grants and project grants can be combined. `--external` applies external-destination source policy to data delivered by the host. Install never enables or invokes an executable automatically. Enable starts subscriptions at the current event cursor, without implicitly replaying events produced while disabled.

The example reacts to new revisions of the selected source, checks for `TODO`/`follow up`, and contributes an attributed observation with an exact historical quote. It does not invoke an agent/model, accept a task or send a message. Inspect the result in Memory. The SDK example and manifest are starting points for a user-authored Rust executable, not a plugin marketplace installation.

Other commands:

```sh
target/debug/thesaurus plugin list
target/debug/thesaurus plugin get example.transcript-review
target/debug/thesaurus plugin capabilities
target/debug/thesaurus plugin configure example.transcript-review --file settings.json
target/debug/thesaurus plugin disable example.transcript-review
target/debug/thesaurus plugin upgrade example.transcript-review --manifest manifest.json --program /absolute/plugin
target/debug/thesaurus plugin uninstall example.transcript-review
target/debug/thesaurus plugin attempts example.transcript-review DELIVERY_ID
target/debug/thesaurus plugin run example.transcript-review review-followups --source SOURCE_ID
target/debug/thesaurus plugin run example.transcript-review summarize-followups --source SOURCE_ID
target/debug/thesaurus plugin run example.transcript-review followups-panel --source SOURCE_ID
target/debug/thesaurus plugin runs example.transcript-review
target/debug/thesaurus plugin run-status example.transcript-review RUN_ID
target/debug/thesaurus plugin cancel-run example.transcript-review RUN_ID
```

Upgrades preserve identity and require a disabled, idle user plugin. Configuration is validated against the manifest; defaults are materialized, unknown fields rejected, and credential settings retain references rather than resolved secrets. Built-in providers cannot be uninstalled through the user-plugin API. Native capability replacement is currently between providers registered by the Rust host; user subprocesses do not receive native window pointers or a Rust dynamic-library ABI.

Installed dependency identities must exist and the proposed dependency graph must be acyclic. Disabled packages can be installed together before activation; enabling requires enabled dependencies. An enabled dependent blocks disabling its dependency, and any installed dependent blocks uninstalling it.

## Invocation and delivery contract

- The host launches the explicitly configured program with argument arrays, no shell, a private working directory and a minimal environment. It sends one JSON invocation on stdin and expects one JSON response on stdout. Diagnostics belong on stderr.
- Input is bounded to 512 KB, output to 128 KB and diagnostics to 8 KB. Each invocation has a ten-second timeout. The host kills the invocation's process group on cancellation/timeout, including descendants on the supported Unix host. The API version and invocation ID must match the request.
- The host negotiates API 1 or 2 from the manifest range; the response must echo the negotiated version. Both supply policy-filtered, pinned document content to `sources.read` subscribers of `document.revision.created`. Other lifecycle events carry references/metadata. API 2 additionally supports explicitly requested contributions as described below. Legacy API 1 subscribers remain supported.
- Domain writes and outbox inserts share a SQLite transaction. Rollback means no event. Each plugin has a durable cursor and delivery history. Pending/running earlier deliveries block later ones for that plugin; unrelated plugins use independent worker slots. Delivery is at least once, with up to five bounded retry attempts and a terminal failure state. Explicit retry can revisit an older event after later work has completed.
- Data scope and current source policy are checked before invocation. The host polls plugin configuration/grant versions during execution and cancels when they change. Before each effect commits, it checks the current plugin version, permission, source access and exact evidence in the same database transaction as the attributed observation.
- Effects use stable keys derived from plugin/event/effect index. Replaying a committed effect does not duplicate it; changing its content causes a conflict. Origin, causation and depth are host-owned. Own effects do not reenter the same plugin, and a maximum depth bounds cross-plugin chains.
- Local executables are trusted code running with the OS user's privileges. Host grants constrain what the host supplies and accepts; subprocess separation is not an OS filesystem/network sandbox. No daemon admin token, inherited API credentials or automatic paid-model fallback is supplied to the plugin.

Each delivery attempt records its status, package/configuration/API version, configuration and executable-definition hashes, approved grant, timestamps and a redacted failure reason. These hashes fingerprint configured definitions; they do not attest the executable's on-disk bytes. Old migrated attempts may have unknown fingerprints/grants. Restart marks an in-flight attempt interrupted and retains the history separately from its delivery's retry count.

## Explicit contributions (API 2)

Use **Plugins → Contributions and run history** or `plugin run`. Select an enabled plugin and explicit sources/projects or a saved evaluation. `commands.invoke` permits command/processor/panel invocation, `connectors.sync` permits connector imports, `actions.propose` permits reviewed local action proposals, and `eval.check` permits saved-evaluation checks. `sources.read` separately permits document/task/evaluation input; `memory.propose` separately permits attributed observations from commands/processors. Panels and evaluation checks cannot mutate memory.

The host persists a queued run with the installation identity, package/configuration/API version, approved grant, configuration/executable-definition hashes, selected scope, parameters and exact document revisions. It accepts at most twenty documents, 400 KB of document text and 32 KB of JSON parameters; the existing total wire limits also apply. The UI supplies optional instructions as `parameters.instructions`; CLI `--input` accepts a JSON object for a plugin's own parameters.

The subprocess receives method `contribution`, `event: null` and `input: { contribution, parameters, documents, tasks, destination_project_id, evaluation }`. It returns `result: null` and `contribution: { text, evidence, panel? }`, plus the effects allowed for that contribution type. Panel content is `{ title, sections: [{ heading, text, evidence }] }`; the shared renderer treats every field as text and supports ordinary source navigation. It accepts no scripts, HTML, arbitrary URLs or plugin DOM access. Every citation must match a supplied document's exact revision, line and quote.

One plugin runs at most one lifecycle delivery or contribution at a time. Workers alternate between the two queues. Configuration, installation, scope, source policy and cancellation are checked before execution, during execution and before commit. All accepted host effects and the completed result commit in one transaction. A failed result leaves no partial imports, memory changes or action previews. Cancelling a running request holds its execution slot until the subprocess is stopped; pending work may then proceed. Restarted running contributions become interrupted and never execute automatically again. A new explicit request is required; the same idempotency key returns its original run. These guarantees cover host effects, not arbitrary side effects performed by the trusted local executable itself.

### Connector snapshots and reviewed actions

Connector responses may include up to ten `snapshots: [{ key, source_name, kind, title, body }]`. The host owns their canonical URI, `plugin://PLUGIN_ID/INSTALLATION_HASH/KEY`; keys are bounded identifiers, not paths or caller-supplied URIs. An installation can update its own snapshots, cannot take over a pre-existing unrelated source, and cannot resume ownership from a previous installation. Repeated identical content retains the revision and reports `changed: false`. New sources default to local-only; later imports preserve the user's disclosure setting and source policies.

Choose a destination in the UI or pass `--destination-project PROJECT_ID`. It is separate from `--project`, which selects input documents. The destination must currently be granted. Its documents are not implicitly supplied to the connector. Source import, project attachment, revision/history/search updates, local processing jobs, ownership mapping, originating lifecycle events and run receipts commit together. Imported document references are retained in run history. Derived discovery refresh runs after commit and cannot cause an automatic connector replay. Successful/failed sync events retain plugin identity and causation; optional subscribers cannot delay recording.

Action responses may include up to ten `actions: [{ provider, target_id, expected_version, status, rationale, evidence }]`. The first concrete provider is `task_status`; arbitrary outbound effects and plugin-defined external action executors are not exposed by this host. The input includes at most thirty pinned current task references from the selected document revisions. Proposals must target one of those task versions and cite its exact evidence. The host creates ordinary action previews without changing task status. Their rationale, plugin/install/configuration/run origin and exact effect fingerprint are recorded. Before execution, the host rechecks the current grant, source policy, plugin version and unchanged task. Explicit user approval is required; execution creates the ordinary receipt and attributed task event atomically. Undo remains available under its normal current-state checks even if the proposing plugin has since been disabled.

Source destination controls list installed scoped plugins as `plugin:PLUGIN_ID`. Blocking one uses the existing destination policy and blocks further host-supplied input and pending review execution without needing to change the plugin's configuration version. Active contribution work is cancelled on revocation. This does not retract data already supplied to trusted local code.

Build `cargo build -p thesaurus-plugin-sdk --example file-workflow` through `scripts/cargo.sh` for a working example. Its manifest is `crates/plugin-sdk/examples/file-workflow.manifest.json`. Configure `path` to an absolute UTF-8 notes file (up to 64 KB), optionally configure `title`, and grant `connectors.sync`, `sources.read`, `actions.propose` and `setup.check`. Enable it, run its `notes-file` setup check and inspect completion before running `sync-file`. Then select its imported source and run `review-tasks`. The example reads the configured file, imports a snapshot and proposes accepting locally extracted candidate tasks; it uses no model or outbound service. Review/execute/undo from the run history or the ordinary action CLI.

### Saved-evaluation check contributions

Evaluation contributions require `eval.check` and `sources.read`. Choose a saved project or personal-suite run and cases in the shared UI, or use:

```sh
target/debug/thesaurus plugin run example.file-workflow check-evidence \
  --eval-run SAVED_EVAL_RUN_ID --eval-case CASE_ID
```

Omit `--eval-case` to check all saved cases. Request JSON uses `evaluation: { run_id, case_ids }`; an empty case list selects all. Source/project input selectors are separate from evaluation mode and cannot be combined with it. The plugin must currently have access to the saved run's entire original source scope. Discovery lists only eligible run metadata, not unrelated source bodies or outputs. Known citations to sources outside the recorded scope are refused before any data reaches the executable.

The host pins a digest of the original saved eval record and its original document revisions. `input.evaluation` contains suite/project identity and the selected case definitions/results, while `input.documents` supplies the pinned source text. Live source changes do not replace saved inputs. Current source/destination policy and plugin grants are checked before invocation, during execution and in the final writer transaction. Changed saved records, revoked access, inputs above the wire budget or more than twenty pinned documents are refused rather than silently truncated. Select fewer cases or a smaller saved source scope when needed.

Responses include `checks: [{ case_id, check_id, title, score, detail, evidence }]`. Return at least one check for every selected case, at most fifty total, with unique IDs per case, finite scores from 0–1 and exact pinned citations. The host calculates pass/fail from the original case threshold. Unknown/missing cases, duplicate IDs, forged pass fields, bad scores or invalid citations fail the entire run and create no partial check receipt. Evaluation responses cannot include connector, action, memory or panel effects.

Each successful invocation atomically stores an attributed check receipt and its completed contribution run, then emits `eval.run.completed` with record kind `plugin_eval_check`, plugin origin and causation. The original eval record, outputs, deterministic scores and baseline remain unchanged. `EvalRun.plugin_checks` loads separate immutable receipts; they are shown both in plugin run history and the existing saved evaluation results UI. They are reported plugin checks with structural/evidence validation, not host proof of their scoring algorithm. They do not spend model allowance or modify judge budgets.

The compiled `file-workflow` example contributes `check-evidence`. Its configurable `minimum-evidence` (default one, range 1–20) requires distinct exact citations, including when the base case permits an empty citation list. Grant `eval.check` and `sources.read` to use it. Project and personal suites use the same saved-run path; no model is invoked by this example.

## Credential and notification providers

`native.credentials` runs in the service. Model API authentication and upstream MCP environment-token reads use the selected provider. Existing `token_env` settings remain supported; an API model connection may instead set `credential_ref = "keychain:NAME"` (mutually exclusive with `token_env`). References are bounded `env:NAME` or `keychain:NAME` values; Keychain items use the fixed `dev.thesaurus.credentials` service namespace. Environment references are read-only. The Rust provider implements Keychain read/write/delete; secret write/delete onboarding UI is future F45 work. Resolved secrets are passed only to the requesting host connection, never included in plugin configuration/delivery records or exposed through the desktop command API.

`native.notifications` runs in the bundled macOS desktop host. **Plugins → OS notification setup** can inspect actual OS authorization, explicitly request permission, and send an explicitly requested synthetic test when authorized. Merely enabling the provider never prompts or sends a notification. Permission requests expose pending/granted/denied/failed state and reject duplicate pending requests; the shared UI polls until completion. Local app builds use an ad-hoc bundle signature unless an Apple signing identity is supplied through the environment, so macOS can recognize the bundle identity and its Info.plist.

With the user's explicit approval, the signed isolated `dev.thesaurus.plugin-startup-verification` app obtained notification authorization and submitted exactly one synthetic notification through the native UI and broker. Its completed receipt reports OS acceptance. Banner presentation was not verified: Focus and global notification settings still apply and were not changed. The opt-in credential-provider test also passed with one uniquely named fake Keychain item, which it read and deleted; it did not access existing credentials.

The named F44 hook families are now bound: service ready/stopping; plugin enable/disable; document revisions; stored MCP fetch completion/failure/interruption; source disclosure, destination and retrieval policy changes; observation creation/effective revision supersession; task and priority transitions; context creation and handoff acknowledgement; agent run states; and completed evaluation runs. A recording start request is not emitted as `recording.started`: the actual ready transition produces that event. A zero-duration pause/resume is distinguished using the prior committed start event.

Memory corrections, suppression and restoration supersede the previous effective view. Retained thread contexts have a stored context reference; returned context previews emit a request reference and scope without retaining the bundle body in the event. These non-document hooks currently supply references/metadata, not a new unscoped content-fetch channel. Event schema versions are persisted with each event. Plugin observations retain both the immediate parent event and the original correlation ID.

Plugin mutations, broker claims/heartbeats and effect commits acquire the SQLite writer reservation before their reads. This prevents deferred transaction upgrades from racing the frequent desktop heartbeat. No provider command or user executable runs inside those plugin transactions. Service shutdown rejects new work, queues its stopping hook with a bounded best-effort deadline, stops capture/runtime ownership and bounds both HTTP server drains; an unread asset response cannot retain daemon ownership indefinitely.

## Declarative setup and readiness

An optional manifest `setup` array declares up to twenty steps. Each has an ID/title/description, `applies_to` contribution or capability IDs, bounded prerequisites, an optional explicit `check`, an `optional` flag and a host-rendered `repair`. Prerequisites reference declared configuration fields, requested operations, declared dependencies or capability availability. Repair targets are configuration fields, grants, provider selection or the host's Audio/Capture/Notifications/Models settings. There is no plugin-supplied executable UI. Legacy manifests without `setup` remain valid; executable checks require API 2 and `setup.check` in the manifest and current grant.

Both clients render **Plugin settings → Setup and readiness**. `GET /api/v1/plugins/{id}/setup` computes current structural prerequisites and inspects receipts without invoking code. States distinguish `ready`, `needs_setup`, `checking`, `unavailable` and `unverified`. A structural-only step explicitly says that invocation/authentication/input permissions have not been checked. Missing executable-check permission directs repair to Grants. Optional steps do not block other contributions; required steps gate their named contributions (an empty `applies_to` applies to all contributions).

The explicit `POST /api/v1/plugins/{id}/setup/{step}/check` accepts `expected_version` and `idempotency_key`. Checks enter the same durable, per-plugin serialized queue as contributions and lifecycle deliveries. A user executable receives method `readiness`, its saved configuration and `input.step`, without source data, model results, an event or resolved credentials. Return only `readiness: { step_id, state, detail }`; `checking` is host-owned. The result is bounded to 2 KB and rejects all host effects, arbitrary result payloads and model proposals. Existing timeout/process-group/cancellation controls apply. A trusted executable still has its OS user's filesystem/network privileges; a readiness check is not an OS sandbox.

Check receipts include the configuration/package/install/executable provenance from ordinary run history. They expire after ten minutes and are invalidated by service restart, configuration/grant changes, dependency changes, native availability/selection changes or desktop reconnection. Failed/interrupted checks do not replay automatically. Saving configuration and skipping an optional step are not readiness evidence. The broader resumable welcome wizard and primary-model selection remain F45.

First-party providers use the same setup descriptors. Native microphone/system-audio and screen checks passively inspect device support and OS authorization, without recording, frame capture or permission prompts. Notification checks inspect the connected desktop host's authorization through the broker without sending a notification. Input selection, transcription setup, credential authentication and actual capture/invocation remain separate checks; provider availability alone does not establish them.

```sh
target/debug/thesaurus plugin setup example.file-workflow
target/debug/thesaurus plugin check-setup example.file-workflow notes-file
target/debug/thesaurus plugin run-status example.file-workflow RUN_ID
```

## Native requests from user plugins (API 2)

Commands, processors and lifecycle subscribers may return `native_commands`, up to five `{ key, capability, command, input }` requests. The manifest must request, and the user must grant, the exact operation: `native.windows.workspace`, `native.windows.companion`, `native.tray.show`, `native.tray.dismiss`, `native.notifications.show` or `native.notifications.dismiss`. Panels, connectors, task-action contributions, evaluation checks and readiness checks cannot issue native effects. Native requests cannot be mixed with memory/import/action/evaluation/model effects in one response.

These requests enter the ordinary desktop broker in a single transaction after current installation, configuration, source/destination policy and grants are checked. The broker checks that authority again before handing work to the selected desktop provider. Configuration changes, disable/uninstall/reinstall and independent source destination revocation prevent queued dispatch. Once a command has been handed to the desktop, an OS effect already in progress cannot be recalled. Deadlines and interrupted/uncertain outcomes never trigger automatic replay. Contribution completion means the plugin returned successfully and its request was accepted; the separate native receipt records provider completion or failure.

`workspace`, `tray.show`, `tray.dismiss` and notification `dismiss` use an empty input object. Companion input is `{ "mode": "panel" | "left" | "right" }`. Notification `show` takes only bounded `title` and `body`. The host derives its notification ID from the installation and request key; using the same key for dismissal can address only that installation's notification. Permission prompts, recording/capture, credentials and browser URLs are not accepted through this extension request surface. No native pointers, host token or provider credential is given to the executable.

Inspect **Plugin settings → Native requests**, `GET /api/v1/plugins/{id}/native-requests`, or `plugin native-requests ID`. Receipts retain the originating contribution run/delivery, package/configuration version, granted scope and provider result. Uninstall/reinstall cannot inherit a previous installation's request history. The host does not implicitly relaunch a plugin when its native command completes.

The Rust SDK example `native-notification` and its manifest demonstrate a manual notification/dismissal command and a transcript follow-up subscriber. Its setup step requires a connected notification provider; configure OS authorization explicitly in native notification settings. Merely enabling the example does not request permission.

```sh
./scripts/cargo.sh build -p thesaurus-plugin-sdk --example native-notification
target/debug/thesaurus plugin install \
  --manifest crates/plugin-sdk/examples/native-notification.manifest.json \
  --program "$PWD/target/debug/examples/native-notification" \
  --source SOURCE_ID --operation sources.read --operation commands.invoke \
  --operation native.notifications.show --operation native.notifications.dismiss
target/debug/thesaurus plugin enable example.native-notification
target/debug/thesaurus plugin run example.native-notification notify --source SOURCE_ID
target/debug/thesaurus plugin native-requests example.native-notification
```

The CLI's own `open` command also uses the selected `native.browser` service provider. It accepts bounded loopback HTTP pairing links, uses OS argument arrays without a shell, stores no pairing fragment in receipts and does not reopen an uncertain request after retry/restart. `open --print` still prints a fresh pairing link without invoking the provider. This administrative browser capability is separate from user-plugin native requests.

## Approved plugin model work

API 2 plugins return one bounded `model_requests` proposal without other effects. The parent contribution waits durably for review; lifecycle subscribers can delegate to the same path without holding up recording. Each request requires `models.request`, explicit connection grants and current source/destination authority. There are at most three separately approved rounds per run.

The host preview shows the selected connection, exact prompt with pinned source revisions, approximate input tokens and output limits. Both usage and returning the response to the plugin require approval. The approval signature binds the prompt, connection configuration and plugin version. Reservation, proposal linkage and existing daily request limits share one transaction; retrying an approval does not reserve twice. A rejected proposal makes no model call. Source/grant/configuration changes invalidate authority, and interruption never automatically reissues a model request.

The service resolves credentials through the selected native credential provider. The executable receives bounded model text and provenance, never provider credentials. API connections request the approved token limit; ACP has no generic output-token cap. Both routes enforce output byte bounds. Plugin ACP sessions receive no memory MCP or host filesystem/terminal capabilities, and tool permission requests are denied. Existing user-initiated ACP runs retain their scoped memory flow. No route silently falls back to a different model/account.

Use the shared contribution history or CLI `plugin model-get`, `model-preview`, `model-approve` and `model-reject` commands. The compiled `model-review` SDK example demonstrates both a manual command and a document-revision subscriber with an approved result callback. Nineteen synthetic browser/API/CLI checks cover a loopback API and a mock ACP agent, exact prompt delivery, host-only credentials, result callbacks, daily budget rejection and no fallback. These checks use no real model subscription or external provider and do not establish native rendering or provider quality.

## Acceptance status

The complete requirement matrix is in [F44 acceptance](plugin-acceptance-audit.md). All six contribution kinds, the API 2 native-request extension, model routing, setup contracts, native provider migration and shared management have implementation and recorded acceptance evidence. The final combined native capture/OCR/search check and synthetic-process cleanup pass.

Current automated evidence: 71 ordinary core tests (65 unit and six workflow), seven CLI tests (six unit and one daemon/two-MCP integration), seven desktop tests and six compiled SDK protocol checks pass. The opt-in OS Keychain check is excluded from the ordinary suite and passed separately. The domain writer audit reviewed 176 deferred transaction sites, moving 81 mutable transactions across 42 files to acquire the writer before mutable reads; 95 read/preflight/monitor snapshots remain deferred. Broad domain mutations run concurrently with desktop heartbeats in a regression check.

Live integration evidence includes fifteen actual service/MCP/ACP/handoff/evaluation lifecycle checks, eleven compiled native-SDK/broker/CLI checks and eight active local API/ACP cancellation, revocation and restart checks. Earlier contribution, connector/action, evaluation, model-routing and readiness walkthroughs remain recorded. The native request receipts render in both clients. Native model review shows the exact pinned prompt, selected connection, limits and disabled approval gate; rejection without usage passed. No real model subscription or account was used.

Sixteen headless generated-audio checks, native keyboard Start/Pause/Resume/Stop and browser button Start/Pause/Resume/Stop pass. Both clients show the synthetic transcript and post-stop details without requiring meeting metadata first. A header pointer-handling defect and relative workspace-path defect found in this pass were fixed; a CLI regression proves canonicalization while rejecting symlink workspace roots. These checks use generated silence and a synthetic transcriber, not real microphone/system audio.

The refreshed isolated macOS bundle is 152.43 MiB. Optimized web, CLI and SDK builds, formatting, strict signature validation and exact packaged service/native-audio/clean-web comparisons pass. Startup/reconnection, full-screen tray, notifications, fake Keychain and native panel/delivery rendering have separate recorded checks. The normal app/service were not replaced or restarted.

Selected-window enumeration, actual PNG retention, Pause and Stop pass for the exact synthetic native window using existing macOS permission. A Vision precompiled-compute failure was isolated and fixed with a fresh-helper CPU compatibility retry. The complete selected-window capture/OCR/search/Pause/Stop check passes through the actual service providers. No permission change or work-window capture occurred. Notification OS acceptance does not establish banner display, and synthetic tests do not establish speech/model quality. The isolated native app, both synthetic browser tabs and all three new synthetic daemons were closed; capture sessions and recordings are stopped. The normal daemon and preexisting fixture processes remain running.

The local Vision helper first uses the current request revision and retries on CPU. If that helper fails, the Rust provider starts a fresh helper using revision 2 on CPU only when the OS reports it supported. A failed compute operation can prevent recovery inside the first process. The first helper has a 120-second limit; both helpers share a 180-second overall deadline, including compatibility startup. The older compatibility revision has different language/recognition support; OCR text remains approximate and should be checked against the retained image. This path uses no remote OCR or model connection.
