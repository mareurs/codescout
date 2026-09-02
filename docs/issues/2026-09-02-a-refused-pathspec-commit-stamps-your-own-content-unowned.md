---
id: '2266ffc0103534cd'
kind: bug
status: open
title: 'BUG: a refused pathspec commit stamps the author''s own content as unowned, and no-op restaging cannot reclaim it'
tags:
- cluster/shared-resource-carries-no-owner
opened: 2026-09-02
owner: marius
severity: medium
unverified: no fix and no regression test. Root cause and minimal trigger are reproduced live on the shared checkout; the fix direction is chosen but not implemented, and tests/hooks-discrimination.sh has no case for a REFUSED commit's index write, which is why this shipped.
---

## Summary

`git commit -- <paths>` writes the pathspec content into the **real** index before hooks run.
`commit` is not one of the staging verbs `staging_op()` recognises, so every pair it stages is
recorded `-` / `not-staging`. If a pre-commit hook then refuses, the author is left holding rows
that say their own work is unowned — and **they cannot reclaim them**, because re-`git add`ing
byte-identical content is not an index write, so `post-index-change` never fires.

The end state is the symptom of
`docs/issues/archive/2026-09-02-a-transiently-empty-index-destroys-stage-log-ownership.md` reached by a
different route. That bug was ownership **destroyed**; this is ownership **never established**.
Its fix (`5e522fa4`) does not reach here by construction: retention preserves rows that exist and
creates none.

## Symptom (Effect)

A bare `git commit` refuses the author's own path under `theirs:`, with
`Staged by: (unrecorded) — not a staging command`. The printed remedy
(`git commit -- <paths>`) is the very command that produced the state.

## Reproduction

Live, on the shared checkout, 2026-09-02. Not a fixture — the repo's own hooks:

```
$ grep -c 'issue-clusters' .git/session-stage-log        # no row for my file yet
1                                                        # (a peer's retained row)

$ git commit -m "probe" -- docs/trackers/issue-clusters.md
   ... refuse a pathspec commit carrying unstaged content ... Failed
$ grep 'issue-clusters' .git/session-stage-log
-	f861250b	docs/trackers/issue-clusters.md	not-staging      # <- MY blob, owner `-`

$ git add docs/trackers/issue-clusters.md                 # the author's retry
$ grep 'issue-clusters' .git/session-stage-log
-	f861250b	docs/trackers/issue-clusters.md	not-staging      # <- UNCHANGED

$ bash scripts/pre-commit-foreign-index.sh; echo $?
Refusing a bare commit: the index holds paths staged by another session.
  theirs:
      docs/trackers/issue-clusters.md
1
```

`f861250b` is the author's own content. The file is staged (`git diff --cached --name-only`
lists it) and reads as a peer's.

## Environment

`experiments`, shared checkout, hooks installed via `scripts/install-hooks.sh`. Observed after
`5e522fa4`, so retention was live and is not implicated.

## Root cause

Two mechanisms compose, each correct alone:

1. **`git commit -- <paths>` stages before hooks.** The hook fires with `PPID` = `git commit`.
   `staging_op()` walks `/proc/$PPID/cmdline` for a recognised staging verb; `commit` is not one,
   so `claimant="-"` and `claim_route="not-staging"`. This is the conservative direction and is
   deliberate — a wrong `-` over-refuses loudly where a wrong claim goes silent.
2. **A no-op `git add` is not an index write.** Restaging byte-identical content changes nothing,
   so git never runs `post-index-change` and the row cannot be corrected. Only a *content change*
   reclaims the pair — verified by appending a byte, after which the row came back `named` under
   the author's id.

Neither is wrong; together they leave a state with **no route out** that does not involve
modifying the work.

## Evidence

### The failure direction is safe, and that is again what makes it expensive

`-` over-refuses, which is correct. But at the point of use it is indistinguishable from a real
capture, and here the refusal names a file the author has just written, in a session that has
touched nothing else. The measured cost on 2026-09-02 was a second forced round through the
documented commit sequence.

