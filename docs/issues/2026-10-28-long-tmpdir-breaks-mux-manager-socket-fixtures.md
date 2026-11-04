---
id: 98379dd228578791
kind: bug
status: fixed
title: Long TMPDIR prevents mux-manager tests from binding their Unix sockets
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- testing
- lsp
closed: 2026-11-04
opened: 2026-10-28
severity: medium
unverified: 'Fixed for the family the record reports: seven lsp::manager tests (the six named plus a_mux_this_call_spawned_that_loses_the_lock_race_connects_to_the_winner) failed with XDG_RUNTIME_DIR unset and a 41-byte TMPDIR, and pass after the fix. A second family is NOT fixed: more than ten lsp::manager tests bind TMPDIR/.tmpXXXXXX/mux.sock through tempfile::tempdir(), and they fail once TMPDIR is about 87 bytes or longer (measured at 90 bytes, XDG_RUNTIME_DIR set). Linux only: the macOS and Windows paths were not run. The production fallback, a long TMPDIR with XDG_RUNTIME_DIR unset, was not changed and may fail to bind a mux socket; that was not tested.'
---

# BUG: long TMPDIR prevents mux-manager tests from binding their Unix sockets

## Summary

With TMPDIR=/var/tmp/codescout-producer-gate-yla5quaz, six mux-manager tests failed in each full-gate Rust lane. With TMPDIR=/var/tmp, the same lean binary's whole lsp::manager::tests module passed: 73 passed, 0 failed, 5 ignored. Production source was unchanged.

## Class fit

This is cluster/repro-env-diverges-from-gate-env: byte-identical tests reach different outcomes because the launcher changes its temporary-directory environment. Unlike a mux timing race, these failures occur while binding Unix sockets, before the intended mux behavior can be exercised.

## Symptom (Effect)

The failed tests were a_mux_started_by_a_third_process_in_the_gap_after_the_holder_died_is_waited_for, a_mux_this_call_spawned_is_connected_to_although_it_answers_its_init_late, a_peer_mux_that_accepts_but_never_sends_init_is_wedged_and_remembered, a_remembered_wedged_mux_is_still_tried_once_and_connects_when_it_has_come_up, a_replacement_holder_that_is_still_starting_is_not_remembered_as_a_wedged_mux and a_silent_socket_whose_holder_died_is_not_remembered_as_a_wedged_mux.

Observed errors: “path must be shorter than SUN_LEN” and, in the fake-mux subprocess, “AF_UNIX path too long”.

## Reproduction

At research worktree HEAD 0cb5ca9f48791ff81f769de48b4679d2f78a7bfa, launch ./scripts/gate.sh with a private directory under /var/tmp as TMPDIR. The exact failed directory was /var/tmp/codescout-producer-gate-yla5quaz. Both lean and default failed the six named tests.

Control: use the same compiled lean binary with TMPDIR=/var/tmp and filter lsp::manager::tests::. Observed 73 passed, 0 failed, 5 ignored.

## Environment

Linux; /tmp/codescout-real-task-20261024. An unrelated /tmp/.git marker required an alternate temporary root. That marker was left untouched.

## Root cause

src/lsp/mux/mod.rs::socket_path_for_workspace appends the socket name to mux_dir. In tests, mux_dir adds a per-process scratch directory below per_user_mux_dir. The observed full socket paths exceed the Unix bind limit. src/lsp/manager.rs::tests::silent_mux unwraps UnixListener::bind; the named peer-mux test reaches it with that derived path.

This does not establish a production runtime failure under a supported launcher.

## Evidence

Main checkout .codescout/resident-private/producer-status-pilot-20261028 contains isolated-gate.log, isolated-gate-result.json, short-temp-mux.log and short-temp-probe.json. The full failed lanes preserve actual names and bind errors; the short-path control uses the identical lean binary.

## Hypotheses tried

Shorten TMPDIR to /var/tmp: all active tests in the targeted mux module pass without source edits.

## Fix

Applied 2026-11-04. In test builds `mux_dir()` now nests its per-process scratch dir in `test_mux_base()` (`src/lsp/mux/mod.rs`). That is the per-user runtime dir when a socket path below it fits, and otherwise the same private dir rooted at `/tmp` (`short_per_user_runtime_dir` in `src/socket_discovery.rs`, built on the same `runtime_dir_from`, so the ownership checks are the same). The budget `TEST_MUX_BASE_MAX` is 37 bytes, derived from the longest path a test builds against the macOS limit of 103 bytes. Production is unchanged.

The existing test `socket_and_lock_share_a_parent_inside_the_per_user_dir` compared the parent with `per_user_mux_dir()`; it now compares with `test_mux_base()`, which keeps its intent (inside a private per-user dir, never bare `temp_dir()`).

## Tests added

Three tests in `src/lsp/mux/mod.rs`: `a_per_user_dir_with_room_for_the_longest_test_socket_path_is_kept`, `a_per_user_dir_one_byte_over_the_budget_is_not_used` and `the_budget_keeps_the_longest_test_socket_path_within_the_bind_limit`. They were red as compile errors (the symbols did not exist) and are green.

The end-to-end evidence is a run, not a test, because it needs a controlled environment: `env -u XDG_RUNTIME_DIR TMPDIR=<41-byte dir> scripts/with-slot.sh cargo test --lib lsp::` gave 244 passed and 1 failed before the existing-test update, then 245 passed and 0 failed after. Before the fix, the same environment failed seven `lsp::manager` tests with `path must be shorter than SUN_LEN` (70 passed, 7 failed).

## Workarounds

Use a short writable temporary root without repository markers above it. TMPDIR=/var/tmp passed the targeted module here.

## Resume

The reported family is fixed. Open: the `tempfile::tempdir()` family (see `unverified`). A fix would give those tests a short socket directory, for example a helper in `manager.rs` tests that builds the temp dir under the same `/tmp` base. Not done: nothing reports a failure at the TMPDIR lengths seen on this machine (26 bytes) or in the gate.

## Fix provenance

- **SHA:** `d045d7fbb` (`experiments`)
- **patch-id:** `409fa1e6c8ef5bd02e2d3137d6d2e90efbca028e`

Gate on the fix tree: `./scripts/gate.sh` exited 0 (FMT 0, CLIPPY 0, LEAN 0, DEFAULT 0; 5996 and 8858 tests passed). The first gate run failed `result_caps every_cap_constant_is_classified` on two new constants; they were classified `NOT_A_CAP` and the second run was green.

## References

- The fresh pilot and gate record exists only on the research branches, in commit `1023edc17` (`docs/research/2026-10-28-fresh-producer-status-pilot.md`); it is not on `experiments`. Read it with `git show 1023edc17:docs/research/2026-10-28-fresh-producer-status-pilot.md`.
