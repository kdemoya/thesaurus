# Work threads and recurring meeting preparation

F19 and F03 extend the local project defined in **Briefs**. Both are available in the shared desktop/browser interface, CLI and source-scoped MCP. They do not require a calendar connection.

## Projects and new sources

Create a project in **Briefs** (`6`); a generated brief is optional. **Add project sources** in Briefs or Meetings expands its source selection. Existing inputs cannot be removed through this control because brief, thread and meeting history may depend on them. Adding a source marks previous briefs stale, including empty sections. Refresh and review a new brief when ready.

A thread may use a subset of its project's sources. Meeting preparation uses the complete current project scope. External agents must have grants covering the complete scope of the record they retrieve, and every source must allow external sharing. A project expansion can therefore make its meeting series unavailable to an older grant until the user explicitly updates that agent's access.

## F19: Work threads

Open **Threads** (`7`). Create a named thread, select its project and choose its sources. Save checkpoints describing current work, next steps and unresolved questions. You can attach an existing task, attributed observation or finished agent run from within the thread's scope. A checkpoint records its contributor and original evidence; it is context, not a replacement source fact. Linked task status is a historical snapshot; resume context also includes current open task states.

- **Manage thread:** rename, close/reopen, or add sources already belonging to the project. Scope additions invalidate unused resume previews.
- **Split:** check checkpoints, supply a new title and move them into a new thread. The child inherits the same source scope. The original checkpoints keep their IDs and attribution; both threads retain operation history.
- **Merge:** choose another active thread in the same project. Its checkpoints move to the selected thread, source scopes are combined, and the donor becomes a historical merged thread. Its earlier run receipts stay associated with the original thread. These operations do not cancel an already running agent.

### Resume in another agent

1. Choose a context budget of 4,000, 8,000 or 12,000 bytes and select **Preview context · local**. This invokes no model.
2. Inspect the exact JSON snapshot and omission counts. It contains source revisions/hashes, recent checkpoints, current accepted/candidate tasks, explicit importance preferences, permitted waiting requests, attributed observations, accepted project-brief sections where the thread covers their entire source scope, and limited source excerpts. Stale task evidence and stale brief sections are marked explicitly. No draft brief or hidden agent conversation is included.
3. Choose an available ACP connection, approve sending that snapshot and using that connection's account allowance, then select **Resume work**. It starts a new session in the chosen agent with source-scoped read-only memory. It does not reopen another provider's private session or transfer its credentials.
4. Use **Activity** for progress, permissions, output and cancellation. The thread retains connection, runtime and native session provenance. **Delivered context** reopens the exact input snapshot later. Attach a finished agent output as a checkpoint when you want it carried into a subsequent resume; outputs are not automatically promoted to accepted facts.

The complete compact UTF-8 JSON must fit the selected byte budget. Excerpts, observations, older brief items, older checkpoints and then lower-ranked open tasks can be omitted to fit; counts are visible before approval. Accepted tasks sort ahead of candidates. Excerpts sample the first three nonempty lines per source, skipping lines over 1,000 bytes. The latest 500 scoped observations are considered; older observations may be absent. Original records remain available through authorized retrieval.

User previews expire after 15 minutes. Before reserving a runtime request, the service verifies the thread version and checks source revisions, all scoped task versions/statuses, the considered observations, importance preferences, waiting state, UTC date and accepted brief against the preview. A changed or already delivered snapshot must be previewed again. Delivery, runtime reservation and read-only grant commit together. Retries with the same idempotency key return the existing runtime; they do not start another model request. The existing ACP daily limits, workspace concurrency limit, timeout, cancellation and crash behavior apply. An interrupted run requires explicit new approval; it is not replayed automatically.

## F03: Recurring meeting preparation

Open **Meetings** (`8`). Create a series for a project, optionally give it a cadence label such as “Weekly”, and link completed transcript or Markdown-note occurrences. Each link pins the selected document revision and a `YYYY-MM-DD` date no later than the current UTC date. Unlink and restore controls preserve the original occurrence record.

The latest active occurrence by meeting date (then link time) supplies the preparation baseline. **Before the next meeting** refreshes locally to show:

- Unresolved accepted and candidate commitments from all project sources, with owners, explicit due dates, exact evidence and matching meeting attribution where available. Candidates remain visibly unreviewed. Source revisions that changed behind an accepted task are flagged.
- Task updates since the baseline, including completed, dismissed and superseded items. Completion reflects a recorded task decision; it is never inferred from a transcript or repository change.
- Questions and blockers from the currently accepted project brief. A stale brief stays labeled stale. Missing reviewed information remains missing.
- Project sources added or revised since the baseline, with baseline/current revision links and up to four short added-line examples per document. Examples are a sample, not a semantic summary or complete diff. Deletions and reordering may have no added-line example; open both revisions to inspect them.

**Baseline meaning:** source and task states are captured when you link the occurrence, not reconstructed for its historical meeting date. The interface displays that capture time. Information imported after the meeting cannot be treated as something the app knew during that meeting. If the linked transcript later changes, its original revision remains pinned and preparation warns about the newer revision.

