---
id: 689fb62e40557480
kind: bug
status: fixed
title: export_augmentations will not rewrite a sidecar whose shape changed, so a schema edit silently does not travel
tags:
- librarian
- augmentation
- sidecar
- silent-default
- cross-machine
closed: 2026-08-30
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


> **✅ Fixed 2026-08-30 by `5f88be65` (patch-id `59ba22f9d7a6dfed66fcd8e551e09455b5c58f32`),
> a peer's commit that does not name this file — found by a verify-open pass, not by a gate.**
> Classic zombie-open: the fix shipped under `fix(librarian): close the one-way door that left
> a committed sidecar stale`, and nothing connects that to `BL-67`.
>
> **Read the title carefully, because it is now describing intended behaviour.**
> `export_augmentations` *still* reports `exported: 0` for an artifact whose sidecar exists and
> whose shape has changed — and that is correct: **export CREATES, it never refreshes**, which
> is what keeps it idempotent and stops an augment call committing files into a repo that never
> asked for them. The fix went in one layer up: **`artifact_augment` now writes a shape change
> through to an existing sidecar**, at both shape-writing sites. So the *symptom* this file was
> opened for — a schema edit silently not travelling — is gone, while the *mechanism* the title
> names is unchanged and deliberate. A future reader seeing `exported: 0` should not re-file it.
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


### Re-run against the fixed code, 2026-08-30 — end-to-end, on a throwaway artifact

The three tests above are unit tests on synthetic fixtures. This file's reproduction is
end-to-end, so it was re-run as one — on a scratch tracker created and deleted in the same
session, so no live tracker's shape was put at risk:

1. `artifact(create)` → `docs/trackers/zz-scratch-bl67-verify.md`.
2. `artifact_augment(prompt="SHAPE-V1 …", params_schema={… enum: [alpha, beta]})`.
3. `librarian(doctor, fix="export_augmentations", confirm=true)` → `exported: 1`; sidecar on
   disk holds `SHAPE-V1` and the two-value enum.
4. `artifact_augment(merge=true, prompt="SHAPE-V2 …", params_schema={… enum: [alpha, beta,
   gamma]})` — **the exact edit this file says will not travel**.
5. Read the sidecar: **`SHAPE-V2`, and `gamma` present.** It travelled, with no export step.
6. `librarian(doctor, fix="export_augmentations")` → `exported: 0` — unchanged, and now the
   *correct* answer rather than the defect, because there was nothing left to do.
7. `artifact(delete)` + removed the sidecar; tree clean.

Step 6 is the reason this file's status needed prose rather than a flag. The number that was
the original symptom is still there, and now means the opposite.
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

**Shipped: none of the three options above.** The file framed the decision as *re-export on
divergence* vs *refuse and report*, both of which are changes to `export_augmentations`. The
actual fix does neither: it hooks `artifact_augment`, which is where a shape change originates,
so divergence never arises rather than being detected after the fact.

`5f88be65` adds `sidecar_write_through` at both shape-writing sites —
`create_or_replace_augmentation` (`merge=false`) and the sibling-field patch inside the
`merge=true` branch. It **never creates** a sidecar: creation stays the export's job. The write
is guarded by a **byte comparison** rather than by where the call sits, so a params-only merge
leaves the file untouched *by construction* rather than by placement.

It also preserves the asymmetry this file's `## Resume` insisted on. Catalog-ahead is a defect
and is now closed at the source; sidecar-ahead is BL-50's legitimate restore path and is
untouched, because `reindex` still attaches only when a row is **absent**.

The commit's own call-site sweep found **five** production writers of augmentation shape, not
the two its plan assumed — `create.rs` is vacuous here (a new artifact has no sidecar) and
`worktree.rs` copies an unchanged row into a shadow at fork time.
## Tests added

Three, in `src/librarian/tools/augment.rs`, all shipped with `5f88be65`:

- `a_shape_change_writes_through_to_the_committed_sidecar`
- `a_merge_true_sibling_change_writes_through_too`
- `write_through_never_creates_a_sidecar_that_does_not_exist`

**Mutation-verified per SITE rather than per feature**, which is what earned the second test:
removing each hook killed a *different* test and neither failed under the other's mutation, so
a single mutation would have supported "the write-through is covered" with the second site
unguarded. Each dies on the assertion naming the defect — hook 1 leaves `left: "before"`, the
**superseded** shape rather than an absent file, so a laxer `exists()`-style assertion would
have passed under the real bug. The never-creates guard was mutated *with parent-directory
creation added*, to give the mutant its best chance, and is still caught on its assertion
rather than on an incidental IO error.
## Resume

**Fixed on `experiments`, and archived.**

- Fix: `5f88be65` (`experiments`) — a peer's commit, not named for this bug
- patch-id: `59ba22f9d7a6dfed66fcd8e551e09455b5c58f32`
- Verify-open pass + end-to-end re-reproduction: 2026-08-30

**Why this needed a verify-open pass to close at all** is the transferable part. The fix landed
under a commit message describing the *mechanism* (`close the one-way door…`) rather than the
*ticket*, so no automated surface connected them, and the entry would have sat `open` against
fixed code indefinitely. That is exactly the fix-then-forget leak CLAUDE.md's verify-open
cadence exists for — and this instance had an extra layer, because the naive check ("does
`export_augmentations` still report 0?") returns **yes**, which reads as *not fixed*.
## References

- `docs/issues/2026-08-28-augmentation-declaration-records-existence-not-shape.md` — BL-50, where this is item (2)
- `docs/augmentations/docs-trackers-open-issue-work-queue.yaml` — the sidecar hand-repaired on 2026-08-30
- `2a8decc5` — the widening, with the hand-edit and the reason in its message
