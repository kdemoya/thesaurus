# Thesaurus

A local work assistant with a Rust background service, a Leptos interface compiled to WebAssembly, and a Tauri desktop app. Capture transcripts and notes, review suggested actions with source evidence, and share scoped work memory with your existing agents.

Pre-release software; macOS on Apple Silicon is the initial desktop target. See [release readiness](docs/release-readiness.md) for the remaining publication and installer checks.

## Current implementation

- [Guided onboarding](docs/onboarding.md) with large dotted step numbers/letters, saved workspace progress, live primary connection setup, audio choices and scoped agent memory preparation.
- Desktop and authenticated local browser access to the same workspace.
- Text and Markdown capture, immutable revisions, SQLite full text search, and durable processing jobs.
- Local commitment extraction with owners, explicit dates, and exact source lines. Accept, dismiss, complete, and reopen actions.
- Shared memory through 22 MCP tools, a local API, and CLI. Each agent has a separate, revocable source grant; observations preserve attribution and idempotency.
- Living project briefs with cited sections, draft review, revision comparisons, and stale-source detection. [Using briefs](docs/living-briefs.md).
- Work threads with checkpoints, split/merge, bounded cross-agent resume, and recurring meeting preparation. [Work continuity guide](docs/work-continuity.md).
- Project priorities with explained changes since review, plus an evidence-backed waiting-for queue and local follow-up suggestions. [Project focus](docs/project-focus.md).
- ACP agent runs with scoped memory, live progress, one-time tool permissions, and cancellation. [Codex setup and compatibility](docs/acp-integration.md).
- Optional Claude Code, Codex CLI, and OpenAI-compatible model adapters. Existing runtimes keep their authentication. No model is enabled by default.
- Automatic meeting summaries using saved onboarding preferences, with model, automation and audio choices in Settings. Source sharing controls, per-connection daily run limits, and real-time updates.

- Anchored notes, correction previews, temporal knowledge, people lookup, question cues and a knowledge gap inbox.
- Reviewed commitment-to-delivery traces, selected Git changes and approved LSP definition/reference queries.
- Upstream MCP sources with index/live/hybrid retrieval, explicit query disclosure, checkpoints and bounded responses.
- Memory correction/conflict history, explained notifications, source coverage and independent destination revocation.
- Saved recipes, reviewed procedure suggestions, budgeted context and explicit handoff receipts.
- Repeated-friction opportunities, cited period outcomes and editable effort/calendar capacity reviews.
- Project and personal eval suites with versioned criteria, baseline regressions, human review and explicitly budgeted model judges.
- Scoped Markdown/JSON exports, local action previews/receipts/undo, and terminal/light/dark/system themes.
- Preconfigured live recording with the system microphone, supported system audio and bundled offline Base transcription. [Transcription choices](docs/transcription-options.md) add downloadable local Normal/Large presets, speech detection, separately authorized Deepgram/Soniox cloud routes and a compatible company endpoint. Local WAV imports also support an installed whisper.cpp adapter and separately approved reasoning. Selected-window macOS capture/OCR passed the recorded synthetic native walkthrough.
- Local plugins with durable lifecycle hooks, scoped native capabilities, commands/processors/panels, connector imports, reviewed actions, evaluation checks and approved model requests. Native product integrations are first-party providers. [Plugin authoring and management](docs/plugins.md).
- [Bragbook](docs/bragbook.md): a built-in plugin that collects cited accomplishment candidates, supports personal review and edits, and exports selected accepted highlights as Markdown or JSON.

Live meeting recording and the menu bar companion are described in [meeting companion](docs/meeting-companion.md). Browser and native generated-audio Start/Pause/Resume/Stop and full-screen tray checks pass; real-device input and speech quality remain unverified.

See [backlog progress](docs/backlog-progress.md) and [verification evidence](docs/backlog-verification.md) for acceptance gaps. Direct Slack synchronization, filesystem watchers, semantic/vector search, hosted multi-user service, and signed distribution remain outside the current implementation.

## Run locally

