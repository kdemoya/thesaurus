# Backlog verification record

Date: 2026-10-17. All exercised source content is synthetic. The workspace is `.local/backlog-review-20261017`, served separately on loopback port 4621. No model/account calls were made.

## Completed runtime walkthrough

The following completed against the running Rust service through authenticated HTTP requests:

- Text capture creates immutable source revision and local extraction candidates; an explicit task acceptance succeeds.
- A commitment linked to an accepted task accepts implementation evidence, creates a completion proposal, and requires a separate explicit confirmation. Confirming the commitment leaves the source task accepted.
- Anchored note creation/edit retains two history entries and materializes attributed note context.
- Memory inspection shows original revision 0; correction appends revision 1. A conflict pins two owner claims; explicit resolution keeps one and suppresses the other. Suppressed memory returns 404 through normal memory retrieval.
- Saved recipe preview expands declared inputs once and pins source revision 1; no runtime is started.
- A reusable personal evaluation suite runs against a project, passes exact retrieval/evidence criteria, compares a second run with its chosen baseline without regressions, and accepts a separate human review.
- Budgeted context stays within the complete JSON character and estimated-token budgets.
- JSON export includes immutable document history and original/effective memory plus correction/conflict histories.

Two separate real MCP stdio proxy processes then initialized successfully and listed **22 tools**. The first contributed an attributed observation; the second retrieved the same observation. Agent access to an admin mutation returned 403. Revoking the first grant made its next request return 401. Processes exited and their temporary private grant files were removed; synthetic grants used by the successful check were revoked.

## Subsequent synthetic walkthroughs

The later pass exercised actual compiled handlers and protocol transports, with fixed synthetic content:

- **Annotations, corrections, knowledge, discovery, delivery and procedures:** 39 checks passed, covering immutable original evidence, protected accepted work, stale/version/idempotency failures, valid-time/known-by queries, uncertain identity history, missing-source revisit/dismissal, live-only cache exclusion, explicit historical provenance, typed Git commit/file/report links, immediate repository disclosure revocation and two-reviewed-success procedure installation. Artifact: `.local/backlog-review-20261017/owned-walkthrough-results.json`.
- **Meetings, briefs, threads and focus:** 24 checks passed, including two captured meeting baselines, rejection/acceptance/staleness of cited brief revisions, split/merge attribution, corrected memory invalidating an old resume, relative priority changes against immutable reviews, and waiting resolution/reopening independent of task status. Artifact: `.local/backlog-review-20261017/continuity-focus-walkthrough-results.json`.
- **Memory and source controls:** correction/suppression/restore, pinned conflict versions and atomic stale rejection, keep-one/keep-both/reopen, scoped denial, independent grant revocation, exclusions/gaps, local retry, notification decisions/mutes. Artifact: `/tmp/memory-controls-walkthrough-results.json`.
- **Protocol adapters, evaluations and handoffs:** MCP stdio/HTTP reads and bounded errors; Git/LSP scopes; mock local WAV transcription and cleanup; exact/stale/duplicate evidence scores, baseline regressions and supplied-cost limits; explicitly budgeted synthetic judges; agent acknowledgement versus user completion; unchanged observation cursors. Full details: [protocol verification](module-protocol-verification-2026-10-17.md).
- **Recipes:** exact preview and approval gate, synthetic ACP completion, same-key replay, one persisted receipt and daily recipe limit. Artifact: `.local/backlog-review-20261017/root-workflow-records.json`.
- **Usage controls:** interactive reserve, failed-primary explicit fallback, separate account approval/replay, removed-rule rejection. A second isolated workspace `.local/usage-review-20261017` exercised five actual loopback extraction requests, separate retry attempts, a queued job remaining paused and an overnight reservation blocked under a reduced current budget with no sixth provider request. Recorded dates were deliberately moved on synthetic rows to exercise rollover without changing the machine clock. The rebuilt worker reports that no provider request was made. Artifact: `usage-walkthrough-results.json` in that workspace.
- **Actions:** task preview leaves state unchanged; approval, exact fingerprint, same-key execution/undo replay, newer-key double execution rejection, stale target rejection and changed-target undo rejection passed. An inserted fixed one-pixel synthetic PNG/OCR frame was deleted atomically from image and FTS storage; receipt retained no OCR, replay returned the same receipt, and unsupported undo rejected. No screen was captured.
- **Capture recovery/retention:** a synthetic active session with a nonexistent selected window recovered to paused/version 3 after service restart, with explicit-resume explanation and no capture worker activation. Fixed synthetic records then verified simultaneous frame-count/age expiry, FTS removal, PNG and OCR inspection, explicit resume approval, idempotent stop and rejection of resuming a stopped session. These checks did not invoke the OS screenshot path.
- **Exports:** current JSON/Markdown include selected source revisions, exact citations, timestamps/watermark and history. Current observation presentation uses corrected bodies and excludes suppressed records, while history retains originals. Markdown history escapes embedded fences. Thirty-one synthetic source revisions exceeded the 2MB payload limit and produced an explicit rejection. Artifact: `.local/usage-review-20261017/export-walkthrough-results.json`. A real CLI Markdown export also produced the expected body in a private 0600 file.

