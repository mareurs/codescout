"""Second pass over sig_tasks.jsonl: keep a candidate only when the commit itself changed
a call `name(` in a file other than the definition file (evidence the signature change
forced caller edits). Usage: python3 -I filter_forced.py <repo> <in.jsonl> <out.jsonl>"""
import json, re, subprocess, sys

repo, src, out = sys.argv[1:4]
kept = []
for line in open(src):
    x = json.loads(line)
    diff = subprocess.run(["git", "-C", repo, "diff", "-U0", x["parent"], x["sha"], "--", "*.rs"],
                          capture_output=True, text=True).stdout
    cur, files = None, set()
    call = re.compile(rf"(^|[^A-Za-z0-9_]){re.escape(x['fn'])}\s*(::<[^>]*>)?\(")
    for l in diff.splitlines():
        if l.startswith("+++ b/"):
            cur = l[6:]; continue
        if l[:1] in "+-" and not l.startswith(("+++", "---")) and cur and cur != x["path"]:
            if call.search(l) and not re.search(rf"\bfn\s+{re.escape(x['fn'])}\b", l):
                files.add(cur)
    if files:
        x["caller_files_edited"] = sorted(files)
        kept.append(x)
with open(out, "w") as f:
    for x in kept:
        f.write(json.dumps(x) + "\n")
print("kept", len(kept), "of", sum(1 for _ in open(src)), "| distinct commits", len({k["sha"] for k in kept}))
for k in kept:
    print(k["sha"], k["fn"], "edited callers in", len(k["caller_files_edited"]), "file(s);",
          "test-only callers" if all("test" in f for f in k["caller_files_edited"]) else "non-test callers",
          "|", k["subject"][:60])
