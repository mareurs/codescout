"""Final, rule-based task selection (no hand-picking). Usage:
python3 -I select_tasks.py <repo> <forced.jsonl> <out.jsonl>
Rules: (1) `fn <name>` defined exactly once in the whole parent tree;
(2) at least one edited caller file is not a test file;
(3) the TYPE shape changed: params with names removed, or the return type, differ."""
import json, re, subprocess, sys

repo, src, out = sys.argv[1:4]

def type_shape(sig):
    m = re.search(r"\((.*)\)\s*(->\s*(.*))?$", sig)
    if not m:
        return sig
    params = []
    depth, cur = 0, ""
    for ch in m.group(1):
        if ch in "(<[": depth += 1
        if ch in ")>]": depth -= 1
        if ch == "," and depth == 0:
            params.append(cur); cur = ""
        else:
            cur += ch
    params.append(cur)
    types = []
    for p in (q.strip() for q in params):
        if not p: continue
        types.append(p.split(":", 1)[1].strip() if ":" in p and not p.startswith("&") else p)
    return (tuple(types), (m.group(3) or "").strip())

kept, why = [], {}
for line in open(src):
    x = json.loads(line)
    defs = subprocess.run(["git", "-C", repo, "grep", "-c", "-E", rf"\bfn\s+{re.escape(x['fn'])}\b",
                           x["parent"], "--", "*.rs"], capture_output=True, text=True).stdout
    ndefs = sum(int(l.rsplit(":", 1)[1]) for l in defs.splitlines() if l)
    if ndefs != 1:
        why[f"{x['sha']} {x['fn']}"] = f"defined {ndefs}x in tree"; continue
    if all("test" in f for f in x["caller_files_edited"]):
        why[f"{x['sha']} {x['fn']}"] = "test-only callers"; continue
    if type_shape(x["old_sig"]) == type_shape(x["new_sig"]):
        why[f"{x['sha']} {x['fn']}"] = "type shape unchanged"; continue
    kept.append(x)
with open(out, "w") as f:
    for x in kept:
        f.write(json.dumps(x) + "\n")
print("selected", len(kept), "| distinct commits", len({k["sha"] for k in kept}))
for k in kept:
    print(" ", k["sha"], k["fn"], "| callers edited in", len(k["caller_files_edited"]), "|", k["subject"][:55])
print("excluded:")
for k, v in why.items():
    print("  ", k, "->", v)
