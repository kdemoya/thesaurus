# Initial release readiness

The intended repository is `kdemoya/thesaurus`, publicly visible after Kelvin's review. Do not push, create a public repository, publish a release, or upload an installer until that review is complete.

## 1. License and ownership

- [x] MIT license with Kelvin De Moya as copyright holder.
- [x] Shared Cargo license and repository metadata inherited by all workspace packages.
- [x] Identify bundled asset notices and document dependency notice requirements in [THIRD_PARTY.md](../THIRD_PARTY.md).
- [x] Generate and review the locked native/WASM dependency notice bundle, original texts and corresponding MPL source archives; include it in app/DMG packaging and expose it through Help.

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

- [x] Document reproducible alpha builds, installation, manual updates, uninstall behavior and fresh-Mac acceptance.
- [ ] Complete the documented fresh-Mac acceptance without developer tools.
- [x] Produce optimized local `.app`, `.app.zip` and `.dmg` artifacts with final checksums and verification reports.
- [x] Implement explicit Developer ID signing, hardened runtime, secure timestamps, notarization, stapling and final verification; local certificates cannot satisfy distribution mode.
- [ ] Configure a real Developer ID identity/notary profile and validate public distribution. Owner selected local alpha first; Apple Developer membership/certificate are not available yet.
- [x] Replace runtime Swift compilation for OCR with a bundled compiled Vision helper; synthetic recognition passes through CPU compatibility mode.
- [x] Verify signatures, all helper architectures/system-library dependencies, pinned model, web assets and 1,247 notice files; mount DMG read-only and verify a disposable copied app.
- [x] Pass 29 synthetic service/Bragbook checks from the copied app with system-only PATH, including private exports and restart persistence.
- [ ] Complete no-developer-tools and native GUI/permission acceptance on a fresh Mac.

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

## Local macOS alpha verification — October 28, 2026

The optimized Apple Silicon app builds and passes strict nested signature checks. Audio-input entitlements are present on the audio helper and containing app. The compiled Vision helper recognizes a generated synthetic image through its CPU compatibility mode. Primary recognition encountered the existing macOS Vision error; the fresh-process retry succeeded. No real recording or window capture was performed.

The local DMG verifies, mounts read-only, contains the app, Applications shortcut, install guide and notice bundle, and detaches after the copied-app test. The app copy passes all resource checks and 29 synthetic service checks: import/search, source scopes, cited Bragbook candidates, review, accepted-only export, private export permissions, deduplication, graceful shutdown and restart persistence. The test uses a disposable workspace and does not open the desktop UI or touch the owner's workspace.

Rust checks: 106 passed, one opt-in Keychain test skipped. Signing and packaging safeguards: 46 Python tests passed. Formatting, shell syntax, workflow validation, notice hashes and source secret scanning pass. Hosted workflow execution and fresh-machine/native-device acceptance remain pending. The local artifact is not Developer ID signed or notarized; it is not approved for public distribution.
