# Evidence panels, dated knowledge and people lookup

F09, F10 and F11 share **Focus → Knowledge** in the browser and native app. Queries are local and extractive, with no model usage or external fetch.

## Evidence and uncertainty

Ask a question in **Evidence & history**, choose the project sources, and inspect the returned registered claims and exact source passages. Each passage includes its immutable document revision and line, source title, import observation date, matched terms, retrieval path/policy, pinned-revision fetch date when available, and current connector errors when available. Current indexed answers exclude live-only source caches and claims derived from those sources. Explicit historical date/observation queries can read stored snapshots, labeled historical_snapshot; they never imply a live fetch. All stored passage freshness remains unverified against its upstream. An import date is not an authored date or a validity assertion.

The answer panel shows supporting citations, differing values under the same registered subject/facet, missing evidence and omission counts. Different values may represent a conflict or a legitimate multivalued facet; the system asks for review and does not choose a winner. Claim values are user-authored assertions or explicitly uncertain. Source retrieval matches literal question terms and cannot establish that absent matches mean absent knowledge. The view does not generate a synthesized answer or silently resolve contradictions.

## Two independent time axes

**Dated claims** register a subject, facet, value, certainty, explicit validity interval and exact supporting evidence. For example, `staging rollout / owner / Alex` can be valid from October 1 until November 1. `valid_until` is exclusive. An omitted end means open ended validity.

The service records `observed_at` when the claim or revision is saved. This observation timestamp cannot be supplied by the client. Amend, retract and restore append immutable revisions with new observation times and user-attributed reasons. Amendments require supporting evidence. Original claim revisions are never overwritten.

An answer specifies:

- **Valid as of**: the calendar date on which a registered claim applies.
- **Known by**: the observation cutoff for what the assistant had recorded, independently of validity. Blank means now.

The query chooses the last claim revision known by the cutoff, then evaluates that revision's validity interval. A claim saved today about last month's ownership can answer a retrospective validity question when known-by is today. It cannot appear in an answer restricted to what was known last month. Source passages use the newest imported revision known by the cutoff and explicitly carry no validity assertion.

**Compare with today as currently known** presents both answers with their separate validity date and observation cutoff, and shows differing returned subject/facet values. The comparison covers the bounded returned claims; omissions can conceal further changes.

## People and expertise

**People & expertise** registers cited identities separately from task owner strings. Each identity has explicit context and supporting evidence. Add separately cited confirmed or uncertain aliases, dated roles and authored work with a topic and an explicit author/reviewer/maintainer relationship. Roles are considered relevant only within their validity intervals; all retained role evidence remains inspectable.

Lookup ranks exact registered names, confirmed aliases, uncertain aliases, partial name matches, cited role terms and authored-work terms, with the basis displayed. Uncertain aliases and partial name matches remain explicitly uncertain. The system does not merge people with similar names, infer identity from an action owner string, infer authorship from mentioning a repository, or certify expertise from a score.

Profile changes, individual alias/role/work withdrawals, archive and restore append immutable user-attributed history. Withdrawing an assertion removes it from current lookup while preserving its original evidence and permission scope in history. Each alias, role and work item requires exact source evidence. All historical supporting source IDs remain in the record's permission scope.

## API and CLI

| Method | Route |
| --- | --- |
| POST | `/api/v1/knowledge/answer` |
| POST | `/api/v1/knowledge/compare` |
| GET | `/api/v1/projects/{id}/claims` |
| POST | `/api/v1/claims` |
| GET / PATCH | `/api/v1/claims/{id}` |
| GET | `/api/v1/claims/{id}/history` |
| GET | `/api/v1/projects/{id}/people` |
| POST | `/api/v1/people` |
| GET / PATCH | `/api/v1/people/{id}` |
| GET | `/api/v1/people/{id}/history` |

```sh
thesaurus knowledge answer PROJECT 'Who owns staging rollout?' --valid-as-of 2026-10-01 \
  --known-by 2026-10-15T18:00:00Z --compare-today
thesaurus knowledge claims PROJECT
thesaurus knowledge create-claim --json '{"project_id":"PROJECT","subject":"staging rollout","predicate":"owner","value":"Alex","certainty":"asserted","valid_from":"2026-10-01","valid_until":null,"evidence":[{"document_id":"DOC","revision":1,"line":4,"quote":"Exact source line"}],"note":"Explicit ownership decision"}'
thesaurus knowledge claim-history CLAIM
thesaurus knowledge people PROJECT --query 'staging rollout'
thesaurus knowledge create-person --json '{"project_id":"PROJECT","display_name":"Alex","identity_note":"Platform contact; verify aliases separately","evidence":[{"document_id":"DOC","revision":1,"line":4,"quote":"Exact source line"}]}'
thesaurus knowledge person-history PERSON
```

`update-claim --json` and `update-person --json` accept the corresponding versioned API request objects. The CLI adds an idempotency key if absent; specify one to safely retry an exact mutation. List and history commands support offsets. The interface provides forms for registration, evidence, versioned changes and paged history.

## Permissions and limits

Source permissions are applied before claims or passages become retrieval candidates. Queries accept 1–50 current project sources; unauthorized sources are rejected. Reading a claim or identity requires all original and later supporting sources, including historical ones. Unauthorized records are omitted from list counts. All mutations require the authenticated user; scoped agent credentials can only read. Browser CSRF, native local transport, optimistic versions, immutable histories and transactional idempotency follow existing project patterns.

The project supports 5,000 claim records and 1,000 identities. Evidence operations require 1–6 nonempty exact lines of at most 2,000 bytes, pinned to stored source revisions. An identity supports 30 aliases, 30 roles and 50 authored works within a 150 KB record. Claim values are limited to 2,000 bytes; identity notes to 2,000 bytes; questions to 1,000 bytes. Lists and histories use pages of 1–50 with a byte limit and `next_offset`; size limits can shorten pages. Answer panels retain up to 30 matching claims, 20 passages and 20 differing-value groups, with additional byte limits and omission counts. The additive `knowledge_*` tables retain immutable claim revisions and person histories.
