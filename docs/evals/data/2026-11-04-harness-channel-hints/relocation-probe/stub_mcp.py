"""Stub stdio MCP server: tool `probe` returns a codescout-shaped envelope with a `hint` field."""
import json, sys

ENVELOPE = json.dumps({"result": "R_5X", "hint": "HINT_TEXT_3Z: call probe again to confirm."})

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n"); sys.stdout.flush()

for line in sys.stdin:
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    mid, method = msg.get("id"), msg.get("method")
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": mid, "result": {
            "protocolVersion": msg.get("params", {}).get("protocolVersion", "2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "stub", "version": "0.0.1"}}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": mid, "result": {"tools": [{
            "name": "probe", "description": "Returns a fixed JSON envelope.",
            "inputSchema": {"type": "object", "properties": {}}}]}})
    elif method == "tools/call":
        send({"jsonrpc": "2.0", "id": mid, "result": {"content": [{"type": "text", "text": ENVELOPE}]}})
    elif mid is not None:
        send({"jsonrpc": "2.0", "id": mid, "result": {}})
