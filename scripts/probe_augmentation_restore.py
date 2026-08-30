#!/usr/bin/env python3
"""Do this repo's COMMITTED augmentation sidecars actually restore?

The end-to-end claim behind `docs/augmentations/`: a machine that has never held
these augmentations gets them back from git alone, via
`librarian(action="reindex")`. Nothing else in the tree tests that conjunction.
The unit tests round-trip a *synthetic* sidecar
(`augmentation_sidecar::tests`, `reindex::tests`), and
`every_committed_sidecar_parses_and_carries_no_params` proves the real files
parse. Both can pass while the real corpus restores nothing.

What it does: copies every artifact declaring `expects_augmentation: <path>.yaml`
plus its sidecar into a throwaway **git repo**, points a codescout MCP server at
an isolated catalog that has never seen them, runs one reindex, and reports what
came back — count, per-artifact `entry_collection`, and whether `params` stayed
empty.

  python3 scripts/probe_augmentation_restore.py [--repo PATH] [--binary PATH] [--keep] [--json]

Exit 1 if fewer artifacts restored than declared a sidecar.

BLIND SPOTS — read before believing a number:

  * **A corpus with no `.git` reports `restored: 0, errors: 0`**, which is
    character-identical to "nothing needed restoring".
    `restore_declared_augmentations` resolves the sidecar against
    `current_project::lookup_git_root(path)` and `continue`s silently when there
    is none. This probe runs `git init` for exactly that reason; if you adapt it,
    keep that or the run is a false negative. (Found the hard way, 2026-08-30.)
  * The count alone cannot see a **collision** — if every artifact restored the
    same sidecar, the count is still N. The discriminator is the restored
    **prompt**, which this probe checks for uniqueness. `entry_collection` is
    NOT a valid one and is printed for information only: two unrelated trackers
    may legitimately both name their array `tasks`, so 8-of-9 distinct is a
    correct reading here, not a finding. Collision is the failure `f565504a`
    fixed, which stem-keyed sidecar names had made invisible to two unit tests
    that agreed with each other.
  * `params` must come back `{}` for every row. A non-empty params here means
    live state travelled in git, which is the drift class BL-29/BL-40/BL-42
    closed.
  * It measures **this working tree**, not HEAD. Uncommitted sidecar edits are
    included; a sidecar that exists only in the index is not.
  * It proves restore-when-absent. It does NOT exercise the never-overwrite half
    (a catalog that already holds a live augmentation) — `reindex_never_
    overwrites_a_live_augmentation` covers that in-process.
"""
import argparse, json, os, shutil, subprocess, sys, tempfile, threading, time
from pathlib import Path

DECL = "expects_augmentation:"


def find_declarers(repo: Path):
    """(artifact_rel, sidecar_rel) for every artifact naming a sidecar."""
    out = []
    for p in repo.rglob("*.md"):
        if ".git" in p.parts or "/archive/" in p.as_posix():
            continue
        try:
            head = p.read_text(errors="replace")[:4000]
        except OSError:
            continue
        if DECL not in head:
            continue
        for line in head.splitlines():
            if line.startswith(DECL):
                val = line[len(DECL):].strip().strip("'\"")
                if val.endswith((".yaml", ".yml")):
                    out.append((p.relative_to(repo), Path(val)))
                break
    return sorted(out)


def build_corpus(repo: Path, dest: Path, pairs):
    for art, side in pairs:
        for rel in (art, side):
            src = repo / rel
            if not src.exists():
                print(f"  WARN missing {rel}", file=sys.stderr)
                continue
            dst = dest / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(src, dst)
    # A git root is REQUIRED — see BLIND SPOTS.
    subprocess.run(["git", "init", "-q"], cwd=dest, check=True)
    subprocess.run(["git", "add", "-A"], cwd=dest, check=True)
    subprocess.run(
        ["git", "-c", "user.email=probe@local", "-c", "user.name=probe",
         "commit", "-qm", "probe corpus"], cwd=dest, check=True)


def reindex(binary: Path, corpus: Path, ws: Path, db: Path, log: Path):
    env = dict(os.environ)
    env["LIBRARIAN_WORKSPACE"] = str(ws)
    env["LIBRARIAN_DB"] = str(db)
    env["CODESCOUT_ALLOW_TEMP_WORKSPACE"] = "1"
    for k in ("LIBRARIAN_EMBED_MODEL", "LIBRARIAN_EMBED_URL", "LIBRARIAN_EMBED_API_KEY"):
        env.pop(k, None)

    p = subprocess.Popen(
        [str(binary), "start", "--project", str(corpus)],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=log.open("w"), env=env, text=True, bufsize=1)

    seen = {}

    def read():
        for line in p.stdout:
            try:
                m = json.loads(line)
            except Exception:
                continue
            if "id" in m:
                seen[m["id"]] = m

    threading.Thread(target=read, daemon=True).start()

    def send(o):
        p.stdin.write(json.dumps(o) + "\n")
        p.stdin.flush()

    def wait(i, secs):
        for _ in range(secs * 10):
            if i in seen:
                return seen[i]
            time.sleep(0.1)
        return None

    send({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2024-11-05", "capabilities": {},
        "clientInfo": {"name": "probe", "version": "1"}}})
    if not wait(1, 60):
        raise SystemExit("FAIL: server never answered initialize; see " + str(log))
    send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    send({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {
        "name": "librarian", "arguments": {"action": "reindex"}}})
    r = wait(2, 180)
    p.stdin.close()
    p.terminate()
    if not r:
        raise SystemExit("FAIL: reindex never returned; see " + str(log))
    if "error" in r:
        raise SystemExit("FAIL: " + json.dumps(r["error"])[:400])
    return json.loads(r["result"]["content"][0]["text"])


