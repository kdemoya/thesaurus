# Anchored notes and correction overlays

F02 attaches a user note to a document revision and transcript line. F05 corrects speaker labels and company terms in derived extractions. Both preserve the original source text.

## Anchored notes

Open a source, click a transcript line or enter its line number, and expand **Conversation notes & corrections**. Capture the note and optionally mark **Emphasize in summaries**. The note retains the complete original line, revision, author and timestamp. Revision-specific notes remain attached to their original context when a source changes.

Edit, archive or restore a note through the same panel. Every version remains in its paged history. The current active note is available as attributed memory to authorized agents and thread context; previous versions are suppressed from current memory while remaining inspectable in history. Summary inputs receive a bounded section of anchored user notes, with emphasis first, original evidence and omission counts. Notes are user interpretations; their presence does not turn an unsupported claim into a source fact.

Notes require a nonempty exact source line of at most 2,000 bytes. Note text is limited to 4,000 bytes, with 10,000 notes per document. Notes, history and corrections use source permissions and the existing authenticated browser or native transport. All mutations are user only, with optimistic versions, atomic audit history and idempotency receipts.

## Speaker and company term corrections

In the source panel, add a source-scoped alias:

- A **speaker** rule maps an exact extracted owner label to the correct label.
- A **term** rule replaces exact, case sensitive literal text in extracted titles. Longest matching aliases take precedence. Replacement is simultaneous: the replacement text is not recursively substituted.

Save the rule draft, then select **Preview affected extractions**. The preview shows each affected extraction's original and corrected title and owner, exact original evidence, and whether the change can be applied. Accepted and completed tasks are protected. Historical candidates, field length violations and unique-key collisions are also protected with an explanation.

Only **Recompute these candidates** applies the preview. A signature covers correction rules and impacted task versions; changes since preview require a fresh preview. Recomputed candidates retain their task IDs, source revision, line and quotation, and receive a new version. Both before and after data remain in the correction audit. Baselines retain the original derived title and owner, so changing or removing correction rules can preview returning unreviewed candidates to those fields. Future explicit analysis applies the current overlay to newly derived fields while retaining original model or rule inputs as baselines. No recomputation makes a model request.

There are at most 100 rules per source, with original and replacement aliases limited to 100 bytes each. Preview supports up to 500 impacts and 600 KB; narrow a rule set if it exceeds this bound. The UI pages note and audit history; API pages report `next_offset` because size caps can shorten a page.

## CLI

```sh
thesaurus annotation create 'Confirm the deployment dependency before scheduling' \
  --emphasis --evidence-json '{"document_id":"DOC","revision":1,"line":8,"quote":"Exact source line"}'
thesaurus annotation notes DOC --revision 1
thesaurus annotation update NOTE 'Updated emphasis' --expected-version 1 --emphasis
thesaurus annotation history NOTE
thesaurus annotation set-corrections SOURCE --expected-version 0 \
  --rules-json '[{"kind":"speaker","from":"Speaker A","to":"Alex"},{"kind":"term","from":"Data Docks","to":"DataDocs"}]'
thesaurus annotation preview SOURCE
thesaurus annotation apply SOURCE --signature PREVIEW_SIGNATURE
thesaurus annotation correction-history SOURCE
```

Each mutation accepts an optional `--idempotency-key`. Updating a note with `--status archived` removes it from current memory and summaries while retaining history.

## API

| Method | Route |
| --- | --- |
| GET | `/api/v1/documents/{id}/notes?revision=1&offset=0&limit=25` |
| POST | `/api/v1/notes` |
| GET / PATCH | `/api/v1/notes/{id}` |
| GET | `/api/v1/notes/{id}/history` |
| GET / PUT | `/api/v1/sources/{id}/corrections` |
| GET | `/api/v1/sources/{id}/corrections/preview` |
| POST | `/api/v1/sources/{id}/corrections/apply` |
| GET | `/api/v1/sources/{id}/corrections/history` |

`include_archived=true` includes archived notes. Notes and histories accept `offset` and `limit` (1–50). Original source revisions are never overwritten. The additive `annotation_*` tables hold records, histories, extraction baselines and recomputation receipts.
