---
id: 3e8826ccef87c8dd
kind: bug
status: fixed
title: 'BUG: append_entry''s anchor is undiscoverable through the very surface its error tells you to use — get truncates headings from the front, the anchor lives at the bottom'
tags:
- librarian
- trackers
- progressive-disclosure
- error-hint
closed: 2026-08-27
opened: 2026-08-27
owner: marius
severity: med
unverified: 'Not live-verified through a running server: needs `cargo rb` + `/mcp`, then one `artifact(action="get")` on a long ledger to confirm `last_heading` appears, and one deliberately-bad `append_entry` anchor to read the new hint. Gate-green and mutation-verified only. Separately still open, and deliberately out of scope: whether `grep` reaching a librarian-guarded artifact is intended or a gap in that guard.'
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


### Re-verified 2026-08-27 18:44 on a fresh build — and the fix direction narrows

Still live. `artifact(action="get", id="5696563f06b2c222")` returns **20 of 90** headings,
every one from the front (lines 1–605 of 4002). `## Template for new entries` — the anchor
this ledger's own augmentation prompt instructs callers to pass — is at the bottom and is
not among them.

**What the re-check corrected.** The response carries `total_headings: 90` and
`headings_truncated: true`, so the truncation is **disclosed, not silent**. Those fields are
not new and were not added by any of today's work: `git log -S` puts them in `3bccb234`
(2026-07-10, *"signal preview.headings truncation instead of silent cap"*). This bug had
been loosely grouped with the day's silent-partial-result findings; it is not one of them,
and saying so matters because it moves the fix.

**So the remedy is not "disclose the truncation".** That shipped six weeks ago and does not
help: the caller is told the list is incomplete and still has no way to reach the missing
part. Disclosure and discoverability are different properties, and this surface has the
first without the second — which is arguably worse than a silent cap, because the caller can
see that something is being withheld and has no argument that would reveal it.

Candidate fixes, re-ranked by that reading:

1. **Make the tail reachable.** A `headings_offset` / `headings_from="end"` argument, or
   simply always including the final N headings alongside the first N. The window's fill
   order is the defect; the flag announcing it is not.
2. **Change the hint to name a surface that works.** `append_entry`'s error could prescribe
   the ledger's own augmentation prompt (which states the anchor verbatim) rather than
   `get`. Cheapest, and correct for every ledger whose prompt already names its anchor.
3. Have `append_entry` fall back to the last heading when `anchor_heading` is omitted —
   removing the need to discover it at all for the conventional case.
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
   *read*, `full=true` returns the whole body. Neither lists headings cheaply. **Still true after
   the fix**: no parameter was added. The tail is now returned unconditionally when the cap bites,
   which needs no argument the caller has to know about.
2. **`read_markdown` on the file** — refused: `librarian_guard` blocks direct reads of a
   managed artifact and redirects to `artifact(get)`, the surface that truncates.
3. **`grep` on the file** — this is what worked, and it is the reason for filing rather
   than shrugging. `grep(path=…, pattern="^## ")` returns the anchor immediately. Whether
   `grep` reaching a guard-refused artifact is intended or a gap in the guard is **still not
   established** — it was not needed for this fix and is left as an open question. Note it is now
   only a *convenience*, not the sole route: the anchor is reachable from `artifact(get)`'s
   `last_heading` and named outright in the failure hint.
## Fix

**`ca8c550b4abb6c9b247e12e931b53d28c6c59b5d`** (`experiments`)
patch-id **`8235e4ae498d65364acf23b5972fd3cf85080f1f`**

Shipped candidates (1) and (2) from the list below, **plus a third surface the list did not
contain** — found only by enumerating the class after fixing (2), and it is the one the caller
actually sees first.

### Three surfaces, one defect class

All three windowed headings from the **front**, and a ledger's append anchor is its **last**
heading. Fill order and anchor position are exact opposites, so the needed heading was dropped
every single time.

| # | surface | was | now |
|---|---|---|---|
| 1 | `preview::headings::cap` (`artifact(get)`) | cap 20, head-only | returns the final heading; stamped as `last_heading` |
| 2 | `allocate_entry_id`'s anchor error | prescribed `artifact(action="get")` — the surface that cannot answer | names the last top-level headings directly |
| 3 | `resolve_section_range`'s *Available headings* | `take(15)`, head-only | keeps both ends, elision counted |

**`last_heading` is a separate field, not an extra element in `headings`.** That array is ordered
by line and a consumer may reasonably read it as a contiguous window; splicing a tail entry into
it would quietly falsify that reading.

### Surface 3 is the one this bug nearly missed

It surfaced from a mutation's own failure output. Reverting the hint (mutation C) showed the
**inner** error already enumerating — *"Available headings: # Ledger, ## R-7 — an entry, ## Template
for new entries"* — which meant the anchor was already discoverable on a **short** ledger and the
hint fix looked redundant. Reading that enumeration found `take(15)`: head-only, same direction,
same defect. On the real 92-heading ledger it lists `## H0 … ## H14` and drops the anchor.

So the recovery path had the defect twice over, and every heading-addressed tool routes through
surface 3 — not just `append_entry`. A caller mistyping a heading on any long document got the
first fifteen and no tail.

### Rejected

**"Have `append_entry` fall back to the last heading when `anchor_heading` is omitted"** (candidate
3 in § *Re-verified*). Omitting `anchor_heading` is an **established contract** meaning *reserve the
id, write nothing* — documented in `get_guide("tracker-conventions")` and in this repo's own ledger
prompts. A fallback would silently convert every reserve-only call into a write.

### Tests

Five, each mutation-verified with the blast radius predicted in advance and matched exactly:

| test | mutation that breaks it, and only it |
|---|---|
| `cap_reports_total_when_truncated` | drop the tail capture in `cap` |
| `a_truncated_preview_still_names_its_final_heading` | drop the tail capture in `cap` |
| `a_bad_anchor_names_the_anchors_that_do_exist` | revert the anchor hint to the `get` referral |
| `a_missing_heading_lists_both_ends_not_just_the_first_fifteen` | restore `take(15)` |
| `a_short_document_lists_every_heading_with_no_elision` | — guards against announcing an elision that did not happen |

Mutation E reproduced the old output verbatim (`## H0 … ## H14`, anchor absent), confirming the
test covers the real defect rather than a lookalike.

Gate: `cargo fmt`, `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`,
`cargo test` — **4737 passed, 0 failed**.
## Tests added

Five, all in-tree and mutation-verified — see the table in § *Fix*.

- `src/librarian/preview/headings.rs` — `cap_reports_total_when_truncated`,
  `cap_no_report_when_within_limit`
- `src/librarian/preview/default.rs` — `a_truncated_preview_still_names_its_final_heading`,
  `an_untruncated_preview_carries_no_last_heading`
- `src/librarian/catalog/augmentation.rs` — `a_bad_anchor_names_the_anchors_that_do_exist`
- `src/tools/file_summary/tests.rs` — `a_missing_heading_lists_both_ends_not_just_the_first_fifteen`,
  `a_short_document_lists_every_heading_with_no_elision`

The negative tests matter as much as the positive ones: an untruncated preview must grow no
`last_heading`, and a short document must not announce an elision that did not happen. Without
them the fix would be free to add noise to every small response.
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
