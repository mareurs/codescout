---
id: ecf7dc5d46f9dad8
kind: bug
status: fixed
title: A recorded write stays live after another session's commit swept its content, so the writer is named as the owner of a path for the whole lookback
owners:
- '3e2b9cc8-4e6d-4f8f-8e80-e1ece200e6af'
tags:
- cluster/gate-keyed-on-unobservable-event
---

# BUG: a recorded write stays "live" after another session's commit swept its content, so the writer is named as the owner of a path for the whole lookback

## Summary

`foreign_writer` in `scripts/post-index-change-stage-log.sh` (`dd7b1590`) calls a recorded write live
until its OWN writer commits the path afterwards, read from `Session-Id` trailers. When a DIFFERENT
session's commit sweeps the write's content, the writer's trailer never appears, so the write stays
live for the full three-day lookback and is returned as the owner of any later stage of that path by
another session.

## Symptom (Effect)

Reported by session `a520c25a` after `6e6dc887`: a pathspec commit of `docs/trackers/bug-fix-session-log.md`
was refused as "Staged by 00113c9d-fbc9-4ad4-8d59-7821452e0fb6, NOT LIVE, ATTRIBUTED BY WRITE", though
the staged diff was exactly the reporter's own two-line change. A refusal naming a party who is not
live and whose work is already committed gives the stager nothing to do but `--no-verify`.

## Reproduction

Observed in this checkout, 2026-10-01, before any change:

- `.git/session-stage-log` row 1: `00113c9d-fbc9-4ad4-8d59-7821452e0fb6  96f41eda  docs/trackers/bug-fix-session-log.md  named-foreign  retained`.
- `git hash-object` of the working-tree path is `96f41eda…`; the HEAD blob is `b83c1d4a…`.
- `usage.db`: `00113c9d` has `doc` `append_entry` and `update` rows against this ledger's id
  (`2dd9d90bc83f9f49`) from 2026-09-30 04:12 to 05:13 UTC, writing `F-184`, `F-185`, `F-186`, `W-147`.
- All four headings are in HEAD, introduced by `0d5d111d` (2026-09-30T08:35+03:00), whose `Session-Id`
  trailer is `3e2b9cc8`, not `00113c9d`. The write is committed; no rule that reads trailers can see it.

## Root cause

`dd7b1590` fixed the opposite error: a commit by anyone is not proof a write was taken, since a commit can carry an older staged blob (a tracker's HEAD lacked entries a peer had appended 25 minutes before another session's commit). It over-corrected to *"only the writer's own commit closes a write"*, which reads the event *"this write is in a commit"* off a proxy, the writer's `Session-Id` trailer. When a different session's commit carries the writer's work, nothing ever names the writer, so the write stays live until the three-day lookback ends. The proxy fails silently in one direction and the guard turns it into a refusal in the other: a false refusal naming a party who is not live and whose work is already committed.

An earlier reading in this record's parent (`dd7b1590`) put the two failure directions on one axis. They are two: *the write landed* and *the writer committed it* come apart in both directions, so a rule has to read the first.

## Measurement

**Population, unit, instant, tree.** Non-merge commits with a `Session-Id` trailer over the three days to 2026-10-01 18:03 UTC at tree `32a54dad`, excluding commits of more than 60 paths (two script-written data commits of 624 and 588 paths otherwise dominate): 218 commits, 637 `(commit, path)` pairs. The unit below is a **triple**, `(commit c, path P, writer X)`, where `X` is not a session `c` names, `X` has a successful recorded write to `P` in the three days before `c`, and that write is live under the rule being replayed. 34 triples have a live-looking writer under one rule or the other.

**The label is the content, defined before any rule.** Each triple is labelled by what the commit did with the writer's text (the longest three lines of 16 or more characters of the newest live write that carries any): `CAPTURED` (all in `c:P`, not all in `c^:P` — a refusal is right), `PRE-COMMITTED` (all already in `c^:P` — a refusal is false), `ABSENT` (in neither — the commit did not take it; overwritten, reverted or never landed), `NOPROBE` (the write carries no distinctive text). **This is circular for any rule that tests the same text:** such a rule cannot lose a `CAPTURED` by construction. What is empirical is the size of each class and which rule keeps which class live.

| rule | refuses | PRE-COMMITTED (false) | CAPTURED | ABSENT | NOPROBE |
|---|---|---|---|---|---|
| A: as shipped, closed only by the writer's own commit | 34 | 20 | 4 | 7 | 3 |
| B: closed by any later commit of the path (pre-`dd7b1590`) | 14 | 2 | 4 | 6 | 2 |
| C0: per row, every live row's text must be in HEAD | 32 | 18 | 4 | 7 | 3 |
| **C: the writer's newest live write's text must be in HEAD, and not already in the copy before the write** | 18 | 4 | 4 | 7 | 3 |

