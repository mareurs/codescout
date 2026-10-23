---
id: e12c30b5e0159852
kind: bug
status: fixed
title: capture-feature tests assume a bash sh and GNU seq, and fail on the Ubuntu and macOS CI runners
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
closed: 2026-10-23
opened: 2026-10-23
severity: medium
unverified: TRACKED 99b56275f0439c09 — the Windows lib tests do not build, so these four tests have not run on Windows.
---

# BUG: capture-feature tests assume a bash `sh` and GNU `seq`, and fail on the Ubuntu and macOS CI runners

## Summary
Four tests added with the IL-3 run-to-completion capture fail on CI run 39915517190 (head `5018dd327`). Two fail on Ubuntu (dash) and on macOS. Two more fail only on macOS (BSD tools). The two Ubuntu failures turned `Test (ubuntu-latest / no-features)` and `Test (ubuntu-latest / local-embed)` red; both were green on the baseline run for `b1fde55c0`. No evidence so far that the capture itself is wrong on these platforms.

## Class fit
The tests pass on the developer machine, where `sh` accepts bash process substitution and `seq` is the GNU one. They fail on the gating runners, where Ubuntu `sh` is dash and macOS ships BSD tools. The reproduction environment diverged from the gate environment.

## Symptom (Effect)
Ubuntu, both jobs (`gh run view 39915517190 --job 120288474086 --log`):

```
tools::command_summary::tests::tee_splice_point_decides_from_the_depth_zero_pipes
  panicked at src/tools/command_summary.rs:3196:13: diff <(seq 3 | sort) <(seq 3) | head -1
  left: None   right: Some(30)
tools::run_command::tests::a_pipe_nested_in_a_group_or_substitution_runs_as_it_does_bare
  panicked at src/tools/run_command/tests.rs:8082:9: `diff <(seq 3 | sort) <(seq 3) | head -1`:
  {"exit_code":2,"stderr":"sh: 1: Syntax error: \"(\" unexpected\n"}
```

macOS `Test (macos-latest / no-features)` fails the same two tests and two more:

```
a_whole_stream_filter_sees_every_line_while_its_capture_is_bounded  (tests.rs:7496)
  the filter sees the whole stream: {"exit_code":0,"stdout":"3e+06\n", ... "producer_exit_code":0}
the_spliced_capture_file_is_bounded_while_the_filter_reads_the_whole_stream  (tests.rs:7563)
  the filter counts the WHOLE stream   left: "1274457"
```

Windows: the lib test target does not compile, on the head run and on the baseline run alike (4 errors E0425: `MUX_INIT_READ_MIN`, `MUX_SPAWNED_CONNECT_WINDOW`, `MUX_CONNECT_RETRY_INTERVAL`, `MUX_CONNECT_ROUNDS`, head run). No capture test has run on Windows.

## Reproduction
Run `cargo test --workspace --no-default-features tools::run_command::tests::a_pipe_nested_in_a_group_or_substitution_runs_as_it_does_bare` on a host where `/bin/sh` is dash (Debian, Ubuntu). Not run locally this session: CI logs only.

## Environment
GitHub runners `ubuntu-latest` (dash as `sh`), `macos-latest`. Branch `experiments`, head `5018dd327`.

## Root cause
Ubuntu: measured 2026-10-23 from the CI log. `sh: 1: Syntax error: "(" unexpected` is dash rejecting `<( )`; the test commands use that bash feature. The first test returns `None` where it expects `Some(30)`: inferred, not read, that the splice step refuses a command that does not parse, as the refusal text says.

macOS, `3e+06`: measured from the CI log and the test source. The command is `seq 1 3000000 | tail -1` and holds no `awk` (the first version of this file said `awk`; that was wrong). BSD `seq` printed 3000000 in `%g` form. macOS, `1274457`: the command is `seq 1 3000000 | grep -c 7`. The same exponent form from 1000000 up changes which lines hold a `7`. That fits the log; it was not run on macOS. macOS cause for the two `<( )` tests: not read, unverified.

## Evidence
`gh run view 39915517190 --json jobs` against `gh run view 39574468685` (baseline, `b1fde55c0`): the two Ubuntu test jobs are the only jobs that went from success to failure. Every other failing job is red on both runs.

