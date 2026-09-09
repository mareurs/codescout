---
status: open
opened: 2026-09-09
closed: ''
severity: high
owner: marius
related: []
tags:
- cluster/guard-narrower-than-its-name
kind: bug
---

# BUG: the pre-push guard filters on the LOCAL ref shape, so `git push origin <sha>:<branch>` bypasses it entirely and publishes foreign commits with exit 0

## Summary

`scripts/pre-push-foreign-session-guard.sh` skips any stdin line whose **local** ref does not
match `refs/heads/*`. A refspec push (`git push origin <sha>:refs/heads/<branch>`) reports the
local side as a raw SHA, so the guard `continue`s past it, examines nothing, and exits 0 —
publishing every foreign commit in the range silently.

Both push forms send **the same commits**. Only the shape of the string on the left of the
colon differs, and that string is the one thing the guard keys its decision on.

## Symptom (Effect)

Observed live 2026-09-09 by sessionId `b0015a98-e290-46de-8ed1-3c94bc73a987`, one command
apart on this checkout:

```
git push origin experiments           -> REFUSED, naming 914c50aa / ad379a7c as foreign
git push origin 0b07f9c8:experiments  -> SUCCEEDED, and published 914c50aa anyway
```

`914c50aa` carries `Session-Id: ad379a7c-a0cf-4c61-bcdb-f0696fea8c30` and that author had
explicitly confirmed the **uncleared** state. It is now an ancestor of `origin/experiments`.

The note the guard printed on the successful push is worse than silence:

```
CODESCOUT_PUSH_ACK named ad379a7c-a0cf-4c61-bcdb-f0696fea8c30, which authored no
commit in this push, so the ack had no effect on it.
```

`authored no commit in this push` is **true of the population the guard built** (empty) and
false of the push. The reassuring reading — *you named the wrong sid* — is the wrong one; the
correct reading is *the guard is not looking there*.

## Reproduction

Throwaway with a local remote; nothing leaves the machine. Verified 2026-09-09 at
`5f0e1fc3`:

```
$ # base commit (mine) pushed to establish refs/heads/experiments, then:
$ #   FOREIGN commit  (sid bbbbbbbb-…)
$ #   mine, on top of the foreign one
$ git push origin experiments                          # A
EXIT=1   REFUSING lines: 1
$ git push origin "$TIP:refs/heads/experiments"        # B, same commits
EXIT=0   REFUSING lines: 0
$ git merge-base --is-ancestor "$FOREIGN_SHA" origin/experiments
PUBLISHED                                              # guard bypassed
```

**The mechanism, isolated** — the guard invoked by hand with the *same range* and the *same
commits*, varying only `local_ref`:

```
local_ref=refs/heads/experiments        exit=1  REFUSING=1
local_ref=1d3367371ee77367d7d2606c22b9  exit=0  REFUSING=0
```

## Environment

Linux, `experiments`, shared checkout, 13 live sessions. Platform-independent.

## Root cause

**The decision is keyed on the SOURCE of the refspec; the thing being protected is its
DESTINATION.**

- `scripts/pre-push-foreign-session-guard.sh:124` —
  `case "$local_ref" in refs/heads/*) ;; *) continue ;; esac`

git's pre-push hook receives `<local ref> <local sha> <remote ref> <remote sha>` per ref. For
`git push origin <sha>:refs/heads/<branch>` the local side is a **commit object**, not a ref,
so git reports the SHA there. The `case` fails, `continue` fires, and the loop body — which
contains the entire foreign-commit analysis — never runs for that line.

**The range computation is CORRECT and is not the defect.** `:131` is
`range=("$remote_sha..$local_sha")`, which is exactly what git will send, ancestors included.
The guard never reaches it. This distinction decides the fix: a repair aimed at the range
would change a line that is already right and leave the bypass intact.