## Corrections found and rechecked

- Concurrent judge reservations initially preserved the budget but one loser returned SQLite contention HTTP 500. A bounded retry of rolled-back reservation transactions now rechecks policy/budgets. Both independent scenarios—one request with 4000 estimated input tokens, and two requests with only 300 total estimated input tokens—returned `[200, 403]`, retained one reservation, and replayed the same runtime. No model call was duplicated.
- Superseded waiting dates and candidate versions produced duplicate current notifications. Current trigger relevance was fixed; the rebuilt service returns exactly one current notification for each reproduced record.
- Background quota rejection previously used a generic model-error summary. The rebuilt worker reports the current budget block and that no provider request was made; the mock upstream counter remained five.

## Browser evidence

A fresh isolated headless Chrome profile was used after normal browser/native computer-use access was unavailable. It contains no accounts or work data. The browser pairs only with the synthetic loopback workspace.

- Bundled JetBrains Mono rendered successfully.
- Theme preview/cancel, apply/reload persistence, and system light/dark changes passed.
- Exact evidence navigation opened original revision 1 while the source was at revision 2, centered line 180 within 0.5px, highlighted/focused it, and retained the cited target while independently moving an annotation anchor to line 179. Screenshot: `.local/ui-audit-exact-evidence-r1-line180.png`.
- An unsaved recipe survived an unrelated source snapshot update.
- Full current navigation and clean-document exception collection are still being checked; historical DevTools exception backlog must not be confused with new document errors.

## Build evidence

- Multiple daemon/core/contracts/client/MCP and Wasm builds completed as modules were integrated.
- `./scripts/build-desktop.sh --debug` completed with current shared release web assets and a macOS debug app bundle (133.23 MiB).
- A subsequent daemon build includes the verified reservation-contention, reminder-relevance and quota-explanation corrections. A later capture-start replay ordering adjustment still requires the final aggregate build.
- `cargo fmt --all` completed before that daemon build; final source changes require formatting/build again.

## Acceptance still open

- Current complete browser navigation/component review and reconnect behavior.
- The native app launches against an explicitly isolated synthetic data directory, but computer-use reported `cgWindowNotFound`. Launch Services reports the built app finished launching, and CoreGraphics exposes its window metadata only offscreen. Capturing that exact synthetic window failed with “could not create image from window.” Native visual/persistence acceptance is not established by successful compilation or window metadata.
- Actual macOS selected-window capture, identity/pause race and local Vision OCR. The loopback daemon's explicit window enumeration returned a permission/unavailable error. No recording permission was requested and no real window was captured.
- Real model, speech recognition, production LSP indexing and external connector data quality are not established by fixed fixtures. The earlier separately approved Codex request remains the only cited live account evidence.

