# Development

## Workspace

| Path | Responsibility |
| --- | --- |
| `crates/contracts` | Serialized API and shared domain types |
| `crates/core` | Configuration, SQLite storage, extraction, model adapters, access policy |
| `crates/client` | Authenticated service client over the Unix socket |
| `crates/mcp` | Official Rust MCP SDK server with 22 memory tools |
| `apps/cli` | CLI, daemon, HTTP API, event stream, browser pairing |
| `apps/ui` | Leptos interface compiled to WebAssembly |
| `apps/desktop/src-tauri` | Native window, service lifecycle, IPC bridge |
| `fixtures` | Synthetic context for the sample workspace |

The browser uses authenticated loopback HTTP; the native shell uses Tauri IPC and the private Unix socket. Both call the same API handlers and core. The MCP server is a client of the service and never opens SQLite directly. Reconnect existing MCP clients after rebuilding to discover newly added tools.

## Build commands

Rust is pinned in `rust-toolchain.toml`. `scripts/bootstrap.sh` installs project-local Rust, Trunk, and a wasm-bindgen CLI matching `Cargo.lock`. `scripts/cargo.sh` uses that installation when present. Xcode Command Line Tools are required on macOS.

```sh
./scripts/bootstrap.sh
./scripts/cargo.sh fmt --all
./scripts/cargo.sh build --locked -p thesaurus
./scripts/build-web.sh --release
./scripts/build-desktop.sh --debug
```

The desktop script builds the web assets, replaces the generated web resource directory to remove obsolete hashed bundles, copies the service and assets into ignored bundle resources, then invokes the pinned Tauri CLI. The debug app includes optimized web assets and a debug Rust executable. Bundles use local signing by default; Developer ID signing and notarization require explicit distribution configuration. See [macOS alpha builds](macos-alpha-build.md). The Tauri CLI is installed under `.tools/tauri` through npm, with its cache under `.tools/npm-cache`.

To run the desktop binary against the same synthetic workspace as the browser:

```sh
export THESAURUS_DATA_DIR="$PWD/.local/dev"
target/debug/thesaurus-desktop
```

The app attaches to a running service or starts one on an available loopback port, discovered through that workspace's private instance descriptor. This lets native startup coexist with a development service on 4620. The default desktop data directory is the platform application-data location when `THESAURUS_DATA_DIR` is unset. The development CLI examples use `.local/dev` explicitly. One daemon owns each data directory. A manually started CLI service defaults to port 4620; stop an existing service before reusing that port, or use `serve --port PORT` (`0` chooses an available port). `thesaurus open` discovers the workspace's actual browser address.

Rebuild web assets and restart the daemon after UI changes so its CSP hashes match the generated bootstrap. Do not use Trunk's development server against real data: authenticated app serving is provided by the daemon.

## Checks

The current test suite includes storage, API security, quota reservation, and a daemon/MCP process integration flow. To run it deliberately:

```sh
./scripts/cargo.sh test --locked -p thesaurus -p thesaurus-core
```

Web compilation uses the wasm target; a host `cargo test --workspace` is not the intended UI check. Native packaging requires macOS build tooling. No real provider invocation is part of the test suite.

## Configuration and credentials

`thesaurus init` creates a commented `config.toml`. Connection IDs must be unique and cannot use the built-in `local` ID. Supported kinds are `claude_cli`, `codex_cli`, `openai_compatible`, and `acp`; see `examples/config.toml`.

CLI runtimes use their own installed authentication. The daemon does not parse their credential files. API tokens are read only from the configured environment variable at request time; launch the daemon from an environment containing that variable. Endpoint URLs must be HTTPS, except local HTTP endpoints; redirects are disabled. A source must allow external sharing for cloud destinations. Availability in the interface indicates configuration/executable presence, not successful authentication.

The API is versioned under `/api/v1`. The private `instance.json` describes the protocol version, socket, and administrative credential. Treat the data directory and instance descriptor as private. Agent grants use separate files and can be revoked through the CLI. Avoid putting grant files or instance descriptors in repositories or model prompts.

## Recovery

A process lock prevents two services from owning one data directory. Local jobs interrupted by restart are queued again; interrupted model jobs fail for explicit review/retry. SQLite revisions preserve the text behind older citations. The current schema is version 5. Startup migrates version 1 by adding runtime run and permission tables, then version 2 by adding project briefs and immutable brief revisions. Version 4 adds work threads, checkpoint history, delivery snapshots, meeting series, occurrence baselines, and the workflow idempotency ledger. Version 5 adds priority preferences, immutable review baselines, waiting records and their attributed history. Interrupted runs and generations fail without replaying model usage. A completed response can be finalized into a draft locally after restart. The worker also reconciles completed brief receipts after transient database failures. See [living briefs](living-briefs.md) for freshness, review, and access rules. See [work continuity](work-continuity.md) for resume and meeting-baseline semantics. See [project focus](project-focus.md) for ranking rules, waiting state and access boundaries. Newer schemas are rejected. Future upgrades must add explicit migrations.

## Live meeting companion

See [meeting companion](meeting-companion.md) for setup, state/queue limits and pending native acceptance. `scripts/build-desktop.sh` builds and bundles `thesaurus-audio`; for a standalone service, run `scripts/build-audio.sh debug` so the bridge is beside `target/debug/thesaurus`. Live recording uses the service computer’s audio devices, including when controlled from a paired browser.
