---
id: fd0e9c149c74c1a8
kind: bug
status: mitigated
title: 'BUG: the ignored mux coherence test asserted a contract that was removed, and in a test runner it never runs the mux'
owners:
- marius
tags:
- cluster/assertion-that-cannot-fail
- lsp
- tests
opened: 2026-11-12
related:
- a73c49456056fb92
severity: low
unverified: The `#[ignore]` reason names a CI job and no workflow under .github was found that runs it (grep of the .yml files only). The 'no mux socket' probe computed the socket path from the fixture path; whether the manager hashes a canonicalised root was not checked. The fallback branch was read in manager.rs and the 0.07 s run time fits it, but its warning log was not read in that run. The two-client test added later the same day does not detect two mux mutations I tried (see a73c49456056fb92).
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

## Hypotheses tried

That the 0.07 s run time meant a warm shared mux: refuted by the socket probe. That the test was red before 43c4ca9df too: not checked.

## Fix

In `src/lsp/mux/coherence_rust.rs`, step 4 now asserts the current contract (B sees `fresh_symbol` with no notification), and its comment says the test is not a mux test and why. The mux transport is covered by a new test in `src/lsp/client.rs`, `hover_follows_a_write_made_outside_codescout_through_the_mux`, which runs `mux::process::run` in the test process and connects `LspClient::connect` to its socket. Mutation: red with the resync disabled, green with it.

Added later the same day: `two_clients_on_one_mux_follow_a_write_made_outside_codescout` (two clients on one in-process mux). Not fixed: the `#[ignore]` reason, and `coherence_rust.rs` itself is still not a mux test.

## Fix provenance

- **SHA:** `b398b53d2` (`experiments`)
- **patch-id:** `20afb87a74c8`

## Tests added

`hover_follows_a_write_made_outside_codescout_through_the_mux` (one client, one mux). See `a73c49456056fb92` for its mutation and load results.

## Workarounds

None needed. Do not read `two_agents_coherent_after_edit` as evidence about the mux.

## Resume

Decide with the owner whether `coherence_rust.rs` should be removed or rewritten as a real mux test now that `two_clients_on_one_mux_follow_a_write_made_outside_codescout` exists in `src/lsp/client.rs`. Until then it is an ignored test that covers `document_symbols` after an outside write on a direct server.

## References

- `src/lsp/mux/coherence_rust.rs`
- `src/lsp/manager.rs`: `get_or_start`, `resolve_mux_binary`
- `src/lsp/client.rs`: `hover_follows_a_write_made_outside_codescout_through_the_mux`
- `a73c49456056fb92`
