"""Mine codescout commits that change a non-test Rust fn signature which has callers.

Read-only (git plumbing only). Usage: python3 -I mine_sig_tasks.py <repo> <since> <out.jsonl>
A candidate needs: the fn name occurs exactly once as a definition in the parent file and in
the commit file (else skipped, never guessed); the whitespace-normalised signature differs
beyond visibility; the fn is not a test; at the parent, the name occurs as a call `name(`
or `.name(` / `::name(` somewhere other than its definition line.
"""
import json, re, subprocess, sys
from collections import Counter

repo, since, out = sys.argv[1], sys.argv[2], sys.argv[3]

def git(*a, ok=False):
    r = subprocess.run(["git", "-C", repo, *a], capture_output=True, text=True)
    if r.returncode != 0:
        if ok:
            return None
        raise RuntimeError(f"git {' '.join(a)}: {r.stderr[:200]}")
    return r.stdout

def norm(s):
    return re.sub(r"\s+", " ", s).strip()

DEF = r"((?:#\[[^\]]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+{n}\b"

def sig(src, name):
    hits = list(re.finditer(DEF.format(n=re.escape(name)), src))
    if len(hits) != 1:
        return None, None, len(hits)
    m = hits[0]
    rest = src[m.start() + len(m.group(1)):]
    cut = min([i for i in (rest.find("{"), rest.find(";")) if i >= 0], default=-1)
    return (norm(rest[:cut]) if cut >= 0 else None), m.group(1), 1

strip_vis = lambda s: re.sub(r"^(pub(\([^)]*\))?\s+)?", "", s)
HDR = re.compile(r"^[-+]\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?(?:unsafe\s+)?fn\s+(\w+)")

shas = git("log", "--no-merges", f"--since={since}", "--format=%H", "experiments", "--", "*.rs").split()
c = Counter()
cands = []
for sha in shas:
    parent = git("rev-parse", f"{sha}^", ok=True)
    if not parent:
        continue
    parent = parent.strip()
    diff = git("diff", "-U0", parent, sha, "--", "*.rs")
    files_changed = git("diff", "--name-only", parent, sha, "--", "*.rs").split()
    cur, seen = None, set()
    for line in diff.splitlines():
        if line.startswith("+++ b/"):
            cur = line[6:]; continue
        m = HDR.match(line)
        if m and cur:
            seen.add((cur, m.group(1)))
    for path, name in sorted(seen):
        c["fn_header_touched"] += 1
        if "test" in path.rsplit("/", 1)[-1] or "/tests/" in path or name.startswith("test_"):
            c["skip_test_path"] += 1; continue
        old_src = git("show", f"{parent}:{path}", ok=True)
        new_src = git("show", f"{sha}:{path}", ok=True)
        if not old_src or not new_src:
            c["skip_file_added_or_removed"] += 1; continue
        old, attrs, n1 = sig(old_src, name)
        new, _, n2 = sig(new_src, name)
        if n1 != 1 or n2 != 1 or not old or not new:
            c["skip_not_unique"] += 1; continue
        if "#[test" in (attrs or "") or "#[tokio::test" in (attrs or ""):
            c["skip_test_fn"] += 1; continue
        if old == new:
            c["skip_unchanged"] += 1; continue
        if strip_vis(old) == strip_vis(new):
            c["skip_visibility_only"] += 1; continue
        grep = git("grep", "-n", "-E", rf"(^|[^A-Za-z0-9_]){re.escape(name)}\s*(::<[^>]*>)?\(",
                   parent, "--", "*.rs", ok=True) or ""
        calls = [g for g in grep.splitlines() if not re.search(rf"\bfn\s+{re.escape(name)}\b", g)]
        if not calls:
            c["skip_no_callers"] += 1; continue
        caller_files = sorted({g.split(":", 2)[1] for g in calls})
        c["candidate"] += 1
        cands.append({
            "sha": sha[:12], "parent": parent[:12], "path": path, "fn": name,
            "old_sig": old, "new_sig": new, "call_sites": len(calls),
            "caller_files": len(caller_files),
            "callers_outside_def_file": sum(1 for f in caller_files if f != path),
            "rs_files_in_commit": len(files_changed),
            "subject": git("log", "-1", "--format=%s", sha).strip(),
        })

with open(out, "w") as f:
    for x in cands:
        f.write(json.dumps(x) + "\n")
print("commits scanned:", len(shas))
print("counts:", dict(c))
multi = [x for x in cands if x["callers_outside_def_file"] >= 1]
print("candidates:", len(cands), "| with a caller in another file:", len(multi))
print("distinct commits among those:", len({x['sha'] for x in multi}))
