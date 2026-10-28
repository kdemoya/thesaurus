# macOS recording permissions and build identity

## What caused the recurring prompt

The reported dialog is **Screen Recording**: macOS asks to record the computer's screen and audio. The current ScreenCaptureKit system-audio path requires that OS permission, although Thesaurus attaches only an audio output and retains no screen frames for meeting recording. Microphone permission is separate.

On October 19, macOS diagnostics showed the microphone grant returning **Allowed (User Consent)**. Screen/system-audio checks repeatedly rejected a stored requirement containing the previous build's code hash. The installed app used ad hoc signing, whose designated requirement was a build-specific hash. The user's screenshot confirms the Screen Recording dialog. This identifies an app signing mismatch; repeatedly requesting microphone access cannot repair it.

## Persistent signing

Use the same signing identity and bundle identifier for successive builds. `scripts/sign-macos.py` signs the service, audio helper and transcription engine individually before signing the outer app. The helper is explicitly signed so its embedded usage descriptions are bound into its signature. The build does not create certificates, alter system trust, reset TCC, or grant capture access.

Prefer an Apple Development identity for development or a Developer ID Application identity for distribution:

```sh
APPLE_SIGNING_IDENTITY='Your existing signing identity' ./scripts/build-desktop.sh --debug
```

For local development without an Apple-issued certificate, an explicitly authorized local identity can be created once:

```sh
python3 scripts/sign-macos.py --create-local-identity
./scripts/build-desktop.sh --debug
```

This creates `.local/signing/ThesaurusDevelopment.keychain-db`, containing a non-exportable code-signing key, plus its private unlock password and public certificate/identity metadata. The directory is mode 0700 and its password/configuration files mode 0600. The certificate is self-signed and valid for ten years. Its key permits the `codesign` tool; it is not imported into the login/System keychain or added to certificate trust settings. The user's existing keychain search list is restored after creation. Future builds explicitly address this keychain and reuse this identity. Preserve this directory privately; deleting/regenerating it changes identity again. Do not commit, publish or share its keychain/password. It is a development identity, not Developer ID signing or notarization for distribution.

`APPLE_SIGNING_IDENTITY` takes precedence over the local identity. An explicit `APPLE_SIGNING_IDENTITY=-` chooses ad hoc signing for a debug build and displays a permission-persistence warning. Release builds require a persistent identity. A configured but unavailable local identity causes an error instead of silently switching signers. Apple distribution signing keeps Tauri's final signing/notarization path; the private local-keychain path finishes its outer signature after Tauri builds the development bundle.

Never modify or sign a running app bundle. Stage a separate app, wait until recordings/transcription/capture/agent work are idle, then quit the app, stop its independent service, preserve the old bundle, and activate the staged one. Keep the existing data directory and bundle identifier.

## After changing identity once

The first switch from ad hoc signing to a persistent signer is a new code identity. macOS may require a fresh grant. In **System Settings → Privacy & Security → Screen & System Audio Recording** (wording varies by macOS), enable the current **Thesaurus** app and follow any system-requested quit/reopen instruction. If an enabled entry still represents the old build, toggling it off/on updates that user decision. Microphone access is managed in the separate **Microphone** pane.

Permission grants remain OS-owned. The helper checks microphone authorization first, reuses an authorized state, requests only an undecided state, and reports denied/restricted states without requesting again. No application preference substitutes for a real OS grant. Do not manually edit TCC databases or relax signature requirements to an identifier-only rule.

## Evidence and limits

The updated Swift helper compiles; explicit signing binds its embedded Info.plist. The staged app's nested signatures and outer signature verify. Live capture/repeated Start/Resume and persistence across a future rebuild still need user-run acceptance after signing/activation. No recording or OS privacy change is performed merely to build/sign the app.

References: [Apple: code signing requirements and privacy identity](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements), [Apple: code signing in depth, self-signed identities and inside-out signing](https://developer.apple.com/library/archive/technotes/tn2206/), [Apple: media capture authorization](https://developer.apple.com/documentation/bundleresources/requesting-authorization-for-media-capture-on-macos), [Tauri: signing configuration](https://v2.tauri.app/reference/environment-variables/).

### Approved activation

The user approved the local identity setup and idle activation. The private keychain was created and confirmed absent from the general keychain search list; no system certificate trust was changed. The installed app, service, audio helper and transcription engine verify with certificate-based designated requirements pinned to the same persistent certificate and their respective identifiers. The correct normal app and bundled service are running, and the native workspace is connected. Existing data counts and all paused/stopped recording states were preserved.

During activation, Launch Services reopened an older temporary `Thesaurus Meeting Processing.app` with the same bundle identifier. Both earlier temporary build outputs were unregistered and reversibly moved into `.local/retired-temporary-builds-*/` with `.bundle-backup` extensions. The signed normal app was registered explicitly. Older bundles remain available for rollback. No recording, model request, privacy-grant change or repeated-capture test was performed; the user may need one fresh OS grant for the new code identity.
