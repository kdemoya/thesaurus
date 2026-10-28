# Independent backlog audit

Date: 2026-10-17. Scope: all 42 requirements in `feature-backlog.md`, including P2. The original table below is a historical code-inspection checkpoint; later sections record fixes and verification. Current acceptance is consolidated in [backlog progress](backlog-progress.md) and [verification](backlog-verification.md), with evidence limited to the workflows actually exercised.

## Original requirement-level implementation audit

| ID | Concrete implementation location | Acceptance evidence still needed |
| --- | --- | --- |
| F01 | UI `briefs::open_evidence`, shared evidence target, document/revision-checked scroll, focused/highlighted line, action Enter and source buttons; `evidence-navigation.md` | Current web and desktop render, long-document scroll, old revision, annotation anchor |
| F02 | `annotations/notes.rs`: exact anchors, user emphasis, immutable note history, superseded observation overlay; brief prompt annotation context | Anchor/edit/archive/restore and emphasized brief prompt in current UI |
| F03 | `continuity/meetings.rs`: recurring series, captured baselines, unresolved work and changes | Multiple real synthetic baselines and permission-denied reads |
| F04 | `discovery/refresh.rs`, `cues.rs`: incremental question markers, user-linked conflict evidence, durable dismissals and quiet inbox | New transcript revision while viewing; dismiss and reconnect; source navigation |
| F05 | `annotations/corrections.rs`: aliases/term rules, exact preview, protected accepted work and explicit apply | Preview/apply/current-revision invalidation, source evidence unchanged |
| F06 | `capture/audio.rs`: installed local whisper executable/model; separately approved ACP reasoning | Installed transcription process with synthetic WAV; output cleanup and denied destination; no automatic reasoning |
| F07 | `capture/activity.rs`, `platform.rs`, native recognition bridge: selected window identity, visible pause, retention and frame/OCR inspection | OS permission, selected-window capture, changed identity/pause race, retention and restart |
| F08 | `retrieval/*`: approved MCP read mapping, index/live/hybrid policy, path attribution and checkpoints | Actual stdio/HTTP reads, changed-query cache identity, all answer paths respecting/explaining policy |
| F09 | `knowledge/answers.rs`: exact passages, dates, differing values, missing evidence and bounded extractive answer | Panel rendering with supporting/conflicting/missing evidence and denied sources |
| F10 | `knowledge/claims.rs`, `answers.rs`: immutable observation times, explicit validity intervals, historical/current comparison | Validity and known-by boundaries; corrections/retractions across dates |
| F11 | `knowledge/people.rs`: cited identity/aliases/dated roles/authored work, uncertain matching and history | Role validity, uncertain alias matches, archived/withdrawn assertions and history |
| F12 | `repository/reports.rs`: selected Git commits/files, relevance hints against accepted work, checkpoints | Synthetic repository range, excluded files, exact commits and denied historical scopes |
| F13 | `repository/symbols.rs`: approved LSP definition/reference requests, UTF-16 positions, selected-file/result filtering | Installed language server lifecycle/capabilities, exact locations, bounds and path escapes |
| F14 | `discovery/gaps.rs`, `refresh.rs`: saved missing-source questions, ingestion-driven possible evidence and user answer review | New evidence arrival, dismissal, source addition, answer/reopen history |
| F15 | Authenticated API/MCP, scoped grants and attributed `append_observation` | Earlier two-process synthetic MCP walkthrough passed; repeat against latest tools and newly added record types |
| F16 | `memory_review.rs`: immutable originals, effective overlays, versioned correction/suppression/restore and inspector | Earlier synthetic correction passed; current UI, older revision access, overlay consistency |
| F17 | `briefs.rs`: cited generated sections, review/differences, source watermarks and accepted-only sharing | Current generation, rejection, accepted revision, staleness and source-policy withdrawal |
| F18 | `context_delivery/mod.rs`: full JSON character/token estimate budgets, citations, scope, omissions, watermark check | Earlier synthetic budget check passed; current mixed-source exclusions and smallest/largest budgets |
| F19 | `continuity/threads.rs`, `context.rs`: checkpoints, split/merge, bounded snapshots, effective memory and delivery | Current split/merge, corrected/suppressed memory, stale context and explicit resume |
| F20 | `memory_review.rs`: pinned incompatible claim snapshots, dates/evidence, user resolution and preserved uncertainty/history | Earlier synthetic keep-one passed; stale-claim rejection, reopen and source revocation |
| F21 | `context_delivery/mod.rs`: delivered/acknowledged/completion-claimed/completed/reopened receipts | User versus authorized agent transitions and restart without implied completion |
| F22 | `delivery/mod.rs`: attributed promises, evidence, versioned changes and history | Earlier commitment flow passed; changed promises and historical scopes |
| F23 | `focus/priorities.rs`: deadline/importance/waiting signals, reviewed baselines and relative influences | Current before/after ranking cases, UTC boundaries and permissions |
| F24 | `focus/waiting.rs`: selected follow-up dates, local drafts, snooze/response/status history | Current reminders, resolving/reopening and absence of outbound sends |
| F25 | `delivery/mod.rs`: later evidence links, proposed completion, separate confirmation | Earlier synthetic proposal/confirmation passed; repository evidence and original task status preservation |
| F26 | `insights/opportunities.rs`: provisional cross-source groups, exact supporting/counterevidence, affected tasks and explicit review | Independent-source validation, omission reporting, manual edits and review; lexical grouping must remain provisional |
| F27 | `insights/weekly.rs`, immutable task transitions: period-scoped completed work, accepted decision diffs, historical blockers and unresolved work | Future transition periods, legacy baseline uncertainty, end-date boundary and pinned source revisions |
| F28 | `insights/capacity.rs`, `calendar.rs`: selected personal tasks, effort minutes, working/event interval unions, deadline pressure and editable proposals | Overlap arithmetic, missing estimates, real ICS preview/unsupported features, stale plan versions |
| F29 | `models.rs`, configuration, ACP and upstream MCP clients: existing CLI accounts, environment API keys and local endpoints | Installed CLI help flags and official references checked; live Claude/API/local routes not exercised |
| F30 | `usage_controls.rs`, queue/reservation/worker checks: request limits, interactive reserve, background pause and explicitly approved fallback | Concurrent reservations, old queued jobs/day rollover, changed policies, fallback preview invalidation |
| F31 | `acp.rs`, runtime store and Codex helper: ACP v1 session/update/permission/cancel/receipt path | Earlier one approved Codex synthetic request passed; current permission/cancellation and other runtime compatibility |
| F32 | `recipes.rs`: versioned input/template/scope/model/output/budget recipe, preview and explicit execution | Earlier preview passed; actual account-approved run, changed inputs/sources, limits and receipts |
| F33 | Runtime observation cursor and UI observer: durable run/session/progress, attach/poll/reconnect without creation | Client close/reopen, older run by ID, transient failure, daemon interruption and no duplicate request |
| F34 | `procedures/mod.rs`: distinct reviewed successful runs, concrete editable recipe proposal, reviewed installation | Two distinct runs, withdrawn success, dismissal, exact recipe installation and no automatic execution |
| F35 | `source_controls/notifications.rs`: deterministic timing/evidence explanations, durable IDs, read/dismiss, kind/source/expiry mutes | Current triggers, expiry, relevance filtering and persistence without external messaging |
| F36 | `source_controls/coverage.rs`: real upstream receipt/error/policy, captured snapshots, declared exclusions/gaps, targeted local and approved upstream retry | Current connector receipt/error states, retry success and declared gap/exclusion UI |
| F37 | `source_controls/policy.rs`, auth scope filter, runtime reservation/launch and worker checks | Withdrawal across knowledge/context/repo/eval reads and all provider launch paths; already delivered data cannot be retracted |
| F38 | Evaluation runner: repeatable saved cases, exact citation/stale/duplicate metrics, baseline cost/token comparison | Earlier deterministic baseline passed; changed cases/model/prompt metadata and meaningful regressions |
| F39 | `actions.rs`: exact previews, current-state fingerprints, local provider receipts and supported task-status undo; frame deletion explicitly irreversible | Preview invalidation, execution replay, changed state preventing undo, irreversible provider behavior |
| F40 | `exports.rs`, memory snapshot export helper: selected-scope JSON/Markdown, source revisions, original/effective memory and full history | Earlier JSON snapshot passed; current Markdown semantic labeling, excluded sources and byte limits |
| F41 | `themes.rs`, semantic CSS tokens: original/dark palettes/light/system, preview/apply/cancel, persisted local preference | Current browser and desktop palette/contrast, restart, system event, storage denial and keyboard focus |
| F42 | Evaluation suite/version/run/review/judge modules: project/personal scope, deterministic/human/selected model review, pinned inputs and baseline comparisons | Current historical suite permissions; explicit judge budget semantics and supported hard bounds; no unverified claim of monetary/output-token enforcement |