No full-backlog completion is claimed until the remaining requirement-level acceptance evidence is established. The authoritative checklist is [backlog progress](backlog-progress.md).

## Final audit corrections (historical checkpoint; subsequently verified below)

Further requirement inspection found gaps beyond the first walkthrough: standalone ledger promises were absent from weekly outcomes; duplicate origins could qualify as independent opportunity evidence; normal thread/brief/recipe context could reuse live-only caches; and evals needed actual changed-agent/prompt replay with producer provenance. These mechanisms are now implemented. Ten new Insights checks passed, including controlled historical boundary fixtures and independent-origin acceptance. The derived-context and eval replay checks await the newest aggregate binary.

Derived-context fixes also remove omitted private source identifiers/policy metadata from provider prompts and limit runtime grants to included sources. Local authorization snapshots retain full scope. Receipt hashes bind the exact delivered context bytes, including legacy serialization. Capture now checks selected window identity again after the system screenshot command and discards the image before OCR/retention if that identity changed. That OS branch remains unverified in an interactive GUI session.

Source-control drafts and selected source options were reproduced resetting during unrelated updates; keyed editors and stable option bindings address this. Insights subview lifecycle and delayed-response guards were also corrected. The latest native Rust/all-target and Wasm checks passed before the final narrow backend precision patches; the aggregate app build is still in progress.

## Latest rebuilt backend results

The full debug app bundle completed successfully at 134.96 MiB. Native Rust checks compiled CLI/core/MCP and existing test targets without running tests; Wasm checks and the release web build passed. Formatting check passed after the final line-wrapping correction. The unused provider-context re-export was removed without changing behavior.

- Actual eval replay and filtered handoff receipt probes passed; see the latest sections of [protocol verification](module-protocol-verification-2026-10-17.md). These include exact historical inputs/no MCP/no grant file, immutable concurrent budgets, model A/prompt v1 → model B/prompt v2 regression, explicit unknown model/usage, legacy saved judge import, and a real runtime grant acknowledging only its own filtered receipt.
- Fourteen derived-context checks passed on the rebuilt service: index baseline then live exclusion across cached descendants, separate local selected/pinned/omitted metadata, accepted brief with known omissions, filtered thread/recipe grants, no private live source identifiers/text/notes in actual wire prompts, recorded actual wire hashes, and index → live → index configuration invalidation. Artifact: `.local/backlog-review-20261017/live-derived-walkthrough-results.json`.
- The fresh browser pass has reopened exact historical evidence and all 24 main/subviews with no new application exceptions. Additional deferred-response/theme/draft probes are still running.

Native diagnosis is now concrete: `CGSessionCopyCurrentDictionary` reports `CGSSessionScreenIsLocked=1` and an on-console session. Launch Services sees the built app finished launching; its windows are offscreen, activation fails and selected screenshot cannot produce an image. No unlocking, recording permission request or actual screen/audio capture was performed by the agent.

## Final browser results and native deferral

The fresh browser pass completed against aggregate build 85004 and the rebuilt daemon:

- All 24 main/subviews rendered under both Paper and Terminal with zero new runtime exceptions. Measured semantic text-token contrast minima were 4.672:1 and 4.977:1 respectively; the Paper screenshot was also visually inspected. This is not a claim of a complete accessibility audit.
- System dark → Terminal and light → Paper, reload persistence, preview/Escape restoration, forward/reverse Tab wrapping and focus restoration passed.
- Exact r1/line-180 evidence navigation while current content was r2 passed, as did independent annotation anchoring and unsaved recipe preservation.
- Deferred capture, repository and retrieval responses after navigation produced no disposed-signal exceptions.
- A 100-event burst coalesced into one snapshot request, maximum concurrency one. Forced SSE reconnect preserved all 30 runtime IDs; this did not simulate a full TCP/network outage.
- Source selection, an unsaved 987-hour value, exclusion text and sharing toggle survived both unrelated ingestion and a new revision of the selected source. Freshness text updated at the same time; no exceptions occurred.

