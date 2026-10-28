#!/bin/sh
# Build the small Apple capture boundary; session and processing logic stays Rust.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PROFILE=${1:-debug}
case "$PROFILE" in debug|release) ;; *) echo 'Usage: scripts/build-audio.sh [debug|release]'; exit 1;; esac
mkdir -p "$ROOT/target/$PROFILE" "$ROOT/.local/swift-modules"
swiftc -parse-as-library -O -module-cache-path "$ROOT/.local/swift-modules" \
  -target "$(uname -m)-apple-macos12.0" \
  -Xlinker -sectcreate -Xlinker __TEXT -Xlinker __info_plist \
  -Xlinker "$ROOT/apps/desktop/src-tauri/AudioInfo.plist" \
  "$ROOT/crates/core/src/capture/record.swift" -o "$ROOT/target/$PROFILE/thesaurus-audio"

# Replace the linker's provisional signature: bind the embedded usage strings
# and stable helper identifier. The final desktop build applies its chosen signer.
codesign --force --sign - --identifier dev.thesaurus.audio "$ROOT/target/$PROFILE/thesaurus-audio"
codesign --verify --strict "$ROOT/target/$PROFILE/thesaurus-audio"
