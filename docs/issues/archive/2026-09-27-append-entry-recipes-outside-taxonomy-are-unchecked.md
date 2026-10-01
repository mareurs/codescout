---
id: 4eb7e1a822959af4
kind: bug
status: fixed
title: 'BUG: append_entry recipes outside TAXONOMY are not checked by the recipe gate'
tags:
- cluster/selector-narrower-than-its-population
- append-entry
- doc-to-code
---

# BUG: append_entry recipes outside TAXONOMY are not checked by the recipe gate

**Valid:** dated 2026-09-27

## Summary

The TAXONOMY recipe gate (`every_taxonomy_append_entry_recipe_is_one_the_code_accepts`, in `src/librarian/tools/append_entry.rs`) reads only the *Main taxonomy* table of `docs/TAXONOMY.md`. Three other surfaces also route writers to `append_entry` and are unchecked: `CLAUDE.md`, the committed augmentation-sidecar prompts under `docs/augmentations/`, and each ledger's own template section. **So are TAXONOMY's own other tables** — the gate reads only *Main taxonomy*, while `## Work-stream-specific prefixes` carries its own recipe-bearing tables (the resume-queue recipe and the work-stream ledgers table, e.g. an `entry_collection="findings"` row); found by the final review of the gate, 2026-09-27. The parent of residual `96b2b1b9a25bb1b0` found a sidecar prompt (test-escape-hardening's) instructing a call that its own frontmatter made impossible at the time.

## Fix

Fixed on `experiments` 2026-10-01. Every surface the bug named is now either gated or measured at zero recipes:

| Surface | State |
|---|---|
| Augmentation-sidecar prompts (`docs/augmentations/*.yaml`) | Gated by `66cf3b49`, authored by a session that ended before committing it and committed by `a520c25a`. **Its mutations were not re-run by the committer and have not been since**, so how many of its tests would catch a real regression is unmeasured. |
| Ledgers' own recipes (`docs/trackers/*.md`) | Gated by `b9cf290d16ed2b180f2c8135cc0782f14e173604`, patch-id `adc734b3971c7c8dcf5e2774a53348e375b1f87b`. 85 ledgers, 35 prose + 2 params + 1 mention of the form = the 38 above, none refused. 21 guarded sites mutated once each, all killed. |
| TAXONOMY outside *Main taxonomy* (resume-queue table, work-stream ledgers table) | Gated by `d1bbb018a6df35c09c0766da460ae2fce171766b`, patch-id `5f0530552cf0fd68c1d36dc72d4115c417b9db29`. 9 + 6 rows (13 prose, 2 params), none refused. 23 guarded sites mutated once each, all killed; the first pass left two inert clauses surviving, which were deleted and the set re-run. |
| `CLAUDE.md` | **Measured, not gated: it holds no recipe.** Its only call-shaped text is `doc(action="append_entry")` with empty arguments, a pointer to "their atomic recipes", which are the DCX and DWF calls in `docs/trackers/` (one each) and so are covered by the ledger gate. A recipe written into `CLAUDE.md` later would be unchecked; that is the residual. |

How each new surface was scoped, because the bug's own wording would have led elsewhere:

- **Ledger bodies: whole body, not the template section.** Only 18 of the 38 calls sit under `## Template for new entries`; the rest are under how-to headings, a code-fence comment line and the preamble. Scanned raw, because the template's own call is inside an HTML comment.
- **A bare mention is counted, not skipped.** `doc(action="append_entry", …)` in a how-to line (one in `bug-fix-session-log.md`) has no `id_prefix` and is not a recipe. The rule is exact: a call whose arguments, after the opener, are only an ellipsis. Any other call with no `id_prefix` is a finding.
- **A call is judged against the ledger it is written in.** A ledger documenting another ledger's call would be refused. None does today.
- **TAXONOMY tables: recognised by exact header.** `### Measured drift` sits in the same section and holds a table whose first header cell is also `Prefix`, so keying on that cell would have raised a false finding. A row mentioning `append_entry` in a table the scanner does not read, or without a bold label, or with an Append cell that is neither `prose` nor `entry_collection="…"`, is a finding, so a third layout cannot pass unchecked.
- **Reproduced the gap at the corpus level, twice.** Corrupting the template call in `bug-fix-session-log.md` (`id_prefix="F"` to `"Q"`) left the TAXONOMY and sidecar tests green and only the ledger test went red. Corrupting the FND row's collection in TAXONOMY left the Main-taxonomy, sidecar and ledger tests green and only the work-stream test went red, naming `docs/TAXONOMY.md:199`.

**Left open on purpose, and the reason it is not this bug:** `GF-N` and `SD-N` are `prose` in TAXONOMY while their sidecars declare `entry_collection` (`findings`, `items`; `structural-debt-refactor`'s prompt teaches the params call). Both ledgers also declare `entry_prefix`, so the code accepts either call and neither is refused. Which shape is meant is undecided, and the gate's contract ("a recipe the code accepts") cannot ask.


## Fix provenance

Three commits close the four surfaces; the last is cited here, the others are in the table above (`b9cf290d16ed2b180f2c8135cc0782f14e173604`, patch-id `adc734b3971c7c8dcf5e2774a53348e375b1f87b`, and `66cf3b49`, which is a peer's).

- **SHA:** `d1bbb018a6df35c09c0766da460ae2fce171766b` (`experiments`)
- **patch-id:** `5f0530552cf0fd68c1d36dc72d4115c417b9db29`


## Reproduction (2026-09-30)

Ran the gate's own refusal conditions (`check_prose`: the call's `id_prefix` is in the ledger's declared `entry_prefix`; `check_params`: the call's `entry_collection` equals the collection declared by the sidecar the ledger's `expects_augmentation` names) over every surface the gate does not read, at `87df2092`. Method: a throwaway script per surface, not committed. It balance-scans `doc(action="append_entry"` calls, pairs a sidecar with its ledger by filename and confirms the pairing from the ledger's `expects_augmentation`.

| Surface | Calls / rows | Refused by the gate's conditions |
|---|---|---|
| `docs/augmentations/docs-trackers-*.yaml` prompts | 14 calls | 0 |
| ledgers' own template sections (`docs/trackers/*.md`) | 38 calls | 0 (one of the 38 is a line of prose about the form, not a recipe) |
| TAXONOMY outside *Main taxonomy*: resume-queue table | 9 rows | 0 |
| TAXONOMY outside *Main taxonomy*: work-stream ledgers table | 6 rows | 0 |

**So the gap is real and nothing is wrong inside it today.** The instance the parent found (a sidecar prompt instructing a call its own frontmatter made impossible) has since been repaired. The gate would not have caught it, and would not catch its return.

Three things the run shows that the Summary does not say:

- **The work-stream table is invisible to the gate for a second reason.** Its recipe sits in an `Append` column as `prose` or `entry_collection="…"`, with no `doc(action="append_entry"` opener, and the scanner keys on that opener. Extending the gate to it needs a second row grammar, not a second input.
- **Two rows describe a different shape than their ledger's own sidecar.** `GF-N` and `SD-N` are `prose` in TAXONOMY, and their sidecars declare `entry_collection` (`findings`, `items`; `structural-debt-refactor`'s prompt teaches the params call). Both ledgers also declare `entry_prefix`, so the code accepts either call and neither is refused. Which shape is meant is undecided; the gate's contract ("a recipe the code accepts") cannot ask.
- **The sidecar prompts are the surface that has already failed once**, and have the fewest rows to scan (14), so they are the cheapest first extension.

## References

- `docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md` — § *Out of scope* and decision 3
- `docs/issues/archive/2026-09-02-two-trackers-have-no-open-append-path.md` — names the four surfaces
