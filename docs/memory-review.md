# Memory inspection, corrections and conflict review

The **Memory** tab includes an inspector for original observations, effective memory, contributor identity, dates, source scope and exact evidence. Suppressed observations remain in this inspector so a user can inspect and restore them. Shared memory and context use the effective body and omit suppressed observations.

## Correct, supersede and undo

Select an observation, choose **Correct / supersede body**, enter the replacement body and a reason, then save. The current version is checked before changing anything. This creates a user-attributed revision. The original contributor, original body, source scope, timestamp and evidence are never rewritten. Replacement text is a reviewed user statement; original citations remain original evidence and are not automatically asserted to prove the replacement text.

**Suppress from shared memory** retains all history while removing that observation from ordinary memory/context responses. **Undo to an earlier revision** takes a version number; version `0` is the original contribution. Undo appends another revision with the old body and status. It never deletes the intervening decisions. The inspector displays source staleness when a citation refers to an older document revision.

## Conflicting claims

Select 2–8 observations using inspector checkboxes, describe why their claims are incompatible, and open a conflict review. The review pins both claim bodies, memory versions, original contribution dates, contributors, scopes and evidence. It stays **pending** until the user explicitly decides. Creating a review does not change either memory or invoke a model.

Decisions include:

- **Keep all claims, acknowledge uncertainty**: retain all claims as active memory and record that the user reviewed the uncertainty.
- **Keep one claim**: keep that claim active and suppress the others, with linked correction history.
- **Suppress all claims**: suppress each claim while preserving originals.
- **Reopen / refresh claim snapshots**: retain the previous decisions in history and open a new pending snapshot of the current bodies and versions. This is also required when a claim changed since the conflict was opened.

Resolving a conflict checks every pinned memory version and the conflict version in one transaction. A changed claim rejects the entire decision. Reopening alone retains previous memory suppression; explicitly keep a claim or restore its earlier revision in the inspector to reactivate it.

Conflict identification is explicit in this implementation. It does not infer contradictions from natural language or treat a later timestamp as proof that a claim is correct.

## Permissions and durability

Reads require access to every original source attached to the observation. Conflict reads require all claim sources and their original observations. Unscoped personal observations are visible to the user and their original contributor. Agents can inspect permitted records through authenticated API reads, but decisions require the authenticated user. Original source and external disclosure policies also apply to effective memory.

Every mutation requires an idempotency key and optimistic expected version. Replaying the same key returns the exact saved response; reusing a key for different content is rejected. Changes and workspace events commit atomically. The additive `memory_review_` tables retain immutable revisions, conflict history and operation receipts. Portable exports include complete original/effective memory and revision/conflict history for the selected source scope in the export's transaction snapshot.

## CLI

```sh
thesaurus memory-review list --offset 0
thesaurus memory-review inspect OBSERVATION_ID
thesaurus memory-review revision OBSERVATION_ID 0
thesaurus memory-review revise OBSERVATION_ID --expected-version 0 correct \
  --body 'Reviewed replacement claim' --reason 'Confirmed in project review'
thesaurus memory-review revise OBSERVATION_ID --expected-version 1 suppress \
  --reason 'The claim is no longer reliable'
thesaurus memory-review revise OBSERVATION_ID --expected-version 2 restore \
  --restore-version 0 --reason 'Restore the original contribution'
thesaurus memory-review flag-conflict --observations FIRST_ID SECOND_ID \
  --reason 'The two claims give different owners for the same decision'
thesaurus memory-review conflicts
thesaurus memory-review conflict CONFLICT_ID
thesaurus memory-review resolve CONFLICT_ID --expected-version 1 \
  --decision keep:FIRST_ID --note 'Confirmed by the decision owner'
thesaurus memory-review conflict-history CONFLICT_ID --offset 0
```

## API and limits

| Method | Route | Result |
| --- | --- | --- |
| GET | `/api/v1/memory/inspect?offset=0` | 25 permitted observation summaries, including suppressed memory |
| GET / PATCH | `/api/v1/memory/inspect/{id}` | Inspect or revise an observation |
| GET | `/api/v1/memory/inspect/{id}/revisions/{version}` | A specific immutable revision, including version 0 |
| GET / POST | `/api/v1/memory/conflicts` | Recent permitted reviews, or create a pending review |
| GET / PATCH | `/api/v1/memory/conflicts/{id}` | Read or decide a conflict |
| GET | `/api/v1/memory/conflicts/{id}/history?offset=0` | 20 immutable review snapshots, with total count |

Inspection includes at most 30 recent revisions within a 100 KB response budget; the original and effective body always remain available. History count exposes omissions. Every revision remains retrievable individually. Conflict listing considers the latest 1,000 reviews and returns up to 100 permitted records. Offsets are capped at 10,000. A correction body is limited to 16,000 UTF-8 bytes; reasons and resolution notes to 2,000. These features do not delete original work data or send external messages.
