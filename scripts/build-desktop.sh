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
SIGNING=${2:---local}
case "$MODE" in --debug) PROFILE=debug; FLAG=;; --release) PROFILE=release; FLAG=--release;; *) echo 'Usage: scripts/build-desktop.sh [--debug|--release] [--local|--distribution]'; exit 1;; esac
case "$SIGNING" in
  --local) SIGN_FLAG=--allow-ad-hoc;;
  --distribution)
    [ "$MODE" = --release ] || { echo 'Distribution signing requires --release'; exit 1; }
    SIGN_FLAG=--distribution
    python3 scripts/sign-macos.py --distribution --check-identity;;
  *) echo 'Choose --local or --distribution signing'; exit 1;;
esac
./scripts/build-audio.sh "$PROFILE"
./scripts/build-ocr.sh "$PROFILE"
./scripts/build-transcription.sh "$PROFILE"
# Notice harvesting is offline and must also work on a clean build host.
./scripts/cargo.sh fetch --locked
./scripts/build-notices.sh "$PROFILE"
./scripts/build-web.sh --release --locked
./scripts/cargo.sh build --locked -p thesaurus $FLAG
# Replace the generated distribution so earlier hashed WASM/JS files do not accumulate.
rm -rf apps/desktop/src-tauri/resources/web
mkdir -p apps/desktop/src-tauri/resources/web
cp "target/$PROFILE/thesaurus" apps/desktop/src-tauri/resources/thesaurus
cp "target/$PROFILE/thesaurus-audio" apps/desktop/src-tauri/resources/thesaurus-audio
cp "target/$PROFILE/thesaurus-ocr" apps/desktop/src-tauri/resources/thesaurus-ocr
rm -rf apps/desktop/src-tauri/resources/transcription
cp -R "target/$PROFILE/transcription" apps/desktop/src-tauri/resources/transcription
cp -R apps/ui/dist/. apps/desktop/src-tauri/resources/web/
rm -rf apps/desktop/src-tauri/resources/notices
cp -R "target/$PROFILE/notices" apps/desktop/src-tauri/resources/notices
# Explicitly sign resource executables before Tauri seals the outer bundle.
# Local review builds allow ad hoc signing; distribution builds require Developer ID.
python3 scripts/sign-macos.py --resources apps/desktop/src-tauri/resources "$SIGN_FLAG"
if [ ! -x .tools/tauri/node_modules/.bin/tauri ]; then
  npm install --prefix .tools/tauri --cache .tools/npm-cache --no-audit --no-fund --save-exact @tauri-apps/cli@2.12.3
fi
cd apps/desktop
set -- build --bundles app --ci --config '{"bundle":{"macOS":{"signingIdentity":"-"}}}'
if [ "$MODE" = --debug ]; then set -- "$@" --debug; fi
# Tauri makes the bundle; our final pass owns the selected keychain, all nested
# helpers, timestamps and hardened runtime. Disable Tauri's automatic notary
# upload so only the explicit packaging command can submit a release to Apple.
env -u APPLE_SIGNING_IDENTITY -u APPLE_ID -u APPLE_PASSWORD -u APPLE_API_KEY \
  -u APPLE_API_ISSUER -u APPLE_API_KEY_PATH -u APPLE_CERTIFICATE \
  -u APPLE_CERTIFICATE_PASSWORD -u APPLE_TEAM_ID \
  "$ROOT/.tools/tauri/node_modules/.bin/tauri" "$@" -- --locked

# A private local development keychain is deliberately outside the general
# keychain search list. Tauri makes the initial ad hoc bundle; finish signing
# it with the persistent local identity after bundling, before any activation.
python3 "$ROOT/scripts/sign-macos.py" --app "$ROOT/target/$PROFILE/bundle/macos/Thesaurus.app" "$SIGN_FLAG"
