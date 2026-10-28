# Quota and fallback controls (F30)

**Connections → Usage budgets** shows the current connection request reservations, interactive capacity and background capacity. Limits cover extraction jobs and ACP runs together for the current UTC day. They are conservative attempt limits, not provider token, credit or currency meters. A failed/interrupted attempt can already have incurred usage. New extraction attempts retain distinct reservation/start days, so retrying an old job does not move all previous attempts into today. Legacy attempt dates that predate this ledger are unknown and conservatively assigned to their last recorded update day during migration.

Set a daily limit no greater than the configured connection cap. Reserve some requests for interactive work: background extraction cannot reserve those requests, while user-started ACP analysis can use the whole daily allowance. Pause background model jobs to keep previously queued work pending. Local deterministic extraction continues. Pausing does not cancel a provider request already in flight; use its explicit cancel control where supported.

Policy changes are versioned, persisted and checked during request reservation and again before queued extraction starts. Reducing a limit below existing reservations blocks additional usage and causes over-budget queued jobs to fail before a provider call. There is no automatic retry after a crash.

## Explicit fallback

An optional rule identifies one other configured ACP connection. A rule does not execute a fallback. After a primary run fails, is cancelled or is interrupted:

1. Enter its run ID in **Explicit fallback after a stopped run**.
2. Inspect the chosen account, exact previous prompt and source scope, input estimate and policy version.
3. Explicitly approve a separate request on that account.

The source destination policy and selected fallback connection's own limit apply. Changed rules or source policy invalidate the preview. The new runtime has a durable link to its primary and approved fallback preview. No automatic fallback chain is performed. An account rule is not evidence of entitlement or remaining provider credits.

## CLI/API

```sh
thesaurus usage list
thesaurus usage set CONNECTION_ID policy.json
thesaurus usage fallback-preview PRIMARY_RUN_ID
thesaurus usage fallback-run PRIMARY_RUN_ID --signature PREVIEW_SIGNATURE --approve-usage
```

Policy JSON: `expected_version`, `daily_limit`, `interactive_reserve`, `background_paused`, nullable `fallback_connection`; CLI supplies `idempotency_key` if absent. The API requires it.

- `GET /api/v1/usage`
- `PUT /api/v1/usage/{connection}`
- `GET/POST /api/v1/runtime/runs/{primary}/fallback`

All controls require the user client. Agent source grants cannot change account budgets or approve fallback. Local API endpoints may have zero provider cost, but still consume their configured attempt quota when selected for model processing.

Verification is tracked in [backlog verification](backlog-verification.md). No live account call is necessary to inspect or change these controls.
