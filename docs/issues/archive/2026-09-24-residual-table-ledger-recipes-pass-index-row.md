---
id: fd6e59eac80544f9
kind: bug
status: fixed
title: 'RESIDUAL: Migrate table-keeping ledger recipes (docs/TAXONOMY.md, tracker-conventions) to pass index_row + index_after_line, or gate appends that omit them'
tags:
- cluster/shared-resource-carries-no-owner
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Migrate table-keeping ledger recipes (docs/TAXONOMY.md, tracker-conventions) to pass index_row + index_after_line, or gate appends that omit them.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md` (status `fixed`):

> The window is closed for callers who USE the new parameters; it is not closed for callers who do not. `index_row` + `index_after_line` are opt-in, so any ledger whose appends omit them keeps the original two-call window unchanged. Nothing migrates the 21 table-keeping ledgers' callers, and no gate requires the parameters — a recipe in docs/TAXONOMY.md or get_guide("tracker-conventions") that still prescribes the second call will keep producing the window. The original file's other unverified: also still stands — the window is now observed on a second ledger (bug-fix-session-log:F-118, this session) but "every table-keeping ledger has it" remains reasoned from the protocol rather than measured per ledger.

## Fix

Done 2026-10-01 in three commits, the migration first so the gate lands green.

**Re-checked against HEAD before acting, as this file asked.** The caveat was still true, and wider than it said. 29 recipes for table-keeping ledgers passed no `index_row`: 23 calls in 14 ledger bodies, 4 TAXONOMY rows (`R`, `OB`, `OP`, `IC`) and 1 sidecar prompt (`reconnaissance-patterns`). The template's own entry-template prose, and eight ledger copies of it, still said to add the row afterwards. So did `get_guide("tracker-conventions")` ("Write the index row after, never before"), the surface the caveat names.

**Migration, `744ec80d`.** Placement is per table, from its measured order. Ascending tables (ten session logs, `operator-rules`, `issue-clusters`) declare `snapshot_anchor` once in frontmatter, so `index_row` alone lands the row at the tail. Newest-first tables (`observer-blindness`, `reconnaissance-patterns`) pass `index_after_line` set to their own separator line, unique in each file, so the row lands at the top. `shell-gating`'s Wins table has its own separator and its recipe names it. `statement-validity`'s W recipe uses `index_after_line` and says the row lands at the top of an impact-ordered table. `U`, `H` and `HY` keep no rows and need nothing. The `reconnaissance-patterns` sidecar was updated in the catalog (merge-only, so params and render template were not reset) and in its committed yaml.

**Gate, `862ea3fe`.** Every prose recipe on the three surfaces that name a target is judged against the ledger it writes: if that ledger keeps rows for the recipe's `id_prefix`, the recipe must pass `index_row` and either `index_after_line` or a `snapshot_anchor` the ledger declares. Red first on the real corpus with 28 findings, cleared by the migration.

**Guide and copies, `7cd664dd`.** The guide's example call and its "after, never before" bullet now say the row rides the same call, and the eight copies' entry-template sentence says the same. The never-BEFORE half stays: the allocator still counts an id an index row claims.

**Limits, stated rather than implied:**

- The gate judges recipes that have a target ledger. A guide example has none, so the guide is fixed by hand and is **not** gated: a future guide recipe that omits `index_row` is not caught.
- A table whose first cell is wrapped (`**F-1**`) is not recognised as holding rows, which errs toward not requiring `index_row`. No ledger here writes one.
- `prompt-surface-compaction`'s Wins Index cannot share its single `snapshot_anchor`, so a W-N row there is still a second write, and its recipe says so.
- The parent's other caveat, that "every table-keeping ledger has the window" was reasoned from the protocol and not measured per ledger, is not answered here: this closes the recipes, not the measurement.

## Tests added

Ten, in the `taxonomy_recipes` module (default lane): which tables count as keeping rows (and the near misses), a missing `index_row`, per-prefix judgement, an `index_row` with each kind of anchor, a form mention and a params call, an undeclared prefix reported once, and the TAXONOMY and sidecar surfaces. `check_main_taxonomy_recipes` is extracted from the corpus test so the wiring can be tested against a fixture, because the corpus tests read the real docs, which stay clean, and deleting the gate call from either TAXONOMY check would otherwise pass.

**Mutation:** 17 sites, all killed, re-run after `rustfmt` changed the file's bytes.

## Fix provenance

- **SHA:** `862ea3fe0ffcbd60c4956401a245e353ede1f5d4` (`experiments`)
- **patch-id:** `7cb6787b17a26e431d343bd1ec89869921b9b3ef`

That pair is the gate. The migration is SHA `744ec80df3e6829b3eb3048f15de59371918d6d3`, patch-id `29f059f81ac452b246cf2db0dff40ffb56119988`; the guide and copies are SHA `7cd664dd9fda9ddaced4a7335b88dd35c37930ae`, patch-id `433d074c2736f021478477420be5da1ab5f99bfb` (`git show <sha> | git patch-id --stable`).

## References

- `docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md` — parent
