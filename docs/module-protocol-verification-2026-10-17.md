# Synthetic protocol verification — 2026-10-17

These manual walkthroughs used an isolated local daemon (`.local/backlog-review-20261017`, port 4621), synthetic project `module-5f0cb14d`, and temporary protocol fixtures under `/tmp/thesaurus-module-probes`. They exercised compiled handlers and persisted SQLite records through authenticated APIs. They did not run test suites, use real accounts/models, read work repositories, enumerate actual windows, request recording permission, capture a screen/microphone, or transcribe real speech.

## Observed results

| Area | Actual operation | Observed result |
| --- | --- | --- |
| MCP stdio | Real SDK handshake/list-tools/call-tool through synthetic child process | Stable source document ID; response changes produced revisions 1 → 2 → 3 |
| Fetch replay | Replay first completed key after revision 2 existed | Returned original revision 1 with `receipt_replay`, `fresh: false`; no new fetch |
| Live consent | Hybrid query without approval | Explicit forbidden live error alongside indexed revision 2; path `index_after_live_error` |
| Live consent | Same source with explicit approval and new key | Live revision 3, path `live`, no error |
| MCP HTTP | Real SDK Streamable HTTP flow against loopback synthetic server | Live result revision 1 |
| Live policy | Ordinary search and budgeted indexed context for live-only source | Zero results/documents; context reported one omitted live-only source |
| HTTP transport bounds | Synthetic server returned a 3MB JSON response | HTTP 400, persisted `last_error`, previous revision 1 unchanged |
| Index policy | Query with upstream approval enabled but source policy index | Path `index`, revision unchanged at 3; no live fetch |
| Fixed resource | Configure then read `synthetic://work` through resources/read | Configuration had zero documents; explicit refresh imported exact fixture text at revision 1 |
| Resource citation | Indexed retrieval of fetched resource | Exact original document/revision/line/quote returned |
| Upstream budget | Second explicit refresh after one-read daily limit | HTTP 403: upstream daily read limit reached |
| Repository report | Two commits in a newly created temporary repository | One changed commit, selected `src/main.rs`, one accepted-task relevance link, checkpoint advanced to version 2 |
| Repository exclusions | Same commit included a synthetic `.env` file | `.env` omitted from reported files; only `src/main.rs` returned |
| LSP definition | Actual Content-Length framed stdio handshake/didOpen/definition against synthetic service | One location in selected `src/main.rs`, line 0, UTF-16 character 3 |
| LSP references | Actual framed references request | Two valid selected-file locations at 0:3 and 1:12 |
| Symbol scope | Query excluded `.env` file | HTTP 400: outside approved repository selection |
| Audio adapter | Local synthetic WAV and mock whisper executable writing expected transcript file | Job completed, source attached to chosen project, zero reasoning runs, temporary audio directory removed |
| Suite permissions | New project evaluation suite | Persisted one pinned source ID |
| Deterministic evaluation | Supplied synthetic answer with exact expected/forbidden text checks | Score 1.0; no immediate model calls; saved judge budget one request / 4000 estimated input tokens |
| Source destination policy | Judge attempt using a project containing private upstream/audio sources | Both requests rejected before runtime launch because source destination permission was withdrawn |
| Judge request budget | Two concurrent synthetic judge requests with distinct keys | Rebuilt daemon returned HTTP 200 / 403; exactly one runtime reserved, with the losing request denied by the immutable budget |
| Judge replay | Repeat winning idempotency key | Same runtime ID; no additional budget reservation |
| Judge input accounting | Completed approved synthetic judge | One reserved request, estimate 230, 3770 estimated input tokens remaining |
| Exhausted request budget | Third unique judge request | HTTP 403 |
| Tiny immutable input budget | Saved budget one estimated input token | HTTP 403; zero judge requests reserved |
| Unrelated runtime | Independently start identical-prompt synthetic runtime, then import as judge | HTTP 400: no budgeted receipt for the saved run/case |
| Positive judge import | Import completed budgeted synthetic runtime's strict JSON | Score 1.0 attributed to `model judge: synthetic-acp`, linked to the correct runtime receipt |
| Scoped upstream ACL | Read-only grant containing another synthetic source requested the HTTP source | HTTP 404 before a permitted fetch; probe grant revoked |
| Suite ACL and writes | Scoped grant read its project suite and attempted suite creation | Pinned scope read succeeded; user-only write returned HTTP 403; grant revoked |
| Exact/stale evaluation | Personal suite required exact original document/revision/line/quote | Exact result scored 1.0; duplicate evidence counted once without score inflation |
| Baseline regression | Same personal suite used older evidence revision and changed model/prompt metadata | Score 1.0 → 0.333, stale count 1, regression true, cost comparison 0 → 10 micro-units |
| Supplied output budget | Submitted cost 10 with maximum 9 | HTTP 400 before saving a run |
| Personal suite scope | Reusable suite required evidence outside chosen project | Preview rejected HTTP 400; no extra source access |
| Context contract | Built source-scoped indexed context under 4000 characters / 600 estimated tokens | Complete compact JSON 1580 characters, estimate 395 exactly matched bytes/4; two citations verified against pinned original lines |
| Handoff lifecycle | Completed synthetic runtime with delivered thread context | Runtime completed while receipt remained delivered/version 0 |
| Agent acknowledgement | Scoped writing grant acknowledged delivery and claimed completion | Attributed acknowledged → completion_claimed transitions persisted |
| Completion verification | Scoped agent attempted completed, then user verified | Agent HTTP 403; user completed/version 3; replayed old acknowledgement key added no history; accepted task remained accepted |
| Run observation | Read existing runtime observation, then same cursor | changed true → false; same completed runtime; no new task |


