# ACP integration

Thesaurus can run an existing agent through Agent Client Protocol v1 with a selected set of read-only memory sources. Runs are visible in Activity, with streamed text, progress, cancellation, and one-time permission requests. The first live integration uses Codex through its existing ChatGPT login.

## Set up Codex

Install the pinned adapter inside this repository. Node/npm is needed for this external adapter; the Thesaurus client, service, storage, and interface remain Rust.

```sh
./scripts/install-acp.sh
codex login status
```

Use Codex's own login flow if it is signed out. Thesaurus does not copy tokens or open an authentication browser automatically. Add this block to your workspace's private `config.toml`, replacing the command with the absolute path to this checkout:

```toml
[[connections]]
id = "codex-acp"
kind = "acp"
command = "/absolute/path/to/thesaurus/scripts/codex-acp.sh"
args = []
daily_limit = 3
```

Restart the daemon. In **Connections**, select the ACP connection and choose **Probe** to negotiate capabilities without making a model request. Choose sources, enter a request, and explicitly approve runtime account usage to start a run. Only sources that permit external agents can be selected. Enabling that permission is a separate source edit in **Sources**.

The bundled helper launches the installed `codex` through `@agentclientprotocol/codex-acp` 2.2.2. It selects the OpenAI provider and ChatGPT login, removes API-key and custom-auth fallback environment variables, starts in read-only mode, disables shell tools, apps/plugins, notifications, web search, and project instruction loading, and supplies process-local configuration overrides. It does not edit Codex settings. These overrides reduce the exposed workflow; an agent executable remains trusted software with the OS user's access. ACP client capability negotiation is not an OS sandbox.

The adapter package also contains a Codex dependency; the helper explicitly selects the already installed CLI through `CODEX_PATH`. Check compatibility when upgrading either component. The installed CLI checked for this milestone was 0.162.1. Codex must be able to write its normal runtime state directory even for capability negotiation.

## CLI

All run commands address the daemon selected by `--data-dir` or `THESAURUS_DATA_DIR`. The probe reads the configured connection directly and does not need the daemon.

```sh
thesaurus runtime-probe codex-acp
thesaurus runtime-run codex-acp \
  --source SOURCE_ID \
  --approve-usage \
  --idempotency-key my-request-001 \
  --prompt "Summarize the open commitments and cite source revisions and lines."
thesaurus runtime-get RUN_ID
thesaurus runtime-cancel RUN_ID
thesaurus runtime-permission RUN_ID PERMISSION_ID OPTION_ID
```

The service allows up to two active ACP runs per workspace, with a 180-second prompt deadline (including permission waits), a 64 KiB response limit, and bounded protocol frames.

The start command returns immediately with a run ID. Reuse its idempotency key with the same inputs after an uncertain response; a replay returns the same run instead of consuming another run reservation. A changed request with the same key is rejected. Each explicit new request needs a new key. Reserved runs count against the UTC daily limit, including interrupted, failed, or cancelled attempts; the limit bounds attempts, not monetary cost.

## Memory and permissions

Each run creates its own source-scoped, read-only MCP grant and private credential file. The agent receives that MCP server through `session/new`. Every memory request rechecks the grant and current source sharing policy. The grant is revoked on completion, cancellation, or failure, and the credential file is removed. Startup recovery revokes grants from prior runs and marks unfinished runs interrupted; it never repeats a model request automatically. Run history remains visible after a client reconnect or daemon restart.

ACP `session/request_permission` requests appear in Activity with the supplied tool description and input. Only the runtime's `allow_once` and `reject_once` options are offered. Thesaurus does not grant permanent tool permissions. Cancelling immediately revokes memory access, resolves pending permissions as cancelled, sends `session/cancel`, waits briefly for acknowledgement, and then ends the process group. Cancellation is best effort for work already sent to a provider and cannot undo an action a user has already approved.

The ACP client advertises no filesystem or terminal capability, rejects those incoming client requests, bounds JSON frames and output, and uses a private working directory for each run. Native agent tools remain subject to that runtime's own controls. Retrieved content and permission descriptions are rendered as text. An ACP result is an agent response; it does not become an accepted task or overwrite source facts.

## Comparison with existing adapters

| Behavior | Codex CLI extraction | ACP runtime |
| --- | --- | --- |
| Purpose | Extract structured, cited action candidates from one supplied document | Run a user request against selected work memory |
| Authentication | Codex-owned account | Codex-owned account; provided helper selects ChatGPT and excludes API-key fallback |
| Protocol | One `codex exec` process and JSON event output | ACP v1 initialize, session creation, updates, permission requests, prompt response, cancellation, optional close |
| Memory | Document text supplied in the extraction prompt | Per-run MCP server with temporary source grant |
| Output | Validated action schema and exact evidence lines | Bounded text and progress in Activity |
| Permissions | Noninteractive extraction with tools disabled | Explicit one-time permission choices exposed to the user |
| Cancellation | Process termination on timeout/service shutdown | Protocol cancellation followed by process-group cleanup |
| Continuity | Durable extraction job | Durable run receipt and native session ID; no automatic session resume |

Use ACP for interactive work and memory-aware agent runs. Keep the CLI/API extraction adapters for their constrained action schema. ACP v2, interactive login, runtime session resumption, writable agent memory, full tool event history, and additional live agent compatibility checks are outside this milestone. A native session ID is retained for provenance; the [F19 work-thread workflow](work-continuity.md) uses a new agent session with an explicit delivery snapshot rather than automatically restoring that native session.

## Live connection result

On October 16, 2026, the capability probe negotiated ACP v1 with Codex ACP 2.2.2 and advertised session load/close, cancellation, and stdio MCP support. One approved synthetic prompt completed with `end_turn` using the existing ChatGPT login. It searched Thesaurus memory, retrieved the synthetic source, and correctly returned Taylor's release-note review, its 2026-10-22 deadline, document ID, revision 1, and line 3. The source facts were available through the scoped memory server, not pasted into the prompt. The temporary grant was confirmed revoked afterward.

The live run did not require a permission prompt or cancellation. Those code paths are implemented but were not exercised against an in-flight paid model request. No second model request or direct-API comparison was made. Capability advertisement alone is not proof of every optional agent behavior. Other ACP runtimes are configurable by absolute command and arguments, but have not been checked live here.

## References

- [ACP v1 initialization](https://agentclientprotocol.com/protocol/v1/initialization)
- [ACP session setup and MCP](https://agentclientprotocol.com/protocol/v1/session-setup)
- [ACP prompt turns and cancellation](https://agentclientprotocol.com/protocol/v1/prompt-turn)
- [ACP tool permissions](https://agentclientprotocol.com/protocol/v1/tool-calls)
- [Codex ACP adapter](https://github.com/agentclientprotocol/codex-acp)
- [Official OpenAI documentation on authentication](https://learn.chatgpt.com/docs/auth)
- [Official OpenAI documentation on configuration](https://learn.chatgpt.com/docs/config-file/config-reference)
