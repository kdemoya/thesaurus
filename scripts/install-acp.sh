#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
npm install --prefix "$ROOT/.tools/acp" --cache "$ROOT/.tools/npm-cache" --no-audit --no-fund --save-exact @agentclientprotocol/codex-acp@2.2.2
printf 'Codex ACP adapter installed locally. Configure command = "%s/scripts/codex-acp.sh"\n' "$ROOT"