def inspect(db: Path, corpus: Path):
    q = """SELECT a.abs_path, COALESCE(g.entry_collection,'-'), g.params,
                  CASE WHEN g.render_template IS NULL THEN 'no' ELSE 'yes' END,
                  length(g.prompt)
           FROM artifact a JOIN artifact_augmentation g ON g.artifact_id = a.id
           ORDER BY a.abs_path;"""
    out = subprocess.run(["sqlite3", "-separator", "\t", str(db), q],
                         capture_output=True, text=True).stdout
    rows = []
    for line in out.splitlines():
        f = line.split("\t")
        if len(f) == 5:
            rows.append({
                "artifact": f[0].replace(str(corpus) + "/", ""),
                "entry_collection": f[1], "params": f[2],
                "render_template": f[3], "prompt_len": int(f[4]),
            })
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--repo", default=".")
    ap.add_argument("--binary", default="target/release/codescout")
    ap.add_argument("--keep", action="store_true", help="leave the temp dir in place")
    ap.add_argument("--json", action="store_true")
    a = ap.parse_args()

    repo = Path(a.repo).resolve()
    binary = Path(a.binary)
    if not binary.is_absolute():
        binary = (repo / binary).resolve()
    if not binary.exists():
        raise SystemExit(f"no binary at {binary} — run `cargo rb` first")

    pairs = find_declarers(repo)
    if not pairs:
        raise SystemExit(f"no artifact under {repo} declares a sidecar — nothing to probe")

    tmp = Path(tempfile.mkdtemp(prefix="probe-aug-"))
    corpus = tmp / "repo"
    corpus.mkdir()
    try:
        build_corpus(repo, corpus, pairs)
        ws = tmp / "workspace.toml"
        ws.write_text(
            f'[[roots]]\nname = "probe"\npath = "{corpus}"\n\n'
            '[[rule]]\nglob = "**/*.md"\nkind = "tracker"\n')
        db = tmp / "catalog.db"
        payload = reindex(binary, corpus, ws, db, tmp / "server.log")
        rows = inspect(db, corpus)

        declared = len(pairs)
        restored = payload.get("augmentations_restored", 0)
        collections = [r["entry_collection"] for r in rows if r["entry_collection"] != "-"]
        # Collision check: two artifacts restoring the SAME sidecar get the same
        # prompt. Prompt length is the cheap proxy for prompt identity, and
        # unlike entry_collection it has no legitimate reason to repeat.
        lens = [r["prompt_len"] for r in rows]
        dup_prompts = sorted({n for n in lens if lens.count(n) > 1})
        result = {
            "declared": declared,
            "indexed": payload.get("added"),
            "restored": restored,
            "errors": payload.get("augmentation_restore_errors", []),
            "distinct_prompts": len(set(lens)),
            "duplicate_prompt_lengths": dup_prompts,
            "distinct_entry_collections": len(set(collections)),
            "nonempty_params": [r["artifact"] for r in rows if r["params"] not in ("{}", "")],
            "rows": rows,
        }
        if a.json:
            print(json.dumps(result, indent=2))
        else:
            print(f"declared sidecars : {declared}")
            print(f"artifacts indexed : {payload.get('added')}")
            print(f"restored          : {restored}")
            print(f"restore errors    : {payload.get('augmentation_restore_error_count')}")
            for e in result["errors"]:
                print(f"  ! {e}")
            print(f"distinct prompts  : {len(set(lens))} of {len(lens)}"
                  "   (a repeat means two artifacts restored ONE sidecar)")
            print(f"distinct entry_collections: {len(set(collections))} of {len(collections)}"
                  "   (informational — repeats are legitimate)")
            if result["nonempty_params"]:
                print("  ! params travelled for: " + ", ".join(result["nonempty_params"]))
            print()
            for r in rows:
                print(f"  {r['artifact']:<48} {r['entry_collection']:<16} "
                      f"params={r['params']:<4} tmpl={r['render_template']} "
                      f"prompt={r['prompt_len']}")

        if restored < declared:
            print(f"\nFAIL: {declared} declared, {restored} restored", file=sys.stderr)
            return 1
        if result["nonempty_params"]:
            print("\nFAIL: params must not travel in git", file=sys.stderr)
            return 1
        if dup_prompts:
            print(f"\nFAIL: {len(dup_prompts)} prompt length(s) repeat — two artifacts "
                  "may share one sidecar (name collision)", file=sys.stderr)
            return 1
        print(f"\nOK: {restored}/{declared} restored into a catalog that never held them")
        return 0
    finally:
        if a.keep:
            print(f"(kept {tmp})", file=sys.stderr)
        else:
            shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
