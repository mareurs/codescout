#!/usr/bin/env python3
"""Fake LSP server that records the params of the `initialize` request it receives.

Used to check what codescout puts on the wire in `initialize` (for example
`initializationOptions`), which no test can see from the Rust side.

Protocol: JSON-RPC 2.0 over stdio with Content-Length headers (LSP transport).

Usage: python3 fake_lsp_records_initialize.py <path to write the initialize params to>
"""

import json
import sys


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
    sys.stdout.buffer.write(header + body)
    sys.stdout.buffer.flush()


def main():
    record_to = sys.argv[1]

    while True:
        msg = read_message()
        if msg is None:
            break

        method = msg.get("method")
        msg_id = msg.get("id")

        # Notifications (no id) — ignore
        if msg_id is None:
            continue

        if method == "initialize":
            with open(record_to, "w", encoding="utf-8") as out:
                json.dump(msg.get("params"), out)
            write_message({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {"capabilities": {"documentSymbolProvider": True}},
            })
        elif method == "shutdown":
            write_message({"jsonrpc": "2.0", "id": msg_id, "result": None})
        else:
            write_message({"jsonrpc": "2.0", "id": msg_id, "result": None})


if __name__ == "__main__":
    main()
