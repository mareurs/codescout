"""Minimal MCP stdio client for the live controls: no model, no Claude Code, no network beyond
the study stack. Runs INSIDE an agent container with the image's python3.

    python3 mcp_probe.py <mcp.json> <calls.json>

Starts the `codescout` server exactly as the arm's mcp.json configures it (command, args, env),
in the current directory (codescout activates its CWD, as under Claude Code), performs the MCP
handshake, lists the tools, makes each call in calls.json ([[tool, arguments], ...]) and prints
one JSON document: {"server": ..., "tools": [...], "calls": [{"tool", "arguments", "result"}]}.
Stdlib only.
"""
import json
import os
import select
import subprocess
import sys
import time

TIMEOUT = float(os.environ.get("MCP_PROBE_TIMEOUT", "300"))


def main():
    server = json.load(open(sys.argv[1]))["mcpServers"]["codescout"]
    calls = json.load(open(sys.argv[2]))
    env = dict(os.environ, **server.get("env", {}))
    stderr = open(os.environ.get("MCP_PROBE_STDERR", "/tmp/mcp-probe-server.log"), "w")
    proc = subprocess.Popen([server["command"], *server.get("args", [])], stdin=subprocess.PIPE,
                            stdout=subprocess.PIPE, stderr=stderr, env=env)
    buf = b""

    def send(msg):
        proc.stdin.write((json.dumps(msg) + "\n").encode())
        proc.stdin.flush()

    def recv(want_id):
        nonlocal buf
        deadline = time.monotonic() + TIMEOUT
        while True:
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                if not line.strip():
                    continue
                msg = json.loads(line)
                if msg.get("id") == want_id and ("result" in msg or "error" in msg):
                    return msg
            left = deadline - time.monotonic()
            if left <= 0:
                raise TimeoutError(f"no response to request {want_id} in {TIMEOUT}s")
            ready, _, _ = select.select([proc.stdout], [], [], left)
            if ready:
                chunk = os.read(proc.stdout.fileno(), 1 << 16)
                if not chunk:
                    raise RuntimeError(f"server exited (code {proc.poll()}) before reply "
                                       f"{want_id}")
                buf += chunk

    out = {"server": {"command": server["command"], "args": server.get("args", [])},
           "calls": []}
    try:
        send({"jsonrpc": "2.0", "id": 1, "method": "initialize",
              "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                         "clientInfo": {"name": "sab-mcp-probe", "version": "1"}}})
        init = recv(1)
        out["initialize"] = {"serverInfo": init.get("result", {}).get("serverInfo"),
                             "error": init.get("error")}
        send({"jsonrpc": "2.0", "method": "notifications/initialized"})
        send({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}})
        tools = recv(2)
        out["tools"] = sorted(t["name"] for t in tools.get("result", {}).get("tools", []))
        for n, (tool, arguments) in enumerate(calls, start=3):
            started = time.monotonic()
            send({"jsonrpc": "2.0", "id": n, "method": "tools/call",
                  "params": {"name": tool, "arguments": arguments}})
            try:
                reply = recv(n)
            except (TimeoutError, RuntimeError) as e:
                reply = {"probe_error": str(e)}
            out["calls"].append({"tool": tool, "arguments": arguments,
                                 "seconds": round(time.monotonic() - started, 2),
                                 "result": reply.get("result"), "error": reply.get("error"),
                                 "probe_error": reply.get("probe_error")})
    finally:
        try:
            proc.stdin.close()
            proc.wait(timeout=20)
        except Exception:
            proc.kill()
    print(json.dumps(out, indent=2))


if __name__ == "__main__":
    main()
