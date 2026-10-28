# Similar Projects and Product Opportunities

Research date: October 16, 2026.

Fifteen adjacent projects show useful patterns for Thesaurus: local meeting capture, searchable work history, company retrieval, durable agent memory, and interchangeable agent runtimes. The strongest direction is to combine source evidence and commitment tracking with memory that existing agents can use.

The [feature backlog](feature-backlog.md) turns these findings into 40 concrete ideas, including existing requirements and proposed extensions. New ideas remain candidates for prioritization; this research does not add them all to the MVP.

## Research scope

The findings below come from project-maintained repositories, READMEs, and official documentation. They describe documented capabilities rather than independently measured quality or performance. Repository moves and product names reflect the sources available on the research date. Commercial products are included where their workflows are directly relevant.

## Meeting capture and work history

| Project and primary source | Documented pattern | Application to Thesaurus |
| --- | --- | --- |
| [Anarlog, formerly Hyprnote](https://github.com/fastrepl/anarlog) | Device audio capture without a meeting bot, local SQLite/files, on-device transcription where supported, and independently configurable transcription and intelligence providers. Its desktop stack includes Tauri and Rust. | Study capture lifecycle, provider selection, transcript storage, and local export. Keep audio transcription separate from reasoning. |
| [Screenpipe](https://github.com/screenpipe/screenpipe) | Continuous local screen/audio history, accessibility/OCR capture, search, agent access, and a Rust service/capture stack. Its README distinguishes local capture from optional off-device services. | Offer selected activity context as an optional future source. Adopt visible capture health and clear destination settings; avoid making full desktop recording a prerequisite. |
| [Granola MCP](https://docs.granola.ai/help-center/sharing/integrations/mcp), [briefs](https://docs.granola.ai/help-center/taking-notes/pre-meeting-briefs), and [recipes](https://docs.granola.ai/help-center/getting-more-from-your-notes/recipes) | Meeting notes and transcripts exposed to other agents through scoped MCP access; meeting preparation and reusable analysis prompts. | Make meeting history useful inside existing agents, prepare recurring-meeting briefs, and save repeatable analyses. Granola's documented brief availability has calendar, plan, and meeting-type limits; Thesaurus's proposed internal-meeting use is an extension. |

## Company knowledge and work continuity

| Project and primary source | Documented pattern | Application to Thesaurus |
| --- | --- | --- |
| [Onyx](https://github.com/onyx-dot-app/onyx) | Indexes connected knowledge with metadata and permissions; supports retrieval, scoped agents, external actions, and an MCP server for other agents. | Build a normalized local index and reuse it across interfaces. Keep source permissions attached to search results and derived context. |
| [Glean connector architecture](https://docs.glean.com/connectors/connectors-power-glean) | Indexed, live, and hybrid retrieval, with permission filtering and connector-specific freshness mechanisms. | Give sources a retrieval policy: index stable knowledge, fetch volatile or sensitive data when needed, and show freshness. This can reduce unnecessary backfill and stale answers. |
| [Khoj](https://github.com/khoj-ai/khoj) | Personal knowledge search across document formats, local/cloud model choice, customized agents, and scheduled research/notifications. | Add saved research questions and briefings driven by active work. Schedule only useful changes and apply user budgets. |
| [Threadline by ADIITJ](https://github.com/ADIITJ/threadline) | Groups captured work events into project threads, exposes resume/handoff tools through MCP, and tracks commitments. Its README describes local capture and reversible cleanup. | Add work threads that can be resumed across agents. Treat commit/task correlation as evidence for review; do not infer completion solely from a matching commit message. Evaluate this smaller project as a design reference, without assuming production maturity. |

## Agent memory

| Project and primary source | Documented pattern | Application to Thesaurus |
| --- | --- | --- |
| [Mem0](https://github.com/mem0ai/mem0) | Persistent user, session, and agent memory, personalization, and time-aware retrieval. The repository distinguishes managed benchmark results from the open-source implementation. | Use explicit memory scopes and remembered working preferences. Evaluate retrieval on Thesaurus data rather than inheriting a vendor's benchmark ranking. |
| [Letta Code](https://github.com/letta-ai/letta-code) | Persistent agent identity, searchable conversations, memory blocks, and Git-backed MemFS context. The [older Letta repository](https://github.com/letta-ai/letta) now points to this implementation. | Make important memories inspectable, versioned, and reversible. Keep each agent's private working state separate from deliberately shared work facts. |
| [Graphiti](https://github.com/getzep/graphiti) | Facts and relationships with temporal validity, source episodes, incremental updates, historical queries, and hybrid retrieval. | Record both when a fact was observed and when it was valid. Support questions such as who owned a project at the time of a decision. Implement the required semantics in SQLite before deciding whether a graph service is needed. |
| [Hindsight](https://github.com/vectorize-io/hindsight) | Retain/recall/reflect operations, isolated memory banks, evidence-backed observations, and maintained knowledge pages that can be read without new inference. | Distinguish raw evidence from derived observations and cache useful project briefs. Refresh only when their supporting evidence changes, preserving provenance and scope. |

## Existing agents and execution

| Project and primary source | Documented pattern | Application to Thesaurus |
| --- | --- | --- |
| [Goose](https://github.com/aaif-goose/goose) and its [ACP provider guide](https://goose-docs.ai/docs/guides/acp-providers/) | Rust desktop/CLI/API agent, MCP extensions, multiple model providers, and Agent Client Protocol adapters that can pass extensions to an underlying agent. The guide lists adapter-specific session limitations. | Strong architectural reference for Rust integration. Evaluate ACP as a common runtime adapter, with explicit capability checks for resume, cancellation, permissions, and MCP forwarding. |
| [Serena](https://github.com/oraios/serena) | MCP tools for symbol-level code exploration and references, using language servers or a JetBrains integration. | Add semantic repository context by connecting an existing service before implementing a full code intelligence stack. Keep read access distinct from editing tools. |
| [Hermes Agent](https://github.com/NousResearch/hermes-agent) | Persistent memory, searchable past sessions, reusable skills, scheduled automations, and configurable model providers. | Preserve compatibility with the user's existing workflows. Learn repeatable procedures as reviewable recipe proposals rather than silently changing agent instructions. |
| [OpenClaw](https://github.com/openclaw/openclaw) | Locally owned state and credentials, messaging interfaces, and swappable model/agent harness plugins. | Keep work memory independent of the runtime performing a task. Make progress and context portable across chosen agents while keeping destination-specific access explicit. |

## Architectural findings

### Separate agent protocols from knowledge protocols

[Agent Client Protocol](https://agentclientprotocol.com/get-started/introduction) standardizes host-to-agent interaction and has an [official Rust library](https://agentclientprotocol.com/libraries/rust). MCP exposes tools and context. Thesaurus can use ACP to drive a supported agent and MCP to supply that agent's work memory.

Evaluate ACP in the foundation milestone alongside the existing app-server and structured CLI options. Select integrations according to actual capabilities and dependencies. An ACP adapter does not itself establish subscription eligibility, and some adapters require a non-Rust runtime. The provider authorization requirements already documented in the implementation plan still apply.

### Give memory a visible lifecycle

Temporal facts, versioned context, and maintained knowledge pages suggest three useful additions: inspect why a memory exists, show what changed, and refresh derived briefs when evidence changes. These are proposed combinations of the memory patterns above, not claims that any one product implements the full Thesaurus workflow.

### Track work through its outcomes

A meeting can create a commitment, a Slack conversation can change its scope, and a repository change can provide completion evidence. The opportunity for Thesaurus is a connected history of those events, with user review when identity, intent, or completion is uncertain.

That history can support follow-up suggestions, priority changes, and recurring pain-point discovery. Correlations should remain distinguishable from explicit task links and accepted decisions.

### Combine indexed context with selective live reads

Company knowledge benefits from a reusable index; current operational state may need a fresh fetch. Add source-specific policies and a freshness indicator rather than requiring every source to fit one ingestion schedule. A context bundle should identify which evidence was indexed, checked live, or unavailable.

### Keep optional capture optional

Meeting imports, selected repositories, and approved source APIs can provide immediate value. Activity recording is a separate feature with its own capture controls, resource use, and retention. Researching Screenpipe and Threadline supports evaluating this source; it does not establish that always-on capture is necessary for Thesaurus.

## Build and integration choices

| Area | Recommended approach |
| --- | --- |
| Work evidence, commitments, memory policy, and priorities | Build in the Rust core; these define the product's behavior and must share one consistent data model. |
| Meeting capture | Study Anarlog's community implementation and evaluate an external collector bridge before building every audio/transcription component. |
| Agent execution | Evaluate the ACP Rust library and supported adapters; retain provider-specific paths when capabilities differ. |
| Repository semantics | Integrate Serena or another approved code intelligence MCP first; develop a native index only for measured gaps. |
| Knowledge retrieval | Start with the planned SQLite/FTS5 design. Use memory projects as semantic references and optional future providers. |
| Existing meeting products | Prefer supported APIs/MCP over scraping private databases when the product offers the needed access. |

For source reuse, inspect the exact revision and module. [Anarlog documents an MIT community layer and commercial enterprise paths](https://github.com/fastrepl/anarlog/blob/main/LICENSING.md); [Screenpipe's current license is commercial](https://github.com/screenpipe/screenpipe/blob/main/LICENSE.md). Public source visibility alone does not establish reusable code. No code or new dependency is adopted by this research.

## Recommended experiments

The most useful additions to prototype after the core evidence path are:

1. **Living project brief:** maintain current goals, decisions, blockers, and unanswered questions with source links.
2. **Resume a work thread:** give a new agent a compact, reviewable account of where work stopped.
3. **Priority change explanation:** show the new evidence that moved an item up or down.
4. **Waiting-for queue:** track unanswered requests and propose a follow-up when appropriate.
5. **Meeting-to-delivery trace:** connect a commitment to discussion and implementation evidence without silently closing it.
6. **Recurring meeting preparation:** surface open threads and changes since the last meeting.
7. **ACP compatibility spike:** compare one supported adapter with the existing provider-specific integration path.
8. **Saved work recipes:** turn a useful repeatable analysis into an editable template with explicit sources and budgets.

The backlog specifies priority, size, existing-plan fit, dependencies, and observable outcomes for these and the other candidates.
