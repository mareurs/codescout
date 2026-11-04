"""Census: Rust edit_code replace calls that CHANGE a fn signature, and whether
`references` on that symbol ran earlier in the same codescout session.

Read-only: opens usage.db with mode=ro, reads old source via `git show sha:path`.
Usage: python3 -I sig_census.py <repo_root> <since>
"""
import json, re, sqlite3, subprocess, sys
from collections import Counter

repo, since = sys.argv[1], sys.argv[2]
db = sqlite3.connect(f"file:{repo}/.codescout/usage.db?mode=ro", uri=True)

def norm(s):
    return re.sub(r"\s+", " ", s).strip()

def sig_from_body(body):
    lines = [l for l in body.splitlines()
             if not l.strip().startswith(("///", "//!", "//", "#["))]
    text = "\n".join(lines)
    m = re.search(r"\bfn\s+\w+", text)
    if not m:
        return None
    rest = text[m.start():]
    cut = min([i for i in (rest.find("{"), rest.find(";")) if i >= 0], default=-1)
    return norm(rest[:cut]) if cut >= 0 else None

src_cache = {}
def old_source(sha, path):
    k = (sha, path)
    if k not in src_cache:
        r = subprocess.run(["git", "-C", repo, "show", f"{sha}:{path}"],
                           capture_output=True, text=True)
        src_cache[k] = r.stdout if r.returncode == 0 else None
    return src_cache[k]

def old_sig(src, name):
    hits = list(re.finditer(rf"\bfn\s+{re.escape(name)}\b", src))
    if len(hits) != 1:          # ambiguous or absent: skip, never guess
        return None, len(hits)
    rest = src[hits[0].start():]
    cut = min([i for i in (rest.find("{"), rest.find(";")) if i >= 0], default=-1)
    return (norm(rest[:cut]) if cut >= 0 else None), 1

rows = db.execute(
    """SELECT id, session_id, project_sha, input_json, outcome FROM tool_calls
       WHERE tool_name='edit_code' AND json_extract(input_json,'$.action')='replace'
         AND called_at >= ? AND session_id IS NOT NULL AND project_sha IS NOT NULL""",
    (since,)).fetchall()

c = Counter()
changed = []
for rid, sess, sha, inp, outcome in rows:
    j = json.loads(inp)
    path, sym, body = j.get("path", ""), j.get("symbol", ""), j.get("body", "")
    if not path.endswith(".rs") or not sym or not body:
        c["skip_not_rust_or_incomplete"] += 1; continue
    name = sym.split("/")[-1]
    new = sig_from_body(body)
    if not new:
        c["skip_not_fn"] += 1; continue
    src = old_source(sha, path)
    if src is None:
        c["skip_no_old_source"] += 1; continue
    old, nhits = old_sig(src, name)
    if old is None:
        c["skip_ambiguous_or_new" if nhits != 1 else "skip_old_sig_parse"] += 1; continue
    if old == new:
        c["fn_sig_unchanged"] += 1; continue
    # A test fn has no callers: drop test files, test modules and #[test]-marked fns.
    m = re.search(rf"((?:#\[[^\]]*\]\s*)*)(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+{re.escape(name)}\b", src)
    attrs = m.group(1) if m else ""
    is_test = ("test" in path.split("/")[-1] or "/tests/" in path or sym.startswith("tests/")
               or "/tests/" in sym or "#[test" in attrs or "#[tokio::test" in attrs)
    if is_test:
        c["fn_sig_changed_test"] += 1; continue
    # Only the PARAMETER list or return type counts; a visibility-only change does not.
    strip_vis = lambda s: re.sub(r"^(pub(\([^)]*\))?\s+)?", "", s)
    if strip_vis(old) == strip_vis(new):
        c["fn_sig_changed_visibility_only"] += 1; continue
    c["fn_sig_changed_nontest"] += 1
    refs = db.execute(
        """SELECT COUNT(*) FROM tool_calls WHERE session_id=? AND id<? AND
           tool_name IN ('references','call_graph') AND
           (json_extract(input_json,'$.symbol')=? OR json_extract(input_json,'$.symbol') LIKE ?)""",
        (sess, rid, sym, f"%{name}")).fetchone()[0]
    greps = db.execute(
        """SELECT COUNT(*) FROM tool_calls WHERE session_id=? AND id<? AND
           tool_name='grep' AND json_extract(input_json,'$.pattern') LIKE ?""",
        (sess, rid, f"%{name}%")).fetchone()[0]
    changed.append((rid, sym, refs > 0, greps > 0, outcome))

n = len(changed)
r = sum(1 for x in changed if x[2])
g = sum(1 for x in changed if x[3] and not x[2])
none = sum(1 for x in changed if not x[2] and not x[3])
print("counts:", dict(c))
if n:
    print(f"non-test signature changes: {n}")
    print(f"  references/call_graph before: {r} ({r/n:.2f})")
    print(f"  grep on the name only (no references): {g} ({g/n:.2f})")
    print(f"  no caller check in codescout: {none} ({none/n:.2f})")
    print("sample (id, symbol, refs, grep, outcome):")
    for x in changed[:15]:
        print("  ", x)
