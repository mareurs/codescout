---
kind: bug
status: fixed
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-10-01
opened: 2026-09-03
owner: marius
related: []
severity: medium
---

# BUG: `librarian(action="reindex")` walks zero files in a worktree and reports success, so a file created there is permanently unfindable

## Summary

`librarian(action="reindex")` run against the linked worktree
`/home/marius/work/claude/codescout/.worktrees/bug-claim-liveness` returns
`added: 0, updated: 0, removed: 0, unchanged: 0` — every counter zero, no error, no warning.
The same call against the main checkout returns `unchanged: 1434`. A zero in `unchanged` over
a root holding 1516 tracked markdown files does not mean "nothing changed"; it means
**nothing was walked**. A bug file created in the worktree therefore never gets a catalog row,
and `doc(action="find")` reports it does not exist — which is indistinguishable from never
having written it.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `fcd451de`) — still live, the REPORTING half; the
zero-file walk itself is deliberate.** Code-read only (a reindex writes the shared catalog, so none was run):
`src/librarian/indexer.rs:291-297` returns early with an empty report when `is_linked_worktree(abs_root)` — the
only signal a `tracing::warn!` no MCP caller sees (read by the coordinator) — added by `9d84f347` (2026-06-14) and
pinned by `index_repo_sync_skips_linked_worktree`. `reindex.rs:226-257` targets the worktree path; `IndexReport`
carries no skip field; the response (`reindex.rs:640-715`) has no worktree key and prints
`"unknown_sample_note": "complete"` regardless. So the defect is a guarded skip reported as success.
**The `unverified:` caveat is NARROWED, not resolved:** it asks why THIS worktree differs from
`.worktrees/tool-collapse`, which has rows. The skip explains zero files walked; it does not explain the
comparison, since rows can arrive by other routes (worktree-overlay shadow rows, rows predating the 06-14 guard).

## Symptom (Effect)

In the worktree (`scope="project"`, then `force=true`, then `scope="repo"` — all three
identical):

```json
{"added": 0, "updated": 0, "removed": 0, "unchanged": 0, "embedded": 0,
 "orphans_removed": 0, "unknown_count": 0, "backfill_error_count": 0,
 "embed_error_count": 0, "embed_note": "0 embedded",
 "unknown_sample_note": "complete",
 "scope": "project",
 "targets": ["/home/marius/work/claude/codescout/.worktrees/bug-claim-liveness"]}
```

`targets` names the right directory. `unknown_sample_note: "complete"` actively asserts the
scan was exhaustive. Nothing in the response distinguishes this from a healthy no-op.

The control, same tool, same session, main checkout:

```json
{"added": 0, "updated": 1, "removed": 0, "unchanged": 1434, "vectorless": 1332,
 "embed_error_count": 16,
 "targets": ["/home/marius/work/claude/codescout"]}
```

And the downstream consequence:

```
doc(action="find", filter={"rel_path": {"contains": "a-gate-against-hand-enumerated"}},
    scope="repo")
→ {"count": 0, "items": []}
```

for a file that exists on disk at
`docs/issues/2026-09-03-a-gate-against-hand-enumerated-sweeps-is-itself-hand-enumerated.md`.

## Reproduction

1. `workspace(action="activate", path="/home/marius/work/claude/codescout/.worktrees/bug-claim-liveness", read_only=false)`
2. `create_file(path="docs/issues/<anything>.md", content=<with `kind: bug` frontmatter>)`
3. `librarian(action="reindex", scope="project")` → all counters zero, `status` implicitly ok
4. `doc(action="find", filter={"rel_path": {"contains": "<anything>"}}, scope="repo")` → `count: 0`

Control: repeat step 3 with `workspace="/home/marius/work/claude/codescout"` → `unchanged: 1434`.

## Environment

Linux. Branch `bug-claim-liveness` at `d864c46f`, rebased onto `experiments`
`26b1f5c613b849787729376126acec91ffa60c54`. Worktree created via
`superpowers:using-git-worktrees` under `.worktrees/`. MCP transport, codescout server shared
across sessions in this checkout.

## Root cause

