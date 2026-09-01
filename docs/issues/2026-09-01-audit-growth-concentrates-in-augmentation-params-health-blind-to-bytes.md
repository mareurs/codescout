---
status: open
opened: 2026-09-01
closed:
severity: medium
owner: marius
related: []
tags: [cluster/capped-result-presented-as-complete]
kind: bug
---

# BUG: audit trail growth is concentrated in augmentation params, and the health block cannot see bytes

## Summary
Every `append_entry`/`update_entry` rewrites the whole `artifact_augmentation.params` blob
(`src/librarian/catalog/augmentation.rs` — two `UPDATE artifact_augmentation SET params = ?1`
sites), and the audit UPDATE trigger stores `json_array(OLD.params, NEW.params)` — both
copies. With ~25KB params (the spec's own figure), one tracker append writes a ~50KB audit
row, in a repo that appends constantly. `audit::health` reports `rows` but no bytes, so the
growth is invisible until the catalog file is large. Found by the T-1 final whole-branch
review (Opus), Important 3.

## Symptom (Effect)
No error anywhere — `catalog_audit` grows ~2× params size per tracker append; doctor's
`audit` block shows a row count that under-communicates the byte cost.

## Reproduction
Append an entry to any augmented tracker; `SELECT length(payload) FROM catalog_audit ORDER
BY seq DESC LIMIT 1` on `~/.local/share/librarian/catalog.db` (note: `length()` on TEXT is
CHARACTERS; use `length(CAST(payload AS BLOB))` for bytes — memory `catalog-sql-hazards`).

## Environment
codescout experiments @ `10972335` (T-1 audit trail).

## Root cause
Design-level, not a defect in a single line: whole-blob params rewrites (pre-existing) ×
full old/new capture on update for changed columns (T-1, by design — `update_diff_expr`
emits `[old, new]` pairs, and for `params` the changed column IS the whole blob).
Inferred from `src/librarian/catalog/audit.rs` `update_diff_expr` + augmentation.rs UPDATE
sites — not yet measured on a real append.

## Evidence
### Final-review finding
T-1 whole-branch review (Opus, 2026-09-01), Important 3: "the spec's volume analysis covers
reindex and never reaches this path; retention is manual-only by design, and audit::health
reports rows but no bytes."

## Hypotheses tried
None — filed on notice per capture discipline.

## Fix
Two-part idea from the review: (1) add `db_bytes`/`page_count` (or summed
`length(CAST(payload AS BLOB))`) to `audit::health` so the trend is observable; (2) consider
clamping any single payload value over N KB to `{"len": …, "sha256": …}`. Retention exists
via `audit_log(prune_before_ms, confirm)` meanwhile.

## Tests added
N/A — not started.

## Workarounds
Periodic `librarian(action="audit_log", prune_before_ms=…, confirm=true)`; the prune leaves
a self-describing marker row.

## Resume
Measure first (a number beats an adjective): one real tracker append, then byte-length of
the resulting audit row and the audit table's total via `dbstat` or
`sum(length(CAST(payload AS BLOB)))`. Then decide health-field vs clamp vs both.

## References
- `src/librarian/catalog/audit.rs` (`update_diff_expr`), `src/librarian/tools/audit_log.rs`
- `docs/trackers/system-retrospective-improvements.md` T-1/T-7
- T-1 final review, Important 3 (SDD run 2026-09-01)
