---
kind: bug
status: fixed
tags:
- prompt-surfaces
- librarian
- doc-vs-code
- guide-duplication
closed: 2026-08-16
opened: 2026-08-16
owner: marius
related: []
severity: medium
---

# BUG: `get_guide("librarian-runtime")` says `artifact(action="move")` preserves the id; a move mints a new one

## Summary

`src/prompts/guides/librarian-runtime.md` § *Where catalog state lives* tells the
agent that `artifact(action="move")` "preserves `id`". A move **mints a new id** —
`id = sha256(abs_path)`, so re-pathing necessarily re-keys the row. The advice the
sentence gives is still correct (prefer `move` over delete+recreate); only the
mechanism it cites is false. That is the dangerous shape: an agent that believes
ids are stable across a move will cache one and hit `unknown id` later — which is
precisely the failure the code comment at `src/librarian/tools/mv.rs:131-132`
anticipates.

Same shape as the archived B-9
(`docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md`),
where an auto-injected guide asserted a carve-out the gate never had; that one
measured 0/10 unaided survival against the false sentence.

## Symptom (Effect)

`get_guide("librarian-runtime")` returns, verbatim:

```
- **They do NOT survive a file delete+recreate.** A recreated file gets a
  new `id`, orphaning the old augmentation. Use `artifact(action="move")`
  to relocate a tracker (preserves `id`), never delete+recreate.
```

The parenthetical is false. Three other guides state the opposite correctly.

## Reproduction

At `52030894` (HEAD when filed):

```
read_markdown(path="src/prompts/guides/librarian-runtime.md",
              heading="## Where catalog state lives")
```

→ contains "preserves `id`". Compare against the tool schema the same server ships:

```
grep(pattern="MINTS A NEW ID", glob="src/librarian/**/*.rs")
```

→ `src/librarian/tools/artifact.rs:104`.

## Environment

codescout `experiments`, HEAD `52030894`. Guide bodies are `include_str!`'d
(`src/prompts/mod.rs:151`), so the served text is fixed at build time — see the
freshness note under *Evidence*.

## Root cause

One fact lives in four separate markdown files with no gate cross-checking them,
so a correction can repair three and miss the fourth. Commit `2d8c7f39`
(2026-08-16) corrected `librarian.md` (+22 lines) and `tracker-conventions.md`
(+29 lines) and **did not touch `librarian-runtime.md`** — measured 2026-08-16 via
`git show 2d8c7f39 --stat`.

The underlying behaviour, cited at `path:line`:

- `src/librarian/tools/mv.rs:133-134` — the response emits `"previous_id": a.id`
  and `"id_changed": grafted.is_some()`.
- `src/librarian/tools/mv.rs:227-228` and `:429-430` — both tests assert
  `result["id_changed"] == true`.
- `src/librarian/tools/artifact.rs:104` — the `new_rel_path` schema states
  "a move MINTS A NEW ID (id = sha256(abs_path)); the artifact's events, links,
  observations and augmentation are grafted onto it and the old row is dropped."

So augmentation *does* survive a move — grafted onto the **new** id. The guide's
recommendation is sound; its explanation inverts the mechanism.

measured 2026-08-16: `grep(pattern="previous_id|id_changed|MINTS A NEW ID",
glob="src/librarian/**/*.rs")` → 4 sites above, read directly.

## Evidence

### The three guides that state it correctly

- `src/prompts/guides/librarian.md` § *Archiving / Moving Trackers* — "**A move
  mints a new id.** … `id = sha256(abs_path)`, so moving a file necessarily
  re-keys it"
- `src/prompts/guides/tracker-conventions.md` § *Bug files* — "**The move mints a
  new id, and reports it.**"
- `src/prompts/guides/tracker-conventions.md` § *Cross-linking* — "**Entry IDs are
  stable; artifact ids are not.**"

### Build-freshness aggravator

Guide bodies are compiled in, so a session receives whatever the running binary
was built with. A binary built before `2d8c7f39` serves a `librarian.md` whose
§ *Archiving / Moving Trackers* lacks the mint-new-id block entirely — meaning
that session gets `librarian-runtime.md`'s wrong claim with **no counterweight
anywhere in its context**. See R-89 in `docs/trackers/reconnaissance-patterns.md`
(build freshness and process freshness are separate facts).

### Adjacent but distinct

