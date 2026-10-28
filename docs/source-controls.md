# Source health, disclosure controls and explained notifications

## Source health and coverage

The **Sources** tab provides health and disclosure controls for a selected source. Health shows the last local ingestion time, current documents with completed extraction, pending and failed current-revision jobs, and how missing coverage can affect answers. A last ingestion timestamp describes the captured local snapshot; it does not establish upstream completeness or continuous capture.

Set an expected update interval in hours to flag old snapshots. Record excluded content and capture gaps with dates, timezone-aware timestamps and descriptions. These are declarations of content missing from the available evidence, rather than commands to erase or filter captured text. Coverage settings and each revision are retained durably.

**Retry local extraction** targets a document in the selected source. It retries a failed current-revision local job, returns an already queued/running/completed job without duplicating it, and does not invoke a model or recover data that was never captured. A remote provider requires the separate explicit Analyze flow and usage approval.

## Data destinations

The disclosure preview lists the source URI, document IDs and known configured model connections and scoped grants. It explains that source documents, task citations and observations can be delivered by an explicitly requested model operation or retrieved through a scoped grant.

Apply a policy to enable or disable external disclosure for this source and deny particular connections or grants. Local capture and indexing remain available. Per-connection restrictions also apply to local model endpoints; the built-in `local` extraction rules remain available. Revoking a grant for one source does not affect its access to other permitted sources.

Policies are checked at ACP reservation, again before launching the provider, immediately before extraction, and during every authenticated grant request. ACP memory grants also inherit restrictions on their originating connection. Queued operations cannot bypass a subsequently withdrawn policy. Policies are independent of ingestion: reimporting a source cannot override a saved external disclosure denial. They supplement existing explicit request approval, source selection and grant restrictions.

Revocation blocks future delivery and retrieval. Content already delivered to another process or provider cannot be retracted. An operation already in progress may possess that content.

## Explained notifications

The **Work** tab lists durable local notifications for:

- Cited action candidates awaiting acceptance.
- A new captured source revision that can make older context stale.
- A failed extraction for the current document revision.
- A waiting request whose user-selected follow-up date has arrived in UTC.

Each notification shows **why now**, the triggering date, source scope, relevant record and cited evidence. These are deterministic local rules and require no model. Follow-up reminders send no outbound messages. Notifications whose triggering condition no longer applies disappear from the current queue while their stored record remains intact.

Mark an item read or dismiss it. Mute similar notifications by kind, source or both, either indefinitely or until a future timestamp. A mute hides matching existing items and prevents matching new notifications from being inserted while active. Unmuting restores the current queue and can surface a still-relevant condition that began during the mute. Stable trigger IDs prevent duplicates across restarts and observation reconnects. A due waiting reminder is created once per request and chosen follow-up date; snoozing gives it a new trigger.

The UI refreshes every 30 seconds and on workspace events. Reconciliation can also run in the daemon and on a notification GET. Listing is limited to 100 currently relevant notifications from the latest 1,000 stored records. No operating-system push notification permission is requested by this implementation.

## CLI

```sh
thesaurus source-controls health SOURCE_ID
thesaurus source-controls disclosure SOURCE_ID
thesaurus source-controls policy SOURCE_ID --expected-version 0 \
  --deny-connection codex --deny-grant GRANT_ID
thesaurus source-controls policy SOURCE_ID --expected-version 1 --external-enabled
thesaurus source-controls coverage SOURCE_ID --expected-version 0 \
  --expected-interval-hours 24 --excluded 'Private channels were not captured'
thesaurus source-controls retry SOURCE_ID DOCUMENT_ID
thesaurus source-controls notifications
thesaurus source-controls notification NOTIFICATION_ID --expected-version 1 read
thesaurus source-controls mute --expected-version 0 --kind waiting_due \
  --source SOURCE_ID --active --until '2026-10-18T09:00:00-04:00'
thesaurus source-controls mutes
```

`policy` replaces the complete per-source deny lists. Include every connection/grant that should remain denied. Omit `--external-enabled` to disable external disclosure. `coverage` replaces the complete coverage settings; capture gaps can be supplied with `--gaps-json`. A mute without `--active` deactivates the matching rule. Existing rules require their current expected version. All changes accept an explicit idempotency key.

## API

| Method | Route | Result |
| --- | --- | --- |
| GET | `/api/v1/sources/{id}/disclosure` | Policy, source scope and configured destinations |
| PUT | `/api/v1/sources/{id}/policy` | Replace disclosure policy with expected version |
| GET | `/api/v1/sources/{id}/health` | Current freshness and coverage explanation |
| PUT | `/api/v1/sources/{id}/coverage` | Replace expected cadence, exclusions and gaps |
| POST | `/api/v1/sources/{id}/retry` | Target current local extraction with `document_id` |
| GET | `/api/v1/notifications` | Reconcile and list current local notifications |
| PATCH | `/api/v1/notifications/{id}` | Set `unread`, `read` or `dismissed` |
| GET / PUT | `/api/v1/notification-mutes` | Read or update mute rules |

Source-health reads honor existing scope permissions. Policies, coverage changes, retry, notifications and mutes require the user. Policy/coverage updates and notification decisions use optimistic versions, idempotency receipts and workspace events. `source_control_` tables are additive and retain policy and coverage history.
