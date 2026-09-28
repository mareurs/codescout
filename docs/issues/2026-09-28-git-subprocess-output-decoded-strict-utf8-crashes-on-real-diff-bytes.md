---
id: b4b7a133612c0236
kind: bug
status: fixed
title: git subprocess output decoded as strict UTF-8 crashes on a real commit's non-UTF-8 diff bytes
owners:
- marius
tags:
- cluster/unclassified
- measure
- encoding
- subprocess
topic: scripts/measure -- Stage 2 correction miner
time_scope: '2026-09-28'
---

## Summary

`scripts/measure/miner.py`'s two git subprocess helpers (`_git()`, used by
`_commit_body`/`_commit_ts_of`/`_parents_of`/`_commit_diff`; and `_blame_shas()`)
called `subprocess.run(..., text=True, check=True)` with no `errors=` argument,
so stdout was decoded under Python's default STRICT UTF-8 text mode. A real
commit in this repo's own history has a diff containing a byte that is not
valid UTF-8, and decoding it raised `UnicodeDecodeError`, which propagates as
an uncaught crash out of `miner.candidates()`.

## Symptom (Effect)

`miner.candidates(events_db, repos, stats)` raised `UnicodeDecodeError` when
run over this repo's real git history (discovered while performing Task 8's
R94(a) real-data cross-reference, not during the unit-test suite, since no
fixture commit happens to contain a non-UTF-8 byte).

## Reproduction

Run `miner.candidates()` (or just `_git(repo, "diff", "-U0", "--no-color",
"--no-renames", "<parent>", "176015f214595deb9c9ec99dbfd2321284bae345")`)
against this repo. Commit `176015f214595deb9c9ec99dbfd2321284bae345`
("feat(run_command): compact short cargo test output; fix the stderr tail")
has a diff carrying byte `0x93` (a Windows-1252-style smart quote) at byte
offset 32879 of the diff text -- not valid UTF-8. Under the pre-fix code
(`subprocess.run(..., text=True, check=True)`, no `errors=`), this raises
`UnicodeDecodeError: 'utf-8' codec can't decode byte 0x93 in position 32879:
invalid start byte`.

## Environment

Python 3.13.11 (`~/work/claude/prompt-engineering/.venv/bin/python`),
`scripts/measure/miner.py`, this repo's own git history as the corpus.

## Root cause

`subprocess.run(text=True)` with no `errors=` argument defaults to strict
decoding under the locale's preferred encoding (UTF-8 here). Git commit
diffs and blame output are NOT guaranteed to be valid UTF-8 -- a committer's
system encoding, or a diffed file's own byte content, can carry arbitrary
bytes. The code implicitly assumed every git subprocess output would decode
cleanly, and that assumption was false for at least one real commit in this
repo's own corpus.

## Evidence

- `.superpowers/sdd/2026-09-26-system1-base-rate-measurement/probes/task8-mutants.txt`
  (header) and `task8-green.txt` (SUPPLEMENT section) record the crash, the
  fix, and the full post-fix mutation re-run.
- Diagnostic script (session scratchpad, not committed) isolated the exact
  sha and byte offset by decode-testing every examined commit's diff/body
  bytes independently of `miner.py`.

## Hypotheses tried

None other than the immediate fix -- the failure was reproduced once,
directly attributable to a single `subprocess.run` call missing `errors=`,
with no ambiguity about the cause.

## Fix

Added `errors="replace"` to both `subprocess.run(...)` calls in `_git()` and
`_blame_shas()`. Undecodable bytes become U+FFFD; every regex match used
elsewhere on the resulting text (subject/body/added-line markers, trailer
lines, hunk headers, blame sha prefixes) still works on everything else in
the text. The failure mode becomes a false-negative on the exact
undecodable byte span (that one span can never match a marker/selector
regex), never a crash.

Fixed in commit `0931fe009f19633a9276846e55b4c8724be4fab6` (patch-id
`86725027e3a4b34ddafbbdcd8ec99878355f1de0`, via `git show <sha> | git
patch-id --stable`), on branch `experiments`.

## Tests added

`tests/test_measure_miner.py::NonUtf8GitOutputDoesNotRaise`, 2 cases:
- `test_a_diff_containing_an_invalid_utf8_byte_does_not_raise` (exercises
  `_git`'s code path, a commit diff);
- `test_blame_porcelain_output_with_an_invalid_utf8_summary_line_does_not_raise`
  (exercises `_blame_shas`'s code path, a `git blame --porcelain` summary
  line).

Both write raw non-UTF-8 bytes to a fixture file via a `_write_bytes` test
helper and assert `candidates()`/`_blame_shas()` returns without raising.
Full suite (21 tests, including these 2) passes: 21 passed in 1.47s. Both
regression cases were also independently exercised as a side effect of
Step 5's `M6_swap_self_peer_session` mutant (3 failures instead of the
other mutants' 1, one of them being the new UTF-8 regression test), showing
they are not vacuous against at least one unrelated mutation too.

## Workarounds

None needed -- fixed before being shipped in the same commit that
introduces `miner.py`.

## Resume

N/A -- fixed, not left open.

## References

- `scripts/measure/miner.py` (`_git`, `_blame_shas`)
- `tests/test_measure_miner.py` (`NonUtf8GitOutputDoesNotRaise`)
- `.superpowers/sdd/2026-09-26-system1-base-rate-measurement/probes/task8-mutants.txt`
- `.superpowers/sdd/2026-09-26-system1-base-rate-measurement/probes/task8-green.txt`
- Commit `176015f214595deb9c9ec99dbfd2321284bae345` (the real commit whose diff
  exposed the bug)
