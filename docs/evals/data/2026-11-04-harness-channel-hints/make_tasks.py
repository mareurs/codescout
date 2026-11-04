"""Draw the eval's task set from sig_tasks_selected.jsonl by a rule fixed before the draw.

Rule (AM-3 of docs/evals/harness-channel-hint-authority-preregistration.md):
  1. one task per commit: the fn whose commit edited callers in the most files; ties -> the
     alphabetically first fn name;
  2. sort the per-commit tasks by sha, then sample N with random.Random(SEED);
  3. the prompt states the old and the new signature, the commit subject as the reason, and
     "keep the workspace compiling". It never mentions callers or tools: that would be a hint
     in every arm, the floor included.

Usage: python3 -I make_tasks.py <selected.jsonl> <out.jsonl>"""
import json, random, sys

SEED = 20261104
N = 10

PROMPT = """In this Rust repository, change the signature of `{fn}` in `{path}`.

Current signature:
```rust
{old}
```

Required signature:
```rust
{new}
```

Reason for the change: {subject}

Make the change and keep the workspace compiling. You do not need to run the test suite."""

src, out = sys.argv[1], sys.argv[2]
rows = [json.loads(l) for l in open(src)]
per_commit = {}
for r in rows:
    key = (-len(r["caller_files_edited"]), r["fn"])
    best = per_commit.get(r["sha"])
    if best is None or key < (-len(best["caller_files_edited"]), best["fn"]):
        per_commit[r["sha"]] = r
pool = [per_commit[s] for s in sorted(per_commit)]
drawn = random.Random(SEED).sample(pool, N)
with open(out, "w") as f:
    for i, r in enumerate(drawn, 1):
        task = {
            "task_id": f"hce-{i:02d}-{r['fn']}",
            "parent": r["parent"], "sha": r["sha"], "path": r["path"], "fn": r["fn"],
            "old_sig": r["old_sig"], "new_sig": r["new_sig"],
            "caller_files_edited": r["caller_files_edited"],
            "prompt": PROMPT.format(fn=r["fn"], path=r["path"], old=r["old_sig"], new=r["new_sig"],
                                    subject=r["subject"]),
        }
        f.write(json.dumps(task) + "\n")
print(f"pool {len(pool)} commits, seed {SEED}, drew {N}:")
for i, r in enumerate(drawn, 1):
    print(f"  hce-{i:02d} {r['sha']} {r['fn']} callers-edited={len(r['caller_files_edited'])}")
