# Project focus: priorities and waiting for responses

F23 and F24 are available in **Focus** (key `9`). Select a project created in Briefs, then choose **Priorities** or **Waiting for**. All scoring, comparisons and follow-up suggestions run locally without a model request. The page refreshes on workspace changes and once a minute while open; **Refresh · local** is also available. Date boundaries use UTC.

## F23: Priority changes explained

Priorities rank **accepted** tasks from the selected project's sources. Candidate actions remain in Work until the user accepts them. A task in overlapping projects can have a different importance preference in each project.

Select a row to see its original evidence, score components, changes since the saved review, and other tasks that affected its relative position. Set an importance preference and optionally explain it in your own words. Preferences remain separate from source facts.

### Scoring rule, version 1

The score adds these components:

| Signal | Points |
| --- | --- |
| Recorded deadline overdue | +60 |
| Due today | +50 |
| Due in the next 1–3 days | +35 |
| Due in the next 4–7 days | +20 |
| Due later | +5 |
| No valid explicit deadline | 0 |
| Importance: low / normal / high / critical | 0 / +10 / +25 / +40 |
| A linked waiting request has a follow-up due | +20 |
| Linked waiting requests exist, but all follow-ups are in the future | −20 |
| No active linked waiting request | 0 |
| Source evidence changed | 0; flag for review |

Normal importance is the default. Waiting points apply once per task, not once per request. A due follow-up changes the next-step cue to **Follow up**; it does not claim that the underlying task can proceed. A changed source does not establish new urgency. Accepted task deadlines remain the recorded inputs and are flagged when their evidence is stale. Invalid dates receive no urgency points.

Ties use the earliest valid due date, then the stable task ID. This is a visible rule, not an inferred assessment of business value or a model-generated confidence score.

### Review baselines

**Mark reviewed** saves an immutable snapshot of all accepted project tasks, their positions, inputs, preferences, source revisions, algorithm version and UTC date. Reading or refreshing a board does not move the baseline.

Later comparisons distinguish:

- A task newly accepted since the review.
- A change in that task's deadline bucket, importance input, waiting state or source revision. Each changed scoring component shows before/after values and its point difference. A user note or source revision can change without changing the score.
- Relative-order changes caused by other accepted work entering, leaving, or moving ahead/behind. Explanations identify the other task, its score changes where applicable, and its evidence. Up to five such influences are shown per row.
- Previously accepted tasks that left the list, with their current recorded status. Completion is never inferred from source text.

**Review history** loads the latest 30 saved review summaries. Older snapshots remain retrievable by ID. Historical scores and citations are labeled as historical. The initial project limit is 500 accepted tasks; create a narrower project when that limit is exceeded. Task quotes over 1,000 UTF-8 bytes are omitted from priority summaries with an explicit reference to the original revision and line.

Saving a review rechecks its signature and expected previous review in the same transaction. The signature covers the UTC date, source revisions, tasks, preferences, active waiting state and baseline. A stale page must refresh rather than silently save an older ranking. Preference edits have their own optimistic versions. Mutation idempotency keys make uncertain retries safe.

## F24: Waiting-for queue

Use **Track a waiting request** from a priority row, or create a standalone request in the queue. Supply:

- What you are waiting for and the person or team as you know them. Names are explicit user labels; they are not matched to Slack accounts or other identities.
- The date you requested it and your chosen interval of 0–365 days before a follow-up.
- An accepted task link, which supplies its original evidence, or an exact source revision and line from a project source.
- Optional context as a user-attributed note.

The queue lists requests, people, follow-up dates and state. Filter by waiting, response received, cancelled, or all; use **Follow-up due** to focus on eligible requests. Pages contain up to 25 records in the interface.

When a waiting request reaches its date, the app proposes a short editable follow-up. Review and send it using your own tool. This feature sends no Slack message, email or other outbound communication.

### Updating a request

- **Edit request:** correct its label, person or next date.
- **Snooze follow-up:** choose a future follow-up date.
- **Record a follow-up I sent:** record your own confirmation and choose the next future date. The timestamp is when that confirmation is recorded; it is not proof that a message was delivered.
- **Response received:** record your confirmation and optional supporting evidence.
- **Cancel request:** close a request that no longer needs a response, separately from a received response.
- **Reopen request:** restore a closed request with a chosen follow-up date.

An optional note and source citation can accompany any update. Added citations must belong to the project. Their sources join the request's permission scope so later reads cannot disclose historical evidence to an agent missing that access. Original request evidence stays pinned. Citation validation checks exact revision, line and quote; the user checks that the passage supports the request or response.

Updates retain attributed history. Reads include the latest 30 events within a 60 KB response budget; older events are retained in storage with an omission count but do not yet have a separate paginated history endpoint. Each mutation checks the record version. Snoozes and recorded follow-ups require a date after today; reopening may use today. Linked tasks remain independent: resolving a waiting request does not complete a task, and completing a task does not silently close its waiting requests.

