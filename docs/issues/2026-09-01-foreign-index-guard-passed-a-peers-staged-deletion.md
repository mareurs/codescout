---
kind: bug
status: fixed
tags:
- cluster/guard-narrower-than-its-name
closed: 2026-09-01
opened: 2026-09-01
owner: marius
related: []
severity: medium
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

**MEASURED 2026-09-01** — upgraded from "Unknown". The cause is **attribution, not
enumeration**, and it is none of the three leads recorded below, though one was the right
family.

`scripts/post-index-change-stage-log.sh` assigned each new `(blob, path)` pair to
whichever session's hook **observed** it first. `post-index-change` fires on **every**
index write, and that includes `git status` — not only the staging commands. Probed by
logging `/proc/$PPID/cmdline` from the hook, one line per invocation:

```
git add a.txt        -> [git add a.txt]
git commit -qm base  -> [git commit -qm base]
git status --short   -> [git status --short]      <- fires, and stages nothing
git rm -q --cached   -> [git rm -q --cached a.txt]
```

So on a checkout where five sessions poll `git status`, a staged batch was claimed by
whoever ran `git status` next, not by whoever staged it. Observed directly: at 01:48 the
log attributed `src/tools/ast.rs` to `6896e62b` (its real stager); by 02:00 **all 13**
staged paths were attributed to a single unrelated id. With no foreign path left to see,
the guard exited 0 and the bare commit swept the deletion.

The guard itself behaved correctly throughout. It refuses when the log names a foreign
owner, and the log had stopped naming one — a correct consumer of corrupted input.
## Evidence
### The commit
`git show --stat 3a422b31` → 3 files, including `src/tools/ast.rs | 579 ---`. Committing
session (this one) ran `git add` on the two docs paths only.

## Hypotheses tried

1. **Deletions are invisible to the stage log** (peer lead, and the most intuitive).
   **Test** — stage a deletion with `git rm --cached` and read `.git/session-stage-log`.
   **Verdict — REJECTED, measured.** Deletions *are* recorded. `git diff --cached --raw`
   gives an all-zero post-image sha for a removal, and the row is present and correct:
   `6896e62b\t00000000\tsrc/tools/ast.rs` was in the live log. The all-zero blob is a
   perfectly good key, and `(blob, path)` stays unique because the path discriminates.
   Pinned by `tests/hooks-discrimination.sh` § *stager wins*, which asserts a staged
   deletion is recorded and later named in the refusal.

2. **Dead-owner allowance** — the liveness probe discounts an owner it cannot prove alive.
   **Test** — read the order of operations in `scripts/pre-commit-foreign-index.sh`.
   **Verdict — REJECTED, measured.** The refuse/pass decision is `((${#theirs[@]})) ||
   exit 0` at `:148`; `resolve_session` is first called at `:191`, inside the message
   block, after `exit 1` is already determined. Liveness shapes what the refusal *says*
   and can never change *whether* it fires.

3. **The install-time baseline** (peer lead, recorded as OB-6's second exemplar
   `6e30e1e2`). **Verdict — RIGHT FAMILY, WRONG INSTANT.** The family is real: a pair
   with no prior row is claimed by the wrong party. But `e3c75306` had already seeded the
   log at install, and that only covers the **bootstrap**. This failure was **steady
   state** — attribution decaying under ordinary peer activity, hours after install. The
   lead pointed at the right mechanism at the wrong moment in its life, which is why the
   seeding fix was in place and the failure happened anyway.
## Fix

**Fixed at `a987df96` on `experiments`** (patch-id
`c1f784b7cd2c229a8da42b9feceb8508d6210bda`).

**The stager wins, not the first observer.** The hook runs as a child of the git process
that wrote the index, so `/proc/$PPID/cmdline` names the operation that caused the write.
A pair first seen during a **non-staging** write is now recorded `-` (unknown) instead of
being claimed.

The direction is deliberate and is the ruling this file asked to have pinned with the fix:
**`unknown` over-refuses recoverably — a reader reads a message — where `mine`
under-refuses silently, and nothing is emitted for anyone to recover from.** Prefer the
noisy wrong answer wherever the quiet one is unobservable. The same rule now governs both
ends of the pair's life: `install-hooks.sh` seeds inherited pairs `-` at bootstrap, and
this fix records unattributable pairs `-` in steady state.
## Tests added

**`tests/hooks-discrimination.sh`** — 21 cases, committed at `07928b44`. Before this the
hooks had no automated test at all: no shellcheck, no CI step for `scripts/`, and the
sibling `pre-commit-unreviewed-content.sh` was verified only by a hand-run matrix pasted
into a commit message, which is evidence about one instant and cannot fail a later build.

§ *stager wins* pins this exact defect: a peer's `git status` must not steal an edit or a
deletion, and a cold log plus a peer's `git status` must record `-` rather than the
passer-by. Both fail against the pre-`a987df96` script.

Every case is a **discrimination** — each asserts the hook is silent where it must be
silent *and* loud where it must be loud. A suite checking only the loud direction passes
against a hook that refuses everything; one checking only silence passes against a hook
that was deleted.

The suite also carries an `assert_throwaway` refusal, because its own first version ran
against the real checkout via a `$(subshell)` `cd` — written up in `07928b44`.
## Workarounds
The safe composition documented in the hook's own header: `git add <paths>` then
`git commit -- <same paths>` — the pathspec form ignores the shared index entirely.

## Resume

N/A — root cause measured, fixed and pinned by a regression suite.

One thing deliberately **not** claimed as covered, since this file's tag is
`cluster/guard-narrower-than-its-name` and that is the trap: the guard covers **CROSS-path**
capture only — my index holding your file. It does not cover **INTRA-path** capture, where
a path is legitimately yours and its *content* gained a peer's lines, and it cannot cover a
`git add -A` under your own id, where every path reads as yours by construction. Both are
stated in the script header. `scripts/pre-commit-unreviewed-content.sh` covers the
add-to-commit window of the intra-path axis, verified in the same suite.
## References
- `scripts/pre-commit-foreign-index.sh` (guard + its own IC-14 disclaimer for the INTRA-path axis — this incident is the CROSS-path axis it claims)
- `docs/issues/2026-08-31-peer-commit-captures-another-sessions-working-tree.md` (both capture shapes)
- Commit `3a422b31` (`experiments`) — the sweep; peer notified via cross-session message
