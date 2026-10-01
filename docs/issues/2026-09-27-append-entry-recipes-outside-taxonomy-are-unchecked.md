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

Partly done. Surfaces, as of 2026-10-01:

| Surface | State |
|---|---|
| Augmentation-sidecar prompts (`docs/augmentations/*.yaml`) | Gated by `66cf3b49`, authored by a session that ended before committing it and committed by `a520c25a`. Its mutations were not re-run by the committer. |
| Ledgers' own recipes (`docs/trackers/*.md`) | Gated by `b9cf290d16ed2b180f2c8135cc0782f14e173604`, patch-id `adc734b3971c7c8dcf5e2774a53348e375b1f87b`. 85 ledgers, 35 prose + 2 params + 1 mention of the form = the 38 above, none refused. 21 guarded sites mutated once each, all killed. |
| TAXONOMY outside *Main taxonomy* (resume-queue table, work-stream ledgers table) | **Not done.** Needs a second row grammar (see the second bullet under *Reproduction*), not a second input. |
| `CLAUDE.md` | **Not done.** Not measured either: the 2026-09-30 reproduction did not scan it. |

How the ledger-recipe surface was scoped, because the bug's own wording would have led elsewhere:

- **Whole body, not the template section.** Only 18 of the 38 calls sit under `## Template for new entries`; the rest are under how-to headings, a code-fence comment line and the preamble.
- **Raw, not rendered.** The template's own call is inside an HTML comment.
- **A bare mention is counted, not skipped.** `doc(action="append_entry", …)` in a how-to line (one in `bug-fix-session-log.md`) has no `id_prefix` and is not a recipe. The rule is exact: a call whose arguments, after the opener, are only an ellipsis. Any other call with no `id_prefix` is a finding.
- **A call is judged against the ledger it is written in.** A ledger documenting another ledger's call would be refused. None does today.
- **Reproduced the gap at the corpus level:** corrupting the template call in `bug-fix-session-log.md` (`id_prefix="F"` to `"Q"`) left the TAXONOMY and sidecar tests green, and only the new test went red.


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
