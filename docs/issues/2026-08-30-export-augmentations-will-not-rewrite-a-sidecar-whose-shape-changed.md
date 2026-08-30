---
id: eab3cb7631fd2689
kind: bug
status: open
title: export_augmentations will not rewrite a sidecar whose shape changed, so a schema edit silently does not travel
tags:
- librarian
- augmentation
- sidecar
- silent-default
- cross-machine
closed: ''
opened: 2026-08-30
owner: marius
related:
- docs/issues/2026-08-28-augmentation-declaration-records-existence-not-shape.md
severity: medium
---

## Summary

`librarian(action="doctor", fix="export_augmentations")` writes an augmentation's *shape*
to a committed sidecar so it survives a fresh clone. It will not rewrite a sidecar that
already exists, even when the catalog's shape has since changed — it reports
`exported: 0` and writes nothing.

So editing a `params_schema` through `artifact_augment` updates the machine-local catalog
and leaves the committed YAML holding the old shape. Nothing fails. The next machine's
`reindex` re-attaches the **stale** sidecar and silently restores the previous schema.

This is item (2) of `docs/issues/2026-08-28-augmentation-declaration-records-existence-not-shape.md`
(`BL-50`), where it was recorded as a known consequence and judged non-blocking because
"the export is idempotent". Filed separately now because it has a confirmed live instance
and its remedy is unrelated to BL-50's remaining item — that one is blocked on another
machine; this one is a write-path defect reproducible here.

## Symptom (Effect)

A schema change appears to succeed and does not travel. On a fresh clone the artifact
comes back with the *pre-edit* shape, and every entry written under the new shape now
violates it.

## Reproduction — performed 2026-08-30, and there was something to run

1. Widened `open-issue-work-queue`'s `status` enum from 7 values to 10 via
   `artifact_augment(id="9a892c2a5976e296", merge=true, params_schema=…)`. Catalog
   updated; `artifact(get)` and `doctor` both saw the new enum immediately.
2. `librarian(action="doctor", fix="export_augmentations")` →
   `{"exported": [], "totals": {"exported": 0, "failed": 0}}`.
3. `grep -n 'enum' docs/augmentations/docs-trackers-open-issue-work-queue.yaml` → still
   the **seven-value** list, at line 68. The sidecar's prose prompt still described the
   seven-value vocabulary too.

Both the schema and the prompt were stale, and the dry run reported success with zero
work to do — which is indistinguishable from "already up to date".

## Root cause

Not yet read at the bytes. The observable contract is that export treats a sidecar's
**existence** as the thing to satisfy, not its **contents** — the same
existence-vs-shape confusion BL-50 was opened about, one layer up. Confirm before fixing:
the reproduction above is cheap and settles whether it is an existence check, a
content-hash comparison that is wrong, or a deliberate no-clobber guard.

## Workarounds

Hand-edit the sidecar YAML to match, which is what was done here — the schema block and
any prose in the `prompt` that restates the vocabulary. Verify with a YAML parse; a
hand-edit that breaks the file fails at the *next machine's* reindex, not here.

## Fix

Not started. The decision the reproduction should settle: re-export when the shape
differs, or refuse and report the divergence. **Refusing and reporting may be the better
half** — a sidecar can legitimately be ahead of a catalog (that is exactly BL-50's
restore path), so an unconditional overwrite would clobber a shape that travelled in from
another machine. A `doctor` check reporting `sidecar_shape_differs_from_catalog` in both
directions is probably the right primitive, with the write behind an explicit fix.

## Tests added

None yet.

## Resume

Run the reproduction first — it decides which of the three mechanisms is in play and
therefore whether the fix is one line or a new check. Note that the two directions of
divergence are **not** symmetric: catalog-ahead means an edit did not travel, sidecar-ahead
means another machine's shape has not been adopted yet, and only the first is a defect.

## References

- `docs/issues/2026-08-28-augmentation-declaration-records-existence-not-shape.md` — BL-50, where this is item (2)
- `docs/augmentations/docs-trackers-open-issue-work-queue.yaml` — the sidecar hand-repaired on 2026-08-30
- `2a8decc5` — the widening, with the hand-edit and the reason in its message