### Not a `run_command` / MCP artefact — hypothesis falsified

First read as "`git` invoked through the MCP `run_command` tool breaks `PPID` detection". **False.**
A throwaway repo using the *real* shim form recorded `named` with the correct session id for a
`git add` issued through `run_command`. `PPID` detection works; the route is `git commit`.

Recorded because the falsification is the useful part: it is the hypothesis that fits the first
observation and sends the reader into `staging_op()`, which is not where the defect is.

### Why the sibling bug's fix does not cover it

Retention carries forward rows for pairs absent from the staged set. Here **no owned row was ever
written** — the first index write touching the pair was the non-staging one. Retention has nothing
to preserve. The two bugs share a symptom and a class and need different remedies.

## Hypotheses tried

1. **Hypothesis** — the MCP `run_command` wrapper breaks `/proc/$PPID/cmdline` detection.
   **Test** — throwaway repo, real shim, `git add` through `run_command`.
   **Verdict** — rejected. Records `SESS-PPID … named`.

2. **Hypothesis** — the `pre-commit` framework's stash/restore cycle stamps the rows.
   **Test** — throwaway repo with a hand-rolled pre-commit doing `git stash push --keep-index` /
   `pop` then `exit 1`.
   **Verdict** — rejected. The refused commit left no row for the path at all. The stamping is
   `git commit`'s own index write, not the framework's.

## Fix

Not implemented. Two directions, and the choice is not obvious:

- **Treat `commit` as a staging verb when it carries a pathspec.** `git commit -- <paths>` is a
  staging act in every sense that matters here: argv NAMES the paths, so the existing
  `names_path` machinery already has what it needs and the claim stays argv-scoped. The risk is
  that `commit` without a pathspec must NOT claim — it commits the whole shared index, and
  claiming there is the capture this guard exists to prevent. So the arm has to key on the
  pathspec, not on the verb.
- **Give the author a reclaim route.** Any explicit `git add` naming a path could be made to
  fire the recorder even when the index does not change — but git offers no hook for a write
  that does not happen, so this needs a wrapper rather than a hook, which is a different shape
  of solution.

The first is narrower and uses machinery that already exists. **Do not fix it by making the
guard trust `-` more** — over-refusing is the correct direction; the defect is in what the
recorder can observe.

## Tests added

None yet. `tests/hooks-discrimination.sh` has no case for a *refused* commit's index write, which
is why this shipped. The regression case is: attempt `git commit -- <path>` under a pre-commit
that exits 1, then assert `owner_of <path>` is the author rather than `-`.

## Workarounds

Stage **before** attempting the commit — `git add <paths>` then `git commit -- <paths>` — which
is already step 4 of `docs/conventions/shared-checkout-commit-sequence.md`. Following that
sequence avoids this entirely; the bug is only reachable by committing by pathspec first, which
is what the `unreviewed-content` refusal then teaches you not to do.

Once in the state, the only reclaim is a content change. `--no-verify` also works and is the
wrong habit.

## References

- `scripts/post-index-change-stage-log.sh` — `staging_op()` and the verb list.
- `scripts/pre-commit-foreign-index.sh` — reads the log; correctly over-refuses on `-`.
- `docs/issues/archive/2026-09-02-a-transiently-empty-index-destroys-stage-log-ownership.md` — the
  sibling: same symptom, ownership destroyed rather than never established. Fixed at `5e522fa4`.
- `docs/conventions/shared-checkout-commit-sequence.md` — step 4 is the workaround.
- `docs/trackers/issue-clusters.md` `IC-17` — the class; this is the residual hole its
  Mechanism status now names.

## Resume

Decide between the two fix directions above. The pathspec-keyed arm in `staging_op()` is the
recommended one: write the regression case first and confirm it REDs, per `CLAUDE.md`
§ *Testing Discipline*.