`docs/issues/archive/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md`
(`61e2360408cb206b` — fixed and archived 2026-08-16) covers frontmatter-vs-catalog id skew
after a move. That is a
different defect: this one is a doc error in a prompt surface, not a state skew.

## Hypotheses tried

1. **Hypothesis** — the guide is describing an older behaviour that was correct
   when written. **Test** — `git log` on `src/librarian/tools/mv.rs` for a commit
   removing id-stability. **Verdict** — deferred; irrelevant to the fix either
   way, since the shipped text must match current code regardless of its history.

## Fix

**Implemented 2026-08-16 on `experiments`.** Both parts, as this file required —
and the second is the one that matters.

### 1. The sentence

`src/prompts/guides/librarian-runtime.md` § *Where catalog state lives*. The
recommendation is unchanged (prefer `move` over delete+recreate); the **reason** is
replaced:

> **`move` also mints a new `id`** — identity is `sha256(abs_path)`, so it cannot
> do otherwise. What makes it the right call is that it *grafts* the augmentation,
> events, links and observations onto the new id and drops the old row, all in one
> transaction; delete+recreate leaves them orphaned. The response reports
> `previous_id` and `id_changed` so prose citing the old id can be re-pointed.

The old text — "use `artifact(action="move")` to relocate a tracker (preserves
`id`)" — gave the right advice for the wrong reason, which is the harder kind of
drift to notice: the recommendation kept working, so nothing forced a re-read.

### 2. The guard test

`no_guide_claims_a_move_preserves_the_id`
(`src/prompts/mod.rs`, beside `iron_laws_detail_gate_claim_matches_path_predicate`).
Scans **every** registered `GUIDE_TOPICS` body for `preserves \`id\``,
`preserves the \`id\``, `preserves id`.

This is the half this file correctly insisted on. Correcting one file is what
`2d8c7f39` already did — it repaired three copies of this claim and missed the
fourth, which is why this bug exists. A per-file fix cannot prevent a per-file
omission; only a scan over the whole set can.
## Tests added

`no_guide_claims_a_move_preserves_the_id` (`src/prompts/mod.rs`).

**Confirmed to discriminate, not merely to pass.** A test written after the
correction is green by construction and proves nothing, so the claim was
temporarily reintroduced into `librarian-runtime.md` and the test re-run:

```
FAILED — guide 'librarian-runtime' says a move preserves `id` — it mints a new
         one and grafts the history onto it.
```

Then removed again and re-run green. Without that step this would be a guard whose
ability to guard was never established.

Gate: `cargo fmt` clean, `cargo clippy --all-targets -- -D warnings` clean,
`cargo test --lib` 3742 passed / 0 failed.
## Workarounds

Trust the tool schema over the guide: `artifact`'s `new_rel_path` description is
generated from the same code path that performs the move. After any
`artifact(action="move")`, read `id` from the response — never reuse a cached id.

## Resume

**Closed 2026-08-16.** Fast-forward promotion, so the `experiments` SHA is the
master SHA — no second SHA to record.

No live verification needed: the artifact is a guide body, and the invariant test
is the check. `get_guide("librarian-runtime")` will serve the corrected text from
the next build.
## References

- `src/prompts/guides/librarian-runtime.md` § *Where catalog state lives*
- `src/librarian/tools/mv.rs:131-134`, `:227-228`, `:429-430`
- `src/librarian/tools/artifact.rs:104`
- `docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md` (B-9, same shape)
- `docs/trackers/reconnaissance-patterns.md` § R-89 (build/process freshness)
- commit `2d8c7f39` — the three-of-four correction that motivated this file

## Fix provenance

- **SHA:** `2d8c7f39` (experiments-only) — positional; does not survive a rebase of `experiments`.
- **patch-id:** `02d35a27b9e6baada8d5a07cc47a5c19fef8d546` — content hash of the diff; survives rebase and cherry-pick.

If the SHA stops resolving, recover the commit by patch-id. Use redirects, not pipes —
codescout's Iron Law 3 blocks an unbounded `git log -p` piped to a trimmer:

```
git log --all -p > /tmp/all.patch
git patch-id --stable < /tmp/all.patch > /tmp/patch-ids.txt
grep 02d35a27b9e6 /tmp/patch-ids.txt
```

Each hit is `<patch-id> <commit>`. Several hits mean the change exists on several
branches (cherry-pick) and any of them is the fix. Recorded 2026-08-19.