**Established 2026-10-01, at the source.** `index_repo_sync` skips a linked git worktree on purpose
(`indexer.rs`, `is_linked_worktree(abs_root)`, added in `9d84f347` and pinned by
`index_repo_sync_skips_linked_worktree`): a worktree's files are indexed through the main checkout,
not as separate artifacts. The skip returned a default, all-zero `IndexReport` and disclosed itself
only through `tracing::warn!`. The tool's response had no field that could express a skip, so it
answered `added: 0, updated: 0, removed: 0, unchanged: 0` with `unknown_sample_note: "complete"`:
the same bytes as an empty root. The earlier "root cause unknown" and hypotheses 5 and 6 were
about a mechanism that does not exist; the difference between the two worktrees in the original
observation was not registration but which one had been written to through `doc`.

## Evidence

### The all-zero response is the whole defect surface

`unchanged: 0` is the discriminator that already exists in the output and that a caller could
act on: a legitimate no-op on a populated root has a large `unchanged`, never zero. Nothing
consumes it. This is `CLAUDE.md` § *Testing Discipline* — *"where a system already names its
own failure state, assert on the name, not on a proxy for it"* — read from the reporting side:
the response carries the discriminator and does not use it.

**Cluster choice, so it can be re-adjudicated rather than inherited.** Tagged
`cluster/selector-narrower-than-its-population` (`IC-18`), whose claim ends *"a zero reads as
'not present' rather than 'not looked at'"* — which is `unchanged: 0` exactly. The selector is
whatever builds `reindex`'s candidate list; it is narrower than the population `targets`
names, it runs to completion over that empty subset, and it returns a well-formed answer.
`IC-13` (`capped-result-presented-as-complete`) was considered and **rejected**: its claim
requires truncation by a limit, and there is no cap here. That is not a judgement call —
`docs/trackers/issue-clusters.md:529-531` records a prior bug rejected from `IC-13` on exactly
this ground (*"fails because nothing is truncated"*), and `IC-13`'s own membership ruling moved
out two members because they *"involve no truncation at all"*.

### It defeats the documented bug-filing workflow

`CLAUDE.md` § *Bug Tracking* requires the `cluster/` tag to be written **through the catalog**
(`doc(action="update", …, patch={tags:[…]})`) because a direct frontmatter edit does not reach
it (BL-48). In a worktree there is no row to update, so the documented path is unavailable and
the fallback the instruction explicitly forbids is the only one left.

### It also hides bugs from `find`

The two class-precedent bug files cited in the sibling bug filed today
(`docs/issues/2026-09-03-two-file-templates-propagate-retired-call-forms-into-new-files.md`)
were **not** returned by `doc(action="find", kind="bug")` from this worktree, semantic or
filtered. They were found only by reading `doctor`'s `worktree_scoped_row` list. So the
"check the ledger before filing" step that `CLAUDE.md` and `_TEMPLATE.md` both mandate returns
a quietly partial answer from a worktree — the failure mode is a short list, not an error.

The same gap nearly caused a duplicate filing: `bd0979bf7e454567`, an open severity-high bug in
the main checkout covering the same class, did not surface in a semantic
`doc(action="find", kind="bug")` from here either. It appeared only once an explicit
`rel_path` filter was run at `scope="repo"`.

### Re-observed 2026-09-24, in a second worktree — and `find`'s own warning is silenced there

In `.worktrees/fix-lessons-friction` (branched from `experiments` @ `506924f2`), a markdown file
created with `create_file` was invisible to `doc(action="find", filter={rel_path: {contains: …}})`
→ `count: 0`. Two facets this file did not yet record, both from the same session:

- **The `unindexed_files` hint is absent in the worktree.** The main checkout's `find` carries
  `unindexed_files: 1` + `unindexed_hint` (the fix from
  `docs/issues/archive/2026-08-17-artifact-find-is-silent-about-files-the-catalog-has-never-seen.md`);
  the worktree's response for the same kind of query carries no such field. So the one surface
  built to say "some files here cannot match" is silent exactly where files go missing — the zero
  reads as trustworthy.
- **`scope.abs_path` names the worktree while every row names the main checkout** —
  `find(kind="tracker")` from the worktree returned `scope.abs_path =
  …/.worktrees/fix-lessons-friction` beside items at `…/Codescout/docs/…`. Supports hypothesis 6.

