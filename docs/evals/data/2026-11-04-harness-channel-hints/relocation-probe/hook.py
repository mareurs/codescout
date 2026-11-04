"""PostToolUse hook for the relocation probe. argv[1] = move | crash.

move:  parse the first text block as a JSON envelope, drop its `hint`, return the rest as updatedToolOutput
       (same shape as the input: list stays a list, string stays a string), and put the hint in additionalContext.
crash: exit 1 after logging, to see what the CLI does with a failing hook (fail-open check).
Always logs the raw tool_response shape to $RP_LOG.
"""
import json, os, sys

mode = sys.argv[1] if len(sys.argv) > 1 else "move"
data = json.load(sys.stdin)
tr = data.get("tool_response")
log = os.environ.get("RP_LOG")
if log:
    with open(log, "a") as f:
        f.write(json.dumps({"mode": mode, "event_keys": sorted(data), "tool_response_type": type(tr).__name__,
                            "tool_response": tr}) + "\n")
if mode == "crash":
    sys.exit(1)

shape, text = None, None
if isinstance(tr, list):
    shape = "list"
    for b in tr:
        if isinstance(b, dict) and b.get("type") == "text":
            text = b.get("text"); break
elif isinstance(tr, str):
    shape, text = "str", tr
elif isinstance(tr, dict) and isinstance(tr.get("content"), list):
    shape = "dict"
    for b in tr["content"]:
        if isinstance(b, dict) and b.get("type") == "text":
            text = b.get("text"); break
try:
    env = json.loads(text) if text else None
except ValueError:
    env = None
if not isinstance(env, dict) or not isinstance(env.get("hint"), str):
    print("{}"); sys.exit(0)
hint = env.pop("hint")
new_text = json.dumps(env)
replacement = [{"type": "text", "text": new_text}] if shape in ("list", "dict") else new_text
print(json.dumps({"hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "updatedToolOutput": replacement,
    "additionalContext": "[cs-hint] " + hint}}))
