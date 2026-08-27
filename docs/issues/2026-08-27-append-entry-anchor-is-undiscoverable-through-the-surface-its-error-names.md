---
id: c131d83129b81e1b
kind: bug
status: open
title: 'BUG: append_entry''s anchor is undiscoverable through the very surface its error tells you to use — get truncates headings from the front, the anchor lives at the bottom'
tags:
- librarian
- trackers
- progressive-disclosure
- error-hint
closed: null
opened: 2026-08-27
owner: marius
severity: med
---

## Summary

`append_entry`'s anchor-not-found error tells the caller to *"Read the current headings
with `artifact(action="get", id=…)` and pass one of them."* On the artifact class this
feature exists for, that instruction cannot produce the needed answer: `get` returns a
**truncated, front-anchored** heading list, and the append anchor is by convention the
**last** heading in the file. The remedy names the one surface that structurally omits the
one heading you need.

## Symptom (Effect)

Composing an `append_entry` call against a prose ledger, the caller must supply
`anchor_heading` verbatim and has no supported way to learn it.

`artifact(action="get", id="5696563f06b2c222")` returns:

```
"line_count": 3743,
"total_headings": 85,
"headings_truncated": true,
headings: [ 21 entries, lines 1 → 602 ]
```

The anchor actually needed — `## Template for new entries` — is at **line 3681 of 3743**.
It is not in the window, and no documented parameter widens it.

Nothing fails loudly. The caller either guesses (safe but blind — see below), pulls the
whole body, or finds an unsanctioned read path.

## Reproduction

No spend, ~2 calls, on this repo:

```
artifact(action="get", id="5696563f06b2c222")     # reconnaissance-patterns.md
# -> total_headings: 85, headings_truncated: true, 21 returned, all from lines 1-602
# -> "## Template for new entries" (line 3681) absent

grep(path="docs/trackers/reconnaissance-patterns.md", pattern="^## Template")
# -> 3681: ## Template for new entries
```

Any ledger long enough to truncate reproduces it. The two in this repo that append most
often — `reconnaissance-patterns.md` (85 headings) and
`prompt-surface-measurement-session-log.md` (73) — both do.

## Environment

- codescout `444d756c` on `experiments`, freshly rebuilt (`cargo rb`) + `/mcp` reconnect
- Observed live during a reconnaissance pass, not from reading the source

## Root cause

Two correct behaviours meeting badly.

`preview.headings` is windowed for progressive disclosure and fills from the top of the
file. `append_entry` inserts **before** its anchor, so a ledger's append point is
conventionally its **last** heading — the template/footer stanza. The window's fill order
and the anchor's conventional position are opposites, so the truncation is not incidental
to this use: for a long ledger it drops the anchor **every time**.

The error hint (`src/librarian/catalog/augmentation.rs:1111`) then prescribes `get` as the
recovery:

```rust
"`anchor_heading` must name a heading that exists in the ledger verbatim. \
 Read the current headings with artifact(action=\"get\", id=…) and pass one \
 of them."
```

Correct for a short artifact. For the long ones, it routes the caller to a surface that
cannot answer.

## Evidence

- `total_headings: 85` vs 21 returned, `headings_truncated: true` — from a live call, not
  inferred.
- Returned window spans lines 1–602 of 3743; the anchor is at 3681.
- **A bad anchor is safe, just uninformative.** Pinned by
  `augmentation.rs::tests::a_bad_anchor_writes_nothing_at_all_not_even_the_high_water_mark`,
  and the error says so (*"no id was allocated and nothing was written"*). So the cost is
  a wasted round-trip, never a burned id — which is why this is `med` and not higher.
- The error does **not** enumerate valid headings, though it is raised at the point where
  the document is already in memory (`updated` is in scope).

## Hypotheses tried

1. **A parameter widens the window** — none found. `headings=[…]` selects sections to
   *read*, `full=true` returns the whole body. Neither lists headings cheaply.
2. **`read_markdown` on the file** — refused: `librarian_guard` blocks direct reads of a
   managed artifact and redirects to `artifact(get)`, the surface that truncates.
3. **`grep` on the file** — this is what worked, and it is the reason for filing rather
   than shrugging. `grep(path=…, pattern="^## ")` returns the anchor immediately. Whether
   `grep` reaching a guard-refused artifact is intended or a gap in the guard is **not
   established here** and is a question for whoever picks this up — the workaround may be
   the thing that gets closed.

## Fix

Not implemented. Three candidates, cheapest first:

1. **Enumerate the anchors in the error.** The document is already in memory at
   `augmentation.rs:1105`; listing the `##`-level headings (or the last N) turns a dead end
   into a one-shot recovery. Smallest change, fixes the reported defect, leaves discovery
   before the first call unsolved.
2. **Surface the append anchor in `get`.** For an artifact with `entry_prefix`, return the
   last top-level heading (or a `suggested_anchor`) alongside `headings_truncated`, so the
   caller never has to guess. Fixes discovery too.
3. **Make the window aware of what it is for.** When `headings_truncated` fires on a
   ledger, include the tail as well as the head. Broadest, and the one most likely to
   disturb other consumers of `preview`.

(1) and (2) are complementary and neither blocks the other. Recommend both; (3) only if a
second use for tail headings shows up.

Fix SHA + `git patch-id --stable`: *not yet fixed.*

## Tests added

None — not fixed. A regression test should build a ledger long enough to truncate,
assert the anchor is absent from `get`'s window, then assert the chosen fix surfaces it
(the error enumerates it, or `get` names it). Pinning the *absence* first is what keeps
the test honest if the truncation threshold later moves.

## Workarounds

- `grep(path="<ledger>", pattern="^## ")` — reliable today; see Hypotheses (3) for why it
  may not stay that way.
- On a ledger you have appended to before, reuse the anchor from that call.
- Convention holds across this repo's ledgers: the anchor is `## Template for new entries`.
  A guess is free (nothing is written, no id allocated) but tells you nothing when wrong.

## References

- `src/librarian/catalog/augmentation.rs:1105-1115` — the error and its hint
- `src/librarian/catalog/augmentation.rs::tests::a_bad_anchor_writes_nothing_at_all_not_even_the_high_water_mark`
- `src/librarian/tools/append_entry.rs:23` — the insert-before contract
- `docs/trackers/reconnaissance-patterns.md` — 85 headings, anchor at line 3681; `R-118`
  was appended during the pass that found this

