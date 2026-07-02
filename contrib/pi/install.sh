#!/usr/bin/env bash
# Symlink the codescout<->Pi integration files into the Pi agent dir.
set -euo pipefail

SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AGENT_DIR="${PI_CODING_AGENT_DIR:-$HOME/.pi/agent}"

mkdir -p "$AGENT_DIR/extensions"

if [ ! -e "$SRC/mcp.json" ]; then
  cp "$SRC/mcp.json.example" "$SRC/mcp.json"
  echo "Created $SRC/mcp.json from mcp.json.example — fill in your API keys before use."
fi

ln -sf "$SRC/mcp.json" "$AGENT_DIR/mcp.json"
ln -sf "$SRC/codescout-mode.ts" "$AGENT_DIR/extensions/codescout-mode.ts"

if [ -e "$AGENT_DIR/AGENTS.md" ] && [ ! -L "$AGENT_DIR/AGENTS.md" ]; then
  mv "$AGENT_DIR/AGENTS.md" "$AGENT_DIR/AGENTS.md.bak"
  echo "Backed up existing AGENTS.md -> AGENTS.md.bak"
fi
ln -sf "$SRC/AGENTS.md" "$AGENT_DIR/AGENTS.md"

echo "Installed codescout<->Pi integration into $AGENT_DIR"
echo "Next: launch pi in a repo, run '/mcp reconnect codescout' once to warm the cache."
