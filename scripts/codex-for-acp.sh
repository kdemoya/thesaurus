#!/bin/sh
# Keep Codex-owned login; override unrelated tools, notifications, and project instructions for this process.
set -eu
exec codex -c 'mcp_servers={}' -c 'plugins={}' -c 'features.apps=false' -c 'features.plugins=false' -c 'features.shell_tool=false' -c 'project_doc_max_bytes=0' -c 'notify=[]' -c 'web_search="disabled"' "$@"
