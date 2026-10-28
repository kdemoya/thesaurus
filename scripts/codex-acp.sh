#!/bin/sh
set -eu
# Finder-launched desktop services do not inherit an interactive shell PATH.
# Use the usual user/Homebrew install locations without reading shell startup files.
export PATH="$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export CODEX_PATH="$ROOT/scripts/codex-for-acp.sh"
export INITIAL_AGENT_MODE=read-only
export CODEX_CONFIG='{"model_provider":"openai","forced_login_method":"chatgpt","approval_policy":"on-request","sandbox_mode":"read-only","features":{"shell_tool":false}}'
unset CODEX_API_KEY OPENAI_API_KEY OPENAI_ACCESS_TOKEN DEFAULT_AUTH_REQUEST MODEL_PROVIDER APP_SERVER_LOGS
exec "$ROOT/.tools/acp/node_modules/.bin/codex-acp" "$@"
