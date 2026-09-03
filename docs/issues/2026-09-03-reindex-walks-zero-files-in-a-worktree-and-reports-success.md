---
status: open
opened: 2026-09-03
closed:
severity: medium
owner: marius
related: []
tags: ["cluster/selector-narrower-than-its-population"]
kind: bug
unverified: 'root cause not established — the mechanism that makes THIS worktree different from .worktrees/tool-collapse, which does have catalog rows, is not identified'
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

**Unknown — see Hypotheses tried.** What is established:

- It is not "worktrees cannot hold catalog rows." `librarian(action="doctor")` reports 13
  `worktree_scoped_row` violations, and **9 of them are bug files under the sibling worktree
  `/home/marius/work/claude/codescout/.worktrees/tool-collapse`**. That worktree's files were
  indexed. This one's are not.
- It is not a scope or target-resolution error. `targets` echoes the correct absolute path,
  and `scope="project"`, `scope="repo"` and `force=true` all behave identically.
- It is not a `declared_root_missing` condition — `doctor` reports 0 for that check.

The difference between the two worktrees is therefore the thing to find, and it is not yet
found — but it is **not registration**: creating an artifact here through `doc(action="create")`
gave this worktree a catalog row, and `reindex` still walked zero files (hypothesis 5). Per
`docs/conventions/` practice this is recorded as *inferred from tool output, not measured at the
source* — no `src/librarian/indexer.rs` line has been read for this bug.

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

*Not yet fixed, and the root cause is not established — do not write a fix before testing
hypothesis 5.*

The reporting defect is separately actionable and does not wait on the root cause: when a
reindex walks zero files under a root that exists and is non-empty, that is not a success. It
should either name the scope it examined and why it was empty, or refuse. Per
`docs/adrs/2026-08-27-negative-results-name-their-scope.md`, a suspicious zero names its scope;
`unchanged: 0` against a 1516-file root is the canonical suspicious zero.

SHA: *pending.* patch-id: *pending.*

## Tests added

None. A regression test needs the root cause first. If hypothesis 5 holds, the test is that
`reindex` on an unregistered linked worktree either indexes it or returns a `RecoverableError`
naming the registration requirement — never all-zero.

## Workarounds

Create bug files and trackers **in the main checkout**, not in a worktree, until this is
understood. If a file has already been created in a worktree, it will get its row when the
branch merges and the main checkout is reindexed; record the owed `cluster/` tag in the file's
own `## Resume` so the step is not lost, as the sibling bug does.

For querying: `doc(action="find", kind="bug")` from a worktree is a **lower bound**. Cross-check
with `librarian(action="doctor")` and read the `worktree_scoped_row` paths before concluding a
bug is unfiled.

## Resume

Read `src/librarian/indexer.rs` `index_repo_sync` and find where the candidate list is built —
specifically whether it walks the filesystem under `targets` or enumerates a catalog-derived
root set. Hypothesis 5 (registration-gated) is already refuted by probe, so the remaining lead
is hypothesis 6: the root set resolves through the main checkout, in which this worktree does
not appear. The tell to confirm at the source is that `targets` is echoed from the *request*
rather than from whatever collection the walk actually iterates.

## References

- Memory `worktree-merge-catalog-reconciliation` — the overlay / fork-on-first-write /
  `worktree_registration` design that hypothesis 5 rests on. Branch commits
  `4450f20f..c2104e90`.
- `docs/superpowers/specs/2026-07-17-worktree-overlay-design.md`
- `docs/adrs/2026-08-27-negative-results-name-their-scope.md` — the rule the all-zero response
  breaks.
- `docs/issues/2026-09-02-indexer-stamps-content-seen-before-it-embeds.md` — a different silent
  indexer failure surfaced in the same session's control run (`vectorless: 1332`); unrelated
  mechanism, same "plausible number rather than an error" shape.
- `docs/issues/2026-09-03-two-file-templates-propagate-retired-call-forms-into-new-files.md` —
  the bug whose filing hit this one.
