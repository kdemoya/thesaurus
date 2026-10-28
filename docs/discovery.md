# Question cues and the knowledge gap inbox

F04 and F14 provide a quiet project inbox in **Focus → Questions & gaps**. Processing runs locally. It does not call a model, fetch remote information, send messages or interrupt a meeting.

## Question cues

New transcript and Markdown revisions are scanned for explicit question markers: lines ending in `?`, or starting with `Question:`, `Open question:`, `Unanswered:` or `TBD:`. These are **unreviewed question candidates**. A marker does not establish that a question is unanswered. Confirm that it needs an answer, dismiss it quietly, or record an evidence-backed resolution.

Question identity uses the project, source and normalized exact question line. The same question in a later source revision updates its current locator while retaining its first observed evidence and review history. Dismissed cues stay dismissed when this same question reappears; resolved cues retain their resolution evidence. Rephrased questions can become new candidates because this feature does not claim semantic identity matching.

Users can add an unanswered question or link **conflicting statements** directly. Conflicts require at least two distinct exact source locators and retain the user's assessment. The system does not infer semantic contradictions. Resolving a cue requires source evidence and an explanation. Original evidence, revisions, attribution and every review remain available in history.

Project mute records the user's preference and labels the quiet inbox. New candidates remain retained. This slice has no audio or push notification delivery.

## Knowledge gaps

Save a question that lacks sufficient evidence, describe the missing information or source, and select the existing project sources to watch. The missing-source description can name information that is not connected yet. Add a new source to the project and edit the gap's watched scope in the native/web question editor or through the CLI/API when that source becomes available.

Saving the gap captures the current source revisions. When watched sources change, new revisions are scanned for words from the question. A passage matching at least two meaningful question terms (one if the question has only one meaningful term) becomes a **possible relevant passage**, with the exact revision, line, text, matching terms and discovery date. Stop words are excluded. This is lexical matching, and relevance or sufficiency remains unproven until the user reviews it.

New possible evidence changes an open gap to `needs_review`. Mark passages relevant or dismiss them. Dismissed passages leave the current bounded queue on refresh and remain in immutable history; their exact revision/line identities remain durably dismissed, making room for later information. Relevance does not answer the question. Only an explicit user answer with exact supporting evidence changes the gap to `answered`. Archive and reopen are explicit; source changes do not reopen answered questions. Selecting **Use for answer** pins the suggestion's exact original revision even if the current source has changed.

Importing a new source revision triggers incremental refresh for projects that contain it. Opening Questions & gaps or selecting **Revisit changed sources** also refreshes it. Durable source watermarks make repeated refreshes idempotent. Gap edits or reopening reset its review watermarks so the next refresh can revisit the selected current sources.

## Permissions and durability

All evidence locators are checked against immutable stored source revisions and current project membership when added. Reading a cue or gap requires every original and later supporting source; historical scope is retained even if project membership changes. Lists omit unauthorized records and do not include them in totals. Only the authenticated user can create, review, resolve, mute or change records. Optimistic versions, operation receipts, history snapshots and workspace events share a transaction.

The additive `discovery_*` tables hold cues, gaps, watermarks, quiet-mode settings and immutable review history. The browser and native app use the same API and local store.

## CLI

```sh
thesaurus discovery refresh PROJECT
thesaurus discovery cues PROJECT
thesaurus discovery review-cue CUE --expected-version 1 dismissed --note 'Already answered in the meeting'
thesaurus discovery create-gap PROJECT 'Who owns the staging rollout?' \
  --missing-source 'Platform rollout ownership decision' --source SOURCE
thesaurus discovery gaps PROJECT
thesaurus discovery gap GAP
thesaurus discovery update-gap GAP --expected-version 2 accept_suggestion --suggestion SUGGESTION
thesaurus discovery update-gap GAP --expected-version 3 answer --answer 'Alex owns staging rollout' \
  --evidence-json '[{"document_id":"DOC","revision":2,"line":4,"quote":"Exact source line"}]'
thesaurus discovery history gap GAP
thesaurus discovery mute PROJECT --muted
```

`create-cue` accepts `--kind conflict` and an evidence JSON array with two distinct statements. `update-gap edit` accepts question, missing-source description and `--source` selections. Every mutation except the deterministic local refresh accepts an explicit idempotency key. List and history commands accept offsets.

## API and limits

| Method | Route |
| --- | --- |
| GET | `/api/v1/projects/{id}/cues` |
| POST | `/api/v1/cues` |
| GET / PATCH | `/api/v1/cues/{id}` |
| GET | `/api/v1/projects/{id}/gaps` |
| POST | `/api/v1/gaps` |
| GET / PATCH | `/api/v1/gaps/{id}` |
| POST | `/api/v1/projects/{id}/discovery/refresh` |
| PUT | `/api/v1/projects/{id}/discovery/mute` |
| GET | `/api/v1/discovery/{cue|gap}/{id}/history` |

Lists and histories accept `offset` and `limit` (1–50) and report `next_offset`; byte limits may shorten a page. Cue lists are bounded to 250 KB, gap lists and history pages to 300 KB. A project supports 5,000 cues and 1,000 gaps. Detection considers at most 100 explicit question lines per document revision; refresh reports omitted lines. Individual evidence lines are nonempty and at most 2,000 bytes. Manual cue/answer operations accept up to six exact citations. Questions and missing-source descriptions are at most 1,000 bytes; explanations 2,000 bytes; gap answers 4,000 bytes.

Gap refresh retains at most ten possible passages per changed document revision and 60 passages per gap, with explicit omission counts; a gap's record is bounded to 180 KB. A source update is a reason to revisit a question, never proof of an answer. Larger semantic question matching, automatic contradiction detection and live audio ingestion remain separate capabilities.
