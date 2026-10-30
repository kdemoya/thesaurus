# Work Agent Dashboard Assessment

Assessment date: October 16, 2026.

The legacy source files described here are not included in this repository; the assessment is retained as historical context.

The three directories preserve useful product behavior and extensive test cases, but the available files do not form a complete runnable application. Thesaurus should carry forward their workflows, provenance rules, and recovery requirements while building a Rust service that also owns ingestion and continuous analysis.

## Directory inventory

| Directory | Available evidence | Implication |
| --- | --- | --- |
| `work-agent-dashboard` | README, stylesheet, CLI and server tests, package initializer, and a Git index listing additional paths. | Baseline dashboard behavior and local security requirements. |
| `work-agent-dashboard-context` | Expanded README, 594 line HTTP server, 234 line Herdr adapter, HTML, context/Garden/implementation/server tests, retention and refresh scripts, and an earlier reuse plan. | Broadest specification of context ingestion, idea workflows, automation, and recovery. |
| `work-agent-dashboard-people` | 1,407 line JavaScript frontend, stylesheet, people/integration/server/frontend tests, package initializer, and retention script. | People attribution, evidence viewing, interaction patterns, and integration health behavior. |

The base directory's Git status reports staged additions with working tree modifications and deletions. Its current branch has no commits. A direct object lookup for the indexed `store.py` blob fails, so index entries alone do not establish that the missing source is recoverable. Neither sibling directory contains a top level `.git` entry in the inspected listing.

Missing working files include `run_dashboard.py` and the core `store`, `contexts`, `people`, `integrations`, `idea_planner`, `idea_implementer`, and Garden modules imported by surviving code and tests. Python bytecode caches are present; they are not a maintainable source baseline. The original folders were inspected without modification, and no legacy tests or services were run.

## Existing product behavior

### Work dashboard

The baseline README describes open/completed/overdue actions, meeting digests, ownership gaps, opportunities, approval drafts, follow-up briefings, and automation health. Actions retain provenance in the Markdown form `action — src: reference — due: date|none`.

The people frontend adds a command palette, keyboard action selection, a context inbox, people profiles, source links, highlighted conversation excerpts, and separate operational refreshes. These are useful interaction requirements for the new interface.

Evidence: baseline README, people frontend, and frontend expectations.

### HTTP and update lifecycle

The available server uses Python `ThreadingHTTPServer`, a source signature watcher, condition variables, and capped server-sent event connections. A change event tells clients to refresh the dashboard. Planner and implementer workers are threads owned by the HTTP server, polling at configurable intervals that default to 60 seconds.

The interface is therefore live with respect to source changes, while intelligence production depends on separate processing workflows. A Rust port of only this server would preserve that dependency.

Evidence: server, particularly `DashboardHTTPServer`, `_watch_events`, `_watch_ideas`, `_watch_implementations`, and `_serve_events`.

### Context ingestion

The context README and tests specify URL classification, canonicalization, deduplication, locked claims, blocked/retry states, normalized digests, and private raw snapshots with retention. Google Docs, Wizard artifacts, and ordinary web links have distinct extraction expectations. The documented processor delegates resolution to an external Hermes workflow.

The distinction between raw capture and durable interpretation should remain. Queue state and retention references should move into transactional storage, with compatibility importers for the existing Markdown envelopes.

Evidence: context documentation, context cases, and CLI expectations. The implementing context module is absent.

### Company knowledge

The context README describes a private SQLite/FTS5 Garden cache populated from a Git repository, exact commit provenance, bounded refreshes, routed retrieval, and relevant changes selected against active work. It reports a prior relevance gate favoring lexical retrieval; the underlying replay results are not included in these folders.

Preserve repository revisions, frontmatter, freshness, and targeted retrieval. Establish a new relevance baseline before adding embeddings.

Evidence: Garden documentation, dashboard expectations, and Garden CLI expectations. The Garden implementation modules are absent.

### People and ownership

People tests specify editable profiles, stable IDs, exact names and unique aliases, grounded observations with source and observation date, and open questions. Ambiguous names remain unresolved. Duplicate stable IDs and unsafe filesystem paths are rejected.

Carry forward explicit identity mappings and evidence dates. A speaker labeled “Them” should not become an inferred coworker identity.