Artifacts: `.local/ui-backlog-audit-results.json`, `.local/ui-theme-audit-results.json`, `.local/source-controls-draft-probe-results.json` and `.local/ui-audit-*.png`.

The user replied that they cannot test now and explicitly selected **Leave native verification pending**. Native rendering/theme persistence and actual selected-window capture/OCR/pause/identity checks remain open. No further native interaction or permission request is being attempted. All earlier pending browser/build statements above are historical checkpoints superseded by these results.

The remaining native acceptance procedure is recorded in [capture](capture.md#native-acceptance-procedure). Final synthetic-workspace cleanup confirmed zero active runtime runs and revoked the three remaining walkthrough grants; no active agent grants remain in that workspace. Artifact: `.local/backlog-review-20261017/grant-cleanup-results.json`. This cleanup does not change browser pairing or any production workspace.

### Additional browser preference acceptance

The final coverage audit also exercised Midnight and Phosphor across all 24 views, with zero runtime exceptions. Both persisted across reload; representative semantic text-token contrast minima were 4.821:1 and 5.141:1. Selected rows and text selection were visually distinct in the inspected screenshots. Together with the earlier Paper/Terminal checks, this covers navigation under all four built-in palettes; it is not an exhaustive accessibility audit.

Unsupported and obsolete JSON-shaped saved values fall back to Terminal; applying a valid choice repairs the stored preference. An isolated, theme-key-only injected `QuotaExceededError` preserved the unsaved preview and explicit error without changing the saved value. Cancel restored the saved theme, and restoring storage writes permitted Apply/reload recovery. This exercises exception handling, not actual disk exhaustion or a changed browser policy. Original Terminal preference was restored. Artifacts: `.local/ui-theme-additional-report.md` and `.local/ui-theme-additional-results.json`.

The final source inspection found no additional stubbed backlog mechanisms. Native GUI/capture acceptance remains the same external dependency: the screen-lock finding, the user's explicit deferral, and the subsequent completion audit have not established an available, authorized native verification session. No full-goal completion is claimed.

## Native retry: completed acceptance

The user subsequently requested a retry and confirmed the synthetic test window was visible. The earlier native deferral and pending statements above are historical checkpoints, superseded by this section. Only `.local/backlog-review-20261017` and the synthetic Thesaurus window were used. No account/model request, OS permission change, microphone capture or real work window capture occurred.

| Check | Actual result |
| --- | --- |
| Native interface | 11 main views plus 13 Focus/Knowledge/Insights subviews rendered through the native Tauri client. These were rendering walkthroughs, not native console instrumentation or an exhaustive accessibility audit. |
| Theme persistence | Paper preview/apply rendered correctly, survived a full native process quit and relaunch, and remained selected in Appearance. Original Terminal restored. Browser tests separately cover all four palettes/system changes. |
| Exact evidence | Native viewer opened immutable r1 while current content was r2 and centered/highlighted line180 with the original exact quote. An offscreen webview delayed its animation-frame scroll until the user made the window visible. |
| Explicit capture | Before activation, opening controls/listing windows left zero active sessions and zero retained frames. An offscreen selection rejected safely. Once visible, the exact approved app/title/PID/window ID produced an actual window-only PNG, inspected locally. |
| Vision OCR and search | Integrated capture retained a PNG of 666,931 bytes and 2,407 OCR characters; `synthetic` returned that exact frame through FTS. Native inspector loaded the OCR hit and exposed the retained PNG. Cold run 113.24 seconds. |
| Visible Pause | Native app-wide CAPTURING/Pause control was observed visually. Clicking Pause through native UI removed the indicator and changed the real session from active/v1 to paused/v2. The probe observed this external UI transition rather than its safety timeout. |
| Window closure | Quitting the selected synthetic client paused capture with an unavailable/changed-window explanation. Resuming the old window rejected 400; no different window was captured. |
| Restart | A real session was explicitly resumed active/v3, then its service stopped/restarted. It recovered paused/v4 with the explicit-resume explanation; no automatic recording restarted. |
| In-flight Pause | After detecting a newly captured temporary PNG, Pause committed at v4. Processing finished 81.55 seconds later, the temporary image disappeared, and retained frame IDs remained exactly unchanged. Runtime helper bytes matched the final source, including the compiler compatibility guard. |
| Final cleanup | All five sessions stopped, no active captures and no active agent grants. Retained synthetic evidence remains subject to configured expiry; exported diagnostic images are separate local review artifacts. |

### Fixes found during native verification

1. **Vision compute failure/cold timeout.** The original 45-second helper timed out. A direct run exposed `TextRecognition.CRImageReaderError.e5rtError` from a compiled compute operation; a CPU-only diagnostic succeeded. The bridge now tries normal Vision processing, retries the same PNG locally on supported CPU devices on failure, and exits with a normal error instead of trapping. The complete helper is bounded to 180 seconds. Modern Swift uses the supported compute-device API; older compilers/runtime OS versions take the legacy CPU option. The installed modern toolchain typechecked and executed this path; older runtime compatibility is inspected, not experimentally established.
2. **Native service port collision.** The exact packaged service command initially failed `Address already in use` while the existing development service owned port 4620. The native launcher now supplies `--port 0`, using the actual address in the private descriptor. A cold native launch started its packaged service on port 60655, returned all 42 sources / 30 persisted runtime records, and served packaged HTML, Wasm and JetBrains Mono successfully. The existing service on port 4620 stayed healthy. All capture sessions remained stopped.

The full debug app rebuild (90389) succeeded at 134.96 MiB; the final launcher-only bundle rebuild (25627) succeeded after it. Final formatting and Swift typecheck passed. Shared web assets remained `thesaurus-ui-3505a1e5bc03b560`; no test suites were run. The synthetic CLI/protocol/browser walkthroughs earlier in this record retain their stated scope.

Native artifacts in `.local/backlog-review-20261017`: `native-retry-baseline.json`, `native-png-diagnostic.json`, `native-ocr-integrated.json`, `native-final-ui-pause.json`, `native-inflight-pause.json`, `native-session-cleanup.json`, `native-autostart-results.json`. Diagnostic images: `.local/native-selected-window.png`, `.local/native-window-with-ocr.png`. CPU/default/fallback diagnostic results are `.local/native-vision-*-diagnostic.json`. Native screenshots and accessibility observations were also inspected through Computer Use.

The final readback (`native-completion-audit.json`) confirmed all five sessions stopped, zero active grants/runtimes, unchanged frame IDs after the in-flight Pause check, both packaged executables present and final formatting passed. This closes the recorded native acceptance gate.

## F43 implementation increment

F43 was added after the completed F01–F42 acceptance pass. It now includes a shared Rust recording session, explicit microphone/system-audio selection, a local Apple capture bridge, bounded ordered whisper.cpp chunk transcription, project transcript revisions, notes/bookmarks, cited context/action review, menu bar controls/app shortcuts and panel/edge-dock surfaces. See [meeting companion](meeting-companion.md) for exact behavior and remaining interactive acceptance.

The optimized Wasm build, native Rust checks, Swift bridge compile and full debug macOS bundle build passed (139.00 MiB). The packaged service and audio helper match the build outputs; the bundled app has the correct identifier and microphone/system-capture usage descriptions. No audio recording, OS permission change, model/account call or test suite occurred in this implementation increment. Compilation does not establish microphone/system audio, permission attribution, final-chunk flushing or companion window behavior at runtime.
