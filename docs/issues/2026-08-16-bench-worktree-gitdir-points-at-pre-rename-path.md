---
id: '5043d3c2e3e4bbfd'
kind: bug
status: open
title: Bench worktree's gitdir still points at the pre-rename repo path, orphaning it from git
tags:
- retrieval
- benchmark
- measurement-validity
- stale-ground-truth
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

Not implemented. Re-register the worktree so `git -C` works again and the baseline is
verifiable from inside it:

```
git worktree repair .worktrees/bench      # rewrites the gitdir pointer + admin dir
git -C .worktrees/bench rev-parse HEAD    # must print ede25e694b63219e1382f359d7ba242f66a516a5
```

`git worktree repair` is the purpose-built path for exactly this (moved/renamed main repo).
If it cannot reconstruct the admin dir, the fallback is to re-add the worktree detached at
the baseline SHA — but confirm the existing tree is unmodified first, since a re-add
discards local state, and this corpus is what every pinned run in
`docs/trackers/retrieval-benchmark.md` was scored against.

Until then the benchmark may still run — the harness takes a `--project-path` and indexes
files — but codescout's own git detection inside that path will fail, so any
provenance/`project_sha` the run records is unreliable.

## Tests added

None. A prerequisite check in the harness ("is `--project-path` a valid git checkout, and
does its HEAD match the expected baseline?") would have surfaced this before a run rather
than after.

## Workarounds

Read the baseline from the tracker (`params.baseline_sha`) rather than from the worktree.

## Resume

Run `git worktree repair .worktrees/bench`, then assert
`git -C .worktrees/bench rev-parse HEAD` equals `ede25e69...`. If it does not, the corpus
has drifted from the pinned baseline and every row in `docs/trackers/retrieval-benchmark.md`
is compromised — the tracker's own rule says to start a new section in that case.

## References

- `docs/trackers/retrieval-benchmark.md` — the pinned 25-TC log; `params.baseline_sha`
- `scripts/run-tc-benchmark.py` — the harness
- `scripts/sweep-bm25-boost.sh:6-15` — the comment describing the same rename fallout