The synthetic ACP fixture emitted fixed JSON; no model judgment quality or real provider billing behavior was evaluated. The audio fixture wrote fixed transcript text; no speech-recognition quality was evaluated. The LSP fixture validates client transport, positions, scopes and persistence, rather than a production language server's indexing accuracy. The HTTP fixture validates transport and bounds, rather than a real Slack/company connector's permissions or data shape.

## Reservation contention correction

The concurrent judge walkthrough exposed a known SQLite busy/locked write-upgrade race. The reservation transaction preserved the immutable budget (only one runtime existed), but the losing request surfaced as a generic HTTP 500. `create_runtime_inner` now retries only rolled-back SQLite busy/locked reservations up to five attempts, rechecking all permissions, versions and budgets. It delays 10/20/40/80ms; exhausted retries return HTTP 409 with idempotency guidance. Agent process launch remains after the committed reservation, so retries cannot duplicate an agent request.

The rebuilt daemon repeated two concurrent reservations with distinct keys. The one-request / 4000-estimated-input-token run returned HTTP 200 and 403, reserved one 230-token request and left 3770 estimated tokens. A two-request / 300-total-estimated-input-token run also returned HTTP 200 and 403, reserved one 230-token request and left 70 estimated tokens. Replaying each winning key returned its original completed runtime without a second reservation. This verifies both immutable request and total-input budgets under the observed contention; retry exhaustion was not forced.

## ACP one-time permission and cancellation

A synthetic ACP process sent a real `session/request_permission` JSON-RPC request while `session/prompt` remained pending. Its tool was explicitly a no-op; no files, shell commands or real accounts were accessed. Each run exposed `waiting_permission` and persisted exactly one pending permission.

| Operation | Observed result |
| --- | --- |
| Offer allow-once, reject-once and allow-always options | Only the two one-time choices reached the application permission record |
| Submit unsupported allow-always ID | HTTP 400; run remained waiting and decision stayed unset |
| Choose reject-once | Selected option returned over ACP, synthetic refusal output completed, decision persisted |
| Choose allow-once | Selected option returned over ACP, synthetic no-op output completed, decision persisted |
| Submit an already resolved decision | HTTP 409; no second decision |
| Replay either completed permission run key | Original runtime ID and terminal record; no second process reservation |
| Cancel an actively running slow prompt | Terminal cancelled within the cancellation window |
| Cancel while waiting for permission | Terminal cancelled and pending permission marked cancelled |
| Replay either cancelled run key | Original cancelled runtime; no restart |
| Inspect scoped runtime grants after terminal state | All four grants revoked |

Synthetic records and scripts were saved under `/tmp/thesaurus-module-probes/state.json`, `run_concurrency_recheck.py` and `run_permission_cancel_probe.py`. Temporary files can be removed after review; the application records remain in the isolated local workspace. These walkthroughs do not measure behavior of a real ACP agent or the quality of its permission explanations.

## Actual ACP evaluation replay and provenance

