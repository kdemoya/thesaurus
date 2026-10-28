# Building the macOS alpha

The initial installer is an Apple Silicon `.dmg` containing Thesaurus.app, an Applications shortcut, installation instructions, and license notices. The app includes its service, audio and Vision OCR helpers, offline Whisper Base transcription, web interface, and third-party notices. No developer tools are needed to execute those bundled helpers.

## Local owner review

On an Apple Silicon Mac with Xcode Command Line Tools, Node/npm, and Python 3.11 or newer:

```sh
./scripts/bootstrap.sh
python3 scripts/package-macos.py
```

This produces optimized local artifacts in a new timestamped folder under `target/macos-alpha/local/`: a DMG, `.app.zip`, `SHA256SUMS`, `release.json`, installation guide, and app-verification report. The assembled app is also at `target/release/bundle/macos/Thesaurus.app`. A persistent project-local identity is reused when already configured; otherwise the explicit local mode allows ad hoc signing. The build never creates certificates, changes trust, resets permissions, installs the app, or uploads artifacts.

`python3 scripts/package-macos.py --skip-build` repackages an existing app after structural/signature checks. It does not prove that the app matches current source. Use a complete build for release candidates. The manifest records the source revision and whether local source changes were present.

To build just the app:

```sh
./scripts/build-desktop.sh --release --local
python3 scripts/verify-macos.py --app target/release/bundle/macos/Thesaurus.app
```

App verification checks Apple Silicon executables, nested signatures, system-only dynamic libraries, the pinned Base model hash, web assets, and bundled notices. It does not claim Gatekeeper approval or fresh-machine acceptance. Local artifacts are explicitly marked unsuitable for public distribution.

Packaging also runs the bundled service in a disposable synthetic workspace with system-only PATH. It verifies import/search, cited Bragbook collection/review/exports, private files and restart persistence, then shuts down its own process and removes the workspace. The resulting `packaged-service-smoke.json` joins the artifact reports. This does not launch the desktop UI or exercise recording devices.

## Developer ID distribution, after owner review

Public distribution needs an Apple Developer membership, an installed **Developer ID Application** certificate with its private key, and a preconfigured `notarytool` Keychain profile. Keep credentials in Keychain; do not put certificates, keys, passwords, or API keys in this repository or chat.

Set `APPLE_SIGNING_IDENTITY` to the exact Developer ID identity name or fingerprint. An explicitly selected signing keychain can be set with `THESAURUS_SIGNING_KEYCHAIN`; a separate notary profile keychain can be set with `THESAURUS_NOTARY_KEYCHAIN`.

```sh
python3 scripts/sign-macos.py --distribution --check-identity
python3 scripts/package-macos.py --distribution --notary-profile THESAURUS_PROFILE
```

The second command **uploads the app and DMG to Apple**. It requires a clean working tree, signs each nested executable and app with hardened runtime and secure timestamps, notarizes and staples the app, builds the DMG from that final app, signs/notarizes/staples the image, validates Gatekeeper acceptance, then generates final checksums. It never pushes to GitHub or publishes a download. Failed or timed-out submissions must be inspected before retry or distribution.

Developer ID and notarization checks fail closed: local development certificates and ad hoc signatures cannot satisfy distribution mode. Approval to publish, clean-machine acceptance, and the remaining [release requirements](release-readiness.md) still apply even after notarization succeeds. No automatic updater is included; initial updates are manual.

See [installation and acceptance](macos-alpha-install.md), [third-party components](../THIRD_PARTY.md), and [Apple's notarization guide](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution).
