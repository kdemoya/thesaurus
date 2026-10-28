# Initial release readiness

The intended repository is `kdemoya/thesaurus`, publicly visible after Kelvin's review. Do not push, create a public repository, publish a release, or upload an installer until that review is complete.

## 1. License and ownership

- [x] MIT license with Kelvin De Moya as copyright holder.
- [x] Shared Cargo license and repository metadata inherited by all workspace packages.
- [x] Identify bundled asset notices and document dependency notice requirements in [THIRD_PARTY.md](../THIRD_PARTY.md).
- [ ] Generate and review the complete notice bundle for the actual distributed installer; include notices in the app and archive.

## 2. Repository and CI

- [x] Contributor guide and private security reporting policy.
- [x] Local build tools, workspaces, generated assets, environment files, and signing bundles excluded from Git.
- [x] Read-only CI workflow for formatting, service/shared/SDK tests, optimized web compilation, and desktop tests; pinned checkout action.
- [x] Full-history CI secret scanning with a pinned, checksum-verified Gitleaks binary and redacted output.
- [x] Dependabot configuration for Cargo and GitHub Actions.
- [x] Review the initial source inventory and run a redacted source secret scan.
- [x] Record and review the first local commit.
- [ ] Owner approves publication and the initial push.
- [ ] Enable GitHub private vulnerability reporting, secret scanning/push protection where available, and branch protection requiring CI.
- [ ] Confirm the first hosted CI run passes. Local validation does not establish hosted-runner success.

## 3. Installable macOS alpha

- [ ] Establish a clean-machine build and install/uninstall walkthrough for macOS Apple Silicon.
- [ ] Produce the intended `.app` and `.dmg` artifacts with checksums.
- [ ] Configure Developer ID signing, hardened runtime, notarization, and stapling for the app and embedded executables.
- [ ] Replace runtime Swift compilation for OCR with a bundled compiled helper, or explicitly exclude OCR from the initial product scope.
- [ ] Verify the installer includes required license notices and runs without developer tools.

## 4. Data recovery and lifecycle

- [ ] Supported backup and restore with a round-trip check.
- [ ] Historical migration fixtures and failed-upgrade recovery.
- [ ] User-facing deletion and retention behavior, including Bragbook review history and its collection limit.
- [ ] Surface collection failures and explain local data storage and protection.

## 5. Product acceptance

- [ ] Complete remaining onboarding (F45) and transcription (F46) acceptance with real devices and a fresh workspace.
- [ ] Repeat the Bragbook collect, cite, review, accept/dismiss, reopen, revoke, and export walkthrough using the release build.
- [ ] State which optional integrations remain experimental.

## 6. Support and updates

- [ ] Provide a version display, redacted diagnostics, and issue-report guidance.
- [ ] Document the supported macOS versions, architecture, and manual update procedure.
- [ ] Define release versioning, changelog, and upgrade/recovery policy. Automatic updates can follow the first alpha.

## Local foundation verification — October 28, 2026

- Formatting and workflow validation (`actionlint` 1.7.12) pass.
- Service, shared crate, integration, and SDK example checks: 96 tests pass; one explicit Keychain integration test is skipped.
- Optimized web build passes with locked dependencies.
- Desktop unit tests: seven pass in a source-only copy after building web assets, with generated bundle resources explicitly omitted.
- Gitleaks 8.30.1 reports no findings in the publication source inventory. Generated output, workspaces, credentials, signing bundles, and review screenshots are excluded.

The source-only copy reuses the local Rust toolchain and dependency/build caches. This verifies missing generated-resource assumptions; it is not a fresh-machine installer test or a hosted CI run.