## Concrete issues identified in this audit

1. **F01 was incomplete:** citation opening discarded the target line. Fixed during this audit with a shared target, immutable revision selection, exact highlighted/focused/centered line and stale-response guard. Annotation-click behavior is retained. The final browser pass verified exact old-revision focus, highlight, scrolling and annotation anchoring; native rendering remains a separate check.
2. **F08 changed-query cache identity:** live fetch code originally generated distinct document IDs for argument-dependent reads while the base documents table allowed one document per source. The retrieval owner fixed this to retain one stable source document with pinned revisions. The coordinator subsequently reported all 14 final F08 backend probes passing, including changed-query and source-policy cases.
3. **F08 cross-feature policy consistency:** ordinary search and general context exclude live-only cached sources, while dated knowledge answers directly read retained revisions. A retained historical snapshot can be a valid input, but its path must be explicit and must not be presented as a live result. The retrieval owner added explicit indexed/historical path labeling and live-source exclusions across these paths; the coordinator reported the final F08 probes passing.
4. **F36 incomplete upstream health was strengthened:** the source health panel now reads actual upstream sync receipts, errors and retrieval policy; it includes explicit targeted upstream retry through the approved existing mapping as well as local extraction retry. The final browser build rendered the targeted retry/query-approval form and verified live ingestion status updates while preserving unsaved settings. This worker did not invoke a connected upstream account.
5. **F42 budget units are explicit:** supplied output cost and deterministic output budgets are checked. Saved runs now retain user-selected aggregate judge request and estimated-input-token budgets, reserved atomically with judge runtime creation. These enforce the selected units across concurrent requests. Provider output tokens and currency cannot be capped through that ACP path; this limitation is documented and must not be described as a monetary cap.
6. **Historical project-suite scope was hardened:** the evaluation owner pinned each suite version's original scope and added read checks. The current project API remains expansion-only. The coordinator reported current deterministic replay and budgeted receipt probes passing.

