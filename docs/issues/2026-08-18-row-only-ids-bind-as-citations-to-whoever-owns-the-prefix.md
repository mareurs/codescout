---
id: b90140b87dbc0442
kind: bug
status: open
title: 'BUG: a row-only ledger''s ids bind as citations to whoever owns the prefix, and no count moves'
tags:
- link-scan
- entry-identity
- false-edge
- ledger
- silent-failure
topic: tracker-entry-identity
---

# BUG: a row-only ledger's ids bind as citations to whoever owns the prefix, and no count moves

## Summary

`docs/trackers/artifact-augmentation-followups.md` keeps 21 tasks as `| T-N |` **table rows**,
with **no defining heading** and **no `entry_prefix`** declaration. A row defines nothing — but
it is still body text, so `link_scan` extracts each `T-N` as a **citation**. Those citations then
bind to whichever artifact *does* define `T-N`.

The result is a wrong edge that no metric reports. `dangling` does not move (the citation
resolved). `ambiguous` does not move (it resolved to exactly one definer). `prefix_conflicts`
does not fire (this file declares nothing, so by that check's discriminator there is no
contradicted claim — correctly). The link graph simply gains an edge between two documents that
have nothing to do with each other.

## Symptom (Effect)

**Confirmed live, not latent.** `artifact(action="get", id="aabef87ec988dc1d",
include_links=true)` returned outgoing `cites` edges to **both** `T`-owning ledgers:

| edge target | why it is wrong |
|---|---|
| `ad1af8262fdce357` (`fable-tuning-tasks.md`) | rows `T-1`…`T-12` matched that ledger's entries. This file contains **no textual reference to fable-tuning anywhere**, and was last written ~2026-07-08 — six weeks before `fable-tuning-tasks` gained any defining heading. The edge cannot have been authored; it can only be derived from the row tokens. |
| `f2ecdd76a6189efb` (`tool-usage-patterns.md`) | rows `T-14`…`T-21` match that ledger's unpadded entries `T-14`…`T-24`. **Eight citations, still mis-bound today.** |

Downstream cost: `librarian(action="context", anchor_id=…)` and `artifact(action="graph")` both
read the graph, so a reader anchored on either ledger gets the other document packed in as a
neighbour on the strength of a coincidence of numbering.

## Reproduction

```
grep -oE '^\|[[:space:]]*T-[0-9]+' docs/trackers/artifact-augmentation-followups.md
#   -> T-1 .. T-21, all rows
grep -cE '^#{1,6}[[:space:]]+`?T-[0-9]+[[:space:]]+[—–-]' docs/trackers/artifact-augmentation-followups.md
#   -> 0   (no heading defines any of them)
grep -c '^entry_prefix' docs/trackers/artifact-augmentation-followups.md
#   -> 0   (undeclared, so no ledger check looks at it)
artifact(action="get", id="aabef87ec988dc1d", include_links=true)
#   -> outgoing cites: ad1af8262fdce357, f2ecdd76a6189efb
```

## Environment

codescout `experiments`, measured 2026-08-18 against the live catalog (1,060 artifacts,
umbrella `codescout-ecosystem`).

## Root cause

Two independent gaps compose, and each one alone would be harmless.

- **A row is not a definition, but it *is* a citation.** The asymmetry is deliberate and
  individually correct: `link_scan` binds a definition only to `## <ID> — <title>`, while
  citations are extracted from body text generally. The unintended consequence is that a
  ledger keeping its entries *only* in rows does not merely fail to define them — it
  **donates them as citations to a foreign namespace**. A row-only ledger in a prefix nobody
  else owns is invisible and harmless; the moment any artifact defines that prefix, every row
  becomes a wrong edge.

- **Every ledger check enumerates *declared* ledgers.** `ledger_defines_nothing`,
  `entry_without_definition` and BL-39's sweep all start from `entry_prefix`. This file has no
  declaration, so it is outside all of them by construction — which is why it survived
  BL-39's step-4 pass over ten ledgers without ever being considered.

