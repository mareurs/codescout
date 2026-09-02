---
id: 572cdbd226139268
kind: bug
status: fixed
title: 'BUG: recoverable_error is never written to usage.db — 0 rows in 57k calls, and two queries filter on it'
tags:
- cluster/declared-not-wired
- usage-db
- telemetry
- error-handling
- analyze-usage
- misleading-instrument
closed: 2026-09-02
opened: 2026-09-02
owner: marius
related: []
severity: medium
---

# BUG: `recoverable_error` is never written — 0 rows in 57k calls, and two queries filter on it

## Summary

`usage.db`'s `outcome` column is documented and queried as a three-value
taxonomy — `success | error | recoverable_error`. In 30 days of live traffic it
holds **two** values. `recoverable_error` is unreachable in production because
the recorder classifies before the router that would distinguish it, so the
column cannot tell a forced-retry (a guard firing correctly) from a hard
failure.

## Symptom (Effect)

**measured 2026-09-02**, `.codescout/usage.db`, 30-day rolling window:

```
success              54,895
error                 2,536
recoverable_error         0
```

Two SQL queries filter on the dead value and therefore evaluate a union whose
second term is always empty:

- `src/usage/db.rs:989` (`query_stats`)
- `src/usage/db.rs:1079` (`recent_errors`)

`/analyze-usage` reads both. Its error breakdown silently reports hard failures
and Iron-Law refusals as one class.

## Root cause

Ordering, at `src/server.rs:1118-1145`. `record_content` wraps
`tool.call_content(...)`, which returns `Err(RecoverableError)`. The recorder's
`Err(e)` arm classifies that as `"error"`. `route_tool_error` — the function
that converts a `RecoverableError` into an `isError: false` response — runs
**afterward**, at `:1145`.

`classify_content_result` (`src/usage/mod.rs:163`) *can* emit
`"recoverable_error"`, but only for a tool that returns `Ok(content)` whose JSON
body carries a top-level `error` key. No live tool does that: codescout's
recoverable errors travel as `Err`, per `get_guide("error-handling")`.

*Ordering read directly this session at the cited lines. The 57k-row
distribution is a subagent measurement, re-derivable from the query below.*

## Evidence

### The tests pin the value without traversing the path that kills it

`src/usage/mod.rs:236` and `src/usage/db.rs:1150` both exercise
`classify_content_result` **directly**. Neither goes through
`record_content` → `route_tool_error`, which is the ordering that makes the
value unreachable. This is the project's own *"loudness is a property of a
PATH"* law: the classifier works, and nothing reaches it.

### The consequence is not cosmetic

`err_family` (taxonomy at `src/usage/db.rs:551-603`) classifies 2,465 of 2,537
error rows, and the top families are Iron-Law refusals: `il3_pipe_to_trimmer`
594, `il3_shell_on_source` 320, `il1_read_overlaps_symbol` 260,
`librarian_managed_artifact` 229. Those are **guards working**. Under `outcome`
they are indistinguishable from crashes — so any analysis ranking shapes by
`outcome='error'` ranks the *most heavily guarded* shapes highest and reads
existing enforcement as unmet need.

## Hypotheses tried

1. **Hypothesis:** no recoverable error occurred in the window.
   **Test:** `SELECT err_family, COUNT(*) ... GROUP BY err_family` — 2,465
   classified rows, dominated by Iron-Law refusal families that are recoverable
   by construction.
   **Verdict:** rejected. They occurred and were recorded as `error`.

## Fix

**Fixed on `experiments` at `b6a7330d`**, patch-id
`805f3f60ff4116cdffcfaa2f6a9f199e567052f1`. The SHA is positional and dies when
`experiments` is rebased; the patch-id is a content hash of the diff and
survives rebase and cherry-pick.

Shape 2 of the two candidates — the `Err` arm downcasts to `RecoverableError`
itself, the same downcast `route_tool_error` already performs. Chosen over
shape 1 (classify after the router) because it needs no call-chain reordering
and leaves `route_tool_error` the single place that decides the wire
disposition; the recorder learns the disposition without becoming a second
authority on it.

**The `Ok` arm is left exactly as it was, and is still unreachable in
practice.** It emits `recoverable_error` for a tool returning `Ok(content)`
whose JSON body carries a top-level `error` key, and no live tool does that.
Removing it was tempting and would have been wrong twice over: it is the
documented contract for a tool that chooses that shape, and deleting a branch
because today's corpus does not reach it is how the reachability claim decays
into a design constraint. Recorded rather than removed.

*Not* addressed here, and left open deliberately: the file's own closing
question of whether `err_family` should subsume `outcome`'s error split
entirely. That is a taxonomy decision with its own migration, not a fix to this
defect — and the three-value column is now honest, which was the thing that
was false.
## Tests added

`record_content_distinguishes_a_recoverable_error_from_a_hard_one`
(`src/usage/mod.rs`, in `content_tests`).

**It traverses `record_content` end to end and asserts the STORED ROW**, which
is why it is sited beside the recorder rather than beside the classifier. This
section's own instruction called for exactly that, and the reason is the shape
of the bug: the defect was *ordering*, so a unit test of
`classify_content_result` would have gone green against the broken path — it
cannot see that `route_tool_error` runs after the write.

**Both directions, in one database, because either alone is monotone.**
Asserting only the recoverable row passes against a classifier that returns
`recoverable_error` for everything — the same two-value column, relabelled.
Asserting only the hard row is what the code already did.

**Mutation-verified both ways:**

| mutation | stored rows |
|---|---|
| `outcome = "error"` unconditionally | `[(edit_file, error), (read_file, error)]` — reproduces the original defect exactly |
| `outcome = "recoverable_error"` unconditionally | `[(edit_file, recoverable_error), (read_file, recoverable_error)]` |

Each failure message names the offending pair rather than reporting a bare
mismatch.
## Workarounds

Use `err_family` rather than `outcome` when distinguishing guard-fired from
crashed. Treat `/analyze-usage`'s error counts as "errors and refusals
combined".

## Resume

**Closed.** The column now separates a guard firing correctly from a hard
failure, and the two live queries that filter on `recoverable_error` will begin
returning rows from the next recorded call.

**Historical rows are not backfilled and cannot be.** Every pre-fix
`recoverable_error` is recorded as `error` and nothing distinguishes it after
the fact — the discriminating information was never written. Any query over
`outcome` that spans 2026-09-02 is reading two different taxonomies under one
column name, and will under-report `recoverable_error` in proportion to how much
of its window predates this commit. Treat a low count as a boundary artefact
before treating it as a finding.

**Gate, reported rather than asserted.** fmt clean; clippy `--workspace
--all-targets --features local-embed -D warnings` exit 0; both lanes report
exactly one failure and it is not this change —
`agent::tests::activate_home_with_read_only_true_is_honoured` is absent from
HEAD and present only in another session's uncommitted diff to
`src/agent/mod.rs`, a red-first test for the read_only bug they are filing.
Verified positively (`git show HEAD:` lacks the name, `git diff` adds it), not
by elimination. Otherwise lean 3297 passed, default 4946 passed.
## References

- `src/server.rs:1118-1145` — the ordering
- `src/usage/mod.rs:163` — `classify_content_result`
- `src/usage/mod.rs:236`, `src/usage/db.rs:1150` — the tests that bypass the path
- `src/usage/db.rs:989`, `:1079` — the two queries on the dead value
- `src/usage/db.rs:551-603` — `err_family` taxonomy
- Distribution query: `SELECT outcome, COUNT(*) FROM tool_calls GROUP BY outcome` against `.codescout/usage.db`
