---
id: f47274c162774e8e
kind: bug
status: open
title: 'BUG: a hand-written Session-Id above the Co-Authored-By block defeats the hook''s --if-exists doNothing, so the trailer is written twice'
owners:
- marius
tags:
- cluster/guard-narrower-than-its-name
- git-workflow
- multi-session
- cosmetic
topic: commit trailer stamping on a shared checkout
---

# BUG: a hand-written `Session-Id` above the final trailer block is invisible to `--if-exists doNothing`

## Summary

`scripts/prepare-commit-msg-session-id.sh` uses `git interpret-trailers --if-exists
doNothing` specifically so re-runs (`--amend`, rebase, squash) never accumulate duplicates.
That guard inspects only the message's **last** paragraph. An author who hand-writes
`Session-Id:` *above* the `Co-Authored-By` block opens a separate, earlier trailer
paragraph the guard never looks at, so the hook adds a second one.

**~~Cosmetic — no consumer is wrong today.~~ FALSIFIED 2026-09-16 — there is now a consumer
that is wrong, and it is an attribution query.** See § *2026-09-16 — a second consumer, and the
remedy text that produces it*. The original sentence is kept struck rather than deleted because
the prediction it made — *"the divergence is silent and will widen"* — is exactly what happened,
and a reader should be able to see that the file called it.

## Symptom (Effect)

The raw body carries the line twice, in two different paragraphs:

```
Session-Id: 8dba66b0-af4b-4cda-a333-54a0605b318e

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
Session-Id: 8dba66b0-af4b-4cda-a333-54a0605b318e
```

Measured across commits since 2026-09-01, by walking every commit reachable from HEAD and
counting `^Session-Id:` in each raw body:

```
commits carrying a Session-Id trailer: 802
of those, carrying a duplicate raw line: 48
```

**48 of 802 (6.0%)**, over a walk of 859 commits. Unit: commits reachable from `HEAD`,
walked with an explicit `--since='2026-09-01 00:00:00'` — the bare-date spelling of that
cutoff returns a different population and is its own bug
(`docs/issues/archive/2026-09-06-a-bare-date-in-gits-since-is-filled-from-the-current-wall-clock.md`).

## Reproduction

```bash
# write a commit message whose body contains a Session-Id line ABOVE the Co-Authored-By
# paragraph, then commit normally with the hook installed:
git log -1 --format='%B' <sha> | grep -c '^Session-Id:'          # 2
git log -1 --format='%(trailers:key=Session-Id)' <sha> | grep -c Session-Id   # 1
```

Observed on `8320d5b0`, `29c5b461`, `91cbdb4f` (all `raw=2, parsed=1`). Control:
`c65b143f` and `c9e6cb6b`, written without a hand-added line, are `raw=1, parsed=1`.

## Environment

`experiments` at `c9e6cb6b`; hook installed 2026-09-04 13:50 via `scripts/install-hooks.sh`.
Shared checkout, four live sessions.

## Root cause

`git interpret-trailers` defines the trailer block as the **final** paragraph of the
message. `--if-exists doNothing` is therefore scoped to that paragraph, not to the message.
A hand-written `Session-Id:` followed by a blank line and then `Co-Authored-By:` produces
two trailer-shaped paragraphs; git treats only the last as *the* trailer block, finds no
`Session-Id` in it, and appends one.

The hook's own header (`scripts/prepare-commit-msg-session-id.sh:76-79`) anticipates
exactly half of this — it chose `interpret-trailers` over a plain append *because* an
append "would open a SECOND trailer block, which `git log --format='%(trailers:key=...)'`
does not read as one". The reasoning is correct and the guard it produced is scoped to the
last block, so a second block opened by **someone else's hand** reaches it from the
direction it was not looking. `cluster/guard-narrower-than-its-name`: `--if-exists
doNothing` reads as "if this trailer exists anywhere" and means "if it exists in the final
paragraph".

Measured 2026-09-06 by comparing `%B` line counts against `%(trailers:key=Session-Id)`
across the population above — not inferred from git's source, which was not read.

## 2026-09-16 — a second consumer, and the remedy text that produces it

**Same root cause, different surface, and this one is not cosmetic.** `git` parses only the
message's final paragraph as trailers — the rule this whole file is about — so a trailer placed
in an earlier paragraph is **readable prose and invisible to every query**.

Measured on `c054113b`:

```
git show -s --format='%(trailers:key=Co-Authored-Session-Id)' c054113b   -> empty
git show -s --format='%B' c054113b | grep -c Co-Authored-Session-Id      -> 2
```

Two sids present to a human reader, zero to a query. **`git log` looks correct throughout**,
which is the half that makes it expensive.

### Why this is a defect and not an author's slip

`scripts/pre-commit-foreign-index.sh` **prints the `Co-Authored-Session-Id:` trailer for the
author to paste**, and the obvious placement — grouped with the other attribution, above the
existing `Co-Authored-By` block — is the one that silently demotes it. So:

