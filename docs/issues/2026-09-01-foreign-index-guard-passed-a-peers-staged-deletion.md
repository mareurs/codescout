---
status: open
opened: 2026-09-01
closed:
severity: medium
owner: marius
related: []
tags: [cluster/guard-narrower-than-its-name]
kind: bug
---

# BUG: foreign-index pre-commit guard passed a bare commit carrying a peer's staged deletion

## Summary
The `foreign-index` hook ("refuse an index commit carrying another session's staged paths",
`scripts/pre-commit-foreign-index.sh`) exists precisely for the CROSS-path axis — my bare
`git commit` sweeping a path a peer staged. On 2026-09-01 it printed `Passed` while doing
exactly that: a peer session's staged **deletion** of `src/tools/ast.rs` rode into commit
`3a422b31` (a docs-only commit staging two markdown files).

## Symptom (Effect)
```
refuse a pathspec commit carrying unstaged content...................................Passed
refuse an index commit carrying another session's staged paths.......................Passed
```
Commit `3a422b31` shows `delete mode 100644 src/tools/ast.rs` (579 deletions) alongside the
two intended docs files. Exit code 0; commit created. User ruled: keep the commit, no
history rewrite (peer sessions notified).

**Escalation (same day):** the sweep broke the committed tree — the deletion's companion
edits (removal of `pub mod ast;` from `src/tools/mod.rs`, tests, fixtures) were only in the
owner's uncommitted working tree, so HEAD and every commit on top (`a1279600` landed on the
broken base) failed to compile. Detected by peer `codescout-b1`, verified here (`git show
HEAD:src/tools/mod.rs` still declared `mod ast`; not pushed). Repaired forward in
`d12bd4af`: `src/tools/ast.rs` restored from `3a422b31^` (blob `a9d8c435`), pathspec
commit, no history rewrite, owner's uncommitted work untouched. The repair fixes the
*consequence* only — this bug (the guard passing) remains open.

## Reproduction
Not re-run (would need a second live session staging a deletion). Shape: session A stages
`git rm`/deletion of a file; session B runs `git add <docs>` + bare `git commit`. Observed
at `3a422b31` on `experiments`, four peer sessions active, `codescout-e8` busy at the time.

## Environment
Shared checkout `/home/marius/work/claude/codescout`, branch `experiments`, pre-commit 4.x,
hooks installed via `scripts/install-hooks.sh` (stage log seeded per `e3c75306`).

## Root cause
Unknown — see Resume. Best lead, from peer `codescout-b1` (recorded pre-incident as OB-6's
second exemplar, `6e30e1e2`): **the install-time baseline**. At install the hook has no
prior stage log, cannot observe who staged pre-existing pairs, and records them as the
installing session's own — so a path staged before install is invisible to the guard
forever after, silently from its first run. `e3c75306` ("seed the stage log at install")
looks like the intended fix; check whether the `ast.rs` staging predates it. Design ruling
worth pinning with the fix: **`unknown` over-refuses recoverably (a reader reads a message);
`mine` under-refuses silently (nothing is emitted)** — prefer the noisy wrong default.
Two earlier leads, both unmeasured, now secondary:
1. **Deletions may be invisible to the stage log**: if `scripts/post-index-change-stage-log.sh`
   records stagings from a listing that omits deleted paths (or `post-index-change` doesn't
   fire for the staging form the peer used), the foreign path has no owner row and the guard
   has nothing to refuse. Inferred from the hook family's design — not measured.
2. **Dead-owner allowance**: `e3c75306` ("detect dead owners instead of describing them")
   may treat an owner it cannot prove alive as ignorable; if the stager's pid/session probe
   misfired, the row is discounted. Inferred from the commit subject — not measured.

## Evidence
### The commit
`git show --stat 3a422b31` → 3 files, including `src/tools/ast.rs | 579 ---`. Committing
session (this one) ran `git add` on the two docs paths only.

## Hypotheses tried
None yet — logged on notice, per CLAUDE.md § Bug Tracking.

## Fix
N/A — not started.

## Tests added
N/A — not started.

## Workarounds
The safe composition documented in the hook's own header: `git add <paths>` then
`git commit -- <same paths>` — the pathspec form ignores the shared index entirely.

## Resume
Test peer lead first: establish whether the `ast.rs` deletion was staged BEFORE the
stage-log seeding (`e3c75306`) landed/installed — if yes, the install-time-baseline
mechanism (OB-6 exemplar `6e30e1e2`) explains the pass and the fix is seeding + the
`unknown`-over-`mine` default direction. Then `scripts/pre-commit-foreign-index.sh` § MECHANISM (below the truncation at ~line 55)
and `scripts/post-index-change-stage-log.sh`; establish (a) whether a staged deletion
produces a stage-log row at all, and (b) what the guard does with an ownerless foreign
path — refuse or pass. Then reproduce with two shells: shell 1 `git rm --cached` a file,
shell 2 stage a doc + bare commit with the hook live. The answer decides whether this is
lead 1, lead 2, or a third mechanism.

## References
- `scripts/pre-commit-foreign-index.sh` (guard + its own IC-14 disclaimer for the INTRA-path axis — this incident is the CROSS-path axis it claims)
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` (both capture shapes)
- Commit `3a422b31` (`experiments`) — the sweep; peer notified via cross-session message
