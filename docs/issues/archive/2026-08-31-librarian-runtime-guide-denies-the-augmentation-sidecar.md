---
kind: bug
status: fixed
title: 'BUG: librarian-runtime guide denies the augmentation sidecar — the same-day corrective sweep named three places and this was the fourth'
tags:
- cluster/doc-contradicted-by-code
- guides
- librarian
- augmentation
- doc-drift
closed: 2026-09-01
opened: 2026-08-31
owner: marius
related: []
severity: medium
---

# BUG: `librarian-runtime` guide denies the augmentation sidecar — the same-day corrective sweep named three places and this was the fourth

## Summary

`src/prompts/guides/librarian-runtime.md` tells the reader that augmentation state
cannot be persisted to disk and that sharing it is "local-only by design". The
augmentation sidecar (`docs/augmentations/*.yaml` + `expects_augmentation:`) exists,
is implemented, and is documented in a *different* guide. A reader who consults the
runtime guide — the topic whose own header advertises it as the place for "where
catalog state lives" — concludes there is nothing to export, and does not export.
The failure is silent: the shape is lost on the next clone or DB loss, and nothing
reports it.

## Symptom (Effect)

Two claims in the served guide text, both false as of today:

`src/prompts/guides/librarian-runtime.md:101`

```
| Augmentation (`prompt`, `params`, `params_schema`, `render_template`, `entry_collection`) | the **catalog DB only** | **No** — there is no on-disk representation |
```

`src/prompts/guides/librarian-runtime.md:123`

```
- **To share a filterable index with teammates**, the structured rows would
  need to be persisted into the file (frontmatter/body) — the catalog alone
  is local tooling state. As of 2026-05, retrofits are local-only by design.
```

The row is not wholly wrong, and the precision matters: **`params` genuinely have no
disk form** and are deliberately excluded from the sidecar. The defect is that the row
lumps `prompt`, `params_schema`, `render_template` and `entry_collection` in with
`params` under one "No", when those four are exactly what the sidecar carries.

Measured 2026-08-31:

```
grep -c "sidecar\|expects_augmentation" src/prompts/guides/librarian-runtime.md   → 0
grep -c "sidecar\|expects_augmentation" src/prompts/guides/tracker-conventions.md → 13
```

## Reproduction

1. `get_guide("librarian-runtime")` — read § *Where catalog state lives (and what is
   not in the repo)*.
2. `get_guide("tracker-conventions")` — read § *Declaring an augmentation*.
3. The two describe incompatible worlds. Only the second matches the code.

## Environment

codescout on branch `experiments`, Arch Linux (`Linux 7.1.9-zen1-2-zen`). Observed from
a downstream consumer repo (`claude-plugins`) via the MCP server, not from a checkout of
codescout itself.

## Root cause

Not decay over months — a corrective sweep that enumerated its targets by memory and
stopped one short. Measured timeline:

| when | what |
|---|---|
| 2026-05-29 `b51b785e` | the guide first documents "no on-disk representation". **True when written.** |
| 2026-06-03 `27bf11dc` | the guide is split core + runtime reference; the line rides into `librarian-runtime.md`. |
| 2026-08-26 `dab5c530`, `4155d041` | `expects_augmentation` and the doctor check land; `tracker-conventions` documents them. |
| 2026-08-30 `e799f29d` | the sidecar ships — *"make augmentation shape travel in git (BL-50)"*. **The claim becomes false here.** |
| 2026-08-30 `e1b91221` | a deliberate sweep: *"state that augmentation shape now travels, **in the three places that said otherwise**"* — `CLAUDE.md`, `docs/conventions/cross-machine-catalog-resume.md`, `src/prompts/guides/tracker-conventions.md`. |
| 2026-08-31 (HEAD) | `librarian-runtime.md:101` and `:123` still say otherwise. It was the fourth place. |

So the drift is **one day old, not three months.** An earlier draft of this file said three
months; that was inferred from the guide's own "As of 2026-05" sentence rather than measured
against the code, and is corrected here.

That reframes the defect. Nobody failed to re-check: the author of `e1b91221` re-checked
prose against code deliberately, and was correct about every surface they listed. The list
was produced from memory, and **a sweep reports what it changed, never what it should have**
— so its completeness is unfalsifiable from the commit alone. "Three places" reads as a
finding; it is an enumeration.

**That the missed surface is this one is not bad luck.** `librarian-runtime.md` offers
itself in its own header as the authority on *"where catalog state lives"* — precisely the
question — while `tracker-conventions`, which *was* swept, is the sibling a reader reaches
only if they already doubt the answer. The sweep corrected the documents that mention the
feature and missed the one that denies it, which is the harder set to enumerate: you cannot
grep for the absence of a concept.

The implementation is present and reachable —

- `src/librarian/augmentation_sidecar.rs` — `AugmentationSidecar` carries
  `schema_version`, `prompt`, `entry_collection`, `params_schema`, `render_template`,
  `append_mode`, `history_cap`; `path_for` at `src/librarian/augmentation_sidecar.rs:79`
  keys the file on the artifact's repo-relative path (deliberately not on the 16-hex id,
  which `artifact(action="move")` re-mints).
- `src/librarian/tools/doctor.rs` — `fix="export_augmentations"` writes the sidecar and
  stamps the declaration; `augmentation_declared_but_absent`, `sidecar_unparseable` and
  `sidecar_shape_drift` are live checks.
- `src/librarian/tools/reindex.rs` — re-attaches a declared sidecar when the row is
  absent, reporting `augmentations_restored`.

Measured 2026-08-31, not inferred: `librarian(doctor, fix="export_augmentations",
confirm=true)` run against `claude-plugins` wrote two sidecars and stamped two
trackers; the follow-up scan reported `sidecar_shape_drift: 0` and
`sidecar_unparseable: 0`.

