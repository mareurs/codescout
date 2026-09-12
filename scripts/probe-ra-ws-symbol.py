#!/usr/bin/env python3
"""Ask rust-analyzer directly what `workspace/symbol` returns for a query.

Exists because every reading taken through codescout is confounded: a query that
returns nothing from the LSP falls through to the tree-sitter arm, and the output
of the two arms is not distinguishable from outside. This talks to rust-analyzer
over stdio with no codescout in the path, so the answer is the protocol's.

Usage: probe-ra-ws-symbol.py <project-root> <query> [<query> ...]
"""
import json
import os
import subprocess
import sys
import threading
import time

ROOT = os.path.abspath(sys.argv[1])
QUERIES = sys.argv[2:] or ["parse"]


def frame(obj):
    body = json.dumps(obj).encode()
    return b"Content-Length: %d\r\n\r\n%s" % (len(body), body)


def reader(proc, out, done):
    """Collect framed responses until the sentinel id arrives."""
    buf = b""
    while True:
        chunk = proc.stdout.read(1)
        if not chunk:
            break
        buf += chunk
        if buf.endswith(b"\r\n\r\n"):
            headers = buf.decode("utf-8", "replace")
            length = 0
            for line in headers.split("\r\n"):
                if line.lower().startswith("content-length:"):
                    length = int(line.split(":", 1)[1].strip())
            payload = proc.stdout.read(length)
            buf = b""
            try:
                msg = json.loads(payload)
            except Exception:
                continue
            out.append(msg)
            if msg.get("id") == 9999:
                done.set()
                return


proc = subprocess.Popen(
    ["rust-analyzer"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.DEVNULL,
)
msgs = []
done = threading.Event()
threading.Thread(target=reader, args=(proc, msgs, done), daemon=True).start()

proc.stdin.write(
    frame(
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "processId": os.getpid(),
                "rootUri": "file://" + ROOT,
                "capabilities": {},
                "workspaceFolders": [{"uri": "file://" + ROOT, "name": "root"}],
            },
        }
    )
)
proc.stdin.flush()
time.sleep(3)
proc.stdin.write(frame({"jsonrpc": "2.0", "method": "initialized", "params": {}}))
proc.stdin.flush()

# rust-analyzer answers workspace/symbol before indexing completes, and answers it
# DIFFERENTLY -- which is one of the candidate explanations. Wait long enough that a
# partial index is not what is being measured.
print("indexing for 90s...", file=sys.stderr)
time.sleep(90)

for n, q in enumerate(QUERIES):
    rid = 9999 if n == len(QUERIES) - 1 else 100 + n
    proc.stdin.write(
        frame(
            {
                "jsonrpc": "2.0",
                "id": rid,
                "method": "workspace/symbol",
                "params": {"query": q},
            }
        )
    )
    proc.stdin.flush()
    time.sleep(4)

done.wait(timeout=60)
time.sleep(1)

by_id = {m["id"]: m for m in msgs if isinstance(m, dict) and "id" in m and "result" in m}
for n, q in enumerate(QUERIES):
    rid = 9999 if n == len(QUERIES) - 1 else 100 + n
    res = by_id.get(rid, {}).get("result")
    if res is None:
        print(f"\n=== query {q!r}: NO RESPONSE ===")
        continue
    kinds = {}
    in_tree = []
    for s in res:
        k = s.get("kind")
        kinds[k] = kinds.get(k, 0) + 1
        uri = s.get("location", {}).get("uri", "")
        if uri.startswith("file://" + ROOT):
            in_tree.append((s.get("name"), k, uri[len("file://" + ROOT) + 1 :]))
    print(f"\n=== query {q!r}: {len(res)} symbols, {len(in_tree)} in-tree ===")
    print(f"    kind histogram (LSP SymbolKind ints): {sorted(kinds.items())}")
    exact = [t for t in in_tree if t[0] == q]
    print(f"    in-tree with name EXACTLY {q!r}: {len(exact)}")
    for name, k, path in exact[:15]:
        print(f"      kind={k:<3} {path}")
    if not exact:
        for name, k, path in in_tree[:10]:
            print(f"      (non-exact) kind={k:<3} {name}  {path}")

proc.kill()