After the replay implementation was rebuilt, two more manual scripts exercised the fresh daemon. They used a new isolated synthetic project (`25bb51f4-64ab-4099-8ecb-deed7287a90f`) and newly imported fixture text. The source advanced from revision 1 to revision 2 after the baseline was saved. These were actual ACP subprocess handshakes and selected-model configuration messages against the local Python mock, with fixed synthetic responses and no provider/model/account access.

| Operation | Observed result |
| --- | --- |
| Import a previously unreviewed pre-upgrade judge receipt | Current regenerated prompt hash differed because additive output metadata changed serialization; exact original receipt/runtime still imported score 1.0 |
| Import an unrelated existing runtime as that judge | HTTP 400, missing parent/case receipt |
| Supply an output with empty model metadata | HTTP 400 before saving; valid manual output explicitly labeled unverified/self-reported and missing agent version normalized to explicit unknown |
| Native CLI replay-get / replay-plan --preview | Restored the saved plan with one reserved request; separate CLI preview showed zero requests and pinned revision 1 |
| Preview/save replay plan | Zero reserved requests; exact pinned revision 1 retained while the current source was revision 2; save-key replay returned same plan |
| Submit wrong prompt hash | HTTP 403 before reservation |
| Start two cases concurrently under one-request immutable plan budget | HTTP 200 / 403, exactly one reservation; original-key replay returned the same runtime |
| ACP replay session inputs | Synthetic process required both empty `mcpServers` and no `memory-grant.json` in its working directory; successful answer proved both checks passed |
| Actual prompt identity | Runtime provenance hash matched the complete saved preview prompt and independently calculated SHA-256 |
| Finalize inspected model A / prompt v1 answer | One-case score 1.0, exact revision 1 citation, zero stale evidence against pinned input, parent baseline linked |
| Repeat finalization with original and new keys | Same saved result; one result per runtime |
| Replay model B / prompt v2 with unchanged criteria | One-case score 0.8, regression true, model and prompt metadata changed, source snapshot unchanged |
| Replay agent-default route without reported model | Explicit `unknown (ACP model unreported)`; no invented model name |
| Replay runtime provenance | Actual mock agent name `synthetic-acp`, reported version `1`; configured models explicitly labeled ACP acknowledged/identity unverified |
| Recorded replay duration | 2056ms / 2179ms / 2078ms, derived from actual persisted runtime timestamps |
| Unknown usage comparison | Each replay output marked ACP tokens/cost unreported; comparison stated numeric totals were not comparable rather than interpreting zero as free usage |
| Import unrelated identical-prompt ordinary runtime | HTTP 400 before output parsing; no replay receipt |
| Start plan with total input estimate budget 1 | HTTP 403; zero requests reserved |
| Preview new criteria requiring evidence absent from pinned parent input | HTTP 400 before a provider request |
| Terminal grants / workspaces | All four completed synthetic runtime grants revoked; no memory grant file retained in the three replay workspaces |

Replay result IDs: `9cda05e3-a3ec-4d7a-bb86-5fc4fd4f3d48` (model A), `0114a676-5edd-464c-b793-7d3fe29d25e6` (model B), `0d784bb2-0b11-44e4-8824-8df15ecaf672` (unreported model). The fixed mock verifies routing, transport, immutable inputs, budgets, scoring and provenance; it does not evaluate real model quality or provider billing. Script: `/tmp/thesaurus-module-probes/run_replay_probe.py`.

## Filtered own-runtime handoff receipt

A separate unique synthetic project combined one indexed source with a newly configured live-only upstream source containing explicitly fetched private mock content. The reviewed local context retained full selected-scope authorization metadata. Its actual provider projection omitted the private live source ID, cache contents and policy metadata; its runtime grant contained only the included indexed source.

The active runtime's own read-only token could inspect its delivered receipt, acknowledge it, and claim completion. The same token could not read the broader work thread (HTTP 404) or mark work completed (HTTP 403). A separately created writing grant with the **same name** as the runtime grant but a different actual grant ID could not inspect the receipt (HTTP 404); it was revoked after the check. The receipt revision matched SHA-256 of the exact original delivered JSON suffix in `runtime.prompt`. Cancellation reached `cancelled`; the real runtime grant was revoked and subsequent token access was denied.

Project `a5be52a9-6f70-4511-b08b-75519c74269f`, runtime `4eceae06-b277-446e-92e5-d860d384c301`. Script: `/tmp/thesaurus-module-probes/run_filtered_receipt_probe.py`. No actual screen/audio capture, work repositories, real accounts or real models were used.
