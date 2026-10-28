#!/bin/sh
# Build a self-contained macOS app with a Rust service and shared browser assets.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
export CARGO_HOME="$ROOT/.tools/cargo"
export RUSTUP_HOME="$ROOT/.tools/rustup"
export PATH="$CARGO_HOME/bin:$ROOT/.tools/bin:$PATH"
export CARGO_PROFILE_RELEASE_DEBUG=0
MODE=${1:---debug}
case "$MODE" in --debug) PROFILE=debug; FLAG=;; --release) PROFILE=release; FLAG=--release;; *) echo 'Usage: scripts/build-desktop.sh [--debug|--release]'; exit 1;; esac
./scripts/build-audio.sh "$PROFILE"
./scripts/build-transcription.sh "$PROFILE"
./scripts/build-web.sh --release
./scripts/cargo.sh build --locked -p thesaurus $FLAG
# Replace the generated distribution so earlier hashed WASM/JS files do not accumulate.
rm -rf apps/desktop/src-tauri/resources/web
mkdir -p apps/desktop/src-tauri/resources/web
cp "target/$PROFILE/thesaurus" apps/desktop/src-tauri/resources/thesaurus
cp "target/$PROFILE/thesaurus-audio" apps/desktop/src-tauri/resources/thesaurus-audio
rm -rf apps/desktop/src-tauri/resources/transcription
cp -R "target/$PROFILE/transcription" apps/desktop/src-tauri/resources/transcription
cp -R apps/ui/dist/. apps/desktop/src-tauri/resources/web/
# Explicitly sign resource executables before Tauri seals the outer bundle.
# Debug builds can opt into the ad hoc fallback; release builds require an identity.
if [ "$MODE" = --debug ]; then
  python3 scripts/sign-macos.py --resources apps/desktop/src-tauri/resources --allow-ad-hoc
else
  python3 scripts/sign-macos.py --resources apps/desktop/src-tauri/resources
fi
if [ ! -x .tools/tauri/node_modules/.bin/tauri ]; then
  npm install --prefix .tools/tauri --cache .tools/npm-cache --no-audit --no-fund --save-exact @tauri-apps/cli@2.12.3
fi
cd apps/desktop
set -- build --bundles app
if [ "$MODE" = --debug ]; then set -- "$@" --debug; fi
# Bind the bundle identifier, Info.plist and resources even for local builds.
# The linker's executable-only signature is insufficient for OS notification
# registration. An explicitly configured release identity keeps precedence.
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  set -- "$@" --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'
fi
"$ROOT/.tools/tauri/node_modules/.bin/tauri" "$@"

# A private local development keychain is deliberately outside the general
# keychain search list. Tauri makes the initial ad hoc bundle; finish signing
# it with the persistent local identity after bundling, before any activation.
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ] && [ -f "$ROOT/.local/signing/identity.json" ]; then
  python3 "$ROOT/scripts/sign-macos.py" --app "$ROOT/target/$PROFILE/bundle/macos/Thesaurus.app"
fi
