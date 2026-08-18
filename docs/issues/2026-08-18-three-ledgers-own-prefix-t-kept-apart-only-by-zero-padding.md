---
id: '51c76e8c6289350b'
kind: bug
status: open
title: 'BUG: three ledgers own prefix T, and zero-padding is the only thing keeping their tokens apart'
owners:
- marius
tags:
- librarian
- link-scan
- entry-identity
- namespace
- latent
topic: tracker-entry-identity
---

# BUG: three ledgers own prefix `T`, kept apart only by zero-padding

## Summary

Three ledgers claim the `T` id namespace. Their tokens do not currently collide, but the
only reason is a **formatting inconsistency inside one of them** — `tool-usage-patterns.md`
spells its first thirteen entries zero-padded (`T-001`…`T-013`) and its later ones unpadded
(`T-14`…`T-24`). Because `link_scan`'s token grammar is `\b[A-Z]{1,3}-\d+\b` and matching is
by **string**, `T-001` and `T-1` are different tokens. That accident is what leaves
`fable-tuning-tasks.md`'s `T-1`…`T-12` unambiguous.

Nothing records the invariant, nothing enforces it, and two things would break it.

## Symptom (Effect)

Latent, not yet observed in a live miscitation. Two distinct failure modes are one edit away:

1. **Allocating `T-13`+ in `fable-tuning-tasks.md`.** `T-14`…`T-24` are live, defined tokens
   in `tool-usage-patterns.md` with citations across ten files. A fable-tuning `T-14` would
   make every one of them **Ambiguous**, which resolves to nothing — the same end state as
   dangling, but harder to notice because no count of "undefined" moves.
2. **Backfilling `researcher/docs/trackers/langfuse-tracing-roadmap.md`.** It owns `T` with
   2 entries, `T-1` and `T-2`. Giving them defining headings makes `T-1` and `T-2` ambiguous
   between two repos, and those are exactly the tokens `prompt-hamsa-audit-log.md` cites.

A third, milder issue exists today: an author writing the natural `T-13` gets a **dangling**
citation, because the entry is spelled `T-013`. There is one unpadded `T-13` in the corpus
(`bug-fix-session-log.md:467`) but it is prose noise — "a parallel session's T-13 commit",
an old plan's task numbering — not a miscitation. So the padding trap is real but unfired.

## Reproduction

