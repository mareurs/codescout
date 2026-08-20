---
status: open
opened: 2026-08-20
closed:
severity: medium
owner: marius
related: ["docs/issues/archive/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md"]
tags:
  - usage-db
  - telemetry
  - worktree
kind: bug
---

# BUG: a worktree's usage telemetry is deleted with the worktree, so calls that succeeded become unaccounted

## Summary

`usage.db` lives at `<project-root>/.codescout/usage.db`, and a git worktree is its own
project root. Calls made with a per-call `workspace` override pointing at a worktree are
logged to **that worktree's** database. When the worktree is removed — the normal end of
its life — the telemetry goes with it. The calls succeeded, were recorded, and are now
unrecoverable, with nothing in the surviving database indicating they ever happened.

## Symptom (Effect)

Two independent shapes, both measured 2026-08-19/20 during a transcript-vs-DB
reconciliation:

**Small case, fully explained.** Session `23b22760`: 94 codescout `tool_use` blocks in the
transcript, 90 rows in this project's `usage.db`. The 4 missing calls (3× `edit_code`,
1× `grep`) each carried:

```
"workspace": "/home/marius/work/claude/codescout/.claude/worktrees/pr14"
```

An exact 90/4 split by presence of that parameter. `git worktree list` confirms the
worktree no longer exists — it was a mutation-probe scratch tree, since removed. Zero
orphaned `tool_use`, zero errors, zero interrupt markers: the calls were clean successes
logged to a file that no longer exists.

**Large case.** Session `bc3d69f9` made 3,983 codescout calls per its transcript. Only
**820** are recoverable across three separate per-project/per-worktree databases. It
activated a worktree 27× in-transcript and no database for it can be found. ~79% of that
session's telemetry is gone.

## Reproduction

```
# HEAD at filing: b4ea12fd989dfc2cbf1604be36090ddd3c99a6a3 (experiments)
git worktree add /tmp/cs-probe HEAD
# from a codescout session, make a call with the override:
#   grep(pattern="fn main", workspace="/tmp/cs-probe")
sqlite3 -column /tmp/cs-probe/.codescout/usage.db \
  "SELECT tool_name, called_at FROM tool_calls ORDER BY id DESC LIMIT 3;"
# the row is there. now:
git worktree remove /tmp/cs-probe
# the row, and every other row in that db, is gone. the main project's db never had them.
```

## Environment

Linux; codescout `experiments` at `b4ea12fd`; per-call `workspace` override (the parameter
exists on every codescout tool for concurrent subagents in different workspaces).

## Root cause

`usage.db` is project-root-scoped by design, and the `workspace` override intentionally
re-resolves a call against a different project root. The two compose into telemetry whose
lifetime is bound to a directory whose whole purpose is to be temporary. Nothing is
aggregated upward before the directory is discarded, and no record in the parent project
notes that the calls were routed away.

inferred from the `workspace` parameter's documented behaviour plus the 90/4 split above —
the routing is measured, the absence of any upward aggregation is a reading of the schema
(one `tool_calls` table per project root, no cross-root sync path), not a runtime
observation of an aggregation attempt failing.

## Evidence

### The 90/4 split

Every one of the 4 unaccounted calls carries the `workspace` override; none of the 90
accounted ones do. Reported by the reconciliation pass; the mechanism is directly checkable
with the *Reproduction* above.

### Consequence for measurement

This is the second reason a transcript-vs-DB count can disagree, distinct from the session
identity pooling in
`docs/issues/archive/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md`.
An analysis that treats "fewer DB rows than transcript calls" as a logging failure will
mis-diagnose this as a dropped write.

## Hypotheses tried

1. **Hypothesis:** the missing calls were composed but never round-tripped (interrupted or
   permission-rejected). **Test:** checked the 4 calls' `tool_result` blocks for errors,
   `is_error`, and interrupt/reject markers. **Verdict:** rejected — all four are clean
   successes with results. **Evidence:** *The 90/4 split*.

## Fix

Not yet implemented, and the right shape is a design question rather than a patch:

1. **Aggregate on worktree teardown** — requires a teardown hook codescout does not
   currently own; `ExitWorktree`/`git worktree remove` are not codescout-mediated.
2. **Write worktree calls to the main project's DB**, tagged with the worktree root. Keeps
   one durable store and makes worktree activity queryable, at the cost of blurring "which
   project root did this call resolve against" unless a column carries it. Note
   `project_root` already exists on `tool_calls`, so the tag may be free.
3. **Accept and document** — declare worktree telemetry ephemeral by design and have
   analyses state the gap rather than silently under-reporting.

Option 2 looks cheapest and reuses an existing column, but it changes where a call's
telemetry lands, which touches the retention sweep's scope. Not decided.

Record the fix SHA **and** its patch-id (`git show <sha> | git patch-id --stable`).

## Tests added

None. A regression test for option 2 would assert that a call with a `workspace` override
pointing into a registered worktree writes its row to the main project's `usage.db` with
`project_root` set to the worktree.

## Workarounds

Before removing a worktree that has seen codescout use, copy its
`.codescout/usage.db` out. For analyses: expect a floor, not a count, and check whether the
sessions under study used `workspace` overrides — `json_extract(input_json,'$.workspace')`
is non-null on exactly the affected rows while the DB still exists.

## Resume

Decide between options 2 and 3 under *Fix*. If option 2: read how `project_root` is
populated in `src/usage/mod.rs` `write_content` and whether the override path already
carries the worktree root there, then check what the 30-day retention sweep would do to
rows whose `project_root` differs from the DB's own root.

## References

- `src/usage/db.rs` — `open_db`, and the retention sweep in `write_record`
- `src/usage/mod.rs` — `write_content`, `workspace_override`
- `docs/issues/archive/2026-08-20-telemetry-session-id-frozen-while-the-ledger-re-keys-per-call.md`
