---
id: cb77dc7fea59a09b
kind: bug
status: open
title: 'BUG: a bare date in git''s --since is filled from the current wall clock, so the same command returns a different population depending on the time of day it is run'
owners:
- marius
tags:
- cluster/selector-narrower-than-its-population
- measurement
- git-workflow
- instrument-hazard
topic: measurement instruments that return a plausible number rather than an error
---

# BUG: a bare date in git's `--since` is filled from the current wall clock

## Summary

`git rev-list --since='2026-09-01'` does not mean "since the start of 2026-09-01". Git's
approxidate parser fills the fields you omit from **the current local time**, so the bare
form means *"since 2026-09-01 at the time of day you happen to be running this"*. The
selector therefore **moves continuously through the day** while the population it names
stands still. It never errors and never warns; it returns a smaller, plausible number.

Measured here at a 26% shortfall — 636 commits instead of 859.

## Symptom (Effect)

Two spellings of the same nominal instant, run seconds apart, in the same repo:

```
wall clock now: 2026-09-06T20:45:38+0300

bare date        --since='2026-09-01'                = 636
explicit midnight--since='2026-09-01 00:00:00'       = 859
explicit 20:40   --since='2026-09-01 20:40:00'       = 637
explicit 20:47   --since='2026-09-01 20:47:00'       = 636
explicit 20:52   --since='2026-09-01 20:52:00'       = 634
ISO w/ tz        --since='2026-09-01T00:00:00+03:00' = 859
```

The bare form tracks the current wall clock, not midnight. **223 commits — 26% of the
population — silently absent**, with exit code 0 and no diagnostic.

The tell that first surfaced it was an inverted ordering, which is what makes this
noticeable at all:

```
--since='2026-09-01'          = 636
--since='2026-09-01 02:01:38' = 807   <- a LATER cutoff returning MORE commits
```

A later cutoff returning more rows is impossible for a correct filter, which is the only
reason the defect was caught rather than published.

## Reproduction

At `c9e6cb6b` on `experiments`, any repo with >1 day of history:

```bash
git rev-list --count --since='<a date ≥2 days ago>' HEAD
git rev-list --count --since='<the same date> 00:00:00' HEAD
```

The two disagree by however many commits landed on that date **before the current
time of day**. Run it in the morning and the gap is small; run it at 21:00 and the gap is
nearly the whole day. Deterministic, not a race.

## Environment

git (system install), Linux, `TZ=+0300`. Not codescout-specific and not version-specific —
this is documented approxidate behaviour, reachable from any git surface that takes a
date: `rev-list`, `log`, `shortlog`, `bisect`. Reproduced from `run_command`; nothing about
the MCP layer is involved.

## Root cause

Git parses date arguments with `approxidate` (`date.c`), which is a *tolerant* parser: it
accepts partial input and completes the missing fields from `time(NULL)` rather than
rejecting the input or defaulting to a boundary. For `2026-09-01` the omitted fields are
hour/minute/second, so they come from now.

Measured 2026-09-06 at 20:45:38 local: bare `--since='2026-09-01'` returned the same count
as explicit `--since='2026-09-01 20:47:00'` (636) and not the same as `20:40:00` (637), so
the effective cutoff sat within minutes of the wall clock. That is the observation, not an
inference from the source — the source was not read.

This is `cluster/selector-narrower-than-its-population` and extends it in one specific way.
Every prior member is a selector that goes stale because the **population** changes under
it — an enumerated field list outlived by the payload it sizes, an allowlist outlived by
the files it names. Here the population is fixed and the **selector itself moves**, on a
24-hour cycle, with no author error and no edit. The class's existing note says a selector
and its population have independent rates of change; this member says the selector's rate
can be non-zero all by itself.

## Evidence

### The full measurement, one command

```
wall clock now: 2026-09-06T20:45:38+0300
bare date        --since='2026-09-01'          = 636
explicit midnight--since='2026-09-01 00:00:00' = 859
explicit 20:40   --since='2026-09-01 20:40:00' = 637
explicit 20:47   --since='2026-09-01 20:47:00' = 636
explicit 20:52   --since='2026-09-01 20:52:00' = 634
ISO w/ tz        --since='2026-09-01T00:00:00+03:00' = 859
```

### The result set's own oldest member confirms the cutoff

```
oldest COMMIT DATE inside the --since='2026-09-01' result set:
2026-09-01T20:47:37+03:00
```

The set does not begin at the start of the named day. It begins at ~20:47 on it.

### `--count` is not the culprit

```
A: --count --since='2026-09-01'           = 636
B: --since='2026-09-01' | wc -l           = 636
C: --count --since='2026-09-01 00:00:00'  = 859
D: --since='2026-09-01 00:00:00' | wc -l  = 859
```

