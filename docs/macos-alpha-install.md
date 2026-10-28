# Installing the macOS alpha

This alpha targets Apple Silicon Macs. The bundle declares macOS 12 as its minimum; acceptance on older macOS versions and a Mac without developer tools is still pending. System-audio recording requires a macOS version supported by Apple's capture APIs. Real-device recording and speech-quality acceptance remain pending.

## Local review build

`local-alpha` artifacts use local development signing. They are not Developer ID signed or notarized, and Gatekeeper may block opening them. They are intended for the owner to review locally. Do not distribute them as a publicly approved installer. Do not disable Gatekeeper or remove quarantine attributes to make a release pass verification.

## Install and open

1. Verify the download against `SHA256SUMS` in the same release folder using `shasum -a 256 -c SHA256SUMS`.
2. Open the `.dmg` and drag **Thesaurus.app** onto **Applications**. Replacing an existing version requires quitting the app and stopping its background service first.
3. Eject the **Thesaurus Alpha** disk image, then open Thesaurus from Applications. macOS may require explicit approval for an unnotarized local build through its standard Privacy & Security controls.
4. Explore the synthetic sample workspace or add a note. No account, developer tools, or model download is required for the bundled app's basic local workflows. Base transcription and OCR helpers are bundled; recording permissions are requested when those features are used.
5. Choose **Help → Third-party licenses** to read bundled license notices. The DMG also includes a `Notices` folder, original license texts, and corresponding sources for included MPL components.

Optional agent runtimes and configured external tools still require their own installations and authentication. Local OCR uses the bundled Vision helper. No compiler is invoked by that helper at runtime.

## Quit, update, and uninstall

Quit from the Thesaurus menu or the menu-bar companion. Quitting pauses an active recording before closing the desktop. The background service persists to support paired browsers and connected agents. Before replacing or removing the app, stop the associated `thesaurus serve` process in Activity Monitor; close paired clients first. No login item or system-wide service is installed by this DMG.

To update, stop the old app/service, retain a copy of the complete workspace directory while the service is stopped, then replace the app in Applications. Automated backup/restore and upgrade recovery are still release requirements. Avoid opening a workspace with an older version after an upgrade.

To uninstall the app, stop it and its background service, then move **Thesaurus.app** from Applications to the Trash. Workspace data is retained in the user's application-support directory, or the directory selected with `THESAURUS_DATA_DIR`. Removing the app does not erase transcripts, credentials, grant files, or Bragbook history. A supported in-app data deletion flow is not included in this alpha.

## Fresh-Mac acceptance checklist

Run this on an Apple Silicon Mac without Rust, Xcode Command Line Tools, Node, or the source checkout, using only synthetic data:

- Verify checksums, mount the DMG, copy the app to Applications, eject the image, and launch from Applications.
- Confirm a fresh workspace opens and the bundled service starts without asking to install developer tools.
- Import a synthetic note, restart the app, and confirm it persists.
- Enable Bragbook, grant the synthetic source, collect, inspect citations, review, accept/dismiss, and export selected accepted entries.
- Open the bundled third-party licenses through Help while offline.
- Exercise selected-window OCR and microphone/system-audio recording with explicit permissions, then confirm pause/stop and offline Base transcription.
- Quit, stop the associated service, replace the app, restart, and confirm data persists.
- Uninstall the app and confirm workspace data is retained.

Record the exact macOS version, app version, architecture, results, and permission failures. Local build checks and a mounted-DMG service smoke test do not establish this clean-machine acceptance.
