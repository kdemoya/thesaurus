# Bragbook (F48)

Open **Bragbook → Collection** in the browser or desktop app. Select sources or projects, enable automatic collection, and save. **Collect existing work** scans the saved selection; new changes are collected automatically after enablement.

Bragbook is the first-party local plugin `thesaurus.bragbook`. It is installed disabled and uses the ordinary plugin settings, source grants and destination policies. Collection makes no model requests.

## Collect and review accomplishments

Candidates come from recorded task completions, explicitly confirmed delivery commitments, and source lines beginning `Accomplishment:`, `Achievement:`, `Impact:` or `Shipped:`. Bulleted markers are supported. These are candidates for review: a task owner's name or a quoted claim does not establish your contribution or measured impact.

Each candidate retains its origin, collection time and exact source revision, line and quote. Source markers have no inferred accomplishment date. Existing task completions receive a date only when a recorded completion transition supplies one; migration baselines do not establish earlier dates. Dates use UTC.

In **To review**, inspect the evidence, edit the accomplishment and your contribution, and optionally add impact and an accomplishment date. Choose **Accept for my bragbook**, confirm your review, and select **Accept entry**. You can also keep a candidate or dismiss it. Accepted and dismissed entries remain editable; dismissed entries can return to review.

Repeated scans preserve reviews. Identical markers in the same document are deduplicated across line moves and revisions. A task and a delivery linked to that task share one collection identity. This is identity-based deduplication; similar wording in different sources is not automatically merged.

Reopening an accepted entry's underlying task or delivery returns it to review. Export checks current completion state immediately, including before the collection worker catches up. An updated source shows an earlier-revision notice and preserves the original quotation.

## Prepare highlights

Under **Accepted & highlights**, select up to 100 entries, choose Markdown or JSON, and select **Prepare highlights**. Inspect the preview and download it. The draft uses the accepted wording and citations; it does not invent additional outcomes. JSON uses `thesaurus-bragbook/v1` and includes entry versions and review timestamps.

Every selected entry must still be accepted, permitted by the plugin's current source selection and destination policy, and supported by the current completion state where applicable. The export is captured in one read transaction and capped at 2 MB. A source removed from the selection hides its entries and blocks review/export; retained history is preserved. Disabling collection stops new candidates and retains access to the saved selection's entries.

## CLI and API

```sh
thesaurus bragbook list --status candidate
thesaurus bragbook collect
thesaurus bragbook review ENTRY_ID --file review.json
thesaurus bragbook export --entry ENTRY_A,ENTRY_B --format markdown --output highlights.md
```

The review file uses `BragbookReview`: `expected_version`, `status` (`candidate`, `accepted` or `dismissed`), `title`, `contribution`, `impact`, optional `occurred_on` and `idempotency_key`. A reused operation key with changed content is rejected. Stale versions fail without replacing edits. CLI exports use private file permissions.

| Method | Route | Purpose |
| --- | --- | --- |
| GET | `/api/v1/bragbook` | Filtered, paged entries; `status`, `offset`, `limit` |
| PATCH | `/api/v1/bragbook/{id}` | Edit and review with an expected version |
| POST | `/api/v1/bragbook/collect` | Explicit scan of existing selected work |
| POST | `/api/v1/bragbook/export` | Selected accepted entries as Markdown or JSON |

All routes require the authenticated user. Personal entries are not exposed to scoped agent credentials. Plugin configuration remains available through `/api/v1/plugins/thesaurus.bragbook` and the existing plugin CLI.

## Bounds and persistence

Collection writes entries, immutable version history and workspace events atomically. The durable outbox cursor advances in the same transaction as automatic collection. Restart resumes from that cursor; enabling skips events from disabled periods. Existing work is scanned only through the explicit action.

The first version supports 10,000 entries, 10 citations per entry (2,000 bytes per source line), a 300-byte title and 4,000 bytes each for contribution and impact. An entry is capped at 32 KB. A source revision contributes at most 100 marker candidates; the explicit scan reports skipped marker lines. Backfill accepts at most 200 documents, 1,000 completed tasks and 1,000 confirmed deliveries; narrow the saved selection if those limits are exceeded. Lists return 1–50 entries per page.

Automatic collection is deterministic. Broader accomplishment inference, semantic merging, attachments and model-written narratives remain follow-up work.

## Verification

The core/service suite passed 96 tests with one existing opt-in Keychain test skipped. Nine focused Bragbook tests cover exact evidence, durable deduplication and restart, review versions and operation keys, selected accepted exports, source revocation, disabled periods, reopened tasks and deliveries, linked-task deduplication and rollback. The public source destination controls were checked separately after integration. Wasm and desktop Rust checks, formatting and the optimized web build pass. An isolated daemon/CLI check verified collection, review, JSON export with mode `0600`, and a repeat scan with no duplicate entries.

An isolated Arc walkthrough verified exact citation navigation, edited contribution and impact, the acceptance checkbox, acceptance and dismissal, the empty review state, collection settings, duplicate-free backfill, automatic collection after a source revision, and both browser downloads. Final captures at 1440×1200 and 390×844 confirmed a scoped fix for long accomplishment titles inheriting a no-wrap rule. The desktop capture includes the full review form; lower phone controls and the export pane have no separate visual capture because the computer tool later lost Arc's window handle. Verification artifacts are retained under the ignored `.local/bragbook-verification/` and `.impeccable/review/` directories.

An independent finish review accepted the final desktop, mobile and 1470-pixel user-width captures with no material fixes remaining. Its assessment of the export pane relies on source and functional evidence.
