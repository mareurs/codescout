---
id: fc491a58e7a9b561
kind: bug
status: open
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

Not started. Reuse the gate's scanner and checks with a second input per surface. The sidecar prompts are YAML strings, not table rows, so the row scanner does not apply there as-is; the call walker and the three checks do.


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
