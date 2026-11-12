---
id: fd0e9c149c74c1a8
kind: bug
status: fixed
title: 'BUG: the ignored mux coherence test asserted a contract that was removed, and in a test runner it never runs the mux'
owners:
- marius
tags:
- cluster/assertion-that-cannot-fail
- lsp
- tests
closed: 2026-11-12
opened: 2026-11-12
related:
- a73c49456056fb92
severity: low
unverified: 'The removal rests on the mutations listed in the second round of measurements (one build per row, 3 runs each; the third row combines three mutations in one build), not on a proof that the removed test detected nothing else. Not measured: a mutation caught by step 4 and by none of the 7 non-ignored `*_follows_a_write_made_outside_codescout*` tests, and a defect in the direct-fallback branch of `get_or_start` that other tests do not reach. The fallback was shown at the process level (strace) in one run, but its warning log was not read. The ''nothing runs it'' claim covers the files of this repository only: .github (hidden files included) and scripts/ were searched, tools/ was not, and a runner-side job or another repository cannot be excluded. The three ignored tests in manager.rs that were reworded were not run by me. Whether nextest archive extraction keeps the `deps` directory name was not checked. The two-client test added the same day does not detect a `disconnect` that closes every file (three unit tests in mux/protocol.rs do); the didChange version rewrite it also missed is now pinned by a unit test (commit 068973305), end-to-end effect on rust-analyzer not measured.'
---

# BUG: the ignored mux coherence test asserted a contract that was removed, and in a test runner it never runs the mux

## Summary

`lsp::mux::coherence_rust::two_agents_coherent_after_edit` is `#[ignore]`d and carries the reason "gated by CI job". Run by hand on 2026-11-12 it was red: step 4 asserted that agent B stays stale until agent A notifies, and the fix for `a73c49456056fb92` removed exactly that staleness. The test also never runs the mux: in a test runner `LspManager::get_or_start` falls back to a direct server. Nobody saw either problem because nothing runs the test.

## Class fit

An assertion that cannot fail: the test is named and commented as a mux coherence check, but with the resync in `LspClient::request` disabled it stays green, because `document_symbols` resyncs on its own, and it runs no mux at all. It differs from the class's usual member in that its stale assertion did fail, loudly, once someone ran it.

## Symptom (Effect)

`cargo test --lib lsp::mux::coherence_rust -- --ignored` failed at `src/lsp/mux/coherence_rust.rs:60`: `B should not see fresh_symbol before notification: ["original_symbol", "fresh_symbol"]`.

## Reproduction

1. On a tree before the edit of this bug's fix commit: `scripts/with-slot.sh cargo test --lib --no-default-features lsp::mux::coherence_rust -- --ignored --nocapture`.
2. The test fails at step 4 in 0.07 s.

## Environment

`experiments` at `43ec3e9e4`, rust-analyzer on PATH, Linux. The test was added on 2026-04-18 (`02a283529`, `32bee39c0`, `d75d47abe`).

## Root cause

Two separate causes.

1. Step 4 was written for the contract "a client does not re-send a file that changed on disk until told". `LspClient::request` and `document_symbols` now re-send drifted files before every request (`43c4ca9df`), so B sees the write without a notification. The test is ignored, so nothing failed when the contract changed.
2. `LspManager::get_or_start` with `mux_override = Some(true)` tries to spawn `resolve_mux_binary()`, which in a lib test is the test binary. The test binary has no `mux` subcommand, so the start fails and the manager takes its "Mux startup failed for {language} in a test runner, falling back to direct LSP" branch (`src/lsp/manager.rs`, around line 1076). A and B in the test are two separate direct rust-analyzer processes.

## Evidence

- Run 1, before the edit: red at line 60, as above.
- A temporary probe after both `get_or_start` calls printed `socket_path_for_workspace("rust", &root)` and whether it exists: the path was `/run/user/1000/cs-tmux-1134374/codescout-rust-mux-217cb55c23001bce.sock`, `exists() == false`.
- Mutation: with the resync in `LspClient::request` disabled (`if false && …`), the updated test still passed in 0.07 s. `document_symbols` calls the resync itself.
- `.github/workflows/ci.yml`: a grep for `ignored|coherence|rust-analyzer` finds comments about language-server lanes and no line that runs this test.

