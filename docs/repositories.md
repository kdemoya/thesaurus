# Local repositories and symbol services

F12 and F13 work with explicitly selected local Git repositories. Register a repository under a project, choose the root path and initial checkpoint reference, and select included/excluded relative file or directory prefixes. Prefixes are literal paths, not shell patterns. The app makes no network Git calls and does not modify checkout files, branches, index or commits.

## Committed changes relevant to active work

A change report pins the checkpoint commit and chosen target reference to full commit hashes. It records selected changed files/statuses, exact commit hashes/subjects and files for up to the most recent 30 commits. Up to 500 selected changed files and 500 paths per commit are included, with explicit omission notes. Reports are persisted. Choose Advance checkpoint to move the durable baseline only after saving a successful report; otherwise the original baseline remains.

Accepted project tasks are linked when a word of at least four characters from their title appears in a selected file path or associated commit subject. Each match includes original task evidence. This deterministic relevance hint is inspectable and is never proof that the task was delivered. This release reports committed history, not uncommitted working-tree changes.

The following directory components are always excluded: `.git`, `node_modules`, `target`, `.ssh`, `.aws`, `.codex`, `.agents`, and names starting with `.env`. Include and exclude selections apply to returned paths. No file contents are read for change reports. Git commit subjects may describe changes outside the file selection when a commit also touches selected files; the selection is a file filter, not a promise to redact commit-message content.

Git uses fixed argument arrays, disables external diffs/textconv/hooks/fsmonitor/pagers, ignores global/system Git config and removes inherited repository overrides. Each command has a 20-second timeout and bounded output. Paths are canonicalized; registration requires the actual root directory. Repository paths and project association are immutable after creation. Updates require expected_version and a stable idempotency key. Checkpoint advances increment the repository version.

## Symbol definitions and references

Configure an absolute local executable, argument array and language ID for an LSP service (for example an already installed rust-analyzer), then explicitly approve that configuration. Each symbol query separately requires approval to invoke the selected executable. The service runs with the user's privileges and may index repository files or launch its own subprocesses; it is an approved local service, not an OS sandbox.

The adapter implements actual [Language Server Protocol definition requests](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.18/language/definition.md) and [references requests](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.18/language/references.md), using the standard initialize → initialized → didOpen lifecycle and Content-Length framing. It advertises UTF-16 positions, checks advertised capabilities, requests a definition or references, and shuts down the process. Rust-analyzer build scripts and proc macros are disabled in initialization options; other services may interpret their own initialization settings differently.

Choose a selected relative file and a **zero-based line and UTF-16 character** position. The app sends one selected file of at most 100KB to the service. It imports no whole repository contents. Returned Location/LocationLink results are canonicalized and filtered back through the allowed repository selection. At most 100 locations are included, with a short excerpt from each returned file (each file at most 100KB). External/symlink-escaping paths, oversized files and malformed locations are omitted and counted.

The service dialogue is limited to 45 seconds, 500 incoming messages per request, 8KB headers and 2MB message bodies. Thesaurus rejects service-initiated edit/command requests. Selected file content hash and repository HEAD are checked again after lookup and persisted with results. Returned snippets represent working-tree contents, and are not claimed to be Git blob contents at HEAD. The approved service's own behavior cannot be constrained by those client-side result filters.

## Interface and CLI

Repositories provides project selection, registration and prefix forms, checkpointed reports, accepted task links, service configuration/approval, symbol position queries and saved results.

```sh
thesaurus repo list
thesaurus repo save --file repository.json
thesaurus repo report REPOSITORY_ID --expected-version 1 --to-ref HEAD
thesaurus repo report REPOSITORY_ID --expected-version 1 --to-ref main --advance-checkpoint
thesaurus repo reports REPOSITORY_ID
thesaurus repo symbols REPOSITORY_ID src/main.rs --expected-version 2 --line 12 --character 8 --method definition --approve-service
thesaurus repo symbols REPOSITORY_ID src/main.rs --expected-version 2 --line 12 --character 8 --method references --approve-service
thesaurus repo symbol-history REPOSITORY_ID
```

`repository.json`:

```json
{
  "expected_version": 0,
  "project_id": "PROJECT_ID",
  "name": "Selected service",
  "path": "/absolute/path/to/repository",
  "initial_ref": "main",
  "include": ["src", "docs"],
  "exclude": ["src/generated"],
  "allow_external": false,
  "symbol_service": {
    "command": "/absolute/path/to/rust-analyzer",
    "args": [],
    "language_id": "rust"
  },
  "approve_service": true
}
```

Set symbol_service to null and approve_service to false to use Git reports alone. CLI adds an idempotency key if omitted; supply a stable key for retry. Requests accept at most 100 include/exclude prefixes and 30 service arguments. The repository list and histories return at most 100 connections and 30 reports/queries respectively; older saved records remain in SQLite. No automatic repository scans or live service invocations occurred during implementation.

## API and access

| Method | `/api/v1` path | Purpose |
| --- | --- | --- |
| GET/POST | `/repositories` | List / register |
| GET/PATCH | `/repositories/{id}` | Read / update selection |
| GET/POST | `/repositories/{id}/reports` | Saved reports / create report |
| GET/POST | `/repositories/{id}/symbols` | Saved results / approved query |

Registration, configuration changes, Git execution and LSP invocation require the local user. Agent reads additionally require explicit `allow_external` on the repository and every current and historically pinned project source permission. Absolute local paths and service executable/arguments are redacted from agent configuration/history reads. Personal filesystem locations and execution controls cannot be changed by agents. Exact source evidence on matched tasks preserves its original provenance.

## Verification

No tests, real work repository reads or live language-server invocations were performed during development. Aggregate compilation is performed by the coordinator. Live LSP capability and indexing behavior depend on the explicitly installed/approved service.