- the guard's **predicate** is right (it refuses correctly),
- the guard's **remedy text** is right (that is the trailer you need),
- the **instruction for applying the remedy is incomplete**, and nothing can catch that.

That is one step past the law `CLAUDE.md` § *Testing Discipline* already holds. It covers
whether a remedy **arrives** and whether its addressee can **answer**; it does not cover
whether following the remedy **literally produces the effect it promises**. No assertion about
who is refused, and no assertion that the message names two addressees, reaches this.

**Failure direction is the costly one.** An auditor asking *who was acked on this commit* runs
the query, gets a clean empty result, and reads it as **nobody was** — a plausible answer,
silently wrong, about exactly the attribution the trailer exists to make durable. The whole
purpose of that field is to survive as a query after every human involved has gone.

### Fix available on the guard's side, and it is cheap

Print the trailer **with its placement requirement stated**, or print it as a complete
last-paragraph snippet the author can paste whole. Either removes the judgement call; neither
edits a message the author wrote, which is what made the other option in § *Fix* above
contentious.

### Provenance, stated precisely because it was asked to be

Found by sessionId `e5691fad` — **not** by looking for it: they shipped the defect in
`c054113b`, read their own commit back, and disclosed it rather than repairing it on a shared
branch. The reusable halves are the reproduction above and the placement rule, and the useful
part of the reproduction is that `git log` reads correctly the whole way through. Verified
independently here before recording. Filed by `29420e72` at their explicit hand-off; if their
operator later authorises them to file, they will add here rather than open a second file.
## 2026-09-16 — a third instance, in the commit that files the second

**The second consumer above is a trailer its own author mis-placed, so the sids survived in prose and a human reader recovers them. This one has no attribution surface at all, and the query an auditor actually runs returns a confident wrong answer rather than an empty one.**

Measured on `7a986ac3`:

```
git show -s --format='%(trailers:key=Co-Authored-Session-Id,valueonly)' 7a986ac3  -> empty
git show -s --format='%B' 7a986ac3 | grep -c 9403d62d                           -> 0

controls, which is what makes those a measurement and not a broken query:
git show -s --format='%(trailers:key=Session-Id,valueonly)'      7a986ac3  -> 29420e72-…
git show -s --format='%(trailers:key=Co-Authored-By,valueonly)'  7a986ac3  -> Claude Opus 5 …
```

That commit carries `docs/trackers/bug-fix-session-log.md` at **32 added / 8 removed** — the exact combined delta of two sessions' uncommitted edits to one shared file: `29420e72`'s `F-168` remedy-sentence fix and `9403d62d`'s two `F-170` repairs, the latter written through `doc(action="update")` in the window between the committer's read and their `git add`.

### The query an auditor runs is `git blame`, and it answers wrongly with no hedge

```
blame on the F-170 paragraph beginning "TWO TEXT SURFACES UNDER ONE ID"
  -> 7a986ac3 -> Session-Id 29420e72…,  Co-Authored-Session-Id: (empty)

control — can blame attribute ANY line of that ledger to 9403d62d?
  -> yes: 0e7a945b, 0fc7eb5d, 2ef25326
```

The control matters: attribution through `blame` is **available in principle** on that file and **absent in this case**, so the miss is a property of the commit rather than of the instrument. An auditor asking *who wrote this* gets one session, confidently, with nothing in the commit to contradict it — which is strictly worse than the empty result the section above documents, because an empty result at least prompts a second question.

### Why no one noticed, and it is not inattention

The committer ran a check written for this exact hazard, minutes after being warned about it:

```
git diff --cached -U0 -- <ledger> | grep -oE "^\+## F-1[0-9]+|^\+\| F-1[0-9]+"
```

New entry **headings** and new **index rows**. The swept edits were neither — prose amended inside an entry that already existed. It returned empty, and *"edits only, no new entries"* was **true and was not the question**. The selector could not express the failure it was written to catch, so looking harder through it returns the same empty at any diff size. Their own general form, which is the reusable half: **a check over a shared file must be scoped to authorship of HUNKS, never to the shape of what was added** — headings and rows are a proxy for *someone added an entry*, and the proxy is silent on the commonest real edit, amending prose already there.

### What this adds to the file's own claim

The `## Root cause` above is unchanged and this instance does not widen it. What it establishes is the **cost band**: this file's original closing line read *"Cosmetic — no consumer is wrong today"*, the section above falsified that with a query returning empty, and this one moves it again to a standard tool returning the **wrong name**. Same defect, three readings, escalating.

And the circumstance is the record, not decoration: **`7a986ac3` is the commit that files the section above.** The defect appeared inside the act of documenting it, committed by an author actively writing about it — `CLAUDE.md` § *Observer Blindness*'s measured claim rather than an illustration of it. The captured session was simultaneously mid-message to the committer about that same mechanism, quoting their own bug file's root cause; knowing the class slowed nothing.

**Not amended, and that is a ruling rather than an omission.** `experiments` is shared and has moved past it; rewriting published history costs more than the defect. The durable substitute is that `F-170`'s text states its own authorship in the first person and is inside the commit — recoverable by a reader, not by a query, which is precisely the gap this file is about.