Current waiting requests and due follow-ups are also included; this queue state is not a historical meeting baseline.

No calendar account, model request, scheduled notification or automatic next-date calculation is involved. Cadence is a label; the user links occurrences and opens preparation before the next meeting. Preparation refreshes with workspace events and can be refreshed manually. The shared response is bounded to approximately 64 KB with explicit omissions; source-line examples and brief items are reduced before task records. Full records remain in storage.

## CLI examples

Use the same `--data-dir` as the running daemon. IDs and current `version` values are returned as JSON. Replace example versions with the current ones.

```sh
thesaurus thread create PROJECT_ID 'Release readiness' --source SOURCE_ID
thesaurus thread checkpoint THREAD_ID --expected-version 1 \
  --body 'Next: verify the release owner; deployment approval is still unresolved.'
thesaurus thread context THREAD_ID --budget 8000
thesaurus thread resume THREAD_ID --context CONTEXT_ID --expected-version 2 \
  --connection AGENT_CONNECTION --approve-usage --idempotency-key release-resume-1
thesaurus thread receipt THREAD_ID CONTEXT_ID

thesaurus thread split THREAD_ID 'Deployment approval' --expected-version VERSION \
  --entry CHECKPOINT_ID
thesaurus thread merge TARGET_THREAD_ID OTHER_THREAD_ID \
  --expected-version TARGET_VERSION --other-expected-version OTHER_VERSION

thesaurus meeting create PROJECT_ID 'Release review' --cadence 'Weekly'
thesaurus meeting link SERIES_ID --expected-version 1 --source SOURCE_ID \
  --revision 1 --held-on 2026-10-16
thesaurus meeting prepare SERIES_ID
```

`thread update`, `thread get/list`, `meeting get/list`, and `meeting occurrence` expose management and history. `thread checkpoint --kind task|observation|agent_run --record ID` attaches an existing scoped record. Manual checkpoint citations can be supplied with `--evidence-json`; exact source/revision/line/quote validation applies. Linked output is limited to 4,000 UTF-8 bytes with an explicit truncation flag. Threads hold at most 200 checkpoints and series retain at most 200 occurrences. `brief sources PROJECT_ID --expected-version VERSION --source ALL_SOURCE_IDS` expands project scope.

## API and MCP

| Routes | Purpose |
| --- | --- |
| `GET/POST /api/v1/threads` | Permitted metadata index / create a thread. |
| `GET/PATCH /api/v1/threads/{id}` | Scoped detail / rename, status and scope additions. |
| `POST .../{id}/entries`, `/split`, `/merge` | Add attributed checkpoints and regroup work. |
| `POST .../{id}/context` | Bounded local context preview; `byte_budget` is 4,000–12,000. |
| `POST .../{id}/resume` | User-approved ACP run with `context_id`, `expected_version`, `connection`, `approve_usage`, `idempotency_key`. |
| `GET .../{id}/contexts/{context}` | User-only stored context and associated run ID. |
| `GET/POST /api/v1/meetings` | Permitted metadata index / create a series. |
| `GET /api/v1/meetings/{id}` | Series detail and occurrence baselines. |
| `POST .../{id}/occurrences` | Link a completed meeting and capture its baseline. |
| `PATCH .../{id}/occurrences/{occurrence}` | Unlink/restore a recorded occurrence. |
| `GET .../{id}/preparation` | Deterministic local preparation. |
| `PATCH /api/v1/projects/{id}/sources` | Add sources to a project with an optimistic version. |

Mutations are user-only and require versions where applicable and idempotency keys. Browser CSRF/session and native Unix-socket authorization apply. Agent grants can retrieve authorized threads, local context and meeting preparation, but cannot create/edit records, inspect private delivery receipts or start paid runs. Agent-created context is ephemeral and cannot be used as a stored user resume preview. Shared thread detail is bounded to 60 KB, with omitted checkpoint/history counts. User snapshot responses include full thread and series history; larger-workspace pagination remains future work.

New MCP tools: `memory_threads`, `memory_thread_get`, `memory_thread_context`, `memory_meetings`, `memory_meeting_prep`. There are now 22 memory tools in total, including the [project-focus tools](project-focus.md). Check source scope, stale markers, candidate status, attribution and omission notices before using any returned context as current fact.

## Storage and verification

Schema version 4 adds work threads, checkpoints, operation history, delivery snapshots, recurring meeting series, occurrence baselines and a mutation-idempotency ledger. Split/merge and source-scope changes commit transactionally. Stored context is retained for actual runs; expired unused previews are removed when another preview is requested. No provider credentials are stored in these records.

This delivery was compiled for daemon, web and macOS desktop. Automated tests, browser interaction checks and live cross-agent resume were not run as part of this slice. The existing MCP tool-count expectation was updated. Earlier ACP connectivity evidence does not verify these new workflows. Other ACP runtimes still require their own configuration and compatibility checks.

Future extensions include calendar linking and reminders, automatic meeting association, editable checkpoint history, source-scope removal, context pagination, richer changed-line navigation, and additional agent compatibility coverage.
