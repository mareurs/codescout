"""Is the declared effect=read honest? Ground truth: usage.db observed_changed (the server snapshots the
git-visible tree around a declared-read run_command). Classifies each command with run_command_ro's rule.
Run from the directory that holds run_command_ro.py."""
import sqlite3, json, collections, re

src = open("run_command_ro.py").read()
exec(src[: src.index("def ts(s):")])          # RO_WORDS ... classify(), no database or file work

db = sqlite3.connect("file:/home/marius/work/claude/codescout/.codescout/usage.db?mode=ro", uri=True, timeout=20)
rows = db.execute("select observed_changed, input_json from tool_calls "
                  "where tool_name='run_command' and effect_class='read'").fetchall()
print("declared read:", len(rows))
by_obs = collections.Counter(r[0] for r in rows)
print("observed_changed:", {("NULL" if k is None else k): v for k, v in by_obs.items()})

tab = collections.Counter()
for obs, inp in rows:
    try:
        cmd = json.loads(inp).get("command", "")
    except Exception:
        continue
    tab[(obs, classify(cmd))] += 1
print("\nobserved x text class:")
for (obs, cls), v in sorted(tab.items(), key=lambda kv: (str(kv[0][0]), kv[0][1])):
    print(f"  observed={('NULL' if obs is None else obs)!s:4} text={cls:7} {v:6}")

seen = sum(v for (o, _), v in tab.items() if o is not None)
changed = sum(v for (o, _), v in tab.items() if o == 1)
changed_ro = tab[(1, "RO")]
print(f"\nobserved declared-read calls: {seen}; tree changed: {changed} ({100*changed/seen:.2f}%)")
print(f"  of those {changed}, read-only by text: {changed_ro} ({100*changed_ro/changed:.0f}%), "
      f"not: {changed - changed_ro}")
unchanged = sum(v for (o, _), v in tab.items() if o == 0)
unchanged_mut = tab[(0, "MUT")] + tab[(0, "VERIFY")]
print(f"  declared-read calls that left the tree unchanged: {unchanged}; my rule calls {unchanged_mut} of them "
      f"not read-only ({100*unchanged_mut/unchanged:.0f}%)")
