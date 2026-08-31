---
status: open
opened: 2026-08-31
closed:
severity: medium
owner: marius
related: []
tags: [librarian, doctor, augmentation, sidecar, misleading-remedy, no-op-reports-success]
kind: bug
---

# BUG: sidecar_shape_drift prescribes `fix="export_augmentations"`, which by construction skips every artifact that check can fire on

## Summary

`librarian(action="doctor")`'s `sidecar_shape_drift` finding tells the reader to resolve
the catalog-is-right case with `librarian(action="doctor", fix="export_augmentations")`.
That call cannot resolve it. The check only fires when the sidecar **exists on disk**, and
the export **skips any artifact whose sidecar already exists** — so the prescribed remedy is
a guaranteed no-op for 100% of this check's findings. It reports `exported: 0` and exits
successfully, which reads as "nothing needed doing" rather than "I declined".

The correct wording is already in the same function, 25 lines above, on the sibling check.

## Symptom (Effect)

A real drift, and the prescribed remedy against it:

```
librarian(action="doctor")
  { "check": "sidecar_shape_drift",
    "artifact_id": "e12cd7e0060ed9b8",
    "path": "docs/trackers/provenance-subsystem.md",
    "detail": "... If your catalog is right (you changed the shape here and it has not
               been published), re-export it: librarian(action=\"doctor\",
               fix=\"export_augmentations\") ..." }

librarian(action="doctor", fix="export_augmentations")     # exactly as instructed
  { "mode": "dry_run", "exported": [], "totals": { "exported": 0, "failed": 0 } }
```

No error, no refusal, no mention of the artifact it declined to touch. A reader who trusts
the remedy concludes the drift is unresolvable or already handled, and the stale sidecar
stays committed.

## Reproduction

Measured 2026-08-31 14:28–14:35 at `4bb0c76e`, against a genuine drift on
`docs/trackers/provenance-subsystem.md` (catalog newer than sidecar on both `params_schema`
and `render_template`):

1. `librarian(action="doctor")` → `sidecar_shape_drift: 1`, detail as quoted above.
2. `librarian(action="doctor", fix="export_augmentations")` → **`exported: 0`**.
3. Move the sidecar aside: `mv docs/augmentations/docs-trackers-provenance-subsystem.yaml <elsewhere>`
4. Re-run the identical call → **`exported: 1`**, file regenerated from the catalog.
5. `librarian(action="doctor")` → `sidecar_shape_drift: 0`; total violations 147 → 146.

Steps 2 and 4 are the **same command against the same catalog**, differing only in whether
the file exists. That is the whole defect.

## Environment

Arch Linux (zen 7.1.11), codescout at `4bb0c76e` on `experiments`, release build via
`cargo rb`, MCP stdio transport, project `codescout`, catalog
`~/.local/share/librarian/catalog.db`.

## Root cause

Two facts that are each correct alone and contradictory together, both in
`src/librarian/tools/doctor.rs`:

1. **`scan_sidecar_shape_drift` fires only when the sidecar exists.**
   `src/librarian/tools/doctor.rs:4337`:
   ```rust
   // Declared-but-absent with a row present is not this check's case ...
   if !path.is_file() {
       continue;
   }
   ```
   So *every* finding this check emits is, by construction, an artifact with a sidecar
   present.

