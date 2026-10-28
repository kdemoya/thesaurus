# Implementation status

Thesaurus 0.1 establishes a local capture-to-action workflow and shared agent memory. It uses one Rust service for desktop, browser, CLI, and MCP clients. All examples in `fixtures/` are synthetic.

## Working scope

| Area | Implementation |
| --- | --- |
| Capture | Paste text in the interface or import a regular UTF-8 file through the CLI. Stable references identify sources; changed content creates immutable revisions. |
| Storage | SQLite WAL, transactional revisions and events, FTS5, persistent job state and optimistic task versions. |
| Suggestions | Automatic local rules recognize checkboxes, explicit action lines, and first-person commitments. Owners and ISO dates are extracted conservatively; missing owners remain unresolved. |
| Review | Evidence links open the original revision with line numbers. Accept, dismiss, complete, and reopen actions. Source changes supersede unreviewed candidates while preserving accepted work. |
| Model analysis | Optional Claude Code and Codex subprocess adapters, or an OpenAI-compatible chat-completions endpoint. Structured action output must cite an exact source line. Explicit approval, source destination policy, bounded output, timeouts, and daily run quotas apply. |
| ACP runtime | Codex through ACP v1, scoped read-only memory, run progress, cancellation, and one-time permission controls. [Setup and live result](acp-integration.md). |
| Living briefs | Source-scoped projects, ACP-generated structured drafts, exact citations, explicit acceptance, revision comparisons, and conservative invalidation when any input changes. [Usage and limits](living-briefs.md). |
| Work continuity | Project threads with attributed checkpoints, transactional split/merge, bounded ACP resume snapshots and durable run provenance. [Workflows and limits](work-continuity.md). |
| Meeting preparation | Manually linked recurring occurrences, pinned transcript revisions, captured project baselines, unresolved commitments and deterministic source/task changes. |
| Project focus | Local priority signals and changes against saved reviews, task importance preferences, evidence-backed waiting requests, follow-up suggestions and attributed history. [Usage and limits](project-focus.md). |
| Shared memory | 22 MCP tools over stdio proxy requests to the private service socket. Source grants apply before retrieval. Observations retain contributor identity, evidence, and idempotency keys. Revocation applies to the next request. |
| Clients | Rust Leptos CSR in the browser and Tauri system webview; local API and CLI. Durable event sequence IDs support browser updates. |
| Packaging | A macOS app bundle includes the Rust daemon and browser assets. The daemon continues running when the window closes. |

## Data and access

By default, sources stay local. External model and agent access requires the source sharing flag and, for agents, a source grant. A `--local-agent` grant is an explicit destination declaration, not a network sandbox for that agent. Once an agent retrieves text, Thesaurus cannot control its subsequent handling.

The data directory is private to the OS user. SQLite content is plaintext on disk; use OS disk encryption for protection at rest. The daemon listens on IPv4 loopback and a private Unix socket. Browser access uses a single-use pairing code, an HttpOnly SameSite cookie, Origin/Host checks, and CSRF protection. Agent tokens are hashed in the database and stored in private grant files for client use.

Context bundles bound the complete compact JSON to the requested character budget, clamped to 500–16,000. Retrieval is lexical. The current bundle includes matching documents and a small set of active tasks and observations; semantic ranking and query relevance for tasks and observations are future work.

Source content and model output are untrusted evidence. Agents cannot overwrite sources or task decisions through MCP. Model subprocesses disable tools where supported and operate in a private working directory; this is not an OS security boundary against a malicious installed executable. Only configure trusted agent runtimes and endpoints.

## Verification and limits

Storage and service checks cover revisions, idempotency, immutable evidence, task conflicts, source permissions, revocation, pairing, CSRF, and quota reservations. A process integration test starts the daemon and two real stdio MCP clients, shares an observation, checks denied access and revocation, then restarts the daemon and checks persistence. Browser checks use only the synthetic local workspace.

F01–F42 have implementation mechanisms and recorded acceptance evidence. F43 adds live meeting audio and companion surfaces, with interactive acceptance pending; see [meeting companion](meeting-companion.md). Synthetic API, actual transport, browser and native walkthroughs cover the workflows listed in [backlog progress](backlog-progress.md) and [verification](backlog-verification.md). Fixed mock responses do not establish model, transcription or production connector quality.

The ACP Codex path completed one approved synthetic model request using the existing ChatGPT login. The CLI extraction and direct API adapters have not been exercised against a live model in this setup. Availability checks do not guarantee account entitlement, remaining quota, or provider schema compatibility. CLI adapters require the supported flags of the installed runtime. External model runs are never silently retried after a service crash because they may already have incurred usage.

The current implementation also includes selected Git history/reporting and approved LSP queries; upstream MCP stdio/HTTP retrieval; anchored notes and correction previews; temporal claims and curated people; discovery cues/gaps; commitment delivery traces; memory inspection/conflict review; saved recipes and reviewed procedure installation; request budgets and explicit fallback; local notifications and coverage; period outcomes and effort/calendar planning; versioned personal/project evaluations; scoped exports; local action receipts/undo; and built-in themes. Usage guides in this directory describe each workflow and its bounds.

Local transcription accepts an existing WAV through an installed whisper.cpp-compatible executable/model. Actual macOS selected-window PNG/OCR/search, native Pause, window-closure rejection, restart recovery and in-flight discard passed using only the synthetic test window. Vision hardware failure now retries locally on CPU; cold OCR can take around two minutes. Capture is opt-in, session-scoped and recovers paused after a service restart. The separate F43 implementation adds explicitly started microphone/system-audio meeting sessions; its interactive acceptance is pending.

The implementation does not include direct Slack/calendar account synchronization, filesystem watchers, semantic/vector search, automatic model-driven contradiction resolution, autonomous publishing, background skill installation, hosted multi-user sync, signed/notarized distribution, or Windows/Linux desktop packaging. Calendar planning imports the explicitly supported ICS subset; unsupported recurrence/timezone features reject rather than silently approximate. Local action providers support task status and retained-frame deletion, not external publishing.

## Acceptance scope

The authoritative checklist is [backlog progress](backlog-progress.md). The recorded F01–F42 backend concurrency/relevance, browser interaction/contrast and native/selected-window capture gates are closed; F43 has a separate pending interactive acceptance record. The packaged app also cold-started its own service on a free loopback port while the existing development service remained available. Live account entitlement, provider quality and other OS/toolchain runtime combinations remain separate from this evidence.