## Policy-path audit

Authenticated grant requests are filtered against the current per-source external master policy, denied grant IDs and the originating ACP connection's deny list. This applies before every API/MCP request. Context, knowledge, repository reports/symbol histories and evaluation runs additionally check their stored source scopes before returning data. Source scopes in claims, people, waiting and gap history grow to retain historical provenance. Project source changes currently expand and never remove membership.

Provider execution paths use the shared ACP reservation/launch checks or extraction worker destination check. Audio reasoning, recipes, brief generation, thread resume, model judges and fallback enter those shared paths. Background extraction also checks the current reserved-request quota/pause policy before provider invocation. The interactive allowance includes queued/attempted extraction and ACP reservations. These observations establish code coverage, not concurrency or live-account proof.

The initial static audit used no builds, automated tests, model calls, capture, language-server calls or upstream account requests. Subsequent authorized synthetic API and browser checks are recorded below. Read-only installed CLI help and primary documentation checks were used for adapter flag compatibility. The full goal remains unproven until the coordinator completes current aggregate builds and requirement-level runtime/UI/native verification.

## Follow-up synthetic API walkthrough

The coordinator subsequently authorized an isolated synthetic API walkthrough against the loopback daemon. `/tmp/memory-controls-walkthrough.py` and `/tmp/memory-controls-walkthrough-results.json` record this manual check; credentials are read privately from instance metadata and never printed. No connected account, model, microphone, screen capture or upstream service was invoked.

The walkthrough confirmed:

- Correction preserves the original body and exact evidence; suppression removes ordinary memory access; restoring version 0 appends history and makes the original body current again. Specific old revisions remain retrievable.
- Conflict creation changes no memory. A stale claim rejects the whole keep-one decision; reopening refreshes pinned versions. Keep-one suppresses the other claim; reopening retains suppression; explicit keep-both reactivates it. Conflict history retains all reviews.
- A scoped writer can inspect only permitted observations, cannot read a conflict containing another source, and cannot make user-only memory decisions.
- Per-source grant denial blocks future document and memory retrieval, appears in disclosure preview, and can be restored independently of ingestion.
- Coverage settings retain exclusions/gaps, explain their effect on answers and reject stale versions. Retrying a document outside the source is rejected; repeated local retry returns the same current job.
- Notifications contain triggering evidence and explanations. Read/dismiss decisions, stale version rejection, source/kind mute and unmute worked.

Two actual duplicate-notification defects were reproduced: editing an already-due waiting date retained both old/current reminder triggers; a candidate task version change retained both old/current candidate triggers. The relevance query now checks the chosen date/current task version, retaining historical records without showing obsolete triggers. Failed-job relevance likewise checks the latest failure timestamp. The coordinator subsequently confirmed exactly one current notification for each reproduced case on the fresh binary. The synthetic records were waiting request `64b18b91-492e-46bf-af8f-f366ef8b79c7` and candidate task `14d2e01a-dc18-4043-85cb-f4b1620c7081`; each returned one current notification after restart.

