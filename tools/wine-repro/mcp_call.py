"""Minimal MCP streamable-HTTP client for the Wine reproduction environment.

Usage: mcp_call.py <port> <tool> '<json args>' [<tool> '<json args>' ...]

Each tool call is timed; the tool's text result (first 1500 chars) and the elapsed seconds are
printed. The server is a Windows codescout.exe under Wine, started with `--transport http
--auth-token wine-test`. The client runs on the Linux side on purpose: a Wine Node process cannot
open a pipe that a Linux process created (`open EBADF`), but HTTP on 127.0.0.1 crosses the boundary.
"""
import http.client
import json
import sys
import time

port = int(sys.argv[1])
calls = list(zip(sys.argv[2::2], sys.argv[3::2]))
HEAD = {
    "Authorization": "Bearer wine-test",
    "Content-Type": "application/json",
    "Accept": "application/json, text/event-stream",
}
session = None


def post(body, want_id=None):
    global session
    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=300)
    headers = dict(HEAD)
    if session:
        headers["Mcp-Session-Id"] = session
    conn.request("POST", "/mcp", json.dumps(body), headers)
    resp = conn.getresponse()
    session = resp.getheader("Mcp-Session-Id") or session
    if want_id is None:
        resp.read()
        return None
    if "event-stream" in resp.getheader("Content-Type", ""):
        for raw in resp:
            line = raw.decode().rstrip("\r\n")
            if line.startswith("data:") and line[5:].strip():
                msg = json.loads(line[5:])
                if msg.get("id") == want_id:
                    return msg
        return None
    return json.loads(resp.read())


init = post({"jsonrpc": "2.0", "id": 1, "method": "initialize",
             "params": {"protocolVersion": "2025-03-26", "capabilities": {},
                        "clientInfo": {"name": "wine-repro", "version": "0"}}}, 1)
print("initialize ok:", bool(init and "result" in init), flush=True)
post({"jsonrpc": "2.0", "method": "notifications/initialized"})

for n, (tool, args) in enumerate(calls, start=2):
    t0 = time.time()
    reply = post({"jsonrpc": "2.0", "id": n, "method": "tools/call",
                  "params": {"name": tool, "arguments": json.loads(args)}}, n)
    took = time.time() - t0
    text = ""
    if reply:
        result = reply.get("result") or {}
        text = "".join(c.get("text", "") for c in result.get("content", [])) or json.dumps(reply)[:1500]
    print("--- %s %s  (%.1fs)" % (tool, args, took))
    print(text[:1500], flush=True)
