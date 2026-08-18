---
id: '51c76e8c6289350b'
kind: bug
status: fixed
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
closed: 2026-08-18
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

### Step (3) was the wrong fix — the rename shipped instead

**Verified on the wire 2026-08-18 after `cargo rb` + `/mcp`. The prediction above held 4/4:**
`counts.prefix_conflicts == 1`, and the single entry was `{prefix: "T", declared_by:
["ad1af8262fdce357"], defined_by: ["ad1af8262fdce357", "f2ecdd76a6189efb"]}` —
`fable-tuning-tasks` and `tool-usage-patterns` — on a live corpus of 1,060 artifacts. Neither
named failure mode fired: not `>1` (discriminator leaking), not `0` (wiring not reaching the
response). The unit fixtures prove the logic; this is the only thing that proves the logic
survives real data.

**Then step (3) was dropped, because checking it showed it was actively harmful.** Normalising
`T-001`→`T-1` would give `tool-usage-patterns` a contiguous `T-1`…`T-24`, and this filing
already recorded that `fable-tuning-tasks` holds `T-1`…`T-12` — so the normalisation would have
made twelve tokens **Ambiguous**, which is *failure mode #1 of this very bug*, self-inflicted.
Worse: `docs/trackers/artifact-augmentation-followups.md` keeps 21 row-only `T-N` tasks, eight of
which (`T-14`…`T-21`) already mis-bind to `tool-usage-patterns` — normalising would have taken
that to all 21. "The guard has to exist first" was true and insufficient: the guard **reports**,
it does not prevent, so its existence never licensed creating the collision.

**What shipped instead: `fable-tuning-tasks` surrendered the prefix, `T` → `FT`.**
`tool-usage-patterns` has the stronger claim — `CLAUDE.md` hard-codes `id_prefix="T"` for id
`f2ecdd76a6189efb`, it holds 24 entries against 12, and it is cited far more widely. Renamed
across every surface the prefix was baked into: frontmatter (`entry_prefix: FT`, plus a
`entry_high_water_FT: 12` that had never existed), the 12 defining headings, all 12 **params**
entry ids, the `params_schema` pattern `^T-\d+$` → `^FT-\d+$`, the augmentation prompt, the
title, and the citations in five other files — `fable-tuning-index`, `prompt-hamsa-audit-log`,
`fable-tuning-findings`, `fable-tuning-research`, `skill-frictions`. Residual fable `T-N` across
those six files: **0**. `tool-usage-patterns`' three `T-005` citations inside
`prompt-hamsa-audit-log`: **untouched**.

One mechanic worth recording: `params_schema` is validated against the **merged** result, so
writing `FT-` ids while the stored pattern still said `^T-\d+$` is refused. Schema and data must
move in a single `artifact_augment(merge=true, params_path=…, params_schema=…)` call — a prefix
rename on an augmented ledger is inherently atomic, which is the right design but not obvious
until the first attempt bounces.

**The rename closes BOTH failure modes this file names — the padding fix would have closed
neither.** #1 is gone because fable no longer owns `T`. #2 is gone because unpadded `T-1`/`T-2`
now have no codescout definer at all (`tool-usage-patterns` spells those `T-001`/`T-002`), so
`researcher/docs/trackers/langfuse-tracing-roadmap.md` can be backfilled without creating
ambiguity. Item **(1)**, the prose ceiling note, is dissolved rather than fixed: `FT` has one
definer and no ceiling, and that note now records the rename instead of a rule to obey.

Still open, handed off: **(4) renaming `researcher`'s prefix** — no longer urgent, per #2.
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

Nothing on this bug. `counts.prefix_conflicts` is **0** on the post-rename corpus, and that zero
is now documented in `resolve.rs` as the *healthy* state — so a later reader who runs the check,
gets nothing, and goes looking for the live `T` conflict this file describes does not conclude the
wiring is dead. The founding case survives as a test fixture, which is where a regression guard
belongs.

One consequence was recorded rather than fixed, and it is the finding most worth carrying forward.
Freeing `T` put project `dangling` **up**, 477 → 542. That is the correct direction: ~65
`T-1`…`T-12` citations that were never about fable tasks — prose references in three retired
`T-N` documents (`archive/i1-session-friction.md` alone has 57 mentions, `archive/i1-refactor-tasks.md`,
`archive/goal-tracker-dogfood-log.md`), plus `artifact-augmentation-followups`' 14 row-only tasks —
had been silently binding into fable's namespace as **wrong edges**, and they are honest danglings
again.

Which means an earlier measurement in this work stream was partly measuring the wrong thing.
`dangling` 548 → 471 was recorded as the BL-39 backfill working; part of that drop was citations
being **mis-bound**, not repaired. Nothing in the report distinguished the two, because nothing
can:

> **A falling `dangling` count is not evidence of repair when a namespace gains a definer.**
> "Citations repaired" and "citations mis-bound" move that number in the same direction. Measure
> a shared-prefix backfill by inspecting the *new edges*, not by watching `dangling` drop.

The still-live half — `artifact-augmentation-followups`' eight rows mis-binding to
`tool-usage-patterns`, which this rename correctly did not touch — is filed separately as
`docs/issues/2026-08-18-row-only-ids-bind-as-citations-to-whoever-owns-the-prefix.md`.
## References

- `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md` (BL-39 — the backfill that surfaced this)
- `docs/issues/archive/2026-08-18-link-scan-dangling-count-is-prefix-gated-so-a-whole-namespace-reads-as-healthy.md` (BL-41 — declared prefixes now widen the gate; archived `fixed` 2026-08-18)
- `get_guide("tracker-conventions")` § *Citing an entry — bare, or qualified*
- `docs/trackers/structural-debt-refactor.md` SD-2 (why a prose-enforced co-change contract is the shape to avoid)
- `docs/issues/2026-08-18-row-only-ids-bind-as-citations-to-whoever-owns-the-prefix.md` (the row-token leak this investigation uncovered — still open, and the reason step (3) was dropped)
