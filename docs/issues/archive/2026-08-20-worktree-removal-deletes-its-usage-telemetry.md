---
status: fixed
opened: 2026-08-20
closed: 2026-08-20
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

Fixed in `experiments` @ `04e8e2c0153606ef9ce20fc4d663e05da6913132` (patch-id
`39a1c54c860b276715b924ba870766faef5136b8`). Chose **option 2** from the list below.

`UsageRecorder::write_content` resolves `project_root` from `with_project_at` exactly as
before, but now redirects where the db is OPENED: `crate::util::path_security::worktree_main_root(&project_root)`
(pure filesystem read of the `.git` gitdir pointer, no `git` subprocess, not feature-gated)
returns the main checkout's root when `project_root` is a linked worktree; `db::open_db`
is called against that root instead. `project_root` itself is left untouched in the
`INSERT` — the row still names the worktree, so it stays distinguishable from the main
checkout's own calls and analyses can still isolate worktree activity by
`WHERE project_root LIKE '%.worktrees%'` or similar.

This fires for BOTH ways a call can resolve to a worktree root: a per-call `workspace=`
override (the symptom this file measured), and a server whose own HOME project happens to
be a worktree (e.g. `--project` pointed at one) — same code path, no extra branching
needed, so the fix closes the root cause rather than only the measured symptom.

**Why option 2 over option 3 (accept and document):** the *Symptom* section measured up to
79% of one session's telemetry silently gone with no marker distinguishing a torn-down
worktree's calls from a call that was never logged — "populated-and-wrong" is the harder
failure mode, but "silently absent with nothing to search for" is not better; a
documented-gap fix leaves every future friction/cost analysis under-reporting with no way
to detect it. Option 2 turned out to cost a 3-line redirect plus a fallback, because
`worktree_main_root` already existed for the librarian's own worktree overlay and needed
no new capability.

**Why option 1 (aggregate on teardown) was never seriously in the running:** it needs a
teardown hook codescout does not own — `ExitWorktree` / `git worktree remove` are not
codescout-mediated — so it was ruled out at filing time, before this fix, on
infrastructure grounds rather than cost.

**Retention sweep interaction, resolved:** `write_record`'s sweep is
`DELETE FROM tool_calls WHERE called_at < datetime('now','-30 days')` — no `project_root`
predicate at all, so a db holding rows from multiple `project_root` values (main checkout
plus N torn-down worktrees) is already the sweep's normal operating shape. Nothing to
change there.
## Tests added

`usage::content_tests::record_content_pinned_into_a_worktree_writes_to_the_main_checkouts_db`
(`src/usage/mod.rs`) — shapes a linked-worktree tempdir (a `.git` FILE with a
`gitdir: <main>/.git/worktrees/<name>` pointer, the same shape `worktree_main_root`'s own
unit test uses), pins one call into it via `workspace_override`, and asserts: (1) the
worktree's own `.codescout/usage.db` is never created, and (2) the main checkout's
`usage.db` gets exactly one row whose `project_root` names the worktree. Full gate green:
`cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --lib`
(4137 passed, 0 failed, 7 ignored).
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
