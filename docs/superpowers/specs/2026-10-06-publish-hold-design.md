# Publish hold — an author records "withheld", and the pre-push guard enforces it

Status: draft for operator review. Bug: `docs/issues/2026-09-06-a-push-publishes-commits-their-author-was-withholding.md`
(`d9d291b44775e50d`). Class: `OB-20` in `docs/trackers/observer-blindness.md`.

## Problem

`git push` sends every unpushed commit on the branch. On a checkout that several sessions share, one
session's push publishes commits that another session is withholding. Git cannot tell a withheld commit
from one that is not pushed yet. The fact "may this be published" lives in an operator conversation, so
only the author can record it.

`scripts/pre-push-foreign-session-guard.sh` already refuses a push that carries another session's commits
unless `CODESCOUT_PUSH_ACK` names them. That covers the pusher side. It has two gaps:

- The author has no way to record a withhold. `CODESCOUT_PUSH_ACK=all` publishes everything.
- A session whose operator approved the commit, and was never asked about the push, is neither "withheld"
  nor "cleared". The binary question cannot represent it (bug file, instance 2026-09-15).

## Goal

An author can record, in state that every session on the checkout sees, that its unpushed commits are
withheld. While the record exists, the pre-push guard refuses to push those commits. No ack overrides it.
Only a release does.

## Non-goals

- The commit side. Nothing stops a session from committing a change its operator said to hold. The
  convention in `docs/conventions/shared-checkout-commit-sequence.md` covers that as a policy.
- Trailerless commits. They carry no `Session-Id`, so a hold cannot match them. The guard already reports
  and allows them. That hole stays and is stated in the guard's header.
- Proving who releases a hold. Every session runs as one git user, so no script can tell an operator from
  a peer.

## Design

### The record

`refs/holds/<session-id>` in the shared `.git`. The ref points at a blob. The blob holds the reason, the
UTC time of the hold and the HEAD sha at the time of the hold.

Verified 2026-10-06 with git 2.56.0 in a throwaway repo:

- A ref under `refs/holds/` can point at a blob.
- A plain `git push origin main` and `git push --all` do not send `refs/holds`.
- `git push --mirror` does send it. `--mirror` sends every ref, so it is already outside normal use here.

Why a ref in `.git` and not a file in the working tree: a working-tree file can be swept into a peer's
commit, which is the failure this spec exists to remove.

Why the session id and not the commit sha: an amend or a rebase changes the sha. Verified in the same
session: a `git notes` entry keyed to a sha is dropped on amend unless `notes.rewriteRef` is set. A hold
keyed to the sha would stop holding without any signal. The `Session-Id` trailer survives amend and
rebase, because the commit message is replayed and `scripts/prepare-commit-msg-session-id.sh` stamps with
`--if-exists doNothing`. This spec requires a test that asserts it (see Tests).

### The recorder: `scripts/hold-publish.sh`

