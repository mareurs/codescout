# Artifact identity redesign — where the work stands (2026-11-13)

Read `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md` (doc id `9bcc00c179ecbdce`) first. It holds the task table, rulings, deferred findings, bugs, "after the plan" steps and the next steps for Plan C1.

## One-paragraph state

Spec v5 (`docs/superpowers/specs/2026-11-11-repo-relative-artifact-key-design.md`): committed files are the only store, the catalog is a derived index, the key is the lowercase filename stem, one closed citation grammar. Plans A, B1 and B2 are merged on `experiments`. Plan C is split: C1 (dual-write of entries and cites, this memory's subject), C2 (events, observations, links), C3 (content-keyed vector cache, needs a persistent derived-file location first). Plan C1 runs by subagent-driven development on branch `feat/artifact-identity-plan-c1-20261112` in worktree `.worktrees/artifact-identity-plan-c1`: Tasks 1-7 complete and reviewed, Task 8 (prose-path cites) in progress, Tasks 9 (rekey) and 10 (CLI write lock) pending, then the whole-branch Opus review, one fix wave, and the landing question to the user.

## Rules that bite when resuming

- A full gate runs only on an UNTOUCHED tree (no implementer holds an open round on that worktree); see session-log F-16.
- `TOOL_SURFACE_CHAR_BUDGET` is exactly 57_664 with zero headroom: any advertised description or schema edit is character-neutral or smaller, or re-measured with a log entry.
- The SDD ledger is git-ignored in the worktree (`.superpowers/sdd/2026-11-12-artifact-identity-plan-c1-dual-write-entries-and-sidecars/progress.md`); the research doc is the committed copy.
- Never merge into `experiments` or push without the user's answer; landing is merge `experiments` into the branch, gate that tree, then `git merge --ff-only` in the shared checkout after checking peers' staged files; remove the worktree with `scripts/worktree-remove.sh`.
- Session log id `abff63149e3c24e8`; B1/B2 rulings docs `8721253ab393c824`, `4c82d96fc9952e98`.
