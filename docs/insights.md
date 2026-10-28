# Opportunities, outcomes and personal capacity

The **Insights** interface selects a project, then offers Opportunities, Outcomes and Capacity. All three operate locally on the selected project's permitted evidence. Creating drafts, reviewing suggestions and saving planning scenarios require the user. These features invoke no model and send no messages or calendar changes.

## Recurring friction and reviewed opportunities (F26)

Enter a topic to find provisional evidence groups. The local discovery pass searches exact topic text and explicit friction markers across project documents. It groups at most 20 supporting source lines from at least two distinct source documents, and searches for matching resolution/improvement language as counterevidence. Identical normalized source snapshots and repeated supporting passages across sources are rejected as copied origins. Matching words do not prove that other origins or causes are independent, that the claims describe the same problem, or that an opportunity exists. The interface labels this uncertainty explicitly.

Edit the title and hypothesis. Add or remove exact supporting lines and counterevidence by selecting a project document and line number. Select affected tasks. Saving requires evidence from at least two distinct sources and validates every citation against the pinned original revision. The saved candidate remains unverified until the user accepts or rejects it with a review note. Acceptance additionally requires the user to attest that the evidence has independent origins after reviewing supporting and contrary evidence; this attributed, dated attestation is retained in history. Earlier accepted records without that attestation are labeled “Independent origins unverified” and can be reviewed again. User acceptance records a judgment; it does not automatically create work or make an external change.

Candidates and subsequent decisions retain versioned history. Evidence is capped at 20 supporting and 20 counterevidence lines of 2,000 UTF-8 bytes each. Discovery reports skipped lines and considered documents. Accepted opportunities preserve counterevidence alongside supporting claims.

## Cited outcomes and loose ends (F27)

Choose an inclusive period of 1–31 UTC dates, ending no later than today. Save a draft with:

- **Completed work:** actual recorded task transitions into `completed` during the period, with the original task evidence.
- **Changed decisions:** added and removed decisions from brief revisions accepted during the period, with their cited evidence.
- **Blockers:** explicit `Blocker:`, `Blocked:` and `Risk:` source lines from the last captured revision available at the period boundary.
- **Unresolved commitments:** accepted task state and standalone ledger promises that were open or proposed at the period boundary. Ledger state is reconstructed from immutable events before the exclusive next-midnight UTC boundary. Original promise and amendment citations, title/owner/deadline/status and supporting history are pinned. A linked accepted task is represented once by its unresolved ledger promise.

Source revisions are pinned when the draft is saved. A historical period never uses a later source revision as though it existed earlier. An end date of today includes only history available through draft creation. Review the evidence and mark the draft reviewed, rejected, or draft again with a note.

Task snapshots and status changes are retained in `insight_task_history` in the same transaction as task changes. Initial extraction, explicit status changes, source supersession and user source corrections record their origin. Existing tasks receive a migration baseline; earlier completion/acceptance dates are unknown and are never invented. Periods preceding that baseline can have incomplete task history. Draft notes describe this limitation and section omissions. Each section is limited to 100 items, ledger snapshots to 100 (with explicit omissions), recorded task transitions to 500, and a persisted draft to 1 MB. The current project source scope determines included work; historical project membership is not reconstructed.

## Personal capacity (F28)

1. Estimate personal effort for accepted tasks in minutes, with assumptions.
2. Explicitly select the accepted work to include in this review.
3. Choose a 1–31 day period and the fixed UTC offset that defines its date boundaries.
4. Add actual working-hours intervals with timezone-aware timestamps.
5. Add calendar commitments manually or import iCalendar occurrences, then select which events should consume capacity.
6. Save the plan to compare working time, selected calendar time, free time and estimated personal effort.

The planner merges overlapping working windows and calendar events. An event consumes only its intersection with working hours and the review period. Overlapping meetings count once for time consumption and are separately counted as scheduling conflicts. Task deadlines are compared against cumulative estimated work due by that date and the free time available before that deadline. Tasks without estimates remain unknown; displayed effort is a lower bound. Tasks due beyond the period are excluded from its load; accepted work without a deadline remains in total effort.

