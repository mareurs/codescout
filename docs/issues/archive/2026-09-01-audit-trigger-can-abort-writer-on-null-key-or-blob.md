---
kind: bug
status: fixed
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related: []
severity: low
unverified: 'NULL-row-id half is closed by documenting the limitation, not by a guard: no audited table permits a NULL key today, so nothing can reach or test one. A schema change making a key column nullable reopens this.'
---

# BUG: an audit trigger can abort the writer's mutation on two measured paths, against the module doc's "never blocks" invariant

## Summary
`src/librarian/catalog/audit.rs`'s module doc states the failure-direction invariant
"nothing is ever blocked or mis-attributed". The T-1 final review (Opus, 2026-09-01)
**measured** two paths where the capture triggers refuse the writer's mutation — neither
reachable from today's Rust writers, both armed for future schema/data changes with no gate.

## Symptom (Effect)
Probed on SQLite 3.53 against the generated trigger SQL:
1. **NULL primary key**: `INSERT INTO commits(hash, git_root) VALUES(NULL, '/r')` is legal
   SQLite (TEXT PRIMARY KEY is nullable) but now fails
   `NOT NULL constraint failed: catalog_audit.row_id` — the write is refused with an error
   naming the audit table. Exposed tables: `artifact.id`, `events.id`, `commits.hash`,
   `worktree_registration.worktree_root`. Safe (NOT NULL already): `artifact_link`,
   `entry_cite`, `artifact_augmentation`.
2. **BLOB value**: DELETE of a row holding a BLOB, or an UPDATE that sets one, fails
   `JSON cannot hold BLOB values` and refuses the mutation. Asymmetry: an UPDATE where the
   BLOB column is *unchanged* passes (the CASE only serializes changed columns), so
   discovery latency would be long and the failure arbitrary-looking.

## Reproduction
In-memory catalog, then the two statements above from any connection.

## Environment
codescout experiments @ `10972335` (T-1 audit trail).

## Root cause
`old_image_expr`/`update_diff_expr` (`src/librarian/catalog/audit.rs`) assume row-id
expressions are non-NULL and column values are JSON-representable; both assumptions hold
for every current Rust writer (all bindings are `String`/ints) and neither is enforced at
the trigger. Measured by the final reviewer's probes; not re-measured here.

## Evidence
### Final-review probes (Opus, 2026-09-01)
Both failure strings quoted above are from live sqlite3 probes against the exact generated
trigger SQL, recorded in the T-1 whole-branch review (Minor 5).

## Hypotheses tried
N/A — mechanism measured at filing.

## Fix

Resolved on `experiments` at **`40ab56f6`** (patch-id `d3021d83634be0f6b8d7c69200f241f80f9e5f96`),
taking **both** options this section offered — one per half, because the two halves differ in
whether any caller can reach them.

**BLOB half — fixed, with a test that was born red.** `value_expr()`'s first arm is
`WHEN typeof(x)='blob' THEN json_object('elided','blob','len',length(x))`, applied to UPDATE
diffs *and* DELETE images. It must be first: `length()` is blob-safe and `json_object()` is
not. `a_blob_value_does_not_abort_the_writer` failed against the old code with the real
production error — `SqliteFailure(..., "JSON cannot hold BLOB values")` raised on the
**writer's** `UPDATE`, not on its audit row — and now asserts the write succeeds and the row
reads `elided: blob`. Reachable by any writer: SQLite is dynamically typed, so nothing in the
schema stops a BLOB landing in a TEXT column.

**NULL row-id half — closed by narrowing the invariant, not by a guard.** The module doc now
states the failure-direction claim narrowly and says what would reopen it. A `COALESCE` was
the alternative and was rejected under CLAUDE.md's loudness law: every audited table's key is
`NOT NULL` or a PRIMARY KEY, so no caller reaches it, no test can fail, and the guard would be
decoration that reads as coverage. The doc now owes the next schema change an explicit debt:
a key column made nullable brings back the `COALESCE` *and* a test that can fail.

That asymmetry is the point the original finding was making — a documented limitation and a
silent reinterpretation cost a reader very differently.
## Tests added
N/A — not started.

## Workarounds
None needed today — no Rust writer produces NULL keys or BLOBs in audited tables.

## Resume
When next touching `src/librarian/catalog/audit.rs`: pick guard-or-document (guard
preferred — it preserves the invariant instead of shrinking it), add the two probe
statements as regression tests either way, and re-run the reviewer's probes to confirm.

## References
- `src/librarian/catalog/audit.rs` module doc + `old_image_expr`/`update_diff_expr`
- T-1 final review Minor 5 + deferred-triage item 4 (SDD run 2026-09-01)
- `docs/trackers/system-retrospective-improvements.md` T-1
