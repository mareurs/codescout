---
kind: bug
status: mitigated
tags:
- cluster/record-asserts-an-unchecked-completion
- worktree
- verification
- disk-space
- stale-record
closed: null
opened: 2026-08-30
owner: marius
related:
- docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md
severity: medium
unverified: 'Mitigated, not fixed — and the residual is now closed by an UNATTRIBUTED removal rather than by the decision this file parked on. Measured 2026-09-01: `.worktrees/bench` is absent, `.worktrees/` holds only `audit-trail-t1`, and `git worktree list` reports neither — so the orphaned gitdir and the 163M are gone. WHO removed it and WHEN is not establishable: no commit in the last 60 mentions the bench worktree, and the `.worktrees/` mtime (02:24) is equally explained by `audit-trail-t1` being created in it, since a directory mtime records its last entry change and not which entry. No regression guard exists for the class: nothing prevents a record asserting an unchecked completion again, and `docs/trackers/retrieval-benchmark.md:76` agrees with reality today by accident rather than by repair. AMENDED 2026-09-02 — the 2026-09-01 reading closed a MEMBER, not the population, and the population regenerates on its own. `.worktrees/` today holds `audit-shards-t7`: 8K, absent from `.git/worktrees/` and from `git worktree list`, containing only a dead session''s gitignored `.buddy/bf44ba81-4cb3-4fdc-a92b-0780646ca7b9/` and `.codescout/cc_session_id`. `audit-trail-t1` is gone and a different unregistered member replaced it inside 24h, so `the orphaned dirs are gone` was true of the instance and false of the class — the same member-vs-population cut CLAUDE.md names under Testing Discipline. The invisibility is doubly-instrumented, which is why nobody trips over it: `git worktree list` reports registrations and cannot see it, `git status` honours `.gitignore:133 .worktrees/` and cannot see it either — two correct instruments whose blind spots coincide, so agreement between them is one blind spot counted twice. Unlike the 2026-08-30 bench case, positive identification IS available and unused here: the residue names its own session id on disk, so the author is given rather than inferred from a directory mtime.'
---

# BUG: an archived bug file records a worktree deletion, with reclaimed-MB figures, that never happened on this machine

## Summary

`docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md`
is `status: fixed`, `closed: 2026-08-16`, and its `## Fix` section states the worktree
"was removed with `git worktree remove --force .worktrees/bench`", closing with
*"174 MB reclaimed, 163 MB of it regenerable `.codescout` index state."*

`.worktrees/bench` is still on disk. It is 174 MB, of which 163 MB is `.codescout`.
The numbers in the closure match the live directory **exactly** — which is what makes
this diagnosable: they were measured, and then recorded as reclaimed without the
removal being confirmed.

Discovered incidentally on 2026-08-30 while auditing registered worktrees; the audit
had no reason to look here, because these directories are invisible to every git
command in this repo.

## Symptom (Effect)

`.worktrees/` holds **358 MB** across eight directories that `git worktree list`
does not report and `git worktree prune` does not clean. Three are orphaned
worktrees of the pre-rename repository:

| Directory | gitdir pointer | size | dir mtime |
|---|---|---|---|
| `.worktrees/bench` | `…/code-explorer/.git/worktrees/bench` | 174M | 2026-05-12 13:17 |
| `.worktrees/bench-legacy` | `…/code-explorer/.git/worktrees/bench-legacy` | 173M | 2026-05-12 12:57 |
| `.worktrees/no-local-embedding` | `…/code-explorer/.git/worktrees/codescout-no-local-embedding` | 12M | 2026-05-12 04:35 |

The remaining five (`feat`, `github-slim`, `output-buffer-threshold`,
`rich-tool-output`, `workspace-onboarding`) are empty shells holding only a
`.code-explorer` or `.codescout` marker file.

## Reproduction

```
ls -d /home/marius/work/claude/code-explorer   # No such file or directory
ls .git/worktrees                              # No such file or directory
git worktree list                              # main checkout only
du -sh .worktrees                              # 358M
cat .worktrees/bench/.git                      # gitdir: …/code-explorer/.git/worktrees/bench
```

## Environment

Linux; codescout `experiments`; observed 2026-08-30. The archived file records its own
environment as `experiments` @ `b641a38a`, 2026-08-16.

## Root cause

Not yet confirmed. The **evidence rules out** deletion-then-recreation, which is the
competing explanation:

- `.worktrees/bench` has dir mtime **2026-05-12 13:17** — untouched since May, three
  months before the 2026-08-16 closure. A delete-and-recreate would stamp a later mtime.
- Its `.git` still names the **pre-rename** `code-explorer` path. The archived file's own
  documented recreation command
  (`git worktree add --detach .worktrees/bench ede25e69…`) would have written a gitdir
  naming `codescout`. So this is the original directory, not a replacement.

**Leading hypothesis, unverified:** the removal command was issued and failed. This repo
has no `.git/worktrees` directory at all, and the referenced repository does not exist, so
`git worktree remove --force .worktrees/bench` has no registration to resolve. The
archived file itself notes the admin dir was *"reconstructed by hand only to inspect the
contents"* — if that scaffolding was removed, or was never sufficient, the subsequent
removal would error. The closure appears to have been written from the *intent* plus the
already-measured `du` output, without a confirming `ls`.

Do not treat the hypothesis as established: it is inferred from the surviving artifacts,
not from a captured error message.

## Evidence

### The reclaimed figures match the live directory