## Evidence

Only this one file is a row-only `T` ledger. The other `T-N`-carrying documents were checked
and are **prose** references, not ledgers — correctly dangling, and not this bug:

| file | defining headings | `T-N` rows | verdict |
|---|---|---|---|
| `docs/trackers/artifact-augmentation-followups.md` | 0 | **21** | this bug |
| `docs/trackers/archive/i1-session-friction.md` | 0 | 0 | prose (57 mentions) |
| `docs/trackers/archive/i1-refactor-tasks.md` | 0 | 0 | prose |
| `docs/superpowers/plans/2026-05-17-i1-refactor.md` | 0 | 0 | prose |
| `docs/trackers/archive/goal-tracker-dogfood-log.md` | 0 | 0 | prose |
| `docs/evals/reconnaissance-trigger.md` | 0 | 0 | prose |

## Why the count could not catch it

This is the finding worth keeping. When `fable-tuning-tasks` was backfilled with headings
(`c7bdfd22`), project `dangling` fell 548 → 471 and that was recorded as the backfill working.
Part of that drop was this defect firing: ~65 `T-1`…`T-12` citations that were never about fable
tasks stopped dangling by being **mis-bound**. Freeing the prefix again (the `T` → `FT` rename)
put `dangling` back up to 542.

So: **a falling `dangling` count is not evidence of repair when a namespace gains a definer.**
"Citations repaired" and "citations mis-bound" move that number in the same direction, and
nothing distinguishes them. Any future backfill of a ledger whose prefix is shared should be
measured by inspecting the *new edges*, not by watching `dangling` drop.

## Fix

Two candidates; both are content decisions, so neither is applied here.

1. **Give the ledger its own declared prefix with defining headings** — e.g. `AA-1`…`AA-21`
   with `entry_prefix: AA`, `entry_high_water_AA: 21`, and one `## AA-N — <title>` heading per
   entry, re-pointing the two in-file cross-references (`T-3` at lines 256, 265). This makes
   the entries citable for the first time and removes both wrong edges. It is the shape
   `get_guide("tracker-conventions")` § *One entry format, never two* prescribes.
2. **De-tokenize the rows** if the entries are genuinely not worth citing — the tracker is
   largely historical (phases 0–4 `done`, phase 5 `open`). Renumbering the rows to a
   non-token form (`1.`, `2.`) removes the citations without inventing a namespace.

(1) is preferred: it is the standard, and the file is still `active`.

## A detector gap worth its own decision

`prefix_conflicts` is right not to fire here — an undeclared file makes no claim to contradict,
and widening it to undeclared co-definers would fire on the eight `F`/`W` session logs, which is
exactly the noise that check was designed to avoid. But nothing else covers this either. A
candidate narrow check: **an artifact with ≥N `PREFIX-M` rows, no defining heading for any of
them, and no `entry_prefix`, where some *other* artifact defines that prefix.** All four
conjuncts are needed — the last is what separates a wrong edge from a harmless private
numbering. Not filed as a fix here because it needs its own false-positive measurement pass
across the corpus.

## Resume

Decide between fix (1) and fix (2) with the user — it is their tracker's identity. If (1), the
`AA` prefix was verified free on 2026-08-18 (`grep -rlE '\bAA-[0-9]+\b' docs` → no matches);
re-verify before allocating, since this session also minted `FT`.

## References

- `docs/issues/2026-08-18-three-ledgers-own-prefix-t-kept-apart-only-by-zero-padding.md` — the `T` collision whose investigation surfaced this
- `docs/issues/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md` (BL-39) — the row-defines-nothing rule; this is its undeclared-ledger blind spot
- `get_guide("tracker-conventions")` § *One entry format, never two*
- `src/librarian/tools/link_scan/resolve.rs` — `prefix_conflicts` and why it stays silent here

