---
id: '51c76e8c6289350b'
kind: bug
status: investigating
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

**Step (2) SHIPPED 2026-08-18 — as `link_scan`'s `prefix_conflicts`, not as a `doctor` check.**
That is a deliberate deviation from this filing, for two reasons. `link_scan` already builds the
definition index and, since BL-41, already carries each artifact's `entry_prefix` declarations on
`DocExtract` — so the information is in hand. A `doctor` version would have re-parsed 1,056
artifacts to recompute it. And the finding is about the index as a whole rather than about any one
row, which is the shape `link_scan` reports in and `doctor` does not.

### The discriminator was the hard part, and the obvious check is useless

A plain "prefix owned by several ledgers" fires on `F`/`W`, where **eight** session logs each
define `F-1`…`F-5`. That is the *documented* per-work-stream convention — each log owns its own
counter, citations are qualified by file stem — so the check would flag a blessed pattern, and
`ambiguous` already quantifies its real cost (~400 citations, 49 of 50 sampled being F/W). A check
whose findings are mostly "yes, by design" is noise.

The pairing that works is **declared × active**:

- **Declared.** A frontmatter `entry_prefix` is an author claiming exclusivity; a second definer
  contradicts the claim. Declining to declare is declining to claim — which is precisely what the
  eight session logs do, and none of the nine prefixes declared in this repo is `F` or `W`.
- **Active.** An archive companion is the compaction ladder's endpoint, not a rival: the resolver
  already binds a token to its sole *active* definer.

Both exclusions are load-bearing on real data rather than defensive. Measured by grep before
writing any code:

| prefix | definers | fires? |
|---|---|---|
| `R` | `reconnaissance-patterns` + its **archived** companion (45 entries) | no — 1 active |
| `U` | `codescout-usage-frictions` + its **archived** companion (3 entries) | no — 1 active |
| `GF` `CAP` `H` `FND` `SD` `HY` | one file each | no |
| `T` | `fable-tuning-tasks` (declared) + `tool-usage-patterns` (active, undeclared) | **yes** |

Without the archived-companion exclusion the check would fire three times, two of them false.

**It compares token STRINGS via the definition index**, never parsed indices — the caveat this
filing flagged. `body_defined_indices` parses `"001"` to `1`, so an index-based comparison would
have concluded `T-001` and `T-1` are the same entry and missed the very case it was written for.

A declared prefix that defines nothing is deliberately silent: `ledger_defines_nothing` owns that
case, and such entries are uncitable regardless of who shares the namespace.

### Still open

- **(3) normalise the padding** in `tool-usage-patterns.md` (`T-001` → `T-1`), re-pointing the 16
  external citations of `T-0NN` in the same commit. **Must come after (2), which it now does:**
  normalising removes the accidental disjointness that is currently the only thing preventing a
  collision, so the guard has to exist first. It does now.
- **(1) the prose ceiling note** in `fable-tuning-tasks.md` remains a convention enforced by
  prose — the shape SD-2 exists to remove. Treat it as a stopgap that (2) has now made
  redundant for detection, though not for prevention: nothing stops an allocation, it is only
  reported.
- **(4) renaming `researcher`'s prefix** — handed off with the cross-repo tracker work.
## Tests added

Four in `src/librarian/tools/link_scan/resolve.rs`, written before the method existed and watched
fail. Three of them exist to pin an *exclusion*, which is where this check's correctness lives:

| test | what it pins |
|---|---|
| `a_declared_prefix_with_a_second_active_definer_is_a_conflict` | the finding fires, and names **both** active definers |
| `undeclared_co_definers_are_not_a_conflict` | the F/W convention stays silent — the false positive that would have made the check noise |
| `an_archived_co_definer_is_not_a_conflict` | the compaction ladder stays silent — worth 2 of 3 real-corpus firings |
| `a_declared_prefix_with_one_definer_is_not_a_conflict` | the healthy case, so the three above cannot pass against a method that returns empty unconditionally |

That last one is the guard against the failure this suite is otherwise wide open to: three
"must be empty" assertions are all satisfied by a stub, so one accepting case is what makes the
other three discriminate.

Gate: `cargo fmt` 0, `cargo clippy --all-targets -- -D warnings` 0, **4168 passed / 0 failed**.
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

**Not yet verified on the wire, and there is a falsifiable prediction to check.** `link_scan` runs
in the MCP server and has no CLI subcommand, so it needs `cargo rb` + `/mcp` first.

**Prediction:** `counts.prefix_conflicts` is **1**, and the single entry is
`{prefix: "T", declared_by: [<fable-tuning-tasks>], defined_by: [<fable-tuning-tasks>,
<tool-usage-patterns>]}` as artifact ids. If it reports more than one, the discriminator leaks and
the extra rows say where; if zero, the wiring does not reach the response. Recorded as a
prediction rather than a result on purpose — the corpus table above came from grep, which is
strong evidence about the data and none at all about the code path.

Then do **(3)**, the padding normalisation, which (2) now makes safe to attempt.

Status is `investigating` rather than `fixed`: step (2) ships the detector, but the hazard this
file reports — two ledgers one edit from colliding — is still present in the data. Detection is not
the fix; (3) is.
## References

- `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md` (BL-39 — the backfill that surfaced this)
- `docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md` (BL-41 — declared prefixes now widen the gate; archived `fixed` 2026-08-18)
- `get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified*
- `docs/trackers/structural-debt-refactor.md` SD-2 (why a prose-enforced co-change contract is the shape to avoid)