macOS is the first desktop target. Install Xcode Command Line Tools. The bootstrap installs Rust and web build tools inside `.tools/` without editing your shell profile.

```sh
./scripts/bootstrap.sh
./scripts/build-audio.sh debug
./scripts/build-transcription.sh debug
./scripts/build-web.sh --release
./scripts/cargo.sh build --locked -p thesaurus
export THESAURUS_DATA_DIR="$PWD/.local/dev"
target/debug/thesaurus init
target/debug/thesaurus serve
```

In another terminal, use the same data directory and open a paired browser:

```sh
export THESAURUS_DATA_DIR="$PWD/.local/dev"
target/debug/thesaurus open
```

Choose **Explore a sample workspace** for synthetic examples, or **Add context** to paste a transcript. CLI file import is also available:

```sh
target/debug/thesaurus import /absolute/path/meeting.md --kind transcript
target/debug/thesaurus search "launch"
target/debug/thesaurus context "launch" --budget 8000
```

Keep `serve` running. `Ctrl-C` shuts it down cleanly; workspace data persists. Browser pairing links expire after two minutes and can be used once. A browser session expires after 24 hours or a service restart; run `open` again to pair.

## Desktop app

The packaging script requires Node/npm for the pinned Tauri build tool; the installed application runs Rust and the system webview.

```sh
./scripts/build-desktop.sh --debug
```

Open `target/debug/bundle/macos/Thesaurus.app`. It bundles the service and browser assets, starts the service when necessary, and shares data with the browser. To use the development workspace, launch with `THESAURUS_DATA_DIR` set; see [development](docs/development.md). For an optimized local bundle use `--release`. Signing and notarization are not configured.

## Connect an existing agent

Get source IDs from **Shared memory** or `thesaurus status`, then create a grant:

```sh
target/debug/thesaurus agent add my-agent --source SOURCE_ID --write
```

The CLI writes a private grant file and prints an MCP configuration snippet. Add that snippet to your agent's MCP settings. External agents can retrieve only granted sources marked **Allow this source in external agents and models**. Use `--local-agent` only for an agent that processes entirely on your device. Omit `--write` for read-only memory. Revoke access with `agent revoke GRANT_ID`.

The MCP server exposes 22 tools for source search, cited memory, budgeted context, project briefs, threads, meeting preparation, priorities, waiting requests, memory inspection, conflicts, commitments, saved recipe inspection and handoff acknowledgement. Use the client’s tool listing for the exact current schema. Repository, knowledge and other advanced records are also available through the scoped local API. Agent contributions retain attribution; task and completion decisions require user review.

## Model connections

Run `init`, edit the workspace's `config.toml` using [examples/config.toml](examples/config.toml), and restart the service. `thesaurus doctor` checks availability without invoking a model. New meetings summarize automatically with the onboarding model; change this under **Settings → Automation**. Separate source-card analyses remain explicit requests. Runtime subscriptions depend on the installed agent's supported authentication and account terms; API connections use the named environment variable. Credentials are never copied from other agents.

## Architecture and planning

- [Implementation plan](docs/implementation-plan.md): architecture, delivery phases, and acceptance criteria.
- [Legacy assessment](docs/legacy-assessment.md): analysis of the three Python dashboard directories.
- [Similar projects](docs/similar-projects.md): 15 adjacent projects and reusable patterns.
- [Feature backlog](docs/feature-backlog.md): 45 feature ideas with priorities and dependencies.
- [Backlog progress](docs/backlog-progress.md): requirement-level implementation and acceptance evidence.
- [Plugins](docs/plugins.md): contracts, lifecycle hooks, native providers and Rust examples.
- [Plugin acceptance](docs/plugin-acceptance-audit.md): the complete F44 requirement audit and verification limits.
- [Evaluations](docs/evaluations.md): personal/project suites, criteria, baselines and judge budgets.
- [Development](docs/development.md): workspace map, commands, and integration details.

## Contributing and license

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup and verification, and [SECURITY.md](SECURITY.md) for private vulnerability reports. Thesaurus is licensed under [MIT](LICENSE). [Third-party components](THIRD_PARTY.md) retain their own licenses.