## Second round of measurements, 2026-11-12 (session `5f715d8f`)

Method: the lib test binary built from the tree at `e610be3a2`, copied to a directory named `deps` (see 5 below, the name matters), run with `--ignored lsp::mux::coherence_rust::two_agents_coherent_after_edit`. Mutants were built from a scratch copy of HEAD, each with a different binary hash, and run 3 times each.

1. **The test passes on the fixed code: 6 of 6 runs, 0.07 to 0.20 s** (four sampled runs, two under `strace`). Two language servers are started, not one.
2. **It never runs a mux, shown at the process level.** `strace -f -e trace=execve,exit_group` on one run: the test binary re-executes itself as `<test binary> mux --socket …` (pid 711019), that child ends with `exit_group(101)`, and then `rust-analyzer` starts (pid 711023). The same pair follows for the second manager (re-exec pid 711149 ends with 101, `rust-analyzer` pid 711187). Both language servers end with status 0 after `shutdown_all`. So for each manager the mux start is tried, fails at once (libtest does not know `--socket`), and the manager falls back to a direct server. A and B are two independent `rust-analyzer` processes. This is stronger than the socket probe above, which only showed that one path did not exist.
3. **What each step detects**, by mutation (scratch copy, 3 runs each):

| Mutant | Result |
|---|---|
| resync in `LspClient::request` removed (`if false && …`) | green 3 of 3 |
| the resync removed in `request` AND in `document_symbols` | red 3 of 3, at step 4 (`coherence_rust.rs:71`): B sees only `original_symbol` |
| `LspManager::notify_file_changed` made a no-op, plus the two `did_change` calls of step 1 deleted, plus A's `notify_file_changed` call of step 5 deleted (one build) | green 3 of 3 |

So the test detects a drift check that is missing everywhere, and nothing narrower. Step 1 and step 5 do not matter to the result: B's own `document_symbols` finds the change in step 4, and `notify_file_changed` only reaches the clients of its own manager, which in this setup is a server that B does not use. Step 5 asserts the same thing as step 4. The three mutations were run together, so each alone was not run. Under the second mutant (both resync sites removed) the 7 non-ignored tests `*_follows_a_write_made_outside_codescout*` in `src/lsp/client.rs` all fail too (7 of 7 in one run, 30 s; the Kotlin one is ignored and the two-client test has a different name, so neither ran). So the regression that step 4 detects is also detected by tests that run in every gate; I did not measure whether any mutation is caught by step 4 and by none of them.
4. **No leaked `codescout-*` file.** `ls /run/user/1000` before and after 4 runs shows no `codescout-*` name added or removed. The only change was a directory `cs-tmux-<pid>` named after the test process. It is codescout's own test mux scratch directory (`TEST_MUX_PREFIX`, `src/lsp/mux/mod.rs:75`): the test process creates it the first time it asks for a socket path, and the next test process removes those of dead processes (`sweep_dead_test_mux_dirs`); the one of run 648732 was gone when I looked later. The archived bug `2026-07-28-index-lock-tests-pollute-runtime-dir.md` rejected this test as the producer only because it "cannot account for 16 files" and left the producer unnamed; this measurement finds no `codescout-*` file from this test. It does not name the producer. (I first wrote that this directory belonged to the shell wrapper; that was wrong, and I checked it in the source before correcting it.)
5. **The fallback depends on where the binary is.** My first run copied the test binary to a plain scratch directory: the test failed in 0.04 s with `mux startup failed for rust: … Unrecognized option: 'socket' … will not fall back to a direct LSP`. `is_test_runner_exe` (`manager.rs`) classifies by location: the parent directory must be named `deps`. Copied into `target/debug/deps/` the same binary passed. So the test passes only when run from cargo's `deps` directory; a harness that runs a copied or extracted test binary elsewhere gets a hard failure, not a skip.
6. **Nothing runs it, and the wording spreads.** No file under `.github` (hidden files included), no script in `scripts/`, no nextest configuration (there is none) and no other file outside `src`, `tests`, `docs`, `tools` and `.codescout` contains `--ignored`, `--run-ignored` or `--include-ignored`, except one line in `CHANGELOG.md`. Three other ignored tests in `src/lsp/manager.rs` (`posix_write_lock_is_held_true_when_another_process_holds_it`, `get_or_start_via_mux_surfaces_wedged_error_when_flock_held_socket_absent`, `reap_holders_of_lock_kills_an_orphan_holder`) say they are "gated like" this test or like each other, so the claim of a CI gate rests on this one reason string, which no job backs. I did not read CI configuration outside this repository.
7. **What a test of the manager's mux path has today.** The only test that drives `get_or_start` to a mux that exists is `get_or_start_mux_success_records_lsp_event` (`manager.rs`), ignored, for Kotlin, and it starts a real `codescout` binary first and skips silently when `target/debug/codescout` is absent. The two mux tests in `src/lsp/client.rs` use an in-process mux and a socket client and do not go through `LspManager`.

