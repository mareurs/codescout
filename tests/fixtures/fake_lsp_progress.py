#!/usr/bin/env python3
"""Fake LSP server that answers `textDocument/documentSymbol` with null for N seconds after
`initialize`, while announcing that work as a `$/progress` token, then answers normally.

The shape rust-analyzer shows on a cold start: a `window/workDoneProgress/create` request, a
`begin`, null answers while it scans, then an `end` and real symbols.

Protocol: JSON-RPC 2.0 over stdio with Content-Length headers (LSP transport).

Usage: python3 fake_lsp_progress.py <null_seconds> [noprogress]
  noprogress: answer null for the same time but announce no progress at all.
"""

import json
import sys
import threading
import time

WRITE_LOCK = threading.Lock()


def read_message():
    """Read a JSON-RPC message with Content-Length header from stdin."""
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line = line.decode("utf-8").strip()
        if line == "":
            break
        if ":" in line:
            key, value = line.split(":", 1)
            headers[key.strip()] = value.strip()

    content_length = int(headers.get("Content-Length", 0))
    if content_length == 0:
        return None

    body = sys.stdin.buffer.read(content_length)
    return json.loads(body.decode("utf-8"))


def write_message(msg):
    """Write a JSON-RPC message with Content-Length header to stdout."""
    body = json.dumps(msg).encode("utf-8")
    header = f"Content-Length: {len(body)}\r\n\r\n".encode("utf-8")
    with WRITE_LOCK:
        sys.stdout.buffer.write(header + body)
        sys.stdout.buffer.flush()


def progress(kind, **extra):
    value = {"kind": kind}
    value.update(extra)
    return {"jsonrpc": "2.0", "method": "$/progress", "params": {"token": "scan", "value": value}}


def main():
    null_seconds = float(sys.argv[1])
    announce = "noprogress" not in sys.argv[2:]
    ready_at = None

    while True:
        msg = read_message()
        if msg is None:
            break

        method = msg.get("method")
        msg_id = msg.get("id")

        # A response to a request this server sent (the client answers `create` with null).
        if method is None:
            continue
        # `exit` ends the process (the client waits for it after `shutdown`).
        if method == "exit":
            sys.exit(0)
        # Other notifications (no id) — ignore
        if msg_id is None:
            continue

        if method == "initialize":
            ready_at = time.monotonic() + null_seconds
            write_message({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {"capabilities": {"documentSymbolProvider": True}},
            })
            if announce:
                write_message({"jsonrpc": "2.0", "id": 9001,
                               "method": "window/workDoneProgress/create",
                               "params": {"token": "scan"}})
                write_message(progress("begin", title="Roots Scanned"))
                threading.Timer(null_seconds, lambda: write_message(progress("end"))).start()
        elif method == "shutdown":
            write_message({"jsonrpc": "2.0", "id": msg_id, "result": None})
        elif method == "textDocument/documentSymbol":
            if ready_at is not None and time.monotonic() < ready_at:
                write_message({"jsonrpc": "2.0", "id": msg_id, "result": None})
            else:
                write_message({
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "result": [{
                        "name": "ready_symbol",
                        "kind": 12,
                        "range": {"start": {"line": 0, "character": 0},
                                  "end": {"line": 0, "character": 10}},
                        "selectionRange": {"start": {"line": 0, "character": 3},
                                           "end": {"line": 0, "character": 9}},
                    }],
                })
        else:
            write_message({"jsonrpc": "2.0", "id": msg_id, "result": None})


if __name__ == "__main__":
    main()