Cost that day: six bug files were written in that worktree and none can be tagged through the
catalog until the branch merges — each carries that step in its own `## Resume`, per the
workaround above.
## Hypotheses tried

1. **Hypothesis:** the file was written somewhere unexpected.
   **Test:** `git status --short` in the worktree.
   **Verdict:** rejected. The file is listed untracked at the expected path.

2. **Hypothesis:** `force=true` would bypass a content-hash cache.
   **Test:** `librarian(action="reindex", force=true, scope="project")`.
   **Verdict:** rejected — byte-identical all-zero response. A hash cache would still report
   `unchanged: 1516`; zero means the walk never produced candidates.

3. **Hypothesis:** `scope="project"` is too narrow in a worktree.
   **Test:** `scope="repo"`.
   **Verdict:** rejected — identical response, identical `targets`.

4. **Hypothesis:** worktrees are simply not indexable.
   **Test:** `librarian(action="doctor")`, read the `worktree_scoped_row` paths.
   **Verdict:** rejected. Nine bug files under `.worktrees/tool-collapse` hold catalog rows.
   This is the hypothesis whose refutation makes the bug interesting rather than expected —
   recorded as a denominator, not discarded.

5. **Hypothesis:** rows exist only for worktrees *registered* by a librarian write (the
   `worktree_registration` / fork-on-first-write overlay described in memory
   `worktree-merge-catalog-reconciliation`), and `reindex` walks registered roots rather than
   the filesystem. `.worktrees/tool-collapse` had librarian writes; this worktree had none.
   **Test:** `doc(action="create", kind="tracker", rel_path="docs/trackers/zz-probe-worktree-registration.md")`
   from this worktree, then re-run `librarian(action="reindex", scope="project")`.
   **Verdict:** **rejected.** The create succeeded and returned id `151d9e5bcd3f19f9`; a
   subsequent `doc(action="find")` returned the row, so the worktree now demonstrably holds a
   catalog row. `reindex` nonetheless returned the identical all-zero response. Registration is
   therefore not the gate. (Probe artifact deleted afterwards via `doc(action="delete")`.)

6. **Hypothesis (untested, current best lead):** `reindex`'s candidate list is built from a
   root set that resolves through the *main* checkout — note that `doc(action="find")` called
   from this worktree returns rows whose `abs_path` is under `/home/marius/work/claude/codescout`,
   i.e. the overlay reads main rows — so the walk enumerates a path set in which this worktree
   simply does not appear, while `targets` echoes the requested path unchanged.
   **Not tested** — needs `src/librarian/indexer.rs` `index_repo_sync` read at the source.

## Fix
**Fixed 2026-10-01.** The skip stays; the report now says so. `IndexReport` gained
`skipped: Option<String>`, set where the skip happens with the reason and the main checkout it
points at, and `reindex` emits `skipped_roots`: one `{root, reason}` per skipped target, always
present and empty when nothing was skipped, so "none skipped" is distinguishable from a build that
predates the field.

**Only production consumer.** `reindex` is the only production caller that consumes an
`IndexReport`. The other callers of `index_repo_sync` and `index_repo` are tests, including
`reindex_cli`, which is `#[cfg(test)]`. **Correction:** the message of the fix commit says the
`codescout index` CLI summary still drops the reason. That is wrong. The function it was read from is
test-only, not a CLI; there is no second site. Left in place rather than amended, because amending
rewrites a commit on a shared checkout.

**Not covered, and not claimed.** The other two facets recorded under *Evidence* are separate from
the all-zero report and this fix does not touch them:
- `find`'s `unindexed_files` hint absent in a worktree: **not reproduced**. The hint appears only when
  an unindexed file exists, and a 2026-10-01 comparison of `find` from a worktree and from the main
  checkout had none in either, so it said nothing about the claim.
- `scope.abs_path` naming the worktree while every row names the main checkout: **confirmed again**
  2026-10-01, and not adjudicated: it may be the overlay design (fork-on-first-write) and not a
  defect.
### Root cause found 2026-09-24 — a deliberate skip that reports itself only to the log

Open-bug sweep (`deep-agent-workflow-observations:DWF-7`), verifier evidence at HEAD `436a8ff6`. **This file's "root cause unknown" and hypothesis 6 are out of date.** The zero-file walk is intentional:

