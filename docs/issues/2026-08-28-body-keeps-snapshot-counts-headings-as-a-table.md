---
id: ec40b63996d15b62
kind: bug
status: open
title: 'BUG: body_keeps_snapshot uses id coverage as a proxy for "has a rendered table", but headings satisfy coverage — so the entry-heading convention the project adopted in August makes it false-positive on every conforming tracker'
owners:
- marius
tags:
- librarian
- augmentation
- append_entry
- update_entry
- false-positive
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

Not started. **Direction, not a decision** — reproduce before building:

Discriminate on **how** an id is anchored, not how many are. An id appearing only
as `## <ID> — <title>` is a prose entry; an id appearing as a table row (`| <ID> |`)
is a snapshot row. Gate on the *row* count, so:

- table-bearing trackers (`windows-platform-support`) keep today's behaviour;
- heading-only trackers (`tool-usage-patterns`) go silent, correctly;
- the `provenance-subsystem` false positive stays suppressed, since scattered
  mentions are neither.

`link_scan`'s extractor already classifies both shapes, so the parsing exists.

**Do not simply raise the threshold**, per Hypothesis 2 — that moves the boundary
between two populations that are on the same side of it.

## Tests added

None yet. The regression test must include a fixture the current suite lacks:
**100% coverage via headings, zero table rows**, asserting `body_keeps_snapshot`
is `false`. Today's fixtures cover 100%-with-table (true) and 21%-scattered
(false); the failing case is the third shape neither pins. Note
`doctor.rs:6746` (`params_behind_body_is_not_gated_on_body_keeps_snapshot`)
deliberately exercises the low-coverage path — the new test is its complement, not
a replacement.

## Workarounds

Ignore `snapshot_hint` on a tracker with no table. Check first:

```
grep -c '^| *<PREFIX>-' <tracker path>
```

Zero rows means the hint is spurious. **Do not add a table to satisfy it** — that
reintroduces the two-formats defect BL-39 removed.

## Resume

Add the failing fixture described under *Tests added* and confirm it fails against
`body_keeps_snapshot` as written. Then change the predicate to count table-row
anchors rather than any anchor, and re-run the three existing snapshot fixtures in
`src/librarian/tools/doctor.rs` (~`:6746`, ~`:6836`) plus
`src/librarian/catalog/augmentation.rs` to confirm the true positives still fire.

## References

- `src/librarian/catalog/augmentation.rs:1405-1413` — the predicate
- `src/librarian/catalog/augmentation.rs:470-487` — the two hint strings
- `src/librarian/tools/append_entry.rs:267-276` — the `snapshot_missing` path
- `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` — the bug this warning was built for; still valid for table-bearing trackers
- `get_guide("tracker-conventions")` § *One entry format, never two* — the convention that makes this fire
- Found during the 2026-08-28 cross-machine restore; see `docs/conventions/cross-machine-catalog-resume.md`

