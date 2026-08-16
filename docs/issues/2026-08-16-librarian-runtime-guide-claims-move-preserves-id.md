---
status: open
opened: 2026-08-16
closed:
severity: medium
owner: marius
related: []
tags: [prompt-surfaces, librarian, doc-vs-code, guide-duplication]
kind: bug
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

`docs/issues/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md`
(`6149f4cfeaa6fab9`) covers frontmatter-vs-catalog id skew after a move. That is a
different defect: this one is a doc error in a prompt surface, not a state skew.

## Hypotheses tried

1. **Hypothesis** — the guide is describing an older behaviour that was correct
   when written. **Test** — `git log` on `src/librarian/tools/mv.rs` for a commit
   removing id-stability. **Verdict** — deferred; irrelevant to the fix either
   way, since the shipped text must match current code regardless of its history.

## Fix

Plan — two parts, matching the two failure modes:

1. **Immediate:** correct the sentence in `src/prompts/guides/librarian-runtime.md`
   § *Where catalog state lives*. Keep the recommendation, replace the reason:
   `move` is right because the catalog grafts events/links/observations/
   augmentation onto the new id, whereas delete+recreate drops them.
2. **Structural:** this fact should have exactly one home. Folding catalog-identity
   semantics into a shared core that all three librarian-family guides reference is
   the open design work — see the guide-dedup design doc, where this bug is the
   motivating case (one fact, four files, a same-day fix that missed one).

## Tests added

None yet. A regression test in the shape of
`prompts::tests::iron_laws_detail_gate_claim_matches_path_predicate`
(`src/prompts/mod.rs`) is the right form: assert no guide body contains a
"preserves `id`" claim about `move`, and that the corrected wording is present.
That test is what makes the fix durable rather than another one-file correction
the next drift undoes.

## Workarounds

Trust the tool schema over the guide: `artifact`'s `new_rel_path` description is
generated from the same code path that performs the move. After any
`artifact(action="move")`, read `id` from the response — never reuse a cached id.

## Resume

Apply Fix step 1 to `src/prompts/guides/librarian-runtime.md` § *Where catalog
state lives* (the bullet beginning "**They do NOT survive a file
delete+recreate.**"), then add the guard test described in *Tests added* to
`src/prompts/mod.rs` next to the B-9 regression test. Re-run
`cargo test --lib -- prompts`. Do not close until the guard test exists — a bare
text fix leaves the four-copy drift mechanism fully intact.

## References

- `src/prompts/guides/librarian-runtime.md` § *Where catalog state lives*
- `src/librarian/tools/mv.rs:131-134`, `:227-228`, `:429-430`
- `src/librarian/tools/artifact.rs:104`
- `docs/issues/archive/2026-08-15-iron-laws-detail-guide-claims-cat-on-source-is-allowed.md` (B-9, same shape)
- `docs/trackers/reconnaissance-patterns.md` § R-89 (build/process freshness)
- commit `2d8c7f39` — the three-of-four correction that motivated this file
