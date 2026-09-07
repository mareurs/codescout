---
id: 5ff6fdfd63d50f24
kind: bug
status: fixed
title: 'BUG: no doctor check measures frontmatter/catalog status divergence, so the corpus population is unknowable'
tags:
- cluster/unclassified
- librarian
- doctor
- catalog-drift
closed: 2026-09-07
opened: 2026-09-07
owner: marius
severity: medium
---

## Summary

`doctor` carries `frontmatter_id_mismatch` and `frontmatter_id_is_not_a_catalog_id` — **id
only**. There is no equivalent for `status`, so a file whose frontmatter `status` disagrees
with its catalog row is not merely unfixed, it is **unmeasured**: no query, check or gate
can name one, and the corpus-wide count has never been derivable.

This is the residual of
`docs/issues/archive/2026-09-06-supersedes-link-moves-catalog-status-without-writing-the-file.md`,
split out rather than absorbed because an instrument that has never existed is different
work from a write path that was missing one call.

## Symptom (Effect)

Nothing observable — which is the defect, and it is the second-order form of the bug it
came from. That one made a *file* say `open` while every `find` said `superseded`. This one
means nobody can ask **how many** files are in that state. Both readers are confident, no
error is raised anywhere, and `git status` is clean because a catalog-only divergence has
nothing to commit.

## Reproduction

```
librarian(action="doctor")   → violations include frontmatter_id_mismatch,
                               frontmatter_id_is_not_a_catalog_id
                             → no check named frontmatter_status_mismatch
```

Then, for the population itself: there is no command to run. That absence *is* the
reproduction.

## Root cause

`doctor`'s file-vs-catalog checks were written for the **id** field, which is the one that
breaks under `doc(action="move")` (`id = sha256(abs_path)`). `status` is the other field
both halves store, it drifts by a different mechanism, and no check was extended to it.

## Why this is worth an instrument rather than a sweep

Two known write paths produced this divergence in **opposite directions**, and each was
fixed without either fix reaching the other:

- catalog → file: `doc(action="link", rel="supersedes")` — fixed `05da2db7`
- file → catalog: `edit_markdown` frontmatter write —
  `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`

A third, on the *id* field, is
`docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`.

So the class has at least three members across two directions and two fields, every one
found by a human noticing an oddity rather than by any check. A one-off sweep would answer
today's count and rebuild no capability; the next write path that forgets to round-trip
lands in exactly the same silence.

## Fix

Add `frontmatter_status_mismatch` to `librarian(action="doctor")`, beside the two id
checks: for each catalogued artifact whose file exists, compare frontmatter `status`
against the row's, and report the pair.

Then **derive the corpus count and publish the derivation, not the value** — every
`supersedes` link recorded before `05da2db7` is a candidate, and the number will decay.

Two design notes, both learned from the id checks rather than guessed:

- **Report, do not repair.** The id checks are read-only by default with an opt-in `fix=`.
  Which side is authoritative is not always the file: a row can be right and a file
  hand-edited. `doctor` should say so and let the reader choose.
- **Missing-file is a distinct outcome, not a mismatch.** `frontmatter_id_mismatch`
  already has a documented false-positive mode for ids minted in another checkout; a
  status check needs its own answer for an artifact whose file is absent, rather than
  reporting it as disagreement.

Fix SHA: `765a1402` (`experiments`)
Patch-id: `19cf2a8cfd047c9aee36399d156cf7c3f62de31e`

**Done 2026-09-07.** `frontmatter_status_mismatch` is wired into the default scan beside
the two id checks and registered in `SCOPED_ROW_CHECKS`, so out-of-project findings
aggregate the same way theirs do. Both design notes above were kept: it **reports and
never repairs**, and a missing file stays [`check_missing_file`]'s finding. A NULL row
status is skipped too, which the file did not anticipate — that would be a finding about
the schema rather than about drift.

### The corpus count, with the scope that makes it a number

**0 findings**, project scope, **4727** catalogued artifacts, run 2026-09-07 against the
tree at `05da2db7`. Derivation rather than value, per this repo's rule: run
`./target/debug/codescout doctor` and read `summary.by_check.frontmatter_status_mismatch`.

**That zero is only worth its controls, and the one first published was the weaker of the
two. Corrected 2026-09-07, same day, before anyone cited it.**

The claim as written was: *"`missing_file` read 4 in the same report, so the scan
demonstrably opens files and reports on them."* Both halves are shakier than they read:

