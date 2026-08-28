---
id: ac7b2b741844aa87
kind: bug
status: fixed
title: 'BUG: body_keeps_snapshot treats a heading as a table row, so it false-POSITIVES on heading-only trackers and false-NEGATIVES on a table that genuinely lags — one predicate, both errors, one cause'
owners:
- marius
tags:
- librarian
- augmentation
- append_entry
- update_entry
- false-positive
closed: 2026-08-28
---

# BUG: `body_keeps_snapshot` counts headings as a table

## Summary

`body_keeps_snapshot` decides whether a tracker maintains a rendered snapshot by
measuring **majority id coverage** between `params` and the body. But a body
satisfies coverage with `## <ID> — <title>` **headings** alone — no table needed.
So `append_entry` / `update_entry` emit a `snapshot_stale` / `snapshot_missing`
hint telling the maintainer "the committed table disagrees with the catalog" for
trackers that have **no table at all**.

The sting: `get_guide("tracker-conventions")` § *One entry format, never two*
requires a `## <ID> — <title>` heading per entry — it is the only shape
`link_scan` reads as a definition. **Conforming to the convention guarantees ~100%
coverage, and therefore guarantees this false positive.**

## Symptom (Effect)

Both write paths emit it, on a tracker with zero table rows:

```
"This tracker renders a snapshot in its body, and its `T-30` row still shows
 the PREVIOUS field values — params changed, the file did not. Update the row
 via artifact(action="update", patch={body_edits: [...]}), or the committed
 table disagrees with the catalog."
```

There is no committed table. `CLAUDE.md` § *Tool Usage Patterns* says so
outright: *"The params table is **not in the file**"* — a stored
`render_template` is projected into `librarian(action="context")` only, and **no
path writes it to disk**.

## Reproduction

Measured 2026-08-28 at `14aab5ff`:

```
$ grep -c '^### T-'  docs/trackers/tool-usage-patterns.md      # 30  headings
$ grep -c '^| *T-'   docs/trackers/tool-usage-patterns.md      #  0  table rows
$ grep -c '^### WIN-' docs/trackers/windows-platform-support.md #  35 headings
$ grep -c '^| *WIN-'  docs/trackers/windows-platform-support.md #  35 table rows
```

`tool-usage-patterns` has 30 params rows and 30 headings → coverage 30/30 → the
gate returns **true** → the hint fires. `windows-platform-support` has a genuine
snapshot table and is a **true** positive. The function cannot tell them apart.

Then run any `update_entry` against `f2ecdd76a6189efb` and read the returned
`snapshot_hint`.

## Environment

Branch `experiments` @ `14aab5ff`, linux, codescout 0.15.0, release build.

## Root cause

`src/librarian/catalog/augmentation.rs:1405-1413`:

```rust
pub(crate) fn body_keeps_snapshot(
    claimed: &BTreeSet<u64>,
    in_body: &BTreeSet<u64>,
) -> bool {
    if claimed.is_empty() || in_body.is_empty() { return false; }
    claimed.intersection(in_body).count() * 2 > claimed.len()
}
```

`in_body` is the set of ids **line-anchored anywhere** in the body. The function's
own doc-comment is explicit that majority coverage is a *proxy*, chosen to separate
two measured populations:

| coverage | shape | what it was |
|---|---|---|
| 100% | contiguous prefix | 11 maintained snapshots, in sync |
| 61% | contiguous prefix | `prompt-hamsa-audit-log.md` — a real lag, caught |
| 21% | scattered, holes | `provenance-subsystem.md` — params-canonical, a false positive |

**The calibration is sound and the conclusion still does not hold**, because the
sample contained no instance of the case that now breaks it: a tracker with **one
heading per entry and no table**. That shape sits at 100% coverage —
indistinguishable, under this predicate, from a maintained table.

Measured on the same machine: `tool-usage-patterns` is exactly that shape, and it
is the tracker `CLAUDE.md` documents `append_entry` / `update_entry` for.

**The codebase already has the missing distinction.** `link_scan` binds an entry
token to a `## <ID> — <title>` heading and treats a table row as defining nothing
(`get_guide("tracker-conventions")` § *Entry headings — the definition rule*). The
concept exists; `body_keeps_snapshot` just does not consume it.

## Evidence

### 1. The false positive, and the true positive, side by side

Counts above. Both trackers have ~100% id coverage; only one has a table.