`du -sh .worktrees/bench` → `174M`; `du -sh .worktrees/bench/.codescout` → `163M`.
The archived file claims "174 MB reclaimed, 163 MB of it regenerable `.codescout` index
state". A coincidence at this precision is not plausible: the author measured this
directory, then recorded the measurement as a past-tense reclaim.

### The audit that found it could not have been looking for it

`git worktree list`, `git worktree prune --dry-run`, and `git status --porcelain` are all
silent about these directories — the first two because no registration exists, the third
because `.worktrees/` is not tracked. Nothing routine surfaces them.

## Hypotheses tried

1. **Hypothesis:** the worktree was deleted as recorded and later recreated by a
   benchmark run. **Test:** compared dir mtime and gitdir pointer against what the
   documented recreation command would produce. **Verdict:** rejected — mtime predates
   the closure by three months, and the gitdir names the pre-rename path that only the
   original could carry.

## Fix

**Partially resolved 2026-08-30**, with the user choosing the scope after being shown
that `bench` is load-bearing.

Recorded on `experiments` at `4b6ce839` (patch-id
`4da36a7932d8ccb56bd41205393bf7026a192798`). The pair is written now rather than owed
later: `experiments` is rebased after every ship, which orphans the SHA, while the
patch-id is a content hash of the diff and survives both rebase and cherry-pick. That
this file exists at all is the argument for the rule — its predecessor recorded a
completed action with no durable pointer and no confirming check, and nothing could
resolve the claim 14 days on.

**Disk state — done, 184 MB reclaimed** (`du` before and after: 358M → 174M).
Deleted with `rm -rf`, not `git worktree remove`, because no git registration exists for
any of them:

- `.worktrees/bench-legacy` (173M) — cited only at `docs/trackers/retrieval-benchmark.md:827`
  as a historical comparison arm (pinned binary at `0795b208`) whose results are already
  recorded in that tracker. No script resolves against it.
- `.worktrees/no-local-embedding` (12M) — cited nowhere in the repo.
- The five empty shells (`feat`, `github-slim`, `output-buffer-threshold`,
  `rich-tool-output`, `workspace-onboarding`), each holding only a `.code-explorer` or
  `.codescout` marker file.

**`.worktrees/bench` (174M) was deliberately KEPT.** The 2026-08-16 closure judged the
corpus to have "nothing worth keeping", but three live surfaces resolve against it:

- `scripts/run-tc-benchmark.sh:18` — `PROJECT_PATH="${CODESCOUT_PROJECT_PATH:-${REPO_ROOT}/.worktrees/bench}"`
- `scripts/sweep-bm25-boost.sh:10` and `docs/PROBES.md` (the `sweep-bm25-boost.sh` row;
  cited without a line number because this one moved from :116 to :141 within a day) — the expected-file lists are
  relative to this pinned corpus, not to HEAD
- `docs/trackers/retrieval-benchmark.md` — 8 references, including the corpus definition

**The record — done.** The archived file's `## Fix` now opens with a dated correction
block, and its frontmatter carries `unverified:` naming the contradiction, so the
canonical bug triage query can reach it.

## Fix provenance

- **SHA:** `4b6ce839` (`experiments`)
- **patch-id:** `4da36a7932d8ccb56bd41205393bf7026a192798`

The disk cleanup (184 MB) and the record correction. Deliberately the only anchor here:
the residual closed later by an **unattributed removal**, not by a commit, so there is no
second pointer to declare — see § *Residual*.
## Residual — RESOLVED 2026-09-01 by an unattributed removal

`.worktrees/bench` no longer exists. Measured 2026-09-01: `.worktrees/` contains only
`audit-trail-t1`, `.worktrees/bench/.git` is absent, and `git worktree list` reports the
main checkout plus `audit-trail-t1` and nothing else. The orphaned gitdir this section was
filed about is gone — and with it the decision it was parked on (re-register against this
repo at a 163M re-index, or accept a plain untracked directory). Neither was chosen. The
directory was removed.

**By whom and when is not establishable — this file's own cluster firing on its own
residual.** No commit in the last 60 mentions the bench worktree, and `.worktrees/` has an
mtime of 2026-09-01 02:24 that is equally explained by `audit-trail-t1` being created in
it: a directory mtime records its last entry change, not which entry. So this note records
an observed end state, not a completion anyone can be credited with — which is the same
shape as the defect this file was opened about, one turn later.

One consequence runs the other way and is worth naming. `docs/trackers/retrieval-benchmark.md:76`
tells the reader to run `git worktree list | grep .worktrees/bench` and says it will be
missing on a fresh host. That check returned nothing while 163M of corpus sat on disk,
which is what produced this bug. Today it returns nothing and the directory really is
absent, so instruction and reality agree again — **by accident, not by repair**. Nothing
stops the same divergence recurring the next time a worktree is created there.
## Workarounds

Treat `.worktrees/` as filesystem state that git cannot see. Audit with `du`/`ls` and by
reading each `.git` pointer, never with `git worktree list` alone.

## Resume

Nothing outstanding on this incident. The disk cleanup and the record correction are done
(see `## Fix`); the residual decision was overtaken by the directory's removal (see
`## Residual`).

What is NOT closed is the class. Nothing prevents a record from asserting an unchecked
completion again, and the removal that closed the residual is itself unattributed — so the
file now contains one instance of its own defect class in each of its two halves. Re-open
if a `.worktrees/` entry is again reported deleted without a check that the path is gone.
## References

- `docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md` — the closure this contradicts
- `docs/trackers/worktree-cleanup-session-log.md` — `F-1`, the audit that surfaced it