- **`missing_file` proves a `stat`, not a read.** It fires when a row's `abs_path` is
  **absent** from disk. This check needs the file *opened and its frontmatter parsed* — a
  strictly longer path. A scan that enumerated rows and stat'd every one while reading none
  would show `missing_file: 4` and `frontmatter_status_mismatch: 0`, which is precisely the
  state the control was supposed to exclude.
- **All four rows are in another repo** (`claude-plugins/docs/trackers/*-session-log.md`),
  so they are outside the project scope the `0` describes. The control and the measurement
  were not taken over the same population.

**What actually carries the weight is `frontmatter_status_mismatch_is_reached_by_the_default_scan`** —
it plants a real divergence, runs the real entry point, and asserts the finding appears,
exercising enumerate → open → parse → compare → report end to end. That test was written
before this correction and is why the conclusion survives it unchanged.

`frontmatter_id_mismatch` also read 0 on that run, so the obvious sibling could not serve
as a control either: two zeroes agreeing is one blind spot counted twice.

**Kept rather than deleted because the method changes how the number reads.** The finding
stands; the reason to believe it moved from a same-report figure to a planted positive. A
reader who cites `missing_file` as this check's control is citing the wrong evidence, and
that is exactly the class of history worth recording. (Raised by sessionId
`89d91024-cd66-4361-9300-c55b87b179ea`, who had just retracted a `missing_file`
hypothesis of their own after a table renderer stripped `archive/` out of the paths they
were reading — the discriminating component destroyed by their own formatter before they
looked at it.)

**Do not cite this as "the corpus is clean."** It is a fact about an instant, on one
project scope, and the instrument is new enough that its first non-zero is still ahead of
it. The prior expectation was that the two fixed write paths had left debris; they had
not, in scope — which is a real finding and not a reason to trust the check less.

## Tests added

Three, and the third is the one that matters.

1. `check_frontmatter_status_flags_only_a_status_that_is_present_and_differs` — the
   predicate, plus all three abstentions asserted rather than trusted.
2. `frontmatter_status_mismatch_detail_names_both_sides` — the message. Separate on
   purpose: because this check never repairs, its text **is** the deliverable, and a
   predicate-only suite leaves that half untested by construction.
3. `frontmatter_status_mismatch_is_reached_by_the_default_scan` — **the check is
   reached**, with an agreeing row beside the diverged one so a scan reporting everything
   answers 2 rather than 1.

**Mutation evidence, on the production path rather than the test's inputs:** dropping
`{row_status}` from the detail reds test 2 **only** and leaves test 1 green — so the two
are independently discriminating, one kill per guarded site. The mutated text still read
perfectly plausibly (*"but the catalog row disagrees"*), which is exactly what a
predicate-only suite would have shipped.

**Why test 3 exists is the reusable part.** The first live run returned 0 across the whole
catalog, and a `0` from a check wired into nothing is byte-identical to a clean corpus.
Nothing in the report distinguishes them, and the sibling that would normally serve as a
control read 0 as well. An alarm nothing reaches is exactly as informative as no alarm.
## Workarounds

None for measurement. For a single artifact, `doc(action="update", id=…,
patch={status:…})` round-trips to the file and is what repairs one by hand — but you have
to already suspect it, which is the whole problem.

## Resume

**Closed 2026-09-07** at `765a1402`. The instrument exists, is reached by the default
scan, and its first reading is 0 in project scope against a `missing_file` control of 4.

No residual. The two design notes this file asked for were both honoured, and the count it
refused to guess has been derived with its scope and its controls attached rather than
published as a bare figure.

One thing deliberately **not** done: no sweep of other project scopes. `row_checks_scoped_by_project`
aggregates them and this check now rides that mechanism, so the numbers are available to
whoever activates those projects — but a count taken from here would be a count of repos
this session was not working in.
## References

- Parent, fixed and archived:
  `docs/issues/archive/2026-09-06-supersedes-link-moves-catalog-status-without-writing-the-file.md`
- Mirror direction, archived:
  `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`
- Same pair, id rather than status:
  `docs/issues/2026-09-05-frontmatter-id-mismatch-asserts-a-move-for-worktree-minted-ids.md`
- Tagged `cluster/unclassified` deliberately. The parent named a candidate class — *"a
  write path updates one half of the file/catalog pair and reports success"* — and this
  file is **not** a fourth instance of it: it is the absence of the instrument that would
  have counted the first three. If that class is promoted, this belongs beside it as its
  measurement gap, not as a member.
