---
kind: bug
status: fixed
title: 'BUG: two lock races in the mux connect path: a wait ended by a freed lock can report wedged without waiting, and the lock probe can make a starting mux exit'
tags:
- cluster/unclassified
opened: 2026-10-17
closed: 2026-10-18
owner: marius
related:
- docs/issues/2026-10-17-symbols-name-lookup-on-kotlin-lsp-says-0-matches-for-a-symbol-that-exists.md
- docs/issues/2026-10-17-connect-to-a-mux-that-never-sends-its-init-message-hangs-forever.md
severity: low
unverified: 'Fix is on branch fix/mux-connect-2026-10-18, not yet on experiments. Race (1) the way the issue describes it (a third process landing between the early stop and round 1 of ONE call) was reproduced only through a test-only seam; the natural two-client stress shows the same wrong answer and the fix removes it, but is not the same interleaving. Race (3), two clients both spawning a mux, is read from the code and not reproduced or fixed.'
---

# BUG: two lock races in the mux connect path: a wait ended by a freed lock can report wedged without waiting, and the lock probe can make a starting mux exit

## Summary

Added with the wait for a peer's starting mux (`connect_to_mux`, `get_or_start_via_mux_waiting` in `src/lsp/manager.rs`): the wait ends early when the mux's ownership flock is found free, and it checks that by briefly TAKING the flock every 200 ms. Both halves had a window. Both were reproduced on 2026-10-18 and fixed (race (1) by the round logic in `get_or_start_via_mux_waiting`, race (2) by a short wait in the mux's own lock step).

## Class fit

The sanctioned `cluster/unclassified` is used; the roster was not walked class by class. What the case shows: a liveness decision made by acquiring the very lock another process is racing to acquire, so the observer changes the outcome it is observing.

## Symptom (Effect)

Measured 2026-10-18 (see Evidence): (1) a client is told "mux ... is wedged or mid-restart" at once, with no wait, when nothing was wedged; (2) a mux that was starting is refused its lock while a client's probe holds it and exits "another mux instance holds the lock".

## Reproduction

Race (2): `scripts/with-slot.sh cargo test --lib a_waiting_clients_lock_probe`. Race (1), through the seam: `scripts/with-slot.sh cargo test --lib -- in_the_gap replacement_holder lock_that_changes`. Race (1), natural two-client stress (about 150 s, ignored by default): `scripts/with-slot.sh cargo test --lib -- --ignored two_clients_waiting`.

## Environment

Linux, `fix/mux-connect-2026-10-18` on `experiments` 7a09623bd; shared machine with other agents compiling and running tests while the measurements were taken.

## Root cause

Race (2). measured 2026-10-18 on the unmodified single-try lock step (extracted unchanged as `acquire_mux_lock`): `72 of 300 mux starts were refused while only a client's probe held the lock`. Mechanism: `claim_mux_lock` takes the flock (`src/lsp/manager.rs`) and drops it at once; a mux's step 1 (`try_lock_exclusive`, `src/lsp/mux/process.rs`) was one try with no retry, so a try landing in that instant failed the whole mux start. The 72 of 300 is the rate under a probe loop of 50 probes per trial that the test uses to make the overlap likely; it says nothing about the production rate (one probe per 200 ms per waiting client).

Race (1). Mechanism: `get_or_start_via_mux_waiting` round 1 returned the wedged error whenever it found the lock held, whatever happened in round 0. After a round 0 that ended because the holder DIED, a lock held in round 1 is held by a new process, whose mux is starting. Reproduced two ways:

- Through a test-only seam (`LspManager::after_early_stop`, runs where the natural flow has no await): with the fix's one condition reverted to `if round > 0`, `a_mux_started_by_a_third_process_in_the_gap_after_the_holder_died_is_waited_for` failed with `a mux that took the lock after the first holder died is starting, not wedged: Some("mux for rust holds its lock (...) but its socket (...) is unreachable ...")`, and `a_replacement_holder_that_never_binds_is_wedged_within_the_same_wait` with `took 1.006485393s` for a 2 s wait. Reason for the seam: there is no await between `connect_to_mux` returning and round 1's claim, so a test on one thread cannot land a third process there.
- Naturally, two clients (two managers) waiting on one holder that dies: with that condition reverted, 8 of 600 clients (300 trials, 2 clients) and, on a second run, 6 of 600 were told a live mux wedged the lock within 2 s, while no mux existed (the only holder was the other client's claim of an instant). With the fix: 0 of 600, twice. Each trial is a fresh workspace; the holder dies 250 ms in, between two probes of both clients.

## Evidence

Race (2) red, extraction commit, before the fix: `assertion left == right failed: 72 of 300 mux starts were refused while only a client's probe held the lock` (`a_waiting_clients_lock_probe_never_makes_a_starting_mux_give_up`). With `MUX_LOCK_GRACE` set to 0 later the same test and `a_mux_start_waits_out_a_lock_held_for_a_moment` failed again (count not recorded).

Race (1) red lines: above. Natural stress: above; the test prints its count in the assertion.

## Hypotheses tried

1. Race (1) as the issue first described it, a third process taking the lock between the early stop and round 1 of one call, with a real second process or thread and no seam. Not reproducible that way: the two statements are adjacent with no await, so the window is a few instructions wide. What was done instead: the seam above, and the two-client stress, where the second client's claim lands inside the first's. The stress shows the same wrong answer by a different interleaving, not the one the issue named.
2. Race (2) with a real mux process instead of `acquire_mux_lock`: not tried; the test binary cannot be the mux.

## Fix

Landed as commit `dd961fe5f` on branch `fix/mux-connect-2026-10-18` (patch-id `31a7f6ee336201688434a4f0b5d8faaa980b819a`); not yet on `experiments`.

Race (2): the mux waits for its lock. `acquire_mux_lock` (`src/lsp/mux/process.rs`, step 1 of `run`) retries the flock every `MUX_LOCK_POLL` (10 ms) for `MUX_LOCK_GRACE` (1 s) before giving up with the same `another mux instance holds the lock`. The probe in `connect_to_mux` still takes the flock; this was chosen over a probe that does not take it because the same instant is also taken by the round claims (`claim_mux_lock` in rounds 0 and 1), which a probe change would not remove, and because it works on every unix. Not covered: a probe or any other holder that keeps the lock for more than 1 s. A second live mux that started at the same moment now takes up to 1 s longer to exit.

Race (1): `get_or_start_via_mux_waiting` remembers whether the last round's holder was found dead (`last_holder_gone`). A lock held in a later round after the last holder died is a new holder's: it is waited for like any peer's, within what is left of ONE wait budget (`wait_deadline`, so the 30 s wait is not served twice; a replacement that never binds is reported at the end of the same wait). Only a lock held again after a round that served the whole window with its holder alive is the wedged error, as before. At most `MUX_CONNECT_ROUNDS` (3) rounds; a lock that changes hands in every round ends in the same unreachable-mux error instead of the old `expect`.

## Tests added

`src/lsp/manager.rs`: `a_waiting_clients_lock_probe_never_makes_a_starting_mux_give_up` (race 2, 300 trials, one-sided: green is exact, red needs a collision), `a_mux_started_by_a_third_process_in_the_gap_after_the_holder_died_is_waited_for`, `a_replacement_holder_that_never_binds_is_wedged_within_the_same_wait` (2 s wait; asserted at least 2 s and under 2.6 s), `a_replacement_holder_that_dies_too_hands_the_lock_to_a_respawn`, `a_lock_that_changes_hands_in_every_round_ends_in_the_unreachable_error` (exactly 3 gaps; under 5 s), the ignored stress `two_clients_waiting_on_one_dead_holder_are_never_told_a_live_mux_wedged_it`, and the `accepted == 1` assertion added to `a_peer_mux_that_accepts_but_never_sends_init_is_wedged_and_remembered`. `src/lsp/mux/process.rs`: `a_mux_start_waits_out_a_lock_held_for_a_moment` (lock released after 300 ms; asserted at least 300 ms and under 3 s), `a_mux_start_gives_up_on_a_lock_that_stays_held_after_the_grace` (at least 1 s, under 4 s), `run_takes_its_lock_through_the_grace` (the wiring of `run`). Real clock in all of them (a paused clock jumps over real socket I/O, measured). Mutants and flake loops: see Resume.

## Workarounds

Retry the call.

## Resume

Done for races (1) and (2). Mutation probes (one per guarded site, each reverted): `MUX_LOCK_GRACE` 0 killed (2 tests), 100 ms killed, 60 s killed, `MUX_LOCK_POLL` 10 s killed, the give-up check removed killed, `run` back to a single try killed; `if round > 0` (the old condition) killed all 4 seam tests; `last_holder_gone` forced true killed 1, forced false killed 4; the window left changed to the whole wait killed 1; `MUX_CONNECT_ROUNDS` 2 killed 1, 4 killed 1 (after the first attempt SURVIVED because the test compared against the constant itself, now a literal 3); the terminal error replaced killed 1. Flake loops: the new and changed tests of this issue and of `2026-10-17-connect-to-a-mux-that-never-sends-its-init-message-hangs-forever.md` were run as one group (21 tests, real clock) 60 times, on a machine other agents were also building on: 59 of 60 passed. The failure was `a_mux_started_by_a_third_process_in_the_gap_after_the_holder_died_is_waited_for`, panicking at the seam's single claim of the lock (panic line `manager.rs:3264:22`, which maps to the `expect` after `claim_mux_lock`; the message was not kept). Candidate cause, not shown: a child forked by a concurrent test holds a copy of the probe's descriptor until it execs, which keeps the flock held that long. The seam now retries that claim for up to 2 s, and 60 of 60 passed afterwards. The ignored stress was run 4 times (the first two as a temporary version of the same test): condition reverted 8 of 600 and 6 of 600, fixed 0 of 600 twice; it was not run in a 60-loop.

Open, not started:

1. Race (3), read from the code and not reproduced: two clients both find the lock free, both spawn a mux, one mux wins the lock and the other exits (`another mux instance holds the lock`, now after the 1 s grace); the loser's call returns the spawn failure instead of connecting to the winner's mux.
2. `acquire_mux_lock` creates the lock file with `File::create`, which truncates it BEFORE the flock is tried, so a second mux start erases a live mux's PID line (OS behaviour measured with python, Rust path read; filed as `docs/issues/2026-10-18-mux-lock-file-truncated-before-its-flock-is-tried.md`).

## References

- `src/lsp/manager.rs` (`connect_to_mux`, `get_or_start_via_mux_waiting`, `claim_mux_lock`, `MUX_CONNECT_ROUNDS`, the `after_early_stop` seam)
- `src/lsp/mux/process.rs` (`acquire_mux_lock`, `run`)