## Evidence

### The runtime guide's own framing is what makes this load-bearing

Its header offers itself as the authority for this exact question:

```
Deep operational detail for working against a live librarian — ... where catalog state
lives, classifier overrides, and event-authorship discipline.
```

A reader routed there for "where catalog state lives" gets the wrong answer and has no
cue to cross-check `tracker-conventions`.

### The check that should catch the consequence cannot

`augmentation_declared_but_absent` fires only on a *declared* sidecar that is missing.
An augmentation that was never exported is undeclared, so the check reads 0 — identical
to "nothing to declare". Measured on `claude-plugins` before the export: two live
augmentations, zero declarations, `augmentation_declared_but_absent: 0`.

## Hypotheses tried

1. **Hypothesis:** the runtime guide documents the sidecar elsewhere in the file and only
   this table row is stale.
   **Test:** `grep -c "sidecar\|expects_augmentation" src/prompts/guides/librarian-runtime.md`.
   **Verdict:** rejected — 0 occurrences in the whole file.

## Fix

**Applied 2026-09-01 on `experiments`.**

- SHA: `0523b823` (`experiments` — orphans on the next rebase)
- patch-id: `9ec0e7c8911be27700318ba60b945454275391e7` (survives rebase and cherry-pick)

**Four sentences, not the two filed here.** Steps 1 and 2 below were applied as written. Reading
the rest of § *Where catalog state lives* found two more claims false for the same reason, which
is this bug's own lesson applied to itself — an enumeration produced from memory stops at the
examples that prompted it:

- *"An augment produces no git diff … `git status` stays clean"* — false since write-through.
  Verified in code rather than inferred from a sibling guide: `sidecar_write_through` is called
  at `augment.rs:248` and `:492`. Corrected to say the `.md` **body** is untouched while
  `docs/augmentations/` is not, and that a params-only merge leaves the sidecar byte-identical.
- The `reindex` bullet was true but incomplete — `reindex` also **restores** an absent
  augmentation from a declared sidecar, reporting `augmentations_restored`. A reader deciding
  whether a clone is safe needs that clause, and its absence is what made the guide read as
  "nothing travels".

Step 3 (a `doctor` check for the undeclared-and-unexported state) was **not** taken — it is a
separate change with its own design question, and is left as this entry's open follow-on rather
than folded in here.

Not yet implemented. Proposed:

1. Split the `librarian-runtime.md:101` table row in two — `params` (no disk form, by
   design) and augmentation *shape* (sidecar, `expects_augmentation:`) — rather than
   flipping the single "No" to "Yes", which would then be wrong about `params`.
2. Replace the `:123` "local-only by design" paragraph with a pointer to
   `tracker-conventions` § *Declaring an augmentation*, so one guide owns the mechanism
   and the other links to it.
3. Consider a `doctor` check for the un-declarable case: an augmentation row with no
   `expects_augmentation:` declaration is currently invisible, which is the state that
   makes the stale guide costly rather than merely wrong.

## Tests added

`prompts::redesign_invariants::no_guide_denies_the_augmentation_sidecar` — scans **every**
registered guide, not the one that drifted, matching its sibling
`no_guide_claims_a_move_preserves_the_id` directly above it. That sibling's doc comment records
the *same* guide section being missed by the *same* kind of three-place sweep on 2026-08-16, so
this file is the second instance and the new test says so.

**It asserts both directions, and both were mutation-verified per site.** An absence assertion
alone is monotone under removal — deleting the section satisfies `!contains` exactly as a correct
section does — so a positive half asserts `librarian-runtime` still *mentions* the mechanism:

| mutation | expected | result |
|---|---|---|
| reintroduce `local-only by design` | absence half fires | **FAILED** — correct |
| strip every `expects_augmentation` / `sidecar` mention | positive half fires | **FAILED** — correct |

Each fires only on its own direction and with its own message, so neither half is carrying the
other. A phrase guard is **not** an instrument for `IC-11`'s behavioural sub-shape in general —
nothing reference-based can be, which is that class's whole finding — it closes this one section
against a third sweep.

None yet. Item 3 above is the testable part; items 1–2 are prose. A guide-text assertion
test would be low value, but the item-3 check is a normal `doctor` unit test in the style
of `check_frontmatter_id_flags_only_an_id_that_is_present_and_wrong`.

## Workarounds

Read `get_guide("tracker-conventions")` § *Declaring an augmentation* instead; it is
correct. To persist augmentation shape today:

```
librarian(action="doctor", fix="export_augmentations")              # dry run
librarian(action="doctor", fix="export_augmentations", confirm=true)
```

Run it on the machine whose catalog still holds the rows — it can only export what that
catalog has. Then commit `docs/augmentations/` together with the stamped frontmatter.

## Resume

Edit `src/prompts/guides/librarian-runtime.md` lines 101 and 123 per § Fix items 1–2.
Then decide item 3: add a `doctor` check for an augmentation row lacking an
`expects_augmentation:` declaration, or record explicitly why undeclared-and-unexported
is acceptable to leave unreported.

## References

- `src/prompts/guides/librarian-runtime.md:101`, `:123` — the stale claims
- `src/prompts/guides/tracker-conventions.md` § *Declaring an augmentation* — the correct account
- `src/librarian/augmentation_sidecar.rs` — implementation
- `src/librarian/tools/doctor.rs` — `export_augmentations`, `sidecar_shape_drift`
- `src/librarian/tools/reindex.rs` — re-attach path (`augmentations_restored`)
- Downstream consumer where this was found: `claude-plugins` commit `9a68b22`