## Hypotheses tried
1. **Hypothesis:** the capture splice itself misbehaves on dash. **Test:** read the panic text. **Verdict:** rejected for the two Ubuntu failures; the shell rejects the test's own command before any splice runs.

## Fix
Written 2026-10-23. Two changes, tests only; no production code moved.

**Fix commit:** `2acc553ad` on `experiments` (patch-id `c6ce785053e3df5b9d4ad73209d4a56fa22637f8`, from `git show 2acc553ad | git patch-id --stable`; the commit also carries the first version of this file). Pushed 2026-10-23.

- **The two `seq` tests** (`src/tools/run_command/tests.rs`, `a_whole_stream_filter_sees_every_line_while_its_capture_is_bounded` and `the_spliced_capture_file_is_bounded_while_the_filter_reads_the_whole_stream`): `seq 1 3000000` became `seq -f %.0f 1 3000000`. GNU and BSD `seq` both print plain integers under that format.
- **The two `<( )` tests** (`tee_splice_point_decides_from_the_depth_zero_pipes` in `src/tools/command_summary.rs`, `a_pipe_nested_in_a_group_or_substitution_runs_as_it_does_bare` in `src/tools/run_command/tests.rs`): the `diff <(seq 3 | sort) <(seq 3) | head -1` row left its table and became an explicit block that takes the host `sh` as the oracle. A `sh` that parses `<( )` must splice (first test) and run the command with exit 0 (second test). A `sh` that rejects it must leave the command untouched (first test) and fail it the way the bare command fails: a nonzero exit and empty stdout (second test). Neither shell world is skipped.

## Fix provenance

- **SHA:** `2acc553adadb126f88518f0dff7bff8583b16715` (`experiments`)
- **patch-id:** `c6ce785053e3df5b9d4ad73209d4a56fa22637f8`

## Tests added
No new test. The four existing tests changed as under Fix. CI verification, run 39931340086 on `2acc553ad`: `Test (ubuntu-latest / no-features)` and `Test (ubuntu-latest / local-embed)` are green again (red on 39915517190). The `Test (macos-latest / no-features)` failure list lost all four capture tests (`tee_splice_point_decides_from_the_depth_zero_pipes`, `a_pipe_nested_in_a_group_or_substitution_runs_as_it_does_bare`, `a_whole_stream_filter_sees_every_line_while_its_capture_is_bounded`, `the_spliced_capture_file_is_bounded_while_the_filter_reads_the_whole_stream`); its other 19 failures were all on the earlier list. The set of red jobs on the new run is the baseline set (12 jobs). The two `seq` fixes are therefore confirmed on macOS by that run, which also settles the macOS `<( )` question that the root-cause section left unread. Local evidence before the push: the four tests pass under bash `sh` and under the `ubuntu:24.04` dash; the pre-fix tests fail under that dash with CI's own panic lines (`command_summary.rs:3196`, `tests.rs:8082`). Windows: not run, see the unverified field. Measured 2026-10-23, `scripts/with-slot.sh cargo test --lib -- <the four names>`: 4 passed under the local `sh` (bash). 4 passed again with the `sh` of `ubuntu:24.04` (dash copied out of the image, first on `PATH`). Red control: the pre-fix tests at `5018dd327` under the same dash shim fail the two `<( )` tests at the same lines and with the same text as CI (`command_summary.rs:3196` `left: None right: Some(30)`; `tests.rs:8082` `sh: 1: Syntax error: "(" unexpected`). Not run on macOS: the two `seq` tests are unverified there.

## Workarounds
None needed for users, as far as read: the failing tests fail at the test's own command line. Whether production `run_command` on a dash host splices correctly was not run here.

## Resume
N/A: fixed and verified on `experiments`.

## References
CI runs 39915517190 (head) and 39574468685 (baseline `b1fde55c0`). Design: `docs/superpowers/specs/2026-10-21-il3-run-to-completion-pipe-design.md`. Audit log note for the P-8 window: A-41 in `docs/trackers/prompt-hamsa-audit-log.md`.
