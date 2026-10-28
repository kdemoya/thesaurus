# Indexed and live retrieval through existing MCP servers

F08 adds explicit indexed/live/hybrid policy to configured upstream sources. The upstream client also provides a read-only way to reuse existing MCP servers and their accounts alongside the app's existing installed-agent/model connections (F29). It uses the [official Rust MCP SDK](https://docs.rs/rmcp/3.5.1/rmcp/transport/index.html), with actual stdio and Streamable HTTP clients.

## Configure a destination and read operation

In Retrieval, choose a project and configure one upstream mapping. Each mapping owns one project source and exactly one operation:

- a fixed `resource_uri`, or
- a user-approved `read_tool` with JSON arguments. `{query}` inside argument strings substitutes the retrieval query only when `allow_query_transmission` is true.

Multiple mappings/servers can cover company knowledge, selected Slack read/search tools, repositories, or other existing MCP sources. Configuration does not connect, discover account content or fetch anything. A new source is local-only by default; ordinary Source controls independently govern external agent access.

Transport choices:

- `stdio`: trusted absolute executable path and argument array, started without a shell. It can use its normal already configured server account/environment. The process runs with user privileges; the app does not scrape login stores, extract agent cookies or infer credentials.
- `http`: configured HTTPS MCP endpoint (HTTP only on loopback), optional `token_env` naming an environment variable in the daemon's environment. The SDK sends that value as bearer authorization only to the configured endpoint. Embedded URL credentials, query strings, fragments and HTTP redirects are rejected. No OAuth enrollment, credential scanning or automatic fallback is performed.

The client advertises no model sampling/elicitation capability and sends no workspace context bundle. It reads only the configured resource, or discovers the first upstream tool page and invokes exactly the configured tool with configured arguments. Tools explicitly advertising mutable/destructive behavior are rejected. User approval of a read-tool mapping is necessary when upstream annotations are absent; neither tool names nor server-supplied hints prove backend behavior. Configure trusted read operations only. Arbitrary agent-chosen upstream tool names and arguments are not accepted by the API.

## Source policies

| Policy | Ordinary indexed search/context | Explicit federated query |
| --- | --- | --- |
| `index` | Search previously imported snapshots only | Indexed results; never fetches automatically |
| `live` | Source excluded, including cached tasks/observations in general indexed context | Approved upstream fetch only; error is visible |
| `hybrid` | Search cached snapshots | Approved live fetch plus indexed results; live errors are displayed alongside any cached results |

Explicit Refresh selected index is available for all policies, including index-only sources. That is a separate approved read, not an implicit consequence of searching. User chooses a query and source IDs; the app does not send a query to every configured server.

Federated results label `index`, `live`, `live_receipt`, or `index_after_live_error`, source policy, fetch timestamp, cached freshness status, exact document revision and evidence lines. Indexed upstream snapshots have unverified current freshness; they are not presented as current remote state. A failed live request never silently falls back. Hybrid may return existing indexed results with the explicit live error. Plain local indexed results have no remote freshness requirement.

## Durable snapshots and limits

Successful reads import text/structured text into the connector's scoped source, create immutable document revisions and source line citations, update FTS and queue ordinary local candidate extraction. Derived observations/insights follow normal provenance rules. Each connector has one stable current cache document, matching the existing one-document-per-source schema. Different queries replace its current cache through immutable revisions; index searches use the latest snapshot, while earlier results remain accessible by their pinned revision. Each fetch receipt retains its query, actual substituted arguments, resource/tool mapping, connector version, destination hash and resulting document revision. This avoids duplicate documents for the same source.

Fetch attempts reserve an idempotency receipt and daily read allowance before connecting. Replaying a completed receipt returns its original document revision and reports that no new live request occurred. Failed/interrupted attempts are not retried automatically; explicitly use a new key. Per-mapping daily limit: 1–1000 reads (failed attempts count). Source permission is checked before contacting the server and again before importing its response. Scoped agents additionally need all project sources and `allow_agent_live`; external source permission alone does not enable upstream execution.

Bounds: 1–20 selected sources, 1–50 returned results, queries ≤1024 bytes, arguments ≤16KB, final upstream text ≤200KB, federated response ≤250KB with omission reporting. One request has a 35-second total upstream deadline. Stdio output is limited to 2MB for the process lifetime. A custom SDK HTTP backend enforces a 2MB bound while streaming JSON and error bodies, before parsing or accumulating more data. POST SSE response bytes are capped at 2MB total; background GET SSE events use the SDK’s 2MB raw-event cap. The final content check remains 200KB. HTTP calls have a 25-second client timeout and no redirects. No live upstream call was made during development.

Images, audio, blobs and other non-text blocks are omitted and counted. Text and structured-content blocks are retained as untrusted source evidence. Current snapshots are indexed; immutable historical revisions remain retrievable through ordinary exact-document memory access. Absolute executables, argument configuration, endpoints and token environment names are redacted from scoped agent configuration reads.

## CLI and API

```sh
thesaurus retrieve list
thesaurus retrieve save --file upstream.json
thesaurus retrieve get UPSTREAM_ID
thesaurus retrieve refresh UPSTREAM_ID 'rollout' --approve-upstream
thesaurus retrieve query 'rollout' --source SOURCE_ID --approve-upstream
```

Example `upstream.json` (existing local server):

```json
{
  "expected_version": 0,
  "project_id": "PROJECT_ID",
  "name": "Company knowledge",
  "transport": "stdio",
  "command": "/absolute/path/to/existing-mcp-server",
  "args": [],
  "endpoint": null,
  "token_env": null,
  "resource_uri": "docs://selected/resource",
  "read_tool": null,
  "tool_arguments": {},
  "policy": "hybrid",
  "allow_query_transmission": false,
  "allow_agent_live": false,
  "approve_upstream": true,
  "daily_limit": 20
}
```

For tool-only servers, set resource_uri null and read_tool to the exact read operation. Example arguments: `{ "query": "{query}", "limit": 10 }`; explicitly enable query transmission. HTTP mappings use transport http, command null, args [], configured endpoint and optional token_env. The daemon must be launched with that existing token environment variable; tokens never appear in requests saved to SQLite or UI forms.

| Method | `/api/v1` path | Purpose |
| --- | --- | --- |
| GET/POST | `/upstreams` | List permitted mappings / configure approved destination |
| GET/PATCH | `/upstreams/{id}` | Inspect / versioned configuration update |
| POST | `/upstreams/{id}/refresh` | Explicit source snapshot refresh |
| POST | `/retrieval` | Federated query using each selected source's policy |

Configuration and manual refresh require the user. Federated query accepts permitted scoped agents only when their source and live-execution controls allow it. Bodies use stable idempotency keys. No write-tool execution endpoint exists. Changes to the approved operation/destination require user approval and the current mapping version.

## Verification and current limits

The coordinator records aggregate compile results. No tests, authenticated server connections, company/Slack account reads or paid requests were performed. OAuth browser onboarding, full upstream resource/tool pagination, background synchronization and provider-side query billing caps are not implemented. The configured daily attempt limit is enforced locally; it is not a server monetary budget. Source policy is configured for upstream mappings; ordinary imported files remain indexed sources.