Enumerating and counting agree with each other under both spellings, so the divergence is
in date parsing and not in `--count`, early walk termination, or `--date-order`
(`F: --date-order = 636`, unchanged).

## Hypotheses tried

1. **Hypothesis:** `--count` terminates the walk early where `| wc -l` does not.
   **Test:** ran all four combinations of `{--count, | wc -l} x {bare, explicit midnight}`.
   **Verdict:** rejected — A=B=636 and C=D=859. **Evidence:** *`--count` is not the culprit*.

2. **Hypothesis:** history is not monotone in commit date (rebases restamp), so `--since`
   stops traversing a line early at a date-inverted commit.
   **Test:** compared `--date-order` against the default; inspected the oldest commit date
   actually present in the result set.
   **Verdict:** rejected — `--date-order` returns 636, identical; and the set's oldest member
   is 20:47 on the named day, which is a *cutoff* signature, not an early-termination one.
   **Evidence:** *The result set's own oldest member confirms the cutoff*.

3. **Hypothesis:** approxidate completes omitted fields from the current time.
   **Test:** bracketed the effective cutoff against explicit times either side of the wall
   clock (20:40 / 20:47 / 20:52) and against ISO-with-offset.
   **Verdict:** confirmed — the bare form matches the current time of day to within minutes,
   and both fully-specified spellings return 859. **Evidence:** *The full measurement*.

## Fix

**None in this repo, and the population of live instances is ZERO — recorded so nobody
re-runs the sweep or credits this with urgency it does not have.**

Swept 2026-09-06 at `c9e6cb6b`:

- `src/**/*.rs` — **0** occurrences of `--since` passed to git.
- `scripts/**` — 9 occurrences of `--since`, and **every one is the script's own argparse
  option, never forwarded to git**: `probe_librarian_scope.py` and `probe_entry_read_grain.py`
  filter `usage.db` rows, `probe-caveat-density.py` takes a `YYYY-MM` era floor over bug
  files, `probe_guide_section_use.py` filters session-file mtimes, and
  `scripts/file-provenance.py` — the instrument `CLAUDE.md` and
  `docs/conventions/shared-checkout-commit-sequence.md` both name for attribution — routes
  `--since` through its own `_key()` at `scripts/file-provenance.py:330-331`.

So no shipped instrument is wrong today. The exposure is **ad-hoc measurement by a session
or a human**, which is how it was found: this session computed a duplicate-trailer ratio as
`48/632` off the bare form and was one step from publishing it. The correct denominator was
802.

The fix is therefore a **reading-surface** change, not a code change — see *Tests added* for
why a gate is the wrong instrument here.

## Tests added

**None, deliberately, and a gate would be the wrong instrument.** There is nothing in-tree
to regress: a test asserting `git rev-list --since='X'` behaves a particular way would be
asserting about git, not about codescout, and would pin behaviour we neither own nor want to
depend on. A lint forbidding bare `--since` in `scripts/` would guard a population that is
currently **empty** and whose members are all false positives (they are argparse options
with the same spelling, so the lint's first nine hits would all be wrong) — an
`IC-14 guard-narrower-than-its-name` in the making, aimed at a class this repo does not
currently instantiate.

What is warranted instead is a `docs/PROBES.md` line, because that page's stated premise is
naming the blind spot that would make you mis-trust an instrument, and this hazard is
invisible at the point of use. Raised independently by `codescout-7f` on the same grounds.

## Workarounds

Always fully specify. Either is sufficient:

```bash
git rev-list --count --since='2026-09-01 00:00:00' HEAD
git rev-list --count --since='2026-09-01T00:00:00+03:00' HEAD   # explicit offset
```

And when publishing any date-bounded count, **state the spelling you used and the instant
you ran it**, the same rule `CLAUDE.md` § *Reaching a Peer Session* already applies to peer
counts. A bare-date count is not reproducible by a reader in another hour, let alone another
timezone, and nothing in its output says so.

## Resume

`N/A` for the defect — root-caused, measured, and unexercised in-tree.

Open follow-up: add the `docs/PROBES.md` line described in *Tests added*. One row, no code.

## References

- `scripts/file-provenance.py:325-334` — the sweep's most load-bearing negative, since this
  is the attribution instrument `CLAUDE.md` names.
- `docs/trackers/issue-clusters/IC-18-selector-narrower-than-its-population.md` — the class.
- `docs/PROBES.md` — the reading surface the remedy belongs on.
- Found while reconstructing another session's pre-push command during the
  `2026-09-06-a-push-publishes-commits-their-author-was-withholding` incident; unrelated to
  that defect beyond the coincidence of measurement.