2. **The export skips exactly that population.** `export_augmentations` creates and never
   refreshes — `get_guide("tracker-conventions")` states it outright ("Export CREATES; it
   never refreshes. It skips an artifact whose sidecar already exists — that is what makes
   it idempotent"), and this very function's own doc comment restates it ("the export skips
   an artifact whose sidecar exists").

The intersection of "sidecar present" and "export acts only when absent" is empty. The
remedy at `src/librarian/tools/doctor.rs:4378` is therefore unreachable-by-construction
advice, not advice that merely fails sometimes.

*measured 2026-08-31: the dry-run/live pair in Reproduction steps 2 and 4; code read at
`src/librarian/tools/doctor.rs:4299-4390`.*

## Evidence

### The correct wording is 25 lines above, in the same function

`sidecar_unparseable` — the sibling finding emitted by the same scanner, for an artifact
whose sidecar is also present — gets it right at
`src/librarian/tools/doctor.rs:4353`:

> "Repair the YAML by hand, **or delete it and re-run** `librarian(action="doctor",
> fix="export_augmentations")` on a machine whose row is correct."

The `delete it and` clause is the entire difference. `sidecar_shape_drift` at `:4378` omits
it and says only "re-export it". Two messages, one function, one present-sidecar
precondition, and only one of them names the step that makes the export do anything.

This is what makes it an oversight rather than a design position: nobody who believed the
export refreshed in place would have written the `:4353` wording.

### The author already knew, in prose, on the same symbol

`scan_sidecar_shape_drift`'s doc comment states the mechanism plainly — "the export skips an
artifact whose sidecar exists, `reindex` attaches only when a row is absent, so before the
write-through landed nothing could update a committed sidecar". The knowledge is present and
correct; only the user-facing string disagrees with it.

### It is not the `fix=` the doc comment refuses to add

Worth stating, because the fix here looks superficially like the thing that comment forbids.
The comment argues at length that `sidecar_shape_drift` must have **no `fix=` mode**, because
drift direction is per-field and a per-artifact repair is incoherent. That argument is sound
and this report does not contest it. The defect is narrower: the *prose remedy* for the case
where the operator **has already made the human judgement** ("your catalog is right") names a
call that cannot execute it. Fixing the sentence adds no `fix=` and does not weaken the
per-field argument.

## Hypotheses tried

1. **Hypothesis:** the export refreshes an existing sidecar and something else made this one
   fail — a permission problem, a scope problem, a parse failure.
   **Test:** run the identical call twice, changing only whether the file is present.
   **Verdict:** rejected. `exported: 0` with the file, `exported: 1` without it, same
   catalog, same scope, ~90 seconds apart.
   **Evidence:** Reproduction steps 2 and 4.

## Fix

Not yet implemented.

Plan: amend the `sidecar_shape_drift` detail at `src/librarian/tools/doctor.rs:4378` so the
catalog-is-right branch names the delete step, mirroring `:4353`. Something like *"delete the
sidecar and re-run `librarian(action="doctor", fix="export_augmentations")`"*. Prefer copying
the sibling's phrasing over inventing a new one, so the two present-sidecar remedies cannot
drift apart again — which is the same failure mode one layer up.

Consider separately whether `export_augmentations` should **report** the artifacts it skipped
rather than silently omitting them. `exported: 0` is the honest count of what it wrote and a
truthful answer to a question the caller did not ask; a `skipped: [...]` alongside it would
make the no-op legible without changing the idempotence that the skip exists to provide. That
is the more general fix, and it is the one that would have surfaced this without a reader
noticing the contradiction by hand.

## Tests added

None yet — the fix is not written.

The regression test should assert the two present-sidecar remedies agree, and must be able to
fail in the direction of the bug: assert `sidecar_shape_drift`'s detail **contains the delete
instruction**, not merely that it mentions `export_augmentations`. A presence assertion on the
token alone is monotone under exactly this defect — the current, wrong string contains it too
— and would have passed throughout (CLAUDE.md § *Testing Discipline*).

`a_present_sidecar_and_a_missing_one_get_opposite_advice`
(`src/librarian/tools/doctor.rs:5621-5689`) is the right shape to copy but does **not** cover
this: it exercises `scan_augmentation_declared_but_absent`, a different check. Its assertion
`!present.contains("export_augmentations")` encodes precisely the principle broken here —
present sidecar, so do not send the reader to a plain export — which is further evidence this
is an oversight, and a reason to extend that test rather than write a new one from scratch.

## Workarounds

For the catalog-is-right case, move the sidecar aside and then export — the export creates
when absent:

```
mv docs/augmentations/<name>.yaml /tmp/<name>.bak
librarian(action="doctor", fix="export_augmentations", confirm=true)
git diff docs/augmentations/          # verify before committing
librarian(action="doctor")            # sidecar_shape_drift must return to 0
```

Move rather than delete, so the prior shape is recoverable if the export is not what you
expected. For the **sidecar-is-right** case the existing advice is already correct and
unaffected: apply the committed values with `artifact_augment`, which writes through.

## Resume

Amend the string at `src/librarian/tools/doctor.rs:4378` to match `:4353`, then extend
`a_present_sidecar_and_a_missing_one_get_opposite_advice` (or add a sibling test) covering
`scan_sidecar_shape_drift`, asserting on the delete instruction rather than on the token.
Confirm the test fails before the fix.

## References

- `src/librarian/tools/doctor.rs:4337` — the `is_file` guard that makes every finding a
  present-sidecar case
- `src/librarian/tools/doctor.rs:4353` — `sidecar_unparseable`'s correct wording
- `src/librarian/tools/doctor.rs:4378` — the defective remedy
- `src/librarian/tools/doctor.rs:4299-4390` — `scan_sidecar_shape_drift`, whose doc comment
  states the skip-if-exists behaviour correctly
- `src/librarian/tools/doctor.rs:5621-5689` —
  `a_present_sidecar_and_a_missing_one_get_opposite_advice`, the adjacent test that encodes
  the same principle for the sibling check
- `docs/conventions/cross-machine-catalog-resume.md` — why a stale committed sidecar matters:
  a fresh clone restores whatever it says and reports success
- `docs/adrs/2026-08-30-a-plausible-value-is-not-a-verification.md` — the class this belongs
  to: `exported: 0` is a plausible value standing in for a refusal that was never reported