| Command | Effect |
|---|---|
| `set [reason]` | Writes the blob and creates `refs/holds/$CLAUDE_CODE_SESSION_ID`. Exits non-zero with a message when the variable is empty. Replaces an existing hold of the same session and keeps the original time. |
| `release [sid]` | Deletes the ref. With no argument it releases the caller's own hold. A `sid` argument releases that session's hold. |
| `list` | Prints each hold: sid, age, reason, and whether the session is live (reuses the guard's session lookup). |

`release <other-sid>` is allowed and prints a one-line notice that the hold belonged to another session.
It is an operator decision. The script cannot enforce that, and `docs/RELEASE.md` must say so.

### The enforcement: a new check in `scripts/pre-push-foreign-session-guard.sh`

In the per-commit loop, before `acked "$sid" && continue`:

1. Read the commit's `Session-Id` through `%(trailers:key=Session-Id,valueonly)`, as the guard does now.
2. If `refs/holds/<sid>` exists, append the commit to a `held_report` and `continue`. Do not run the ack
   test for it.
3. After the loop, if `held_report` is non-empty, the guard prints a distinct refusal section and exits 1.
   This applies to the pusher's own commits too. The message says to release first.

The refusal for a held commit prints:

- the holder's sid, reason, age and liveness;
- the release command;
- the refspec prefix that is still pushable. The guard computes it directly from the parent of the oldest
  held commit, as `git push <remote> <parent>:<branch>`. It does not take it from the ladder. When nothing
  below the oldest held commit is unpublished, it says so instead.

`CODESCOUT_PUSH_ACK` never clears a held commit. When the ack names a held sid, the guard says so, because
an ack that is silently inert is a failure mode the guard's header already records.

The guard keeps its current behaviour when `CLAUDE_CODE_SESSION_ID` is empty (exit 0, silent). A human
release flow is not blocked.

### Failure modes

| Case | Result |
|---|---|
| Hold for a dead session | Still refuses. Fails closed. The refusal names the release command. |
| Hold forgotten | The refusal shows its age. The author or the operator releases it. |
| `refs/holds` unreadable | The guard degrades to its current behaviour and prints one warning line. It never produces a wrong "held" claim. |
| Session sets a hold, then commits more | All its unpushed commits are held, because the key is the session. |
| Prefix push below the first held commit | Still allowed. The ladder already supports it. |

## Change scenarios absorbed

- An author amends or rebases a held commit. The key is the session id, so the hold stays.
- A peer pushes with `CODESCOUT_PUSH_ACK=all`. The held commit is still refused.
- A new session joins the checkout. It needs no setup to be protected from other sessions' holds, and it
  can hold its own work with one command.

## Consequences

- Easier: a withheld state exists in git state, so a peer's push cannot publish it.
- Harder: a forgotten hold blocks a session's commits for every pusher until it is released.
- Granularity is the session, not the commit. An author who has cleared commits and withheld commits in the
  same unpushed stack must push the cleared prefix by sha, as the ladder already says.

## Revisit when

- A hold set for the wrong session id blocks a push.
- Session-level granularity proves too coarse in real use.
- `--mirror` pushes become a real workflow here.

## Tests

Extend `tests/pre-push-foreign-session-guard.sh`. Every case builds a throwaway repo, as the file's header
requires. Each case asserts both the refusal and the silence it must keep.

1. A held session's commit is refused when `CODESCOUT_PUSH_ACK=all`.
2. The same push passes after `release`.
3. A hold on one session does not refuse another session's commits.
4. The pusher's own held commit is refused, and the message says to release.
5. The hold survives `git commit --amend` and a `git rebase` of the held commit (the trailer is still there
   and the guard still refuses).
6. A prefix push below the first held commit is allowed.
7. A plain `git push` and `git push --all` do not publish `refs/holds` (assert on a bare remote).
8. Negative control: with the new check removed from a copy of the guard, case 1 fails. The guard's own
   comments record that an absence assertion passes on a dead script, so case 1 must be paired with a
   positive assertion on the refusal text.
9. `scripts/hold-publish.sh` unit cases: `set` without a session id fails, `set` twice keeps the original
   time, `release` of an absent hold is a no-op with exit 0, `list` marks a dead session.

## Files touched

- New: `scripts/hold-publish.sh`.
- New: `scripts/resolve-sids.sh`. The guard's session lookup (`resolve_sids`) moves into this sourced file so
  that `hold-publish.sh list` and the guard share it.
- Edited: `scripts/pre-push-foreign-session-guard.sh` also sources `scripts/resolve-sids.sh`, and
  `tests/pre-push-foreign-session-guard.sh` gains a case for the lone-guard fallback of that lookup.
- Edited: `scripts/pre-push-foreign-session-guard.sh` (the check, and its header, which says the author half
  is open).
- Edited: `tests/pre-push-foreign-session-guard.sh` (cases above).
- Edited: `docs/conventions/shared-checkout-commit-sequence.md` (the section added 2026-10-06 names the
  mechanism and the existing guard).
- Edited: `docs/RELEASE.md` (one bullet, and the release-is-an-operator-decision note).
- Edited: the bug file (`## Fix`, `## Resume`).

## Verification before this is called done

- `./scripts/gate.sh` is green.
- A live check on a throwaway clone with two session ids: hold, push refused under `CODESCOUT_PUSH_ACK=all`,
  release, push passes.
