# Contributing to Thesaurus

Thesaurus is an early local-first work assistant. macOS on Apple Silicon is the initial desktop target. Start with the [README](README.md) and [development guide](docs/development.md). The [Bragbook guide](docs/bragbook.md) and [plugin guide](docs/plugins.md) describe the built-in accomplishment workflow and plugin API.

For a substantial feature or change to a public API, open an issue describing the problem and proposed behavior before implementing it. Small fixes can go directly to a pull request. Include the problem, resulting behavior, relevant checks, and any limitations in the description. Use synthetic examples when reporting a bug.

## Development and verification

Install Xcode Command Line Tools, then run:

```sh
./scripts/bootstrap.sh
./scripts/cargo.sh fmt --all -- --check
./scripts/cargo.sh test --locked --all-targets \
  -p thesaurus-contracts -p thesaurus-client -p thesaurus-mcp \
  -p thesaurus-plugin-sdk -p thesaurus-core -p thesaurus
./scripts/build-web.sh --release --locked
TAURI_CONFIG='{"bundle":{"resources":[]}}' \
  ./scripts/cargo.sh test --locked -p thesaurus-desktop
```

CI runs these checks on macOS Apple Silicon. Web compilation uses the WebAssembly target, so a host `cargo test --workspace` is not the intended check. Desktop unit tests omit generated bundle resources through `TAURI_CONFIG`; they do not verify packaged sidecars. The Keychain integration test is opt-in; the default suite must not require a real provider account, recording devices, or credentials. Packaging and real-device acceptance are separate release checks.

Add regression coverage when changing behavior. Review source grants, evidence citations, revocation, and duplicate/retry behavior when changing collection or export. Keep dependency changes intentional and commit `Cargo.lock` with them. Follow the repository's Rust formatting and existing module structure.

## Keep workspaces private

Use a disposable directory under `.local/` and set `THESAURUS_DATA_DIR` when running locally. Do not attach personal transcripts, databases, pairing links, instance descriptors, agent grants, credentials, or signing material to an issue or pull request. Check screenshots and logs for private content. See [SECURITY.md](SECURITY.md) for vulnerability reports.

## Licensing

Contributions are submitted under the project's [MIT license](LICENSE). Only contribute material you have permission to share under those terms. Preserve third-party copyright and license notices; see [third-party components](THIRD_PARTY.md).
