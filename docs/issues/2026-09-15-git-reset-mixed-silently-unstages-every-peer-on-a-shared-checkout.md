---
status: open
opened: 2026-09-15
closed:
severity: medium
owner: marius
related: []
tags: [cluster/shared-resource-carries-no-owner]
kind: bug
---

# `git reset --mixed` silently unstages every peer on a shared checkout

## Summary

**The prohibition already exists and I broke it.**
`docs/conventions/shared-checkout-commit-sequence.md` § 6 reads *"Do not `reset`, do not
`amend`, do not `stash`. On a shared tree the repair destroys work that the defect only
mislabels."* I ran `git reset --mixed` to repair an amend, and found out that page existed
when the pre-commit hook printed it back to me — on the commit filing this bug.

So this file is a **sharpening**, not a discovery, and it is filed for the one thing that
page does not say: *which* work the repair destroys, and why nobody sees it go.

`.git/index` is **one file per checkout**, not per session. Six sessions commit to this
one. `git reset --mixed` (the default) rewrites that index to match the target commit —
so it discards whatever *any other session* had staged at that instant, with no output,
no error, and nothing for the victim to attribute it to.

`git reset --soft` moves HEAD and leaves the index alone. On this checkout that is
almost always the flag you want.

## Symptom (Effect)

A peer runs `git add` as part of a stage → read `git diff --cached` → commit sequence.
Another session runs any `git reset` without `--soft`. The peer's next command finds
nothing staged. Their `git add` did not fail; it was undone by a process they cannot see,
between two of their own commands.

**The asymmetry is the point.** The commit-rewrite this was the repair for
(`docs/issues/2026-09-15-git-log-1-answers-what-is-head-and-is-read-as-what-did-i-commit.md`)
is *visible to its victim* — the sha changes, and they can verify the whole account. This
is not: the index carries no owner, no history and no reflog, so a cleared `git add` leaves
no artifact that says it was cleared, let alone by whom. Enumerating the live peers, which
is this repo's standing answer to shared-checkout questions, does not help — it tells you
who is present, never who holds a staged path.

That is `IC-17` exactly: a shared resource carries no owner, so enumerating the peer does
not help.

## Measurement

Reproduced 2026-09-15 in a throwaway repo — never this checkout — with a peer's `git add`
in flight across the reset:

| repair flag | my change | **peer's staged file** |
|---|---|---|
| `git reset --mixed <sha>` | unstaged | **gone — silently** |
| `git reset --soft <sha>` | staged | **intact** |

`--soft` is not merely safer here, it is *also* what the repairing session wants: it leaves
the change being rescued staged and ready to re-commit, so it saves a step rather than
costing one.

## How it was found

Not by the session that ran it. Session `29420e72-c262-4236-82c2-52d769fdc549` raised it
on being told their commit had been amended and restored — they checked their own staging,
found it had survived, and reported the **near miss** rather than the cost:

> "mine survived (your reset evidently preceded my `git add`), so this is a near miss I can
> report rather than a cost."

That is the § *Testing Discipline* instruction about instrumenting the doubt: a
re-derivation that *confirms* is a denominator, and publishing it is what stops the
population looking self-correcting. The session that ran the reset had already moved on to
the next task and would not have looked.

## Fix (proposed, not shipped)

Unresolved, and deliberately filed before deciding, because the obvious remedies are both
weak in the way this repo has paid for before:

- **"Remember to use `--soft`"** is a policy, not a mechanism — § *Observer Blindness*
  position 3. The party who needs it is mid-incident, repairing something else.
- **A pre-`reset` hook** has no natural trigger: git has no `pre-reset` hook, and
  `reference-transaction` fires after the fact and cannot see the index.

The shape that would actually close it is probably the one used elsewhere here: make the
correct path the one that ends in a safe state — e.g. a `scripts/` wrapper for the
reset-and-repair sequence that defaults to `--soft` and says why, in the same way
`scripts/fmt-mine.sh` replaced "remember to run `--check` first".

**Not started.** Filed on notice, per CLAUDE.md § *Bug Tracking*.

## Repro

1. `git add <file-a>` in session A.
2. In session B, `git reset --mixed <any commit>`.
3. Session A: `git diff --cached` is empty. Nothing reported it.
