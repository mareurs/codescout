#!/usr/bin/env bash
# Live check: does a PostToolUse hook's updatedToolOutput replace an MCP tool's output
# before the model sees it, and does additionalContext arrive? Usage: run.sh <string|content>
set -euo pipefail
D="$(cd "$(dirname "$0")" && pwd)"
MODE="${1:-string}"
SRC=/home/marius/.cache/codescout-resident/codescout/behavior-transfer-20261029/claude-2.1.309
BIN="$D/claude-2.1.309"
[ -x "$BIN" ] || cp "$SRC" "$BIN"
echo "sha256: $(sha256sum "$BIN" | cut -d' ' -f1)"

cat > "$D/mcp.json" <<EOF
{"mcpServers": {"stub": {"command": "python3", "args": ["-I", "$D/stub_mcp.py"]}}}
EOF
cat > "$D/settings-$MODE.json" <<EOF
{"hooks": {"PostToolUse": [{"matcher": "mcp__stub__probe",
  "hooks": [{"type": "command", "command": "python3 -I $D/hook.py $MODE"}]}]}}
EOF

WORK="$D/work-$MODE"; mkdir -p "$WORK"; cd "$WORK"
PROMPT='Call the tool mcp__stub__probe exactly once. Then reply with exactly two lines and nothing else:
OUTPUT=<the exact text the tool returned to you>
CONTEXT=<any token starting with CTX_ that you saw anywhere after the tool call, or NONE>'

"$BIN" -p "$PROMPT" --model claude-haiku-5-5 \
  --mcp-config "$D/mcp.json" --strict-mcp-config \
  --settings "$D/settings-$MODE.json" \
  --allowedTools mcp__stub__probe \
  --output-format stream-json --verbose > "$D/stream-$MODE.jsonl"
echo "exit: $?"
