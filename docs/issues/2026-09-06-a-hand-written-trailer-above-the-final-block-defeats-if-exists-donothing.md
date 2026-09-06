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

**Cosmetic — no consumer is wrong today.** Filed because the divergence is silent and will
widen, not because anything is broken.

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
(`docs/issues/2026-09-06-a-bare-date-in-gits-since-is-filled-from-the-current-wall-clock.md`).

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
