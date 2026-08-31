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

## Residual — still open

`.worktrees/bench` retains the orphaned gitdir this bug's predecessor was filed about:
`.git` reads `gitdir: /home/marius/work/claude/code-explorer/.git/worktrees/bench`, and
that repository does not exist. Consequences, unchanged:

- `git worktree list` will never report it; `git worktree prune` will never clean it
- any `git -C .worktrees/bench <cmd>` fails
- it is invisible to every git-based hygiene check, which is how it survived a closure
  that believed it deleted

`git worktree repair` cannot fix this — it requires the referenced repository to exist.
The options are to re-register it against this repo
(`git worktree add --detach .worktrees/bench ede25e694b63219e1382f359d7ba242f66a516a5`
after moving the existing directory aside, which costs a 163M re-index), or to accept it
as a plain untracked directory and note that in `docs/trackers/retrieval-benchmark.md`
so the next reader does not run `git worktree list` and conclude the corpus is missing.
Not decided.
## Workarounds

Treat `.worktrees/` as filesystem state that git cannot see. Audit with `du`/`ls` and by
reading each `.git` pointer, never with `git worktree list` alone.

## Resume

The disk cleanup and the record correction are both done (see `## Fix`). What remains is
the single decision in `## Residual`: whether to re-register `.worktrees/bench` against
this repo — costing a 163M re-index — or to accept it as a plain untracked directory and
say so in `docs/trackers/retrieval-benchmark.md:76`, which currently tells the reader to
check for it with `git worktree list | grep .worktrees/bench`. That check returns nothing
today and always will, which is the trap that produced this bug.
## References

- `docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md` — the closure this contradicts
- `docs/trackers/worktree-cleanup-session-log.md` — `F-1`, the audit that surfaced it