Evidence: people cases and people interface. The profile parser is absent.

### Ideas and execution

The context README and implementation tests describe priority topics, idea dismissal, planning queues, isolated worktrees, retry recovery, permissions, review, and approval drafts. They describe different Codex and Claude/Herdr lifecycles: the former can produce reviewed local artifacts, while the latter hands control to an interactive session.

The surviving Herdr adapter validates response shapes and session identity and invokes the CLI with argv arrays. It is useful evidence for a future external execution adapter. Recreating autonomous coding should follow a useful ingestion and suggestion release because it introduces a large independent workflow.

Evidence: workflow documentation, implementation cases, and Herdr adapter. The planner and implementer modules are absent.

### Integration health

Integration tests specify bounded caching, asynchronous cold probes, normalized safe errors, stale heartbeat detection, and handling for MCP configuration and agent usage. Existing probes inspect external clients such as Hermes, Claude, and Codex. Client configuration or a successful discovery command does not establish that a particular tool call works.

Thesaurus should maintain its own MCP sessions and separately report configured, authenticated, connected, and last successful operation states. Optional telemetry should appear only when a provider reports it.

Evidence: integration cases, frontend expectations, and the context README. The integration monitor module is absent.

## Migration decisions

| Existing concept | Thesaurus treatment | Sequence |
| --- | --- | --- |
| Markdown todos and meeting digests | Import with stable source identity and evidence references; keep legacy files authoritative for imported records during coexistence. | First useful release |
| Context queue and digests | Import resolved records; explicitly reconcile pending work without executing it during import. | Ingestion milestone |
| People profiles and aliases | Import curated identities; retain unresolved ownership. | First useful release |
| Raw context cache | Inspect format when complete source is available; preserve retention policy and require an explicit import selection. | Ingestion milestone |
| Garden index | Rebuild from the canonical repository through a generic company knowledge adapter; preserve source revisions. | Knowledge milestone |
| Priority topics and dismissed ideas | Import into user preferences and durable suppression records. | Suggestion milestone |
| Outbox and approvals | Import as inert drafts with provenance; recreate execution authorization in Thesaurus. | Approval milestone |
| Planning and implementation envelopes | Retain as historical records first; add execution adapters later. | After MVP |
| Hermes cron IDs and machine paths | Move into private adapter configuration where still needed. | Optional bridge |
| Vanilla JavaScript interface | Reuse interaction requirements and visual cues in the shared Rust UI. | First useful release |
| Markdown lifecycle locks and macOS atomic swaps | Replace internal queue mutations with SQLite transactions; use source revision checks for optional file writes. | Foundation |

## Boundaries to preserve

The available server implements loopback Host validation, mutation origin checks and tokens, request size limits, restrictive response headers, static path confinement, and Markdown-only document reads. Tests and documentation add concurrent-write handling, safe filesystem access, output bounds, secret screening, and explicit approval before external actions.

Implement these requirements as shared policy in the Rust service so both interfaces enforce them. Imported documents, transcripts, repository text, and MCP results remain untrusted data. Source content cannot grant tool permissions.

## Earlier plan

The September reuse plan proposes provider contracts, configuration, normalized records, capability-driven navigation, and optional automation adapters. Those architectural ideas remain useful.

Its implementation assumes a complete Python project and includes historical line counts, dirty-state counts, and estimates. Those figures are not the current baseline. The Rust plan replaces its Python packaging and extraction phases with a new shared runtime, transactional ingestion, and a desktop shell.

## Main gaps relative to Thesaurus

1. The dashboard primarily consumes artifacts; the new assistant must own source synchronization and analysis scheduling.
2. Live display updates need durable processing jobs, resumable event delivery, and incremental transcript handling behind them.
3. Knowledge, identity, tasks, and suggestions need one consistent model across multiple sources.
4. Source edits, deletions, permission changes, and duplicate captures must invalidate derived results.
5. Suggestions need quality measurement, feedback, explanations, and notification controls.
6. Desktop lifecycle, keychain integration, background operation, packaging, and shared browser access need deliberate ownership.

A complete legacy source copy would improve compatibility work, but it is not a prerequisite for implementing the new core against synthetic fixtures and the surviving behavioral specifications.