### 2. The convention makes the false positive the default

BL-39 (`docs/trackers/open-issue-work-queue.md`, done/archived) moved the project
to headings-as-index precisely because *"a table row defines no citable token"*.
Its backfills gave many trackers a heading per entry. Every tracker that complied
now trips this predicate. The better the corpus conforms, the more this fires.

### 3. The failure mode is a wild-goose chase, not a crash

The hint instructs the reader to fix a table via `body_edits`. On a table-less
tracker there is nothing to edit, and a maintainer who takes the advice literally
would **add** a hand-maintained table — reintroducing the "two entry formats" defect
BL-39 spent its whole life removing.


### 4. The same root cause ALSO produces the opposite error — a false negative

**Added 2026-08-28, after the hamsa restore. This is the more serious half.**

`docs/trackers/prompt-hamsa-audit-log.md` is a table-bearing tracker whose table
genuinely lags:

```
$ grep -c '^| A-'  docs/trackers/prompt-hamsa-audit-log.md   # 30  index rows
$ grep -c '^## A-' docs/trackers/prompt-hamsa-audit-log.md   # 34  entry headings
$ grep -c '^| A-3[1-4] ' docs/trackers/prompt-hamsa-audit-log.md   # 0
```

A-31 … A-34 exist as `## A-N — <title>` headings with **no index row**. That table
is not decorative: the tracker's own body calls it *"its **git-durable snapshot**"*.
So four rows live only in a machine-local catalog — precisely the defect
`snapshot_drift` was built for
(`docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`).

`librarian(action="doctor")` run immediately after restoring its augmentation:

```
any snapshot_* check present: []
snapshot_drift violations naming 59ebeebb6ed05c89: []
```

**Zero.** The check that exists for this case does not fire on it.

**Mechanism** — inferred from the two observations together, not read off a
single line:

- `tool-usage-patterns` has **0** table rows and the gate fires ⇒ `in_body` counts
  heading anchors, not just row anchors.
- Therefore, on the hamsa log `in_body` = all 34 (headings) rather than the 30 that
  are actually *in the table*, `claimed` = 34, intersection = 34 — no id looks
  missing, and the four-row table lag is invisible.

One predicate, two opposite failures, one cause:

| tracker shape | reality | reported |
|---|---|---|
| headings only, no table | nothing to drift | **false positive** — "the committed table disagrees" |
| table + heading-only new entries | table genuinely lags by 4 | **false negative** — silence |

The false negative is the worse one: a false positive wastes a maintainer's time,
while this one lets rows sit in a git-ignored catalog while the check that was
built to notice reports healthy. It also means the fix under **Fix** is not merely
noise-reduction — gating on row anchors repairs the miss as well as the nag, and
both follow from the same one-line change.
## Hypotheses tried

1. **Hypothesis:** the subagent that first reported this misread a real drift.
   **Test:** counted table rows directly (`grep -c '^| *T-'` → 0) and read
   `CLAUDE.md`'s statement that the table is not in the file.
   **Verdict:** rejected — there is no table, so no drift is possible.

2. **Hypothesis:** the gate is simply miscalibrated and a threshold change fixes it.
   **Test:** read the doc-comment's measured populations; the false case
   (`provenance-subsystem`, 21%) and this case (100%) sit on **opposite** sides of
   any coverage threshold.
   **Verdict:** rejected — no threshold separates them. Coverage is the wrong axis,
   not the wrong number.

## Fix

**Fixed 2026-08-28.** `experiments` SHA `16b5b243`, patch-id
`2293ef75e6a6525efc99c14d3c80b1eec0e25081`.

The *Direction* above was right and was followed. One refinement worth recording,
because this file's own title asserts the opposite: **the predicate was innocent.**
`body_keeps_snapshot` never saw the distinction it is accused of ignoring —
`body_claimed_indices` folds `## F-12` and `| F-12 |` into one `BTreeSet<u64>`
before the call. The fix is therefore upstream of it, and the predicate's body is
unchanged.

Added `body_snapshot_row_indices` (the `|`-anchored subset) and routed the three
snapshot call sites through it:

| site | asks |
|---|---|
| `augmentation.rs` `snapshot_stale_note` | does the body's ROW still show old values? |
| `augmentation.rs` `append_entry` → `snapshot_missing` | which rows is the table missing? |
| `doctor.rs` `scan_snapshot_drift` | has the rendered table fallen behind params? |

