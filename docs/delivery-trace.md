# Commitment ledger and meeting-to-delivery trace

F22 and F25 share a durable project commitment record. Open **Focus → Delivery** in either the browser or native app. This workflow uses local storage and exact source quotations. It does not make a model request or send a message.

## Capture and follow a promise

1. Choose a project and capture the promise, person or team, promise date, optional deadline, and its exact source line. An accepted task can supply its original evidence instead. Source evidence is pinned to a document revision and line; it remains available when the current document changes.
2. Link later **discussion** or **implementation** evidence from project sources, or expand **Link a repository report / commit** to select a registered repository, immutable report, full commit ID and 1–20 actual reported paths. Repository links preserve the report digest, exact from/to range, commit ID/subject, selected file paths, report timestamp and omission notices. Each link retains its author, timestamp and explanation; text quotations remain pinned to source revisions.
3. Use **Amend promise** to change its wording, owner or deadline. An amendment requires supporting evidence. Original promise evidence remains intact and the attributed history records both the new promise state and why it changed.
4. Choose implementation links and **Propose completion**, with a rationale. A proposal changes the commitment to `proposed`; it does not complete the promise.
5. Read the implementation evidence and explicitly **Confirm reviewed completion**, or reject the proposal. Confirming changes only this commitment. The accepted task has an independent lifecycle.

Links or amendments invalidate a pending completion proposal, which must then be proposed and reviewed again. Completed or cancelled commitments require an explicit reopen before editing. Cancellation requires a reason. Reopening preserves the entire delivery trace and history.

The interface has status filters, ledger paging, source previews, original and subsequent evidence, and a paged promise history. A source revision change is shown as stale evidence without rewriting old evidence. All changes are attributed to **You** because only the authenticated user may mutate commitments. Agent access is read only.

## CLI

```sh
thesaurus commitment list PROJECT --status all
thesaurus commitment create PROJECT 'Ship the import workflow' \
  --owner 'Alex' --promised-on 2026-10-17 --task ACCEPTED_TASK
thesaurus commitment get COMMITMENT
thesaurus commitment update COMMITMENT --expected-version 1 link \
  --kind implementation --note 'Import workflow landed' \
  --evidence-json '[{"document_id":"DOC","revision":2,"line":4,"quote":"Exact source line"}]'
thesaurus commitment update COMMITMENT --expected-version 2 propose_completion \
  --link-ids IMPLEMENTATION_LINK --note 'Implementation and delivery confirmed by this evidence'
thesaurus commitment update COMMITMENT --expected-version 3 confirm_completion
thesaurus commitment repository-link COMMITMENT --expected-version 4 \
  --report REPORT --commit FULL_COMMIT_SHA --path src/import.rs --note 'Implements the import commitment'
thesaurus commitment history COMMITMENT --offset 0 --limit 25
```

`create` also accepts `--evidence-json`, `--due`, and `--note`. `update` supports `amend`, `link`, `propose_completion`, `confirm_completion`, `reject_completion`, `cancel`, and `reopen`. Amendments accept `--title`, `--owner`, `--due` or `--clear-due`, with mandatory evidence. Use `--idempotency-key` to safely retry an exact mutation; reuse with changed input is rejected. The UI retains the same operation key when a response is uncertain.

## API

| Method | Route | Behavior |
| --- | --- | --- |
| GET | `/api/v1/projects/{id}/commitments` | Scoped ledger; `status`, `offset`, `limit` |
| POST | `/api/v1/commitments` | User creates a promise |
| GET | `/api/v1/commitments/{id}` | Record, evidence, latest history |
| PATCH | `/api/v1/commitments/{id}` | User action with `expected_version` and `idempotency_key` |
| POST | `/api/v1/commitments/{id}/repository-links` | Attach an existing project repository report, full commit ID and selected reported paths |
| GET | `/api/v1/commitments/{id}/history` | Complete history through bounded pages |

Creation requires `project_id`, `title`, `owner`, `promised_on`, `evidence` or an accepted `task_id`, and `idempotency_key`. Mutations use the same native local transport or paired browser session and CSRF protection as other user actions. Scoped read credentials must cover every original and subsequently linked source; unauthorized records are neither returned nor counted. A source removed from project membership does not erase historical scope requirements. Adding evidence is restricted to current project membership.

## Storage and bounds

The additive, repeatable migration creates `delivery_commitments` and `delivery_history`; it does not change the existing schema version. A mutation stores the record, immutable history snapshot, workspace event and idempotency receipt in one SQLite transaction. Optimistic versions prevent lost edits and prevent confirming an outdated proposal. Source line, revision and exact text are validated against immutable stored revisions.

A project supports 10,000 commitments. Each commitment supports 100 later evidence links; each evidence-bearing operation accepts 1–6 nonempty lines of at most 2,000 UTF-8 bytes. A record is limited to 120 KB before history. Detail responses retain the newest history entries within 180 KB; the remaining immutable history is available through paging. Lists use pages of 1–50 records within 300 KB; follow `next_offset`, because size limits can shorten a page. History pages are bounded to 180 KB and also report `next_offset`. Promise and owner text are limited to 300 and 200 bytes; context and rationale to 2,000 bytes. All dates use UTC calendar dates.

Repository links require an existing immutable report in the same project and its source scope. Selected paths must occur in the selected report commit; no arbitrary commit/file assertions are accepted. Linking performs no Git, filesystem or network operation. Scoped readers additionally require the linked repository's current disclosure flag, even when their source grant covers the project; private repository metadata cannot escape through a commitment or its history. New repository links invalidate prior completion proposals.

This feature provides explicit source-backed and repository-backed linking and user review. It does not claim that a source line or commit proves a change was deployed, fetch remote repository URLs, infer identity from a person's label, or automatically verify external delivery.

## Build evidence

The Rust daemon and CLI compiled with these modules integrated. No automated tests or live work-account calls were run for this feature slice. The parent implementation tracks the aggregate web and desktop build status.
