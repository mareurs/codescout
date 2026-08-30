---
kind: bug
status: open
opened: 2026-08-30
closed:
severity: medium
owner: marius
related: ["docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md"]
tags:
  - worktree
  - verification
  - disk-space
  - stale-record
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

Not yet decided. Two independent things need fixing:

1. **The disk state.** 358 MB of unreachable directories. Because no git registration
   exists, `git worktree remove` is not the tool — plain `rm -rf` is, which is precisely
   why it needs explicit human authorisation rather than a drive-by cleanup.
2. **The record.** The archived file asserts a completed deletion. Any future reader
   sizing disk usage or auditing worktrees will believe it. Its `## Fix` section needs a
   correction noting the removal did not take effect on this machine, and the file
   arguably needs `unverified:` set — the field exists for exactly this, a terminal
   status whose claim was not re-checked.

## Workarounds

Treat `.worktrees/` as filesystem state that git cannot see. Audit with `du`/`ls` and by
reading each `.git` pointer, never with `git worktree list` alone.

## Resume

Get authorisation for the 358 MB deletion, then correct the archived file's `## Fix`
section and set `unverified:` on it. Before deleting, confirm nothing still cites
`.worktrees/bench` as a benchmark corpus — `docs/trackers/retrieval-benchmark.md` is
named by the archived file as documenting its divergence, and the recreation command
`git worktree add --detach .worktrees/bench ede25e694b63219e1382f359d7ba242f66a516a5`
is the stated way to rebuild it if it is still wanted.

## References

- `docs/issues/archive/2026-08-16-bench-worktree-gitdir-points-at-pre-rename-path.md` — the closure this contradicts
- `docs/trackers/worktree-cleanup-session-log.md` — `F-1`, the audit that surfaced it
