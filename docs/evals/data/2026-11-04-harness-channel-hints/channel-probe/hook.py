"""PostToolUse hook: replace the stub's output and add context. Mode from argv[1]."""
import json, sys

mode = sys.argv[1] if len(sys.argv) > 1 else "string"
json.load(sys.stdin)  # consume the hook input
replacement = "REPLACED_OUTPUT_K4"
if mode == "content":
    replacement = [{"type": "text", "text": "REPLACED_OUTPUT_K4"}]
print(json.dumps({"hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "updatedToolOutput": replacement,
    "additionalContext": "CTX_TOKEN_9M"}}))