`_remote_ref` is bound with a leading underscore at `:121` — deliberately unused. It holds
`refs/heads/experiments` under **both** push forms, and it is the field that actually answers
*"am I publishing to a branch?"*.

Measured 2026-09-09 by the direct probe above, not inferred from reading the `case`.

## Evidence

### The suite cannot catch this by construction

`tests/pre-push-foreign-session-guard.sh` drives the guard by piping stdin lines, and every
one of them is written `refs/heads/main`. No fixture ever supplies a SHA-shaped local ref, so
the excluded branch is unreachable from the suite however many assertions are added — the
same shape as the linked-worktree gap fixed at `3f0d13b7`, in the same file.

### It is the reachable half of a pair, and the other half over-refuses

Reported by `b0015a98-e290-46de-8ed1-3c94bc73a987`, **not reproduced here**: force-pushing a
*branch* ref computed its population against `origin/result-cap-marker-gate` (`2a32c043`),
orphaned by a rebase, and demanded acks for **26 sessionIds** to publish **1** new commit —
508 of the 509 in that range were already on `origin/experiments`. Same guard: refusal
inflated by a stale positional ref, refusal evaded by a narrow selector, both silent. Filed
here as context; it wants its own reproduction and may want its own file.

## Hypotheses tried

1. **Hypothesis** — the population is the tip commit only, so ancestors are never counted
   (the shape first reported to me).
   **Test** — read `:126-132`; then invoke the guard by hand with one range and two
   `local_ref` shapes.
   **Verdict** — **rejected.** The range is `"$remote_sha..$local_sha"`, which includes every
   ancestor the remote lacks; with `local_ref=refs/heads/…` the same range refuses correctly.
   The defect is upstream of the range, in the ref-shape filter. The correction matters: the
   proposed fix (derive the population from `git rev-list <remote-ref>..<local-sha>`) would
   rewrite a correct line and leave the bypass in place.

## Fix

Not applied. **Key the filter on the destination, not the source:** test `_remote_ref` against
`refs/heads/*` instead of `local_ref`. That field is `refs/heads/<branch>` under both push
forms, which makes the two routes agree by construction — the property currently missing.

Keep a guard for `local_sha = ZERO` (branch deletion), which is orthogonal and already
handled at `:123`.

Second, independent of the filter: **the inert-ack note must distinguish *no such author in
the range examined* from *nothing was examined*.** As written it reports the same sentence for
both, and for the second case that sentence is a false reassurance. An ack that matched
nothing because the population was empty should say the population was empty.

## Tests added

None yet. The regression test must supply a **SHA-shaped `local_ref`** on stdin — the one
input the existing 86-assertion suite cannot express — and assert refusal, paired with the
`refs/heads/` form asserting the identical outcome over the identical range. Acceptance
criterion is an observed RED: with the filter restored to `local_ref`, the SHA-shaped case
must fail while the `refs/heads/` case stays green, so the pair discriminates in both
directions.

## Workarounds

**Push by branch name.** `git push origin <branch>` is examined; `git push origin
<sha>:<branch>` is not. The guard's own ladder advice — *"you push yours by refspec once
nothing foreign sits beneath it"* — is safe only under a precondition it does not check, so
until this is fixed that advice must not be followed while anything foreign is beneath you.

## Resume

Change `:124` to filter on `_remote_ref` (renaming it, since it stops being unused), add the
SHA-shaped-`local_ref` fixture described above, and confirm the observed RED by reverting the
filter. Then re-word the inert-ack note to separate an empty population from an unmatched
author.

## References

- `scripts/pre-push-foreign-session-guard.sh:121-132` — the stdin loop, the ref-shape filter,
  and the (correct) range computation.
- `docs/trackers/observer-blindness.md` OB-20 — why the guard exists.
- `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md` — the
  hazard this bypass re-opens.
- Reported by sessionId `b0015a98-e290-46de-8ed1-3c94bc73a987`, who measured the live pair;
  the mechanism correction and the isolating probe are this file's.
