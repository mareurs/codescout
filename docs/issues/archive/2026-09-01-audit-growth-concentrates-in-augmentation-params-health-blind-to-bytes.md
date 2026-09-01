---
kind: bug
status: fixed
tags:
- cluster/instrument-omits-the-dimension-that-grows
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related: []
severity: medium
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
sites — **now confirmed by measurement, see Evidence.**
## Evidence
### Final-review finding
T-1 whole-branch review (Opus, 2026-09-01), Important 3: "the spec's volume analysis covers
reindex and never reaches this path; retention is manual-only by design, and audit::health
reports rows but no bytes."

### Measured 2026-09-01 (discharges this file's Resume step)
Live catalog, 1.74-hour window, 27,914 audit rows total:

| statistic | value |
|---|---:|
| `artifact_augmentation` update rows | 23 |
| min / avg / max payload chars | 170 / **34,207** / **104,613** |
| total chars in those 23 rows | 786,771 |
| share of **all** payload bytes in the table | **88%** |

So the inference was right about direction and understated the size: the review's estimate
was ~50KB per append against ~25KB params, and the observed maximum is 104KB in one row.
Twenty-three rows out of 27,914 (0.08% by count) carry 88% of the bytes.

`length()` here is characters, not bytes (memory `catalog-sql-hazards`); for these payloads
the two are close because the content is ASCII JSON, but a byte figure needs
`length(CAST(payload AS BLOB))`.
## Hypotheses tried
None — filed on notice per capture discipline.

## Fix
Two-part idea from the review: (1) add `db_bytes`/`page_count` (or summed
`length(CAST(payload AS BLOB))`) to `audit::health` so the trend is observable; (2) consider
clamping any single payload value over N KB to `{"len": …, "sha256": …}`. Retention exists
via `audit_log(prune_before_ms, confirm)` meanwhile.

## Tests added

Fixed on `experiments` at **`40ab56f6`** (patch-id `d3021d83634be0f6b8d7c69200f241f80f9e5f96`).

Both halves of the review's proposal shipped:

1. **`health()` reports bytes** — `payload_bytes` and `largest_payload_bytes`, via
   `sum/max(length(CAST(payload AS BLOB)))`. The *largest* field is what makes the finding
   visible: a total alone reads as uniform growth, and the whole point here is that 0.08% of
   rows hold 88% of the bytes. Test `health_reports_payload_bytes`.
2. **Oversized values are stood in for** — `value_expr(clamp=true)` emits
   `{"elided":"oversize","len":N,"head":"<120 chars>"}` above 512 chars, in UPDATE diffs
   **only**.

The clamp is UPDATE-only because the measurement said so: this file's own Evidence shows the
bytes are in updates (old AND new of a whole-blob rewrite), while 19 `artifact` deletes
averaged 740 chars. Clamping DELETE images too would have broken the spec's "full OLD row on
delete" rule — the forensically precious payload for the vanished-rows bug — and saved
nothing. Guarded by `a_delete_image_keeps_oversize_values_verbatim`, whose only job is to
fail if a later tidy-up extends the clamp to delete images.

No `sha256` in the stand-in, deliberately: SQLite's bundled build has no hash function, and
registering one via `create_scalar_function` would make the trigger raise for any **foreign**
connection that never registered it — aborting that writer. A head is more useful anyway; a
diff carries a column only when it changed, so "did it change" is already answered.

Paired with `a_small_update_value_is_recorded_verbatim` so over-clamping is covered too.
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