**Provenance.** Capture measured and reported by the session whose content was taken (`9403d62d`); root cause of the missed check derived and filed by the committer (`29420e72`) as a separate instance; reproduction of the trailer half by `e5691fad`. Recorded here because a captured session noticing is a **channel and not a mechanism** — it depends on the injured party checking, which nothing guarantees.

## Evidence

### git's own parser is unaffected

```
8320d5b0 raw=2 parsed=1
29c5b461 raw=2 parsed=1
c65b143f raw=1 parsed=1
c9e6cb6b raw=1 parsed=1
```

`%(trailers:key=Session-Id)` returns the single correct value in every case, including the
duplicated ones. This is what caps the severity: every mechanical consumer is right today.

## Hypotheses tried

1. **Hypothesis:** the hook runs twice (e.g. `prepare-commit-msg` firing on both the
   initial message and an amend).
   **Test:** checked whether affected commits were amended, and whether unaffected commits
   from other sessions share the pattern.
   **Verdict:** rejected — the split is by *author session*, not by commit history:
   every duplicate is a commit whose body hand-writes the line, and no commit lacking a
   hand-written line has one. **Evidence:** *git's own parser is unaffected* (the control rows).

2. **Hypothesis:** `--if-exists doNothing` is scoped to the final trailer paragraph, so an
   earlier hand-written trailer paragraph is invisible to it.
   **Verdict:** confirmed — consistent with the position of the duplicate in every affected
   body (hand-written line above a blank line above `Co-Authored-By`), and with the hook
   header's own account of why `interpret-trailers` was chosen.

## Fix
### Re-verified 2026-09-24 — three modes, one fixed, one dormant, one belongs elsewhere

Open-bug sweep (`deep-agent-workflow-observations:DWF-7`). Verifier evidence; reproductions were run in temp repos only.

- **(a) Duplicate `Session-Id` is dormant.** It still reproduces: a message carrying `Session-Id: abc` above the `Co-Authored-By` paragraph, run through `interpret-trailers --if-exists doNothing`, ends with raw=2, and the guard at `scripts/prepare-commit-msg-session-id.sh:81-85` is unchanged. But 0 of the 246 commits since 2026-09-17 carry a duplicate raw `Session-Id` line: the habit stopped, the defect did not.
- **(b) A pasted `Co-Authored-Session-Id` invisible to queries is fixed** by `05c6f153` (2026-09-16; not a merge, an ancestor of HEAD, patch-id `20ef09260e836da6f933f809b865a573970bc3db`). The hook now writes the acked sids itself with `addIfDifferent`. A hand-pasted line above the block now parses once (parsed=1); only the raw text duplicates.
- **(c) The `7a986ac3` capture with no ack** is invisible to this hook by construction. It belongs to `e421be689a23ae2a` / `04e069162e1fb15f`.

**Guidance drift to fix alongside:** `scripts/pre-commit-foreign-index.sh:355-358` and `:466-475` still tell the committer to record or paste these trailers, which the hook now writes itself. No `Co-Authored-Session-Id` trailer appears in any commit since 09-17, so the ack path has not been exercised in practice.

**Not fixed. Two options, and the cheaper one is not a code change.**

- **Stop hand-writing the line.** The hook has stamped it unconditionally since 2026-09-04
  13:50 (74 of 74 commits since install), so a hand-written `Session-Id:` is redundant as
  well as duplicating. This is the whole fix for the ongoing case and costs nothing.
- **Widen the guard** — have the hook strip any `^Session-Id:` outside the final block
  before calling `interpret-trailers`. Correct, but it edits a message the author wrote,
  which is a meaningfully larger behaviour than stamping an absent field, and it would be
  the second guard in this repo to grow past its name rather than the first to be replaced.

Not choosing between them here: the ongoing rate is one author's habit, and it is theirs to
change. Recorded so the next reader does not re-derive the mechanism.

## Tests added

**None.** Nothing in-tree consumes the raw duplicate, so there is no behaviour to regress:
a test would assert on message cosmetics, and would fail for every historical commit rather
than for a defect. The 48/802 measurement above is the record; re-derive it with the
command in *Symptom* if the rate matters later.

## Workarounds

Read the trailer with git's parser, never with `grep`:

```bash
git log -1 --format='%(trailers:key=Session-Id,valueonly)' <sha>
```

This returns the correct single value on duplicated and clean commits alike, and is the
form that should be used anywhere authorship is resolved programmatically.

## Resume

`N/A` for the mechanism. If someone decides to act: the choice in *Fix* is between a habit
change (free) and widening the guard at
`scripts/prepare-commit-msg-session-id.sh:81-85`.

## References

- `scripts/prepare-commit-msg-session-id.sh:76-85` — the guard and its stated reasoning.
- `docs/trackers/issue-clusters/IC-14-guard-narrower-than-its-name.md` — the class.
- `docs/trackers/observer-blindness.md` OB-20 — where the trailer's read-side gap is
  recorded; this bug is the write side and is much smaller.
