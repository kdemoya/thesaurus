# Project and personal evaluations

F38 and F42 share one local evaluation runner. Evaluation suites pin versioned cases, exact expected/forbidden text, exact evidence locators, a review rubric, and a pass threshold. Updating a suite appends a version and pins its original project source IDs; old runs retain the suite version they executed. Project suite reads require both current and pinned sources, and validate every required-evidence document against that version’s source scope (legacy suites conservatively validate against current scope).

## Use the interface

Open Evaluations, choose a project, and create a project suite or reusable personal suite. The case editor is JSON so every criterion remains visible and exportable. Supported targets:

| Target | Actual evaluated output |
| --- | --- |
| `retrieval` | Existing FTS search results scoped to project sources; exact matching source lines become citations |
| `memory` | Existing bounded shared context bundle, including its task citations |
| `brief` | Current accepted project brief; no draft generation or model call |
| `priorities` | Current explainable priority board |
| `supplied` | User supplied agent/workflow output, evidence, and required nonempty agent/model/prompt version metadata (explicit unknown is allowed; self reported provenance is unverified) |

Preview usage, select an optional previous run as baseline, and run locally. Open saved results to inspect each criterion, score, stale/invalid/duplicate citations, output and regression flag. Choose any previous run of the same suite and project as a baseline. Changed criteria remain visible in the immutable suite versions, so score changes across different case definitions require interpretation.

Deterministic score is the fraction of passing expected-text, forbidden-text, required-evidence, and citation-integrity checks. Text comparison is case sensitive. An exact evidence check requires matching document ID, revision, 1-based line number, and complete line quote, both in the output and pinned input snapshot. Citation integrity fails on stale revisions or invalid locations/quotes. Duplicate citations are counted separately and do not inflate required-evidence scores. Pass means score meets that case's threshold. Regression means score fell or a previously passing case stopped passing.

Append human reviews with a 0–1 score and explanatory note. These are append-only records and never overwrite deterministic results. Model judgments are recorded with the actual runtime ID and selected connection, independently from human reviews.

## Replay after changing an agent, model or prompt

Each case may define `replay_prompt` (up to 4000 bytes) and a nonempty `prompt_version`. Save a new suite version when its prompt changes. Start from an existing saved deterministic or replay run: its immutable source documents, revisions, complete numbered lines and content hashes form the replay input. You can select a newer version of the same suite without reading newer source contents. Preview rejects required evidence not present exactly in that parent snapshot, and rejects a complete prompt exceeding 16KB before a provider request can start.

In **Replay with an agent**, select the suite version, case and configured ACP connection, then preview exact input and save a replay plan. Saving calls no model. The plan freezes connection configuration hash, configured model, source snapshot, prompts/versions/hashes, maximum requests, total estimated input tokens and imported answer byte limit. A changed connection configuration requires a new plan. To compare different models, configure/select the appropriate ACP connection; the requested model must be acknowledged through ACP configuration before prompting.

Each case starts only after separately approving its exact saved prompt hash and input estimate. Runtime reservation, source destination permissions, daily connection limits and immutable replay request/input budgets are checked in one transaction. Concurrent starts cannot overrun either budget; failed/cancelled/interrupted reservations count. Retrying the original key returns the original runtime. Replays use the ordinary permission/cancellation/recovery controls and never fall back to an API or another account.

The exact previewed prompt is the ACP text input: there is no added generic work prompt. No memory MCP is offered, so a replay cannot silently retrieve newer source revisions through Thesaurus. The installed agent remains trusted local software with its own privileges; this is not an operating-system sandbox. Output is strict JSON `{ "text": "answer", "evidence": [...] }` with exact pinned citations. Inspect the completed runtime/output, then explicitly approve import with a review note. Import requires the receipt for that plan/case/runtime, exact prompt and source scope, matching actual wire prompt hash and completed status. An unrelated identical-prompt runtime is rejected.

Finalization appends a **one-case** evaluation run, rescoring against the pinned parent snapshot and using that parent as baseline. The complete saved suite version remains attached for inspection; only `results` entries were executed. Other suite cases were not run and are not reported removed. Changed targets or criteria generate comparison warnings and are not labeled regressions. Finalizing the same runtime again returns the existing run. No batch starts or unattended provider requests occur.

