#!/usr/bin/env bash
# Relocation probe. Usage: run.sh <claude-binary> <label> <move|none|crash>
# One haiku call. Evidence is the transcript (tool_result text and hook_additional_context), not the model's words.
set -euo pipefail
D="$(cd "$(dirname "$0")" && pwd)"
BIN="$1"; LABEL="$2"; MODE="$3"
RUN="$D/runs/$LABEL-$MODE"; rm -rf "$RUN"; mkdir -p "$RUN/work"
echo "binary: $("$BIN" --version 2>&1 | head -1)  sha256: $(sha256sum "$BIN" | cut -c1-16)" | tee "$RUN/binary.txt"
cat > "$RUN/mcp.json" <<EOF
{"mcpServers": {"stub": {"command": "python3", "args": ["-I", "$D/stub_mcp.py"]}}}
EOF
if [ "$MODE" = none ]; then
  echo '{}' > "$RUN/settings.json"
else
  cat > "$RUN/settings.json" <<EOF
{"hooks": {"PostToolUse": [{"matcher": "mcp__stub__probe",
  "hooks": [{"type": "command", "command": "python3 -I $D/hook.py $MODE"}]}]}}
EOF
fi
SID="$(python3 -I -c 'import uuid;print(uuid.uuid4())')"
echo "$SID" > "$RUN/session-id.txt"
cd "$RUN/work"
set +e
RP_LOG="$RUN/hook-log.jsonl" "$BIN" -p 'Call the tool mcp__stub__probe exactly once, then reply with the single word DONE.' \
  --model claude-haiku-5-5 --session-id "$SID" --setting-sources local \
  --mcp-config "$RUN/mcp.json" --strict-mcp-config --settings "$RUN/settings.json" \
  --allowedTools mcp__stub__probe --max-budget-usd 0.2 \
  --output-format stream-json --verbose > "$RUN/stream.jsonl" 2> "$RUN/stderr.txt"
echo "exit: $?" | tee "$RUN/exit.txt"
echo "$SID"