```
indexer.rs:286-297
if is_linked_worktree(abs_root) { tracing::warn!(...); return Ok((report, Vec::new())); }
```

It is pinned by the test `index_repo_sync_skips_linked_worktree` and was added in `9d84f347` (2026-06-14). The worktree's catalog rows exist because a doc write there forks its row on first write, not because reindex ran.

**What remains is the reporting half, and it is this file's defect exactly.** The skip is disclosed only through `tracing::warn`, which no caller sees. The tool response is an all-zero report carrying `unknown_sample_note: "complete"`, with nothing in the response path naming the skip. Remedy direction: put the skip in the report itself (a field naming the root that was skipped and why), per `docs/adrs/2026-08-27-negative-results-name-their-scope.md`. No live reindex was run for this check, since it would write to the catalog.

*Superseded 2026-10-01: fixed, and the root cause is established. See the text at the top of this
section; the paragraphs below are the history that led there.*

The reporting defect is separately actionable and does not wait on the root cause: when a
reindex walks zero files under a root that exists and is non-empty, that is not a success. It
should either name the scope it examined and why it was empty, or refuse. Per
`docs/adrs/2026-08-27-negative-results-name-their-scope.md`, a suspicious zero names its scope;
`unchanged: 0` against a 1516-file root is the canonical suspicious zero.

SHA and patch-id: see *Fix provenance*.

## Tests added

In `src/librarian/indexer.rs` and `src/librarian/tools/reindex.rs`:
`index_repo_sync_says_why_it_skipped_a_linked_worktree` (the report carries the reason and names the
main checkout; a walked root is the control), `a_reindex_of_a_linked_worktree_names_the_root_it_skipped`
(the tool's response names the root) and `a_reindex_that_walks_every_root_reports_no_skipped_roots`
(the control: the field is present and empty). Red before the change on each assertion, green after.
Mutated one per site on the final bytes, each killed by the intended test: the indexer not recording
the reason kills the indexer test and the tool test; the tool not collecting it kills the tool test
alone; the response emitting an empty list kills the tool test alone; attaching a reason to every
walked root kills only the control. Librarian tests run only in the default lane, so the names were
read out of that lane, once each.

## Fix provenance

- **SHA:** `3259a5f51116c0f5590cef46fba04ed1d6b61e95` (`experiments`)
- **patch-id:** `f989f9d902499b92e54f10a5708e75ec77014b2a` (`git show <sha> | git patch-id --stable`)

Verified on `experiments` 2026-10-01: gate `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, with the three tests
read by name out of the default lane, and four mutations observed killed on the final bytes.

## Workarounds

Create bug files and trackers **in the main checkout**, not in a worktree, until this is
understood. If a file has already been created in a worktree, it will get its row when the
branch merges and the main checkout is reindexed; record the owed `cluster/` tag in the file's
own `## Resume` so the step is not lost, as the sibling bug does.

For querying: `doc(action="find", kind="bug")` from a worktree is a **lower bound**. Cross-check
with `librarian(action="doctor")` and read the `worktree_scoped_row` paths before concluding a
bug is unfiled. **Confirmed cost, 2026-09-03:** this very session nearly filed a duplicate of an
open severity-high bug because the worktree `find` did not return it; it surfaced only from
`doctor`. Filing from the main checkout has no such gap — after this branch merged, a reindex
there picked up both new files (`added: 2`) and their `cluster/` tags immediately.

## Resume

Closed. The reporting defect is fixed; the two facets under *Fix*, *Not covered*, are open questions
and have no bug file of their own, deliberately, because one is unreproduced and the other is
unadjudicated.

## References

- Memory `worktree-merge-catalog-reconciliation` — the overlay / fork-on-first-write /
  `worktree_registration` design that hypothesis 5 rests on. Branch commits
  `4450f20f..c2104e90`.
- `docs/superpowers/specs/2026-07-17-worktree-overlay-design.md`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the rule the all-zero response
  breaks.
- `docs/issues/archive/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md` — a different silent
  indexer failure surfaced in the same session's control run (`vectorless: 1332`); unrelated
  mechanism, same "plausible number rather than an error" shape.
- `docs/issues/2026-09-03-two-file-templates-propagate-retired-call-forms-into-new-files.md` —
  the bug whose filing hit this one.