The original 42-row table distinguishes implemented mechanisms from remaining acceptance evidence. It does not assert natural-language contradiction detection, automatic semantic independence certification, expertise certification, automatic unsupported calendar recurrence expansion, or monetary caps: those capabilities are neither implemented nor established by the current deterministic rules. F26's grouping remains provisional. The final delivery review added duplicate-origin guards and requires explicit independent-origin attestation before acceptance. It also found and fixed F27's missing standalone commitment-ledger outcomes, with period-end history snapshots and evidence.

## Browser verification follow-up

The coordinator's isolated headless Chrome profile was used at 1440 × 1000 after the fresh daemon/assets restart. A synthetic 220-line document produced an accepted action at revision 1, line 180; importing revision 2 changed that line. Opening the accepted action still displayed exact revision 1 text, focused and highlighted line 180, scrolled 3,817 pixels with the row centered within 0.5 pixels. Clicking line 179 updated the annotation anchor while retaining line 180's citation highlight. Screenshot: `.local/ui-audit-exact-evidence-r1-line180.png`.

The walkthrough visited 11 main views and 13 Focus/Knowledge/Insights subviews, and an unsaved recipe name survived an unrelated observation/SSE snapshot. Memory inspector record changes and refresh also completed without exceptions. Capturing console output and exception counts by phase isolated a real lifecycle issue when leaving Insights opportunities/outcomes: dynamic subview construction disposed local signals while old effects could still read them. Insights now uses keyed project ownership and stable conditional subviews; the effort editor also stays mounted as estimates update. The final release build passed all 24 views in both Paper and Terminal, including Insights project changes, with zero runtime exceptions. Historical CDP exception replay is cleared before each check, so previous-page errors are not counted as current failures.

Browser evidence and screenshots are in `.local/ui-backlog-audit-results.json` and `.local/ui-audit-*.png`. This browser walkthrough does not establish native window rendering or real provider compatibility.

A further synthetic SSE check reproduced coverage/disclosure draft resets and a source-picker selection jump after an unrelated capture event. Source controls now retain editors under source/version keys while health and destination status remain reactive; new source options explicitly retain the selected source. The final browser pass confirmed hours/exclusions/sharing drafts and source selection survive an unrelated capture and a newer revision of the selected source. The latter also updates live ingestion time, with zero runtime exceptions. The reproducer and results are `.local/source-controls-draft-probe.mjs` and `.local/source-controls-draft-probe-results.json`.


### Final browser evidence

The release assets from build 85004 passed the following manual checks:

- 24 views under Paper and 24 under Terminal; zero runtime exceptions, including Briefs, Threads, nested Insights and project changes.
- Theme persistence after reload, preview/Escape restoration, focus restoration, forward/reverse Tab trapping, and system dark/light resolution plus reload. Semantic text-token contrast across normal, row, selected and surface backgrounds was at least 4.672:1 in Paper and 4.977:1 in Terminal. Screenshots were visually inspected.
- Three pending read callbacks (upstreams, repositories, audio jobs) and a separate pending capture-session read were released 300 ms after navigating away from Sources; zero disposed-signal exceptions.
- 100 synchronous synthetic client change events caused one snapshot request, with maximum concurrency one.
- Forced EventSource close/recreation reused the application's registered change handler and received actual server replay. One open event occurred and all 30 persisted runtime IDs remained unchanged. This establishes forced reconnection/replay, not an automatic TCP-drop scenario.
- Source controls retained its selected source, interval `987`, exclusion text and unsaved sharing toggle across both unrelated and selected-source events, while live ingestion time advanced.

Detailed results: `.local/ui-backlog-audit-results.json`, `.local/ui-theme-audit-results.json`, `.local/source-controls-draft-probe-results.json`. Screenshots include `.local/ui-audit-paper-work.png`, `.local/ui-audit-terminal-work.png`, `.local/ui-audit-system-light-work.png` and `.local/ui-audit-exact-evidence-r1-line180.png`. These checks use synthetic local workspaces and establish browser behavior; native rendering and real provider compatibility remain separately qualified by the coordinator.

## Native acceptance closure

The user resumed native verification and made the synthetic test window visible. Native rendering, saved theme persistence through a full process restart, exact historical evidence, actual selected-window PNG/OCR/search, visible Pause, window-closure rejection, restart recovery and in-flight frame discard passed. That pass found and fixed a Vision compute failure with a bounded local CPU retry, and a native startup port collision with automatic free-port selection. The rebuilt bundle cold-started its packaged service and served its bundled web assets. Final readback confirmed all five capture sessions stopped and no active grants/runtimes.

The native deferral in earlier checkpoints is superseded. The exact results, artifacts and limits are in [native verification](backlog-verification.md#native-retry-completed-acceptance). Cold OCR took 113 seconds; other operating systems, older macOS runtime combinations and live provider quality are not established by these checks.
