#!/usr/bin/env python3
"""Stage 5.0 firing census: drive the real codescout server over MCP stdio, no model.

usage: python3 -I census.py <task_id|ALL> <scratch_dir> [--raw]
"""
import json, os, re, select, shutil, subprocess, sys, time
from pathlib import Path

BIN = "/home/marius/work/claude/codescout/target/release/codescout"
DATA = Path("/home/marius/work/claude/hce-data")
TASKS = Path("/home/marius/work/claude/codescout/docs/evals/data/2026-11-04-harness-channel-hints/tasks.jsonl")
PROJECT_TOML = '[project]\nname = "codescout"\nlanguages = ["rust"]\n'


SYMBOL_OVERRIDE = {"hce-03-cross_root_cites_edges": "impl DoctorScope/cross_root_cites_edges"}


class Mcp:
    def __init__(self, cwd, env):
        self.p = subprocess.Popen([BIN, "start", "--debug"], cwd=cwd, env=env, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.n = 0
        self.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                "clientInfo": {"name": "census", "version": "0"}}, timeout=60)
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "method": "notifications/initialized"}) + "\n")
        self.p.stdin.flush()

    def rpc(self, method, params, timeout=300):
        self.n += 1
        mid = self.n
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": mid, "method": method, "params": params}) + "\n")
        self.p.stdin.flush()
        end = time.time() + timeout
        while time.time() < end:
            r, _, _ = select.select([self.p.stdout], [], [], 1.0)
            if not r:
                continue
            line = self.p.stdout.readline()
            if not line:
                raise RuntimeError("server closed stdout")
            try:
                msg = json.loads(line)
            except ValueError:
                continue
            if msg.get("id") == mid:
                return msg
        raise TimeoutError(method)

    def call(self, name, args, timeout=300):
        return self.rpc("tools/call", {"name": name, "arguments": args}, timeout)

    def close(self):
        try:
            self.p.stdin.close()
            self.p.wait(timeout=10)
        except Exception:
            self.p.kill()


def text_of(msg):
    res = msg.get("result", {})
    return "\n".join(c.get("text", "") for c in res.get("content", []) if c.get("type") == "text")


def new_body(old_body, fn, new_sig):
    m = re.search(r"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?fn\s+" + re.escape(fn) + r"\b", old_body)
    if not m:
        raise ValueError("fn header not found in body")
    brace = old_body.index("{", m.end())
    return old_body[:m.start()] + new_sig + " " + old_body[brace:]


def run_task(t, scratch, raw):
    work = Path(scratch) / t["task_id"]
    if work.exists():
        shutil.rmtree(work, ignore_errors=True)
    shutil.copytree(DATA / "tasks" / t["task_id"] / "tree", work, symlinks=True)
    (work / ".codescout").mkdir(exist_ok=True)
    (work / ".codescout/project.toml").write_text(PROJECT_TOML)
    env = dict(os.environ, HOME=str(Path(scratch) / "home"), CARGO_HOME=str(DATA / "cargo-home"),
               CARGO_NET_OFFLINE="true", RUSTUP_TOOLCHAIN="stable", RUSTUP_HOME="/home/marius/.rustup")
    (Path(scratch) / "home").mkdir(exist_ok=True)
    out = {"task_id": t["task_id"], "fn": t["fn"], "path": t["path"]}
    s = Mcp(str(work), env)
    try:
        a = s.call("workspace", {"action": "activate", "path": str(work)}, 120)
        if raw:
            print("ACTIVATE:", json.dumps(a)[:600])
        src = (work / t["path"]).read_text()
        m = re.search(r"(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?fn\s+" + re.escape(t["fn"]) + r"\b", src)
        if not m:
            out["error"] = "fn header not found in file"
            return out
        i = src.index("{", m.end())
        depth = 0
        for j in range(i, len(src)):
            if src[j] == "{":
                depth += 1
            elif src[j] == "}":
                depth -= 1
                if depth == 0:
                    break
        body = src[m.start():j + 1]
        nb = new_body(body, t["fn"], t["new_sig"])
        e = s.call("edit_code", {"action": "replace", "path": t["path"], "symbol": SYMBOL_OVERRIDE.get(t["task_id"], t["fn"]), "body": nb}, 600)
        etxt = text_of(e)
        out["edit_reply"] = etxt
        try:
            ej = json.loads(etxt)
        except ValueError:
            ej = {}
        out["advice"] = ej.get("advice")
        out["hint"] = ej.get("hint")
        out["verified"] = (ej.get("advice") or {}).get("verified")
        return out
    finally:
        s.close()


if __name__ == "__main__":
    which, scratch = sys.argv[1], sys.argv[2]
    raw = "--raw" in sys.argv
    tasks = [json.loads(l) for l in TASKS.read_text().splitlines() if l.strip()]
    if which != "ALL":
        tasks = [t for t in tasks if t["task_id"] == which]
    Path(scratch).mkdir(parents=True, exist_ok=True)
    rows = []
    for t in tasks:
        try:
            r = run_task(t, scratch, raw)
        except Exception as ex:
            r = {"task_id": t["task_id"], "error": repr(ex)}
        rows.append(r)
        print(json.dumps({k: v for k, v in r.items() if k != "edit_reply"})[:700], flush=True)
    (Path(scratch) / "census.json").write_text(json.dumps(rows, indent=2) + "\n")