## Hypotheses tried

That the 0.07 s run time meant a warm shared mux: refuted by the socket probe. That the test was red before 43c4ca9df too: not checked.

## Fix

In `src/lsp/mux/coherence_rust.rs`, step 4 now asserts the current contract (B sees `fresh_symbol` with no notification), and its comment says the test is not a mux test and why. The mux transport is covered by a new test in `src/lsp/client.rs`, `hover_follows_a_write_made_outside_codescout_through_the_mux`, which runs `mux::process::run` in the test process and connects `LspClient::connect` to its socket. Mutation: red with the resync disabled, green with it.

Added later the same day: `two_clients_on_one_mux_follow_a_write_made_outside_codescout` (two clients on one in-process mux).

Then, after the second round of measurements above and the user's approval of the recommendation, the test was removed: `fb72a59bd` deletes `src/lsp/mux/coherence_rust.rs`, `src/lsp/mux/test_support.rs` (its only user), the fixture `tests/fixtures/lsp-mux/rust/` and the two `mod` lines. The three ignored tests in `src/lsp/manager.rs` that said they were "gated like" it now say no CI job or gate lane runs them, and a stale comment in `src/lsp/client.rs` that pointed at the removed file now names the mux tests that file has.

## Fix provenance

- **SHA:** `b398b53d2` (`experiments`): step 4 asserts the current contract, and the mux tests in `client.rs`
- **patch-id:** `20afb87a74c8`
- **SHA:** `fb72a59bd` (`experiments`): removal of the test, its harness and fixture
- **patch-id:** `df8b958bcee9` (`git show fb72a59bd | git patch-id --stable`)

## Tests added

`hover_follows_a_write_made_outside_codescout_through_the_mux` (one client, one mux) and `two_clients_on_one_mux_follow_a_write_made_outside_codescout`. See `a73c49456056fb92` for their mutation and load results. The removed test left no replacement: the mutations I ran show that what it detected is detected by tests that run in every gate.

## Workarounds

None needed.

## Resume

Fixed by `fb72a59bd` (see `## Fix provenance`). Archive after the owner confirms, with the fix verified on `experiments`: the gate was green at that tree (FMT, CLIPPY, LEAN 9682 passed, DEFAULT 9776 passed, Python 1836 passed).

Open, none blocking: no test that runs in every gate drives `LspManager::get_or_start` to a mux that exists. The only one, `get_or_start_mux_success_records_lsp_event` in `src/lsp/manager.rs`, is ignored, is for Kotlin, and starts a real `codescout` binary that it silently skips without. Whether that gap needs a test is the owner's decision; two patterns exist (an in-process `mux::process::run`, which skips `LspManager`, or a pre-started real binary). The removal rests on the mutations listed in the second round of measurements, not on a proof that the removed test detects nothing else.

## References

- `src/lsp/mux/coherence_rust.rs` (removed in `fb72a59bd`)
- `src/lsp/manager.rs`: `get_or_start`, `resolve_mux_binary`
- `src/lsp/client.rs`: `hover_follows_a_write_made_outside_codescout_through_the_mux`
- `a73c49456056fb92`