Dates are explicit UTC calendar dates. Creation accepts a past or current request date and a follow-up 0–365 days after that date, so an already overdue request can be entered. Rescheduling supports dates up to a year ahead. There may be at most 1,000 active waiting requests per project. Each request uses 1–6 original citations with complete lines of at most 2,000 UTF-8 bytes. The app validates source locators but does not infer that a response has arrived.

## Shared context and access

Thread resume context now includes permitted waiting requests and explicit importance preferences for its tasks. They share the existing byte budget and have separate omission counts. Up to the earliest 20 waiting requests are considered before fitting the bundle. Changes to waiting state, preferences or the UTC date invalidate unused resume previews. Already delivered context receipts remain historical.

Recurring meeting preparation also shows active project waiting requests, due follow-ups and original request evidence. It keeps its existing overall response budget and reports any waiting records omitted to fit. This is current queue state, not a reconstruction of waiting status at the meeting date.

New MCP tools:

- `memory_priorities`: project ranking and explanations; requires every current project source, because relative ranks can disclose other work. Agent pages default to 25, maximum 50, and shared responses are capped at 300 KB. Ranking is computed across the whole project before paging. Inspect `total_items`, `offset`, `items_omitted` and `departed_omitted`.
- `memory_waiting_for`: paged waiting requests in a project. Only records whose entire original and historical source scope is permitted are returned or counted. Shared responses are capped at 300 KB and can contain fewer items than the requested limit; advance the offset by the number returned.
- `memory_waiting_get`: one permitted request and its recent user-attributed history.

There are 22 memory tools in total. Reconnect existing MCP clients after rebuilding to discover them. Source sharing flags and grant revocation apply on each new request. Agents can read permitted priorities and waiting context, but cannot change importance, mark a review, or update waiting state. User-only routes retain browser CSRF/session and native Unix-socket protection.

## CLI and API

Use the running daemon's data directory. Replace versions and signatures with current values returned by the CLI.

```sh
thesaurus priority list PROJECT_ID
thesaurus priority preference PROJECT_ID TASK_ID --expected-version 0 \
  --importance high --note 'Required before the release review'
thesaurus priority review PROJECT_ID --signature CURRENT_SIGNATURE \
  --expected-review-id PREVIOUS_REVIEW_ID --idempotency-key review-1
# Omit --expected-review-id for the first review.
thesaurus priority history PROJECT_ID
thesaurus priority get-review PROJECT_ID REVIEW_ID

thesaurus waiting create PROJECT_ID 'Deployment approval' --person Taylor \
  --task TASK_ID --requested-on 2026-10-17 --follow-up-on 2026-10-20
thesaurus waiting list PROJECT_ID --due-only
thesaurus waiting get WAITING_ID
thesaurus waiting update WAITING_ID --expected-version 1 follow_up \
  --follow-up-on 2026-10-23 --note 'I followed up in the project channel'
thesaurus waiting update WAITING_ID --expected-version 2 resolve \
  --note 'Taylor confirmed the approval'
```

Standalone requests and updates accept `--evidence-json` with an array of exact `document_id`, `revision`, `line`, `quote` locators. All mutations accept an optional `--idempotency-key` to reuse on an uncertain retry.

| Endpoint | Purpose |
| --- | --- |
| `GET /api/v1/projects/{id}/priorities` | Board with `offset` and `limit`; user default 500, agent default 25. |
| `PATCH .../{id}/priorities/tasks/{task}` | User importance/note, preference `expected_version`, idempotency key. |
| `GET/POST .../{id}/priorities/reviews` | User review history / save a signature-checked baseline. |
| `GET .../{id}/priorities/reviews/{review}` | User-only historical snapshot. |
| `GET /api/v1/projects/{id}/waiting` | Permitted records; `status`, `due_only`, `offset`, `limit` (1–50). |
| `POST /api/v1/waiting` | Create an evidence-backed request. |
| `GET/PATCH /api/v1/waiting/{id}` | Read permitted detail / apply a versioned user update. |

Schema version 5 adds priority preferences, immutable priority reviews, waiting records and attributed waiting history. Existing workflow idempotency and change events are reused. No credential or external connector configuration changes are needed.

## Implementation checks and boundaries

Daemon, web and macOS desktop builds are the checks for this slice. Automated tests, browser interaction checks and live provider requests were not run. The existing MCP tool-count expectation was updated to 15. The new behavior therefore has compilation evidence, not an end-to-end verification result.

Later extensions include configurable scoring weights, workspace-wide ranking, richer identity mappings, calendar-aware capacity, connector-backed response detection, notifications, paginated old waiting history, and user-approved outbound actions.