B misses one genuinely uncommitted write (`src/symbol/edit.rs`, 139 minutes before the commit, label `ABSENT`) and one `NOPROBE`: that is the defect `dd7b1590` fixed. C closes 16 of A's 20 false refusals and keeps all 4 captures and all 7 unlanded writes live. C0 closes only 2 of the 20, because a later edit by the same session rewrites the words an earlier one added: on the reported ledger two of the writer's six live rows named text its own later edits had changed.

The 4 false refusals C leaves: three are writes whose only text is a divider comment the file already held, which proves nothing and stays live on purpose; one ran in a linked worktree (`3094869ba182deab`, whose population this measurement enlarges: the writer's 25 of 26 rows on that path ran in worktrees).

**Time bounds were measured and rejected.** The lag between a write and the commit that took it, in minutes: `CAPTURED` 0, 22, 1121, 1234; `PRE-COMMITTED` 23, 45, 53, 65, 121, 123, 161, then 1148 to 2164. The ranges overlap, so no threshold separates them.

**A stage-log blob test was not measured**: the log keeps about 1000 rows, so it cannot be replayed over three days, and it records what was staged, not what was committed.

**On the reported case.** Run against the live `usage.db`, `foreign_writer` for the tracker and for `src/librarian/tools/append_entry.rs` returned `00113c9d` before this change and returns nothing after it, for a stager of `a520c25a` and of `2e3f6b65`; two calls took 0.37 s together.

## Fix

`write_in_head` in `scripts/post-index-change-stage-log.sh`, asked from `foreign_writer` for **only the newest live write of each other writer**: the write closes when the longest three lines (16 characters or more, trimmed) of the new text it put in the file are all in HEAD's copy of the path and are not all already in the copy as of just before the write. The text is read from `new_string`, `body`, `content`, `title`, `patch.body` and the same keys of each `edits[]` and `patch.body_edits[]` item; `old_string` is the text the write removed and is not read. Anything it cannot show leaves the write live, as every earlier version did: no distinctive line, an unreadable row, a path HEAD does not hold, a line missing from HEAD, text that predates the write.

Why the newest only: a commit is a snapshot, so one that holds a writer's newest write holds the earlier ones, and asking each row cannot close a writer whose later edits rewrote their earlier words. The exception is a hunk-split commit that takes a later hunk and not an earlier one, which this reads as closed.

The refusal in `scripts/pre-commit-foreign-index.sh` said *"since their last commit"* and *"re-staging does not change that while the record stands"*; both are now wrong. It names what was examined (successful codescout tool writes, three days, closed by the writer's own commit or its newest text in HEAD) and tells a stager whose staged diff is only their own change that the row is stale and how to ask the record again: `git reset -q -- <path>` then `git add -- <path>`. A pair that left the index (a `retained` row) is asked again by a plain named add; a pair still staged keeps its row (cases 24h and 24i).

**Does the existing stale row heal?** It is `retained` (the pair left the index), so the next named `git add` of the path with the same working-tree blob drops it and asks again with this rule; no manual repair. If the peer's blob changes it is a new pair with no row at all.

## Tests added

`tests/hooks-discrimination.sh` cases 24 (13 call shapes, each as a swept and a still-pending pair), 24n-a to 24n-l, 24h and 24i, and two shape assertions on the refusal text under case 1. Red on the unchanged recorder: the 13 swept cases, 24h and 24i (15 assertions). The pending halves and 24n-a, b, c, d, e, g pass on unchanged code and rest on mutation. Suite 269 passed, 0 failed.

**Mutation:** 73 sites in the recorder and guard, re-run after the last byte change; 72 killed, each by the test that names it. The survivor is the SQLite busy timeout, which is tuning and is annotated at both of its uses. Two guards and one sanitisation were deleted rather than tested after their mutations survived: a `json_valid` wrapper (a malformed row yields an empty copy either way), the `|| return` guards on `git show` and on the text (an empty copy fails every probe either way) and a digits-only filter on the row id.

## Fix provenance

- **SHA:** `17b06d4692817939c7a6dce502fb4541a4e7b635` (`experiments`)
- **patch-id:** `791238e8e3b956ece6e15eecb893782c53283e80`

Gate FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0; `tests/commit-mine.sh` 53/0, `tests/install-hooks-check-population.sh` 33/0, `tests/pre-push-foreign-session-guard.sh` 136/0. Residual: a writer whose newest write carries no distinctive text, or whose text was edited since, stays live (conservative, as before); the worktree population is `3094869ba182deab`; a flaky precondition found on the way is `931c28128081ad46`.

## References

- `docs/issues/2026-09-07-the-stage-log-records-the-stager-so-git-add--A-makes-you-the-owner.md` — the parent record.
- `docs/issues/archive/2026-10-01-the-stage-log-recorder-reads-a-subagents-composite-session-id-as-a-peer.md` — the previous follow-up; this one is a defect in the liveness rule `dd7b1590` introduced.
