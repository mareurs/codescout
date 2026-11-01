---
id: '333f9ed846f4f778'
kind: bug
status: open
title: 'BUG: hooks-discrimination''s "and it still offers the narrowing remedy" case failed in 2 of 6 full runs, both while another copy of the suite ran'
tags:
- cluster/unclassified
opened: 2026-11-01
owner: marius
severity: low
---

## Summary

In `tests/hooks-discrimination.sh` section 5, the case `has "and it still offers the narrowing remedy" "$mixout" "git commit -- c.txt"` failed in 2 of 6 full runs of the suite on 2026-11-01. Both failures happened while another copy of the suite ran on the same machine at the same time. The cause is not known.

## Symptom (Effect)

```
  PASS  pathspec with one path of MINE + an ack -> still refuses
  FAIL  and it still offers the narrowing remedy
        missing: git commit -- c.txt
```

The case before it passed in the same run, so the guard refused (`EXIT=1`), but its output did not contain the `git commit -- c.txt` remedy line. The failing output was not captured.

## Reproduction

Not reproduced on demand.

- Failed: `@bg_00000025` (4 other suite copies and the gate running) and `@bg_0000002b` (1 other suite copy and the gate running). Both ran from the real `tests/` path.
- Passed: 4 other full runs from the real path, two from a copy whose `scripts` is a symlink to the real one (foreground and background), and each run from a scratch copy of `scripts/`.
- The fixture alone, looped 40 times against the current scripts and 40 times against the `HEAD` scripts of `fb536ab0b`'s parent, never failed (`runs=40 fails=0` both).

## Environment

`experiments`, around `fb536ab0b`. Heavy load: `./scripts/gate.sh` was running in all six runs.

## Root cause

Unknown. Not caused by `fb536ab0b` as far as checked: the case is a pathspec commit, and `fb536ab0b`'s new guard block skips pathspec commits (asserted by section 13 case 13c2); the isolated fixture passes with and without that commit's scripts. The pattern (failures only with an overlapping suite run) suggests a resource two runs share, but none was identified. Not verified: whether the shared resource is real `usage.db` lookups, `~/.claude*/sessions`, or something else.

## Class fit

This is a test that fails intermittently under concurrent runs with no captured output, so the evidence is two FAIL lines and a timing pattern, not a mechanism. It resembles the archived 2026-09-16 record of three hooks-discrimination cases that failed once and did not reproduce.

## Hypotheses tried

- Pipe `SIGPIPE` in the `has` helper under `pipefail`: unlikely, because the refusal is 4,583 bytes, far below a pipe buffer.
- Caused by the lost-stage change: not supported, see Root cause.

## Fix

None.

## Tests added

None.

## Resume

Next step: capture the failing output. Run two copies of the suite at the same time from the real path, with the case's `$mixout` written to a file, until it fails.
