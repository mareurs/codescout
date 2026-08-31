---
kind: bug
status: fixed
tags:
- librarian
- doctor
- augmentation
- sidecar
- misleading-remedy
- no-op-reports-success
closed: 2026-08-31
opened: 2026-08-31
owner: marius
related: []
severity: medium
unverified: 'The MECHANISM half is confirmed live 2026-08-31 (skipped[] with 9 rows, totals.skipped 9, the all-skipped hint) against server e25850d6. The PROSE half — the corrected sidecar_shape_drift remedy — is test-guarded but was not observed live, because there is no drift left to trigger it: this session''s fix took sidecar_shape_drift to 0. Absence of a live observation here is a consequence of the repair, not a gap in it.'
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

**Fixed on `experiments` at `e9d3525b`** — patch-id
`b2ddd98e7311ea770c656dde4e5f32e292fd2785`.

Two halves, and this file's own *Fix* section had them the wrong way round: it proposed the
string edit first and the reporting change as a "consider separately". The reporting change
is the one that matters, and the project's Observer Blindness doctrine says why — a prose
remedy asks the reader to check harder, where a reported skip is the check that runs when
nobody is worried.

**Mechanism.** `export_augmentation_sidecars` returns a third bucket; the response carries
`skipped[]` and `totals.skipped`, each row naming the path, the sidecar, and the reason. The
`hint` gains an all-skipped arm placed **first**, because that is where a misled reader
actually lands: it states that the fix creates rather than refreshes, that every
`sidecar_shape_drift` finding has a sidecar by construction, and what to do instead.
Idempotence is untouched — `exported` still reports `0` on a second run, and the pinned test
that asserts it still passes.

**Prose.** The catalog-is-right branch now names the delete step, matching what
`sidecar_unparseable` — the same scanner's other present-sidecar case — has always said 25
lines above.

### Corrected while fixing: `artifact_augment` is not an alternative remedy

This file did not claim it was, but the reasoning was worth closing off, because
`write_through`'s own doc comment makes it look like one — it renders the catalog's shape
and byte-compares against disk, so it *would* repair a catalog-is-right drift if it ran.

It does not run. Both `sidecar_write_through` call sites in `src/librarian/tools/augment.rs`
(`:218`, `:431`) sit inside branches that **write the augmentation row**, and the
params-only merge path below them does not call it at all. So reaching it means re-supplying
shape, which is the thing you are trying to avoid when the catalog is already correct.

That makes delete-then-export the actual remedy rather than a workaround, and it is why the
fix is a corrected instruction rather than a redirect to a different call:

| path | behaviour on a catalog-is-right drift |
|---|---|
| `export_augmentations` | skips — `declared_already && sidecar_abs.is_file()` |
| `write_through` | never reached without a shape-writing call |
| `reindex` | attaches only when the row is **absent** |
## Tests added

Two, both RED first, in `src/librarian/tools/doctor.rs`.

- `the_drift_remedy_names_the_delete_step_without_which_the_export_is_a_no_op` — asserts the
  detail contains the **delete instruction**, never the token `export_augmentations`. This
  file predicted that trap before the fix existed and the prediction held: the shipped wrong
  string contains that token too, so a presence check on it is monotone under this exact
  defect and would have passed throughout. The RED printed the whole shipped string, which is
  what made it obvious the assertion was aimed correctly.
- `an_already_exported_artifact_is_reported_as_skipped_not_silently_omitted` — drives the
  real tool through `call(&ctx, …)`, so it guards the **response shape**, not just an
  internal return value. RED was `totals.skipped` returning `Null`.

**The drift fixture's load-bearing detail is annotated on its own line:** the committed
prompt must *differ* from the row's, because that is what makes `drifting_fields` non-empty
and so what makes the check fire at all. Match the two and the test reports nothing and
asserts nothing — passing, and blind. The `.git` marker is annotated for the same reason;
without it `lookup_git_root` fails and the artifact takes the `continue`.

**Both sites are guarded, unlike the previous fix in this session.** The remedy string is
covered directly, and the `skipped[]` shape is covered through the tool's own entry point —
so re-hardcoding either one fails a test rather than only a live check. What is *not* yet
done is verification against a rebuilt binary; the frontmatter's `unverified:` records that
as a freshness caveat rather than a coverage gap.

Counts confirm the placement: the default lane went 4970 → **4972**, the lean lane stayed at
3409. That is correct rather than a miss — `doctor.rs` is behind the `librarian` feature,
which the lean lane has off.
### Confirmed live 2026-08-31 — the mechanism half

Against the rebuilt binary (server `git_sha` `e25850d6`, pid 4024625). The same call that
opened this report — `librarian(action="doctor", fix="export_augmentations")` — now returns:

```
"totals": { "exported": 0, "failed": 0, "skipped": 9 }
"skipped": [ { "path": "docs/trackers/provenance-subsystem.md",
               "sidecar": "docs/augmentations/docs-trackers-provenance-subsystem.yaml",
               "reason": "already exported and declared — this fix CREATES sidecars and
                          never refreshes them. To republish a shape this catalog owns,
                          delete the sidecar and re-run." }, … 8 more ]
"hint": "Nothing written: every augmented artifact in scope already has a declared
         sidecar … If you arrived here from sidecar_shape_drift, note that all of its
         findings have a sidecar on disk — so this call could not have repaired one …"
```

`exported: 0` is unchanged and still correct. What is gone is the silence around it: the
run now names all nine artifacts it declined to touch and states, unprompted, the exact fact
this report had to be reconstructed by hand from source — that a `sidecar_shape_drift`
finding can never be repaired by this call.

**The prose half was NOT observed live, and the reason is a good one.** The corrected
`sidecar_shape_drift` remedy needs a live drift to appear in, and this session's repair took
`sidecar_shape_drift` to **0**. So the absence of a live observation here is a consequence
of the fix rather than a gap in it. That half is covered by
`the_drift_remedy_names_the_delete_step_without_which_the_export_is_a_no_op`, which asserts
on the delete instruction rather than on the token — deliberately, since the shipped wrong
string contained the token too.
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
