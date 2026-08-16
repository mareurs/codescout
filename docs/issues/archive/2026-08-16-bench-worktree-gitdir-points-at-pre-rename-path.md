---
id: '7fccb387dc7334cd'
kind: bug
status: fixed
title: Bench worktree's gitdir still points at the pre-rename repo path, orphaning it from git
tags:
- retrieval
- benchmark
- measurement-validity
- stale-ground-truth
closed: 2026-08-16
---

## Summary

`.worktrees/bench` — the pinned corpus the retrieval benchmark scores against — is no
longer a registered git worktree. Its `.git` file points at `code-explorer`, codescout's
pre-rename name, at a path that no longer exists. The files are all intact; only the git
registration is broken.

## Symptom (Effect)

```
$ cat .worktrees/bench/.git
gitdir: /home/marius/work/claude/code-explorer/.git/worktrees/bench

$ git -C .worktrees/bench rev-parse HEAD
fatal: not a git repository: (null)

$ git worktree list
/home/marius/work/claude/codescout                                     02a87a83 [experiments]
/home/marius/work/claude/codescout/.claude/worktrees/peer-delegation   5c44f512 [feat/peer-delegation]
```

`.worktrees/bench` is absent from the list, and `.git/worktrees/` contains only
`peer-delegation` — no `bench` admin directory. Any `git -C .worktrees/bench` command
fails, so the baseline SHA cannot be confirmed from inside the corpus it pins.

## Reproduction

`git -C .worktrees/bench rev-parse HEAD` at any point on `experiments`.

## Environment

codescout `experiments` @ `b641a38a`, 2026-08-16.

## Root cause

The repository was renamed `code-explorer` → `codescout`. A linked worktree stores an
absolute path to its admin directory in its `.git` file, so the rename orphaned it. The
same rename broke `scripts/sweep-bm25-boost.sh`'s `PROJECT_PATH` default, and **that** was
fixed — the fix is described in a comment sitting three lines from this pointer's blast
radius (*"The previous default pointed at '.../code-explorer' — codescout's pre-rename
name — and had resolved to nothing since the rename"*). The worktree pointer was missed.

*Measured 2026-08-16: the three commands above, plus `ls` confirming the corpus files are
present on disk.*

## Evidence

The corpus itself is fine. Both files a HEAD-relative check reports as missing are present
in the pinned tree:

```
$ ls .worktrees/bench/docs/FEATURES.md .worktrees/bench/src/embed/index.rs
.worktrees/bench/docs/FEATURES.md
.worktrees/bench/src/embed/index.rs
```

And all five paths a current-HEAD existence check flags resolve at the baseline commit:

```
$ git cat-file -e ede25e694b63219e1382f359d7ba242f66a516a5:docs/FEATURES.md   # → present
# same for src/embed/index.rs, src/prompts/server_instructions.md,
#          src/prompts/onboarding_prompt.md, docs/TODO-tool-misbehaviors.md
```

## Hypotheses tried

1. **Hypothesis:** `scripts/run-tc-benchmark.py`'s `expected` lists cite deleted files, so
   several TCs are unpassable at any boost value.
   **Test:** `git cat-file -e <baseline_sha>:<path>` for each of the five paths that a
   current-HEAD `ls` reports missing.
   **Verdict:** **rejected — this was the original claim of this bug file and it was wrong.**
   All five exist at `ede25e69`. The harness scores against the pinned corpus, never against
   current HEAD, so a HEAD-relative existence check says nothing about it. The ground truth
   and its corpus agree. Filed 2026-08-16, retracted the same day; the entry is kept because
   the mistake is the reusable lesson — **check the corpus the instrument actually reads,
   not the one you happen to be standing in.**

## Fix

**Resolved by deletion, 2026-08-16.** The worktree was not repairable in place —
`git worktree repair` requires the referenced repository to exist, and
`/home/marius/work/claude/code-explorer` does not exist on this machine. The admin dir
(`.git/worktrees/bench/{commondir,gitdir,HEAD}` + `read-tree`) was reconstructed by hand
**only to inspect the contents** before deciding, then the whole worktree was removed with
`git worktree remove --force .worktrees/bench`.

Removal was the right call rather than repair because the copy had nothing worth keeping and
was actively misleading:

- Corpus was complete (851/851 baseline files) but diverged from `ede25e69` in two files, both
  documented in `docs/trackers/retrieval-benchmark.md` as *"redundant patches … because main
  already had `CODESCOUT_QUERY_PREFIX` support"*, from an experiment whose verdict was
  *"drop nomic-embed-code-7B from consideration"*.
- It was a leftover from a different machine, which is why its gitdir named a path that is not
  here — and that foreignness is what produced the retracted claim in Hypotheses-tried.
- It is reproducible in one command:
  `git worktree add --detach .worktrees/bench ede25e694b63219e1382f359d7ba242f66a516a5`.

174 MB reclaimed, 163 MB of it regenerable `.codescout` index state.
## Tests added

None, and none applies — the resolution is the deletion of an untracked local directory, not a
code change. The durable guard is documentation instead: `docs/trackers/retrieval-benchmark.md`
§ Prerequisites now states that the worktree is absent on a fresh host and gives the recreate
command, and `host` was added to that tracker's anchored-dimensions list so the next session
does not treat a foreign-host artifact as canonical.
## Workarounds

Read the baseline from the tracker (`params.baseline_sha`) rather than from the worktree.

## Resume

N/A — closed. If a bench run is wanted on this host, recreate the worktree with the command
above and read the 2026-08-16 entry in `docs/trackers/retrieval-benchmark.md` first: the pinned
table was measured on other machines, so results here start a new baseline rather than
continuing it.
## References

- `docs/trackers/retrieval-benchmark.md` — the pinned 25-TC log; `params.baseline_sha`
- `scripts/run-tc-benchmark.py` — the harness
- `scripts/sweep-bm25-boost.sh:6-15` — the comment describing the same rename fallout
