# Budgeted context, handoff receipts and run observation

## F18: context budgets

`POST /api/v1/context/budgeted` accepts `query`, `character_budget` (500–16000), `token_budget` (125–16000), and optional `source_ids`. Empty source selection means all permitted sources; explicit selection must be entirely permitted. At most 50 sources can be included. The existing `/context` API and CLI keep their character budget input and receive the same additive metadata.

The complete compact JSON must fit both budgets, including query, source revision hashes, exact line citations, task/observation evidence, omission counts, token estimate and event watermark. Token estimation is UTF-8 bytes / 4 rounded up and is explicitly an estimate, not a provider tokenizer. Metadata that cannot fit causes a clear error; narrow scope or raise budgets. Source metadata is never silently dropped.

The builder uses actual FTS results and scoped accepted/candidate tasks and observations. It considers up to 50 search hits, includes up to 10 hits, 20 tasks, 10 observations, and extracts up to 30 exact source lines from those hits. Long lines over 1000 bytes are excluded. Metadata explains those consideration limits; further removals for budgets are counted by category. Source snapshots expose immutable revision/hash locators. A global event watermark before and after all reads must match, preventing mixed workspace snapshots.

The interface includes source selection, both budgets and a full inspectable result. CLI:

```sh
thesaurus delivery context 'rollout decision' --source SOURCE_ID --characters 12000 --tokens 4000
```

## F21: delivery and acknowledgement

A thread context already persists its exact context revision and associated runtime ID. Receipts add a separate, append-only user/agent acknowledgement lifecycle:

1. `delivered`: the saved context was associated with a specific runtime; this does not prove it was read.
2. `acknowledged`: a user or authorized writing agent explicitly confirms receipt.
3. `completion_claimed`: a user or authorized writing agent records a completion claim.
4. `completed`: the user explicitly verifies the work; scoped agents cannot perform this transition.
5. `reopened`: the user reopens previously verified work; it can be acknowledged again.

Agent runtime status appears separately. An ACP runtime finishing normally never marks the handoff or any task completed. Receipt actions do not change source tasks, project facts, or source documents.

Reads require all current project and pinned context sources. Scoped writes additionally require write permission. Mutations require current receipt `expected_version`, an explanatory note (1–2000 bytes), and an idempotency key. Receipt history is limited to 100 events and 250KB serialized size. Context revisions are hashes of the exact saved delivery JSON, with original watermark and runtime provenance retained.

```sh
thesaurus delivery receipt THREAD_ID CONTEXT_ID
thesaurus delivery update THREAD_ID CONTEXT_ID acknowledged --expected-version 0 --note 'Reviewed the pinned context'
thesaurus delivery update THREAD_ID CONTEXT_ID completion_claimed --expected-version 1 --note 'Agent reports the proposed work is finished'
thesaurus delivery update THREAD_ID CONTEXT_ID completed --expected-version 2 --note 'Verified the delivered result'
```

API: `GET/POST /api/v1/threads/{thread}/contexts/{context}/receipt`. Body: `expected_version`, `action`, `note`, `idempotency_key`.

## F33: reconnect without duplicate work

Agents includes “Reconnect to an existing run”. Attach any stored runtime ID, including one older than the recent run list. The observer reads the saved runtime, session ID, progress, output, error, pending permissions, state and workspace watermark. Cancellation and permission responses target that exact runtime. Attaching never calls runtime creation or session resume.

The last attached ID and cursor are stored in browser/webview local storage. Reopening restores observation using those values. Each cursor is a digest of the full stored runtime; unchanged observations report `changed: false`. Active runs are polled every two seconds. Polling stops on terminal state, explicit detach or a connection error; Attach / refresh reconnects. Current state is reconstructed from persisted runtime records rather than assuming browser memory or the recent list still exists.

The daemon survives client closure. A daemon restart marks previously running ACP sessions interrupted using existing recovery. Observation shows that fact; it does not silently rerun the task or claim to reconnect an agent process that has stopped. This feature observes persisted current state; it is not a complete token event log.

```sh
thesaurus delivery observe RUN_ID
thesaurus delivery observe RUN_ID --cursor PREVIOUS_HEX_CURSOR --polls 30
```

Observation API: `GET /api/v1/runtime/runs/{id}/observe?cursor=HEX_DIGEST`, local user authentication only. It returns `run`, `cursor`, `changed`, `watermark`, and `observed_at`. The CLI limits observation to 1–300 polls; no live model invocation is performed.

## Verification

These changes add no tests and perform no model requests. The coordinator records aggregate build results.

### Filtered runtime receipt permission

An active runtime's exact grant may read and acknowledge/claim completion of its own delivered receipt using only the included pinned source scope, even when live-only project sources were omitted. Identity is bound to the runtime's stored grant ID and authenticated contributor, not a user-chosen grant name. The exception provides receipt metadata only; it grants no thread, document or broader memory access. Other grants still require all current project and pinned source permissions and write permission for updates. Only the user may mark work completed or reopen it. Terminal/revoked runtime grants cannot use this exception.
