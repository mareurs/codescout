---
kind: bug
status: fixed
tags:
- cluster/unclassified
claimed_at: 2026-09-12
claimed_by: f3c594ce-c424-40d3-a603-9693cfef3f63
closed: null
opened: 2026-09-08
owner: marius
related: []
severity: low
---

# BUG: the staleness reason says "N of M anchored files changed" while N counts changed plus deleted, so the number a reader acts on names the wrong population

## Summary

`workspace(action="status")` renders each stale memory's reason as
`"{N} of {M} anchored files changed"`. N is `report.stale_files.len()`, which includes
anchors whose file was **deleted**, not only those whose content changed. The
distinction matters to the reader: a changed anchor means "re-read this and check the
claim"; a deleted anchor means "this memory may describe something that no longer
exists", which is a different and usually more urgent kind of wrong.

## Symptom (Effect)

Observed live 2026-09-08, before the memory refresh:

```
architecture:  "17 of 23 anchored files changed"
               changed_files: 15 entries
               deleted_files:  2 entries   (src/tools/usage.rs, src/tools/ast.rs)

worktree-merge-catalog-reconciliation:
               "4 of 8 anchored files changed"
               changed_files: 3 entries
               deleted_files: 1 entry
```

The response carries `changed_files` and `deleted_files` as separate arrays, so the
correct decomposition is right there — only the prose summary conflates them. A reader
who trusts the sentence and skips the arrays plans the wrong work.

## Reproduction

`workspace(action="status")` on a project with a memory anchoring a deleted file.
Compare the `reason` string against `len(changed_files)`.

## Environment

codescout 0.15.0, branch `experiments`. Observed at `5ec4bac8`; the two example
memories have since been repaired, so reproducing needs a memory that currently anchors
a deleted path.

## Root cause

`src/memory/anchors.rs:258-263` builds the string as
`format!("{} of {} anchored files changed", total_stale, total_anchored)` where
`total_stale = report.stale_files.len()`.

`check_path_staleness` (`src/memory/anchors.rs:184-207`) classifies each anchor as
`AnchorStatus::Deleted` when the file is missing and `AnchorStatus::Changed` when
`hash_file(&full) != anchor.hash`. Both land in `stale_files`. So the count is correct
as a count of *stale* anchors and wrong only as a description of *why* they are stale.

Measured 2026-09-08 from two live `workspace(action="status")` responses, decomposition
read from the response's own `changed_files` / `deleted_files` arrays.

## Evidence

The response shape already separates what the sentence merges:

```json
{ "topic": "architecture",
  "reason": "17 of 23 anchored files changed",
  "changed_files": [ ...15... ],
  "deleted_files":  [ "src/tools/usage.rs", "src/tools/ast.rs" ] }
```

## Hypotheses tried

1. **Hypothesis:** deleted anchors are rare enough that the conflation never misleads.
   **Test:** of 16 stale memories on 2026-09-08, two carried deleted anchors — 12.5%.
   **Verdict:** rejected as a reason to leave it, though it does justify `severity: low`.

## Cluster — why `unclassified` rather than a forced fit

`IC-20` (`floor-published-under-the-name-of-a-total`) is the near miss and does **not**
fit, in two ways that matter. Its claim turns on the true value being **unknowable
because the walk stopped**; here it is fully knowable from the same response, which
carries `changed_files` and `deleted_files` as separate arrays. And the direction is
inverted: `IC-20` is a *subset* published under the whole population's name, this is the
*whole* published under a subset's name. The remedies rhyme (both are renames) but the
diseases differ. Filed under the escape hatch rather than moved toward `IC-20`'s
threshold on a member that is really its mirror image.

## Fix

Fixed. `check_all_memories` (`src/memory/anchors.rs`) now selects the noun from the actual
decomposition rather than publishing the total under one class's name:

```rust
let reason = match (changed.len(), deleted.len()) {
    (_, 0) => format!("{total_stale} of {total_anchored} anchored files changed"),
    (0, _) => format!("{total_stale} of {total_anchored} anchored files deleted"),
    (c, d) => format!("{total_stale} of {total_anchored} anchored files stale ({c} changed, {d} deleted)"),
};
```

**Chose the decomposed form over this file's own "smallest correct change" suggestion**
(renaming "changed" to "stale" everywhere), and the over-match guard below is why: "stale"
is accurate but strictly less informative than today's string in the ~87% single-class
case, and the information it drops is already computed two lines above. This way the pure
cases keep today's exact wording **and length** — no bytes added where nothing was wrong —
and only the mixed case grows, which is the case that needs it.

**Mutation-verified.** The mixed-case test was observed RED against the old string,
failing with the defect stated literally: `a deleted anchor must be named as deleted, not
folded into a changed-count: 2 of 2 anchored files changed`. GREEN after.

**§ Resume's doc check, performed:** the old wording appears five more times in-tree —
`src/memory/anchors.rs:85` (a doc comment quoting a past observation), two
`docs/superpowers/plans/2026-03-06-memory-staleness-*` files, and two archived observation
records. All five are **historical records of what was observed then**, not live
prescriptions of the format, so all five are left as-is per CLAUDE.md § *Parsers Over a
Namespace* ("just make the text current and delete the past" applies to live citations;
these are measurements whose method matters). No test asserted on the string — the
pre-existing `check_all_memories_stale` asserts on `changed_files` only.

**SHA:** `f97a9a40794b3ee8e5c87ee138cbcc44f08b229c`
**patch-id:** `4dbd1eb7f63bdbd879c433a3077a6cf97069cd09`
## Tests added

`src/memory/anchors.rs`, next to the existing `check_all_memories_stale`:

- `check_all_memories_stale_reason_does_not_call_a_deleted_anchor_changed` — a memory
  anchoring two files, one mutated and one deleted. Asserts the reason names `deleted` and
  is not the old `"2 of 2 anchored files changed"`. The **mixed** fixture is load-bearing
  and annotated as such: a one-deleted fixture would also pass against a string that
  hardcodes "deleted", and a one-changed fixture passes against the *buggy* string, so only
  the mixed shape separates a correct summary from either hardcoding.
- `check_all_memories_stale_reason_says_changed_when_nothing_was_deleted` — over-match
  guard: an all-changed staleness must still say `changed` and must **not** volunteer
  `deleted`. This is what rules out the blanket "stale" rename, which would satisfy the
  first test while degrading the common case.

Asserts on the NAME of each failure state rather than on a count — the discriminator was
already in the output, unused, which is the shape CLAUDE.md § *Testing Discipline* asks
for. Observed RED/GREEN as described in § Fix; full `memory::anchors::` module green
(25 tests).
## Workarounds

Read `changed_files` and `deleted_files` from the response rather than the `reason`
string. The arrays are correct; only the summary is not.

## Resume

Done — see § Fix. Nothing left to resume.
## References

- `src/memory/anchors.rs:258-263` (the string), `:184-207` (`check_path_staleness`)
- `src/tools/config/mod.rs:591-607` (the status handler that surfaces it)
- Found while re-deriving all 16 stale memories, `09ecb58d`