When known effort exceeds capacity, the plan proposes specific scope-reduction or delegation scenarios for user review. These proposals use arithmetic and task order; they do not establish task importance, ownership suitability or feasibility. **Edit with proposal** copies the proposal into an editable version of that plan. You can also construct a tradeoff manually: reduce estimated scope, defer beyond the review period or delegate personal effort. Saving recalculates the scenario. The actual task deadline/owner and all external calendars remain unchanged.

Each saved version pins its effort estimates, selected task snapshots, working windows, calendar commitments and tradeoffs. Editing with a stale plan version is rejected. Plans are capped at 500 selected accepted tasks, 100 working intervals, 500 calendar events, 500 tradeoffs and 1 MB of serialized data.

### iCalendar import

Import follows a bounded subset of [RFC 5545 content lines, date/time values and events](https://www.rfc-editor.org/rfc/rfc5545). Folded lines and escaped summaries are decoded. UTC events are supported. Floating times and all-day date values use the explicitly selected fixed offset; all-day end dates are exclusive. Transparent events begin unselected; cancelled events are omitted. Imported alarms are never run.

The importer explicitly rejects named non-UTC `TZID` values, recurrence properties (`RRULE`, `RDATE`, `EXDATE`, `RECURRENCE-ID`) and duration-only timed events. Export concrete UTC occurrences with explicit start/end times or enter them manually. Unsupported recurrence is never silently discarded. A fixed offset does not implement daylight-saving transitions; verify each interval across an offset change. Import is capped at 200 KB, 20,000 unfolded lines, 8 KB per property and 500 events. No calendar account is connected.

## CLI

```sh
thesaurus insights discover PROJECT_ID 'release handoff'
thesaurus insights opportunities PROJECT_ID
thesaurus insights create-opportunity PROJECT_ID --file opportunity.json
thesaurus insights review-opportunity PROJECT_ID OPPORTUNITY_ID \
  --expected-version 1 accepted --attest-independent --note 'Reviewed both independent sources and counterevidence'
thesaurus insights create-weekly PROJECT_ID --start 2026-10-12 --end 2026-10-17
thesaurus insights weekly PROJECT_ID
thesaurus insights get-weekly PROJECT_ID OUTCOME_ID
thesaurus insights effort PROJECT_ID TASK_ID --expected-version 0 \
  --minutes 120 --note 'Includes review and preparation'
thesaurus insights efforts PROJECT_ID
thesaurus insights import-calendar --file selected.ics --utc-offset-minutes=-240
thesaurus insights save-capacity PROJECT_ID --file capacity.json
thesaurus insights capacity PROJECT_ID
```

Opportunity and capacity files use the typed `OpportunityCreate` and `CapacitySave` contracts. CLI file inputs are capped at 1 MB. A missing idempotency key is generated by the CLI. Capacity requests include `selected_task_ids`; calendar entries include an explicit `selected` flag.

## API

All project routes begin `/api/v1/projects/{id}`.

| Method | Route suffix | Purpose |
| --- | --- | --- |
| POST | `/opportunities/discover` | Preview provisional evidence grouping for a topic |
| GET / POST | `/opportunities` | List recent candidates or save a candidate |
| PATCH | `/opportunities/{record}` | Explicit user review with expected version |
| GET / POST | `/outcomes` | List recent drafts or create a period draft |
| GET / PATCH | `/outcomes/{record}` | Read or review a saved draft |
| GET | `/efforts` | Current project task estimates |
| PUT | `/efforts/{task}` | Versioned effort estimate |
| GET / POST | `/capacity` | List or save versioned capacity plans |

Opportunity acceptance requires `attest_independent: true` in its review body; the CLI flag is `--attest-independent`. `POST /api/v1/calendar/import-preview` returns a calendar preview. Opportunity lists contain the latest 50 entries; outcome and capacity lists contain the latest 30. Source permissions apply to project scope and stored historical scopes; selecting a project never grants access to its evidence. Mutations use optimistic versions, durable operation receipts and atomic workspace events.
