---
id: 6974efe45c7f28b7
kind: bug
status: fixed
title: 'BUG: hooks-discrimination''s "and it still offers the narrowing remedy" case failed in 2 of 6 full runs, both while another copy of the suite ran'
tags:
- cluster/unclassified
closed: 2026-11-01
opened: 2026-11-01
owner: marius
severity: low
---

## Summary

In `tests/hooks-discrimination.sh` section 5, the case `has "and it still offers the narrowing remedy" "$mixout" "git commit -- c.txt"` failed in 2 of 6 full runs of the suite on 2026-11-01. Both failures happened while another copy of the suite ran on the same machine at the same time. The cause is a race in the `has` test helper under `pipefail` (see Root cause), fixed in `d0ef9e995`.

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

The `has` and `hasnt` test helpers, not the hook. Both were `printf '%s' "$2" | grep -qF "$3"` under `set -o pipefail`.

- Bash's `printf` writes this 4,583-byte refusal in two `write()` calls, 4,096 bytes then the rest (measured with `strace` on a 4,627-byte string: `= 4096`, `= 531`).
- `grep -q` exits as soon as it matches. The needle is on line 20, inside the first chunk.
- If `grep` exits before the second write, `printf` dies of `SIGPIPE` (141), and `pipefail` makes the pipeline fail although `grep` found the text.

The proof that the check itself was nondeterministic: in an instrumented copy, `has` failed and an identical `printf | grep -qF` check on the same `$mixout`, run on the next line, found the text (2 of 3 overlapping runs, no saved output). A deterministic harness (write 4,096 bytes, sleep 0.3 s, write the rest) gives `PIPESTATUS=141 0`. The race needs the writer to be descheduled between its two writes, which is why it showed only under load: 0 failures in 12,000 tight-loop tries on a quiet machine.

`hasnt` has the reverse and worse failure: with the text present, the 141 sends it down its `|| ok` branch, so it PASSES. The deterministic harness shows that too.

`fb536ab0b` did not cause it. It made the refusal neither longer nor different for this case.

## Class fit

This is a test that fails intermittently under concurrent runs with no captured output, so the evidence is two FAIL lines and a timing pattern, not a mechanism. It resembles the archived 2026-09-16 record of three hooks-discrimination cases that failed once and did not reproduce.

## Hypotheses tried

- Pipe `SIGPIPE` in `has`: first set aside because the refusal is far below a pipe buffer. That reasoning was wrong: the buffer size does not matter, the chunking and the reader's early exit do. Confirmed (see Root cause).
- Caused by the lost-stage change: not supported. The case is a pathspec commit, which that change skips, and its fixture alone passed 40 of 40 with and without it.
- A resource shared between overlapping suite runs: not needed. Overlap only supplied the load.

## Fix

`d0ef9e995`: `has` and `hasnt` match with `[[ "$2" == *"$3"* ]]`, which has no pipe and so no race. Applied to the nine test suites that set `pipefail` and define these helpers: `commit-mine.sh`, `hold-publish.sh`, `hooks-discrimination.sh`, `install-hooks-check-population.sh`, `mutation-probe.sh`, `pre-commit-ledger-divergence.sh`, `pre-commit-ledger-growth.sh`, `pre-push-foreign-session-guard.sh`, `probe-hook-replay.sh`. The six suites with the same helpers and no `pipefail` (`attribute-red.sh`, `count-with-members.sh`, `file-provenance.sh`, `fmt-mine.sh`, `git-safe-reset.sh`, `rb-guard.sh`) take `grep`'s status alone and cannot race, so they were left as they are. A comment above the helpers in `hooks-discrimination.sh` says why.

## Tests added

No new test: the fix removes the pipe, so the race has no place to happen. Evidence:

- All nine changed suites pass (`exit=0` each), so the stricter `hasnt` revealed no assertion that had been passing falsely.
- Three overlapping runs of `hooks-discrimination.sh` after the fix: 422 of 422 each. That run was lighter (1 m 53 s) than the ones that failed (3 m 21 s), so it is weak evidence alone.
- Gate `PYTHON_LIGHT=0 FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Fix provenance

- **SHA:** `d0ef9e995` (`experiments`)
- **patch-id:** `eb542fd923d78edc5e1524d5f5f85be1420e2a8e`

## Resume

Fixed in `d0ef9e995`. Nothing left to do here.
