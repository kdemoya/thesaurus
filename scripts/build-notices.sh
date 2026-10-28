#!/bin/sh
# Build a locked, offline-readable notice bundle for the macOS application.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
exec python3 "$ROOT/scripts/build-notices.py" "${1:-debug}"
