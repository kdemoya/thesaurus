# Living project briefs (F17)

A brief is a derived, versioned account of a project's goals, decisions, blockers, and open questions. It keeps the source revisions behind its conclusions. The user reviews a generated draft before agents can retrieve it as shared memory.

## Use the interface

1. Open **Briefs** (key `6`). Create a project with a name and 1–50 existing sources. Creation is local and uses no model allowance. Use **Add project sources** to expand its scope later. Existing sources cannot be removed through that control; source additions mark earlier briefs stale.
2. Configure an [ACP connection](acp-integration.md) if necessary. Enable external sharing for every selected source in **Sources**.
3. Choose the agent, approve account usage for this generation, and select **Generate draft**. The daemon sends the complete selected source snapshot and instructions to produce four structured sections. It does not send previous briefs as source evidence.
4. Open **Activity** for streamed output, runtime progress, one-time permission controls, and cancellation. Generation can also be cancelled from the brief. Existing ACP quotas, a two-run workspace limit, timeouts, and grant cleanup apply. There is no fallback or automatic model retry.
5. Review each claim and its cited passage. Citation links open the immutable document revision with numbered lines. Accept the latest fresh draft or reject it. Acceptance makes that revision available to agents whose grants include every selected source. A previous accepted brief remains available while a new draft is generated or reviewed.
6. After a source changes, the brief displays **STALE** and names the changed sources. Select **Refresh draft**, review the differences, and accept a new revision. Refresh always requires another explicit usage approval.

## Evidence and freshness

Every generated item must include 1–6 exact citations: document ID, revision, line, and the complete source line as its quote. The response must contain exactly the four required sections, with at most 12 items per section and 1,000 characters per claim. Invalid JSON, out-of-scope revisions, missing evidence, and incorrect line/quote pairs fail the draft. Original output remains in the private runtime receipt for inspection. A valid locator does not prove that a claim follows from the text; the user must review that relationship.

Empty sections say that the selected evidence supplied no supported information. They do not assert that no goals, blockers, or questions exist.

Each revision records all selected documents' revisions and content hashes plus the event watermark captured during generation. Freshness compares these inputs with current documents when reading or accepting a brief. A change to any selected document, including a new document within a selected source, invalidates **every section**. This conservative policy covers new evidence that changes an uncited conclusion or introduces a blocker into an empty section. Reimporting identical content does not make a brief stale. Historical revisions and their citations remain readable.

Acceptance is guarded by the project's optimistic version, the latest draft revision, and a fresh comparison of all input revisions in the same transaction. A source that changes during generation produces a reviewable stale draft that cannot be accepted. New drafts retain additions and removals relative to the accepted revision at generation time, including changes to citations. The revision selector preserves accepted, rejected, and failed history.

## Bounds and recovery

The initial input limit is **16,000 UTF-8 bytes for the generated prompt, including instructions, project name, JSON metadata, and numbered source lines**. This is smaller than 16 KB of plain source text. Oversized selections fail before reserving allowance or starting an agent. No source is silently truncated or omitted. Large transcript summarization and incremental section updates remain future work.

Generation uses the existing ACP v1 adapter and source-scoped read-only MCP grant. It also embeds the immutable source snapshot directly in the prompt, so this path can work without tool calls. Source permissions are checked when reserving and immediately before starting the runtime. The agent is instructed to use only the supplied snapshot and cite it exactly. A rejected draft still consumed its model request; acceptance and rejection do not invoke a model.

Schema version 3 adds project and brief-revision tables. The brief revision, runtime reservation, grant, and events commit together, so a crash cannot create a billed run without a corresponding brief record. Startup marks interrupted runtimes and associated generations failed without replaying them. A completed persisted response can be finalized locally after a crash or transient database failure; this does not invoke the model again. Accepted revisions are preserved. Creation and generation support idempotency keys for uncertain request retries.

## CLI and API

Use the same data directory as the running daemon. Commands return JSON.

```sh
thesaurus brief create 'Release project' --source SOURCE_ID,OTHER_SOURCE_ID
thesaurus brief list
thesaurus brief get PROJECT_ID

# Use the current project.version returned above.
thesaurus brief generate PROJECT_ID --connection codex-acp \
  --expected-version 1 --approve-usage --idempotency-key release-brief-1

# Read progress and the updated project.version; do not reuse the example version blindly.
thesaurus brief get PROJECT_ID
thesaurus runtime-get RUN_ID
thesaurus brief review PROJECT_ID --revision 1 --expected-version 3 accept
```

| Endpoint | Access and purpose |
| --- | --- |
| `GET /api/v1/briefs` | User sees full history; agents see only accepted, wholly permitted briefs. |
| `GET /api/v1/briefs/{id}` | Same access filtering, including source freshness. |
| `POST /api/v1/briefs` | User only; `name`, `source_ids`, `idempotency_key`. |
| `POST /api/v1/briefs/{id}/generate` | User only; `connection`, `expected_version`, `approve_usage`, `idempotency_key`; returns a runtime receipt. |
| `POST /api/v1/briefs/{id}/review` | User only; `revision`, `expected_version`, `decision` (`accept` or `reject`). |

Browser CSRF/session protection and the native Unix-socket API apply to these routes. Grant tokens cannot create, generate, or review briefs.

## Shared agent memory

MCP exposes `memory_briefs` and `memory_brief_get` in addition to the original five memory tools. Agents receive only the currently accepted revision, never draft/rejected history, run prompts, or comparison baselines. A grant must cover **all** selected sources; external grants also require each source to allow external disclosure. Source-policy changes and grant revocation apply on subsequent requests.

Accepted stale briefs remain available with explicit revision, section `stale` flags and `changed_sources`. Agents must inspect changed evidence before using those claims as current facts. Thesaurus cannot retract information already delivered to an agent. Brief generation cannot use agent contributions as original source evidence or automatically publish its own conclusions.

## Implementation checks and remaining scope

Daemon, web, and desktop compilation are the implementation checks for this slice. No automated tests or live brief model requests were run as part of F17. The earlier synthetic Codex ACP check establishes transport connectivity; it does not verify the new structured brief response path.

Future extensions include source-scope removal, large-input retrieval and section refresh, manual claim editing, section-specific dependency analysis, source deletion/retention, pagination for long history, and provider compatibility coverage. These are not enabled implicitly by this feature.