Runtime initialization preserves reported ACP agent name/version and default model when available. A configured model is labeled requested and ACP acknowledged, with identity unverified; an unreported model/version is explicitly `unknown`. Runtime receipts retain the selected connection and configuration hash. Eval outputs preserve runtime ID, agent/version, model, prompt version/hash, provenance kind and usage status. Accepted-brief cases resolve their producing runtime instead of attributing generation to the deterministic evaluator; older brief records explicitly mark missing model/version provenance. Manually supplied outputs require nonempty agent, model and prompt version, and their provenance/usage remains labeled self reported and unverified.

ACP does not currently report enforceable output-token or monetary usage. Replay outputs display unreported usage; numeric zero is not a claim that a model request was free. Request count and prompt-byte input estimates are the enforceable budgets; the imported answer size is checked after completion. Output collection still has the runtime's 64KB cap.

## Optional model judge

A rubric is required. Before saving the deterministic run, choose immutable `max_judge_requests` and `max_judge_input_tokens` (total estimated input tokens across all judges). Both default to zero, disabling model judges; choose positive values explicitly to enable them. Prepare model judge displays the exact prompt, source scope and approximate input token estimate. Select an installed ACP connection, set a maximum input estimate, and explicitly approve one request using its existing account. This invokes the same runtime reservation, daily connection request limit, user-visible permission flow, cancellation, and recovery as Agents. It uses no API fallback. Inspect completion in Agents; append the judgment using the completed runtime ID. Import verifies a durable budgeted receipt linking that exact parent run/case to the runtime, its prompt hash, completed status and source scope and parses strict JSON `{ "score": 0.8, "note": "..." }` from that runtime, ignoring manually entered score/note fields.

The estimate is prompt bytes / 4 rounded up, **not a provider token counter or billing cap**. The evaluator rejects requests exceeding the chosen estimate before starting. Connection daily request limits apply. The parent run’s remaining request count and total estimated input token budget are checked inside the same SQLite transaction that creates the runtime grant, runtime row and judge receipt. Concurrent requests cannot exceed either budget. Failed, cancelled and interrupted reserved requests still consume that budget. Replaying an idempotency key returns the same runtime without reserving another call. Actual provider output tokens and costs cannot be capped by the current ACP protocol integration; do not interpret the input estimate as a hard monetary budget. A judge does not run as part of a deterministic evaluation.

## Bounds, snapshots and permissions

- New suites use expected_version 0. Updates require the current version, with immutable personal/project scope. Copy cases into a new suite to change scope.
- 1–50 cases, unique IDs, suite request at most 256KB, queries at most 1024 bytes, rubrics at most 4000 bytes, at most 50 expected/forbidden terms per case, at most 50 required evidence references.
- Project input snapshot at most 2MB. Complete source documents and their revisions/content hashes are saved with each run. Workspace/source changes during execution reject the run instead of saving mixed inputs. This is intentionally conservative even if another unrelated project changes.
- At most 64KB output per case, 512KB total output per run, 100 citations per output. User chooses a lower total output budget and maximum cases; the runner rejects exceeding either.
- User reported supplied-output costs are checked against the chosen integer micro-unit run budget; input/output token counts and cost are preserved with agent/model/prompt versions. Deterministic output has zero reported token cost. The app cannot independently verify external agent usage.
- Writes and judge operations require the local user session. Project suite/run/review reads require every current and pinned project source. Personal suite definitions are user-only. Applying a personal suite never grants extra source access; required document evidence outside the chosen project is rejected.
- Saves, runs, and review appends use idempotency keys. Histories return the latest 100 runs/reviews; all older records remain persisted and runs can be retrieved by ID. Suite list is limited to 500.

## CLI

```sh
thesaurus eval suites
thesaurus eval save --file suite.json
thesaurus eval suite SUITE_ID --version 1
thesaurus eval run SUITE_ID --file run.json --preview
thesaurus eval run SUITE_ID --file run.json
thesaurus eval history PROJECT_ID
thesaurus eval get RUN_ID
thesaurus eval review RUN_ID CASE_ID --score 1 --note 'Correct and well supported'
thesaurus eval judge-preview RUN_ID CASE_ID
thesaurus eval judge RUN_ID CASE_ID --connection codex --max-input-tokens 20000 --approve-usage
thesaurus eval review RUN_ID CASE_ID --judge-runtime RUNTIME_ID
thesaurus eval reviews RUN_ID
thesaurus eval replay-plan RUN_ID --file replay.json --preview
thesaurus eval replay-plan RUN_ID --file replay.json
thesaurus eval replay-get PLAN_ID
thesaurus eval replay-start PLAN_ID CASE_ID --prompt-hash PREVIEW_HASH --max-input-tokens 4000 --approve-usage
thesaurus eval replay-finalize PLAN_ID CASE_ID --runtime RUNTIME_ID --review-note 'Inspected the completed answer and exact citations'
```