```
grep -n '^#\{1,6\}[[:space:]]\+`\?T-[0-9]\+' docs/trackers/tool-usage-patterns.md
#   -> T-001..T-013 padded, T-14..T-24 unpadded
artifact(action="get", id="ad1af8262fdce357", entry_filter={"id": {"prefix": "T"}})
#   -> T-1..T-12
artifact(action="get", id="53796e12647f711e")            # researcher, 2 `tasks` entries, prefix T
```

## Environment

codescout `experiments` @ `c7bdfd22`. Measured 2026-08-18 against the live catalog
(1,055 artifacts, umbrella `codescout-ecosystem`).

## Root cause

Two independent gaps compose:

- **Prefix ownership is not exclusive and nothing checks it.** `entry_prefix` is a
  declaration, not a claim against a registry. Three ledgers can each declare `T` and no
  surface reports the overlap. `doctor` reports entries with no definition and ledgers that
  define nothing; it does not report *two ledgers defining the same token space*.
- **The resolver matches token strings while the allocator parses numbers.**
  `body_defined_indices` does `"001".parse::<u64>()` and gets `1`, so `doctor`'s numeric view
  treats `T-001` as index 1 — while `link_scan`'s `by_token` map keys on the literal
  `"T-001"`. The two disagree about whether `T-001` and `T-1` are the same entry, and the
  disagreement is invisible because each is internally consistent.

## Evidence

| Ledger | Repo | Spelling | Defined? |
|---|---|---|---|
| `docs/trackers/tool-usage-patterns.md` | codescout | `T-001`…`T-013`, `T-14`…`T-24` | yes, 24 headings |
| `docs/trackers/fable-tuning-tasks.md` | codescout | `T-1`…`T-12` | yes as of `c7bdfd22` |
| `docs/trackers/langfuse-tracing-roadmap.md` | researcher | `T-1`, `T-2` | **no** |

The padded form is genuinely in use, so it cannot simply be normalised away without
re-pointing citations: `\bT-0\d\d\b` matches 34 times across 11 files — 18 in
`tool-usage-patterns.md` itself and 16 in ten others.

## Hypotheses tried

- *"Backfilling `fable-tuning-tasks` would create ambiguity."* **Wrong**, and it was the
  premise recorded at the previous session close. Checked rather than assumed: the padding
  makes the spaces disjoint, so the backfill was safe. Recording it here because the wrong
  version of this belief nearly blocked a correct piece of work.

## Fix

Not implemented. Ordered cheapest-first; the first is worth doing regardless.

1. **Write the invariant down where an allocator would hit it.** Done in part by
   `c7bdfd22`, which records the ceiling in `fable-tuning-tasks.md`'s own body. That is a
   convention enforced by prose — the failure mode SD-2 exists to remove — so it is a
   stopgap, not the fix.
2. **Report prefix overlap in `doctor`.** A `prefix_owned_by_several_ledgers` check over
   `entry_prefix` declarations plus observed definitions. Cheap, and it turns a latent trap
   into a visible one. Note it must compare **token strings**, not parsed indices, or it will
   miss exactly this case.
3. **Normalise the padding in `tool-usage-patterns.md`** (`T-001` → `T-1`) and re-point the
   16 external citations in the same commit. This removes the trap at its source but
   *creates* the collision that (1) and (2) guard — so it must come after a prefix decision
   for the other two ledgers, not before.
4. **Rename the smaller claimants' prefixes.** `researcher`'s 2-entry ledger is the cheapest
   to rename. Out of scope for a codescout session: different repo.

## Tests added

None yet. A regression test belongs with fix (2).

## Workarounds

**Cite `T-N` qualified by file stem** — `fable-tuning-tasks:T-7`,
`tool-usage-patterns:T-17`. The qualifier is the documented mechanism for a shared prefix
and is correct today *and* after any of the fixes above.

`prompt-hamsa-audit-log.md` already disambiguates in prose ("run as fable-tuning **T-7**"),
so converting those four citations to the qualified form only formalises what the author
wrote. Deliberately not done in `c7bdfd22`: with the researcher ledger left unbackfilled,
`fable-tuning-tasks` is the sole definer and the bare tokens resolve correctly, so the edit
would have been churn. It becomes required the moment researcher's `T-1`/`T-2` gain headings.

## Resume

**Scoped to codescout 2026-08-18.** Both claimants that matter here are codescout's own —
`tool-usage-patterns.md` and `fable-tuning-tasks.md` — so the hazard is fully ours to fix even
though a third claimant sits in `researcher`.

Do **(2) report the overlap in `doctor`** first. It is the only step that converts a latent trap
into a visible one, and it must compare **token strings** rather than parsed indices —
`body_defined_indices` parses `"001"` to `1`, so an index-based comparison would miss precisely
this case and ship a check that cannot see the bug it was written for.

Then **(3) normalise the padding**, re-pointing the 16 external citations of `T-0NN` in the same
commit. Not before (2): (3) removes the accidental disjointness that is currently the only thing
preventing a collision, so the guard has to exist first.

**(4) is out of scope** — renaming `researcher`'s 2-entry prefix is that repo's call, and it was
handed off with the rest of the cross-repo tracker work. The original Resume named "whoever
backfills researcher's ledger without reading this file" as the immediate risk; that backfill is
no longer queued here, so the immediate risk is now simply that (3) gets done before (2).

The interim guard remains the ceiling note written into `fable-tuning-tasks.md`'s own body by
`c7bdfd22` — a convention enforced by prose, which is the shape SD-2 exists to remove, so treat
it as a stopgap with a known expiry rather than a fix.
## References

- `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md` (BL-39 — the backfill that surfaced this)
- `docs/issues/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md` (BL-41 — declared prefixes now widen the gate)
- `get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified*
- `docs/trackers/structural-debt-refactor.md` SD-2 (why a prose-enforced co-change contract is the shape to avoid)
