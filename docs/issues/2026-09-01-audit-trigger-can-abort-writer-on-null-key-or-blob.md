---
status: open
opened: 2026-09-01
closed:
severity: low
owner: marius
related: []
tags: [cluster/guard-narrower-than-its-name]
kind: bug
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
Either one expression each — `COALESCE(<row_id expr>, '<null>')` and a
`typeof(...)='blob'` guard emitting a placeholder — or, per CLAUDE.md § Parsers Over a
Namespace, narrow the module-doc invariant at the refusal site ("never blocks, for
JSON-representable values and non-NULL keys"): a documented limitation and a silent one
cost a reader very differently. Decide next time audit.rs is touched.

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
