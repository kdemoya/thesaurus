#!/bin/sh
# Compile the Vision boundary once; installed apps never invoke a Swift compiler.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PROFILE=${1:-debug}
case "$PROFILE" in debug|release) ;; *) echo 'Usage: scripts/build-ocr.sh [debug|release]'; exit 1;; esac
if [ "$(uname -s)" != Darwin ]; then
  echo 'The native OCR helper requires macOS and the Apple command line tools.' >&2
  exit 1
fi
mkdir -p "$ROOT/target/$PROFILE" "$ROOT/.local/swift-modules"
swiftc -O -module-cache-path "$ROOT/.local/swift-modules" \
  -target "$(uname -m)-apple-macos12.0" \
  "$ROOT/crates/core/src/capture/recognize.swift" -o "$ROOT/target/$PROFILE/thesaurus-ocr"

# The desktop bundle build replaces this provisional signature with its signer.
codesign --force --sign - --identifier dev.thesaurus.ocr "$ROOT/target/$PROFILE/thesaurus-ocr"
codesign --verify --strict "$ROOT/target/$PROFILE/thesaurus-ocr"