To use a model judge, set positive saved-run budgets (for example `max_judge_requests: 1`, `max_judge_input_tokens: 4000`) before running; the later judge command cannot increase them. Preview reports both reserved budget limits and zero immediate model calls.

The CLI generates a new idempotency key if the input JSON omits it; provide a stable key for retries.

`suite.json`:

```json
{
  "expected_version": 0,
  "name": "Project retrieval checks",
  "project_id": "PROJECT_ID",
  "cases": [{
    "id": "decision-search",
    "name": "Find rollout decision",
    "target": "retrieval",
    "query": "rollout",
    "expected_contains": ["rollout"],
    "forbidden_contains": ["unrelated private project"],
    "required_evidence": [],
    "rubric": "Does the result provide useful evidence for the rollout decision?",
    "threshold": 1.0
  }]
}
```

Use `project_id: null` for a personal suite. Evidence can be added as `{ "document_id": "DOCUMENT_ID", "revision": 1, "line": 3, "quote": "Exact full line" }`.

`run.json`:

```json
{
  "suite_version": 1,
  "project_id": "PROJECT_ID",
  "baseline_id": null,
  "outputs": [],
  "max_cases": 50,
  "max_output_bytes": 512000,
  "max_cost_microunits": 0,
  "max_judge_requests": 0,
  "max_judge_input_tokens": 0
}
```

`replay.json` (the CLI adds a retry key when omitted):

```json
{
  "suite_version": 2,
  "case_ids": ["decision-search"],
  "connection": "codex",
  "max_requests": 1,
  "max_input_tokens": 4000,
  "max_output_bytes": 64000
}
```

Add `"replay_prompt": "Identify the rollout decision using the pinned sources.", "prompt_version": "decision-v2"` to the case before saving suite version 2. The same case's criteria can remain unchanged for a comparable model/prompt regression check.

## API

All paths share `/api/v1` and the standard local authentication and CSRF protection.

| Method | Path | Purpose |
| --- | --- | --- |
| GET/POST | `/evaluations/suites` | List / create |
| GET/PATCH | `/evaluations/suites/{id}` | Read (`?version=N`) / append version |
| POST | `/evaluations/suites/{id}/preview` | Validate and preview local usage |
| POST | `/evaluations/suites/{id}/runs` | Execute and persist |
| GET | `/projects/{project}/evaluations` | Run summaries |
| GET | `/evaluations/runs/{id}` | Saved suite, source snapshot and results |
| GET/POST | `/evaluations/runs/{id}/reviews` | List / append review |
| GET | `/evaluations/runs/{id}/cases/{case}/judge-preview` | Exact judge prompt and estimate |
| POST | `/evaluations/runs/{id}/cases/{case}/judge` | Explicit approved selected ACP judge |
| POST | `/evaluations/runs/{id}/replay-preview` | Exact pinned prompts/configuration and immutable proposed usage budget; no model call |
| POST | `/evaluations/runs/{id}/replay-plans` | Save immutable replay plan; no model call |
| GET | `/evaluations/replay-plans/{id}` | Restore plan and current reserved usage |
| POST | `/evaluations/replay-plans/{id}/cases/{case}/start` | Approve one exact saved prompt |
| POST | `/evaluations/replay-plans/{id}/cases/{case}/finalize` | Import reviewed completed receipt-bound output and append one-case score |

Judge body: `connection`, `max_approximate_input_tokens`, `approve_usage: true`, `idempotency_key`. Review body: `case_id`, `score`, `note`, nullable `judge_runtime_id`, `idempotency_key`.

Replay start body: `prompt_hash`, `max_approximate_input_tokens`, `approve_usage: true`, `idempotency_key`. Finalize body: `runtime_id`, nonempty `review_note`, `idempotency_key`. Plan preview/create body matches `replay.json` plus an idempotency key. Preview and plan writes require the user session; plan reads inherit current and pinned source scope. Every provider start rechecks sharing/destination permission.

## Verification

Manual synthetic protocol walkthroughs exercised compiled ACP replay, configured model/prompt changes, immutable snapshot revisions, atomic concurrent budgets, exact runtime receipts, legacy judge import, provenance and permission boundaries. See [protocol verification](module-protocol-verification-2026-10-17.md). No test suites or real model/account requests were run. Aggregate compilation is recorded by the implementation coordinator. Unattended schedules and automatic judge billing enforcement are not implemented.