Every message at those sites already said *"the row"* and *"the committed table"*,
so all three were always asking the narrower question and being handed the wider
answer.

**Two call sites are deliberately NOT changed**, and narrowing either would be a
silent regression:

1. **Id allocation** in `append_entry` keeps the wide reading. Its own comment says
   the set answers *"one read, both directions"* — `body_max` drives the next id, and
   a heading claiming `F-33` **must** still block reissuing `F-33`, or every
   historical citation of it silently re-points.
2. **`scan_params_behind_body`** subtracts the other way round. An id the body
   defines by heading which `params` has never seen is a real finding there, and
   that check is documented as deliberately un-gated.
## Tests added

**Eight.** Three were red before the fix; two exist to stop the fix over-correcting.

| test | pins |
|---|---|
| `body_snapshot_row_indices_reads_rows_and_ignores_headings` | the split itself, against the same fixture the wide reader uses |
| `body_snapshot_row_indices_is_empty_for_a_headings_only_body` | the shape this bug asked for: 100% coverage, zero rows |
| `append_entry_does_not_claim_a_snapshot_when_the_body_is_headings_only` | **red before** — and asserts `undefined_in_body` still fires, so the advisory is corrected rather than lost |
| `append_entry_still_reports_a_missing_row_when_the_body_renders_a_table` | guard: a real table one row behind still fires |
| `snapshot_drift_is_silent_when_the_body_has_only_headings` | **red before** |
| `snapshot_drift_fires_when_headings_mask_a_lagging_table` | **red before** — the false-negative half |
| `snapshot_drift_is_silent_when_the_table_is_complete` | guard: not "always fire when a table exists" |
| `undefined_entries_covers_the_headings_only_body_snapshot_drift_now_ignores` | the coverage this fix depends on, asserted rather than assumed |

### One existing fixture changed, and why

`snapshot_drift_does_not_accept_a_prose_mention_as_a_snapshot_row` anchored its
three ids as **headings**. That no longer exercises this scan, so the anchors are
now index rows. Name, intent and assertions are untouched — prose must not count as
anchored, which is orthogonal to this change.

The shape it used to cover is not left unguarded: the new sibling test asserts
`scan_undefined_entries` reports that entry instead. That is the check whose remedy
— *"add the `## BL-4 — title` heading"* — is the one a headings-only tracker's
maintainer actually needs, where `snapshot_drift`'s *"re-render the table"* names a
table that does not exist.

### A first draft of the headings-only test was VACUOUS

Worth recording, because it passed and looked like evidence. It gave the body
headings for **every** params id. That shape passes today, **with the bug present** —
coverage clears the gate, then `claimed.difference(in_body)` is empty, so
`missing.is_empty()` hits `continue` before the table question is ever asked. It
asserted nothing and would have shipped as a green regression test.

Caught by running it and reading which tests went red. The fixture now omits one
params id from the headings, so the false positive is reachable and the test bites.
## Workarounds

Ignore `snapshot_hint` on a tracker with no table. Check first:

```
grep -c '^| *<PREFIX>-' <tracker path>
```

Zero rows means the hint is spurious. **Do not add a table to satisfy it** — that
reintroduces the two-formats defect BL-39 removed.

## Resume

**Done.** Nothing outstanding on this defect.

One deliberate non-goal, recorded so it does not read as an oversight: the
measured 100% / 61% / 21% coverage table in `body_keeps_snapshot`'s docstring was
**annotated, not re-measured**. Those figures came from the wide reading, so they do
not describe what callers now pass. They are kept because they are what justifies
the majority threshold — and that reasoning is unaffected, since the defect was
never the threshold. Re-deriving them under the narrow reading would be a fresh
measurement, not a correction, and nothing currently depends on the number.
## References

- `src/librarian/catalog/augmentation.rs:1405-1413` — the predicate
- `src/librarian/catalog/augmentation.rs:470-487` — the two hint strings
- `src/librarian/tools/append_entry.rs:267-276` — the `snapshot_missing` path
- `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` — the bug this warning was built for; still valid for table-bearing trackers
- `get_guide("tracker-conventions")` § *One entry format, never two* — the convention that makes this fire
- Found during the 2026-08-28 cross-machine restore; see `docs/conventions/cross-machine-catalog-resume.md`
