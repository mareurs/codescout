---
id: 88f38bdfcecead01
kind: bug
status: fixed
title: 'BUG: a cold rust-analyzer answers documentSymbol with null for 53 s on this repo, so references fails after the 5 s cold-start budget; the nested linked worktrees are what make the window long'
owners:
- marius
tags:
- cluster/lazy-warmup-bills-the-first-caller
closed: 2026-10-24
opened: 2026-10-23
related:
- docs/issues/archive/2026-08-27-references-symbol-not-found-while-lsp-warms.md
severity: medium
unverified: 'STANDING no run on real rust-analyzer has shown a window above 5 s failing before the fix and passing after it: the live test does not discriminate because the window varies from 4.0 s to 15.3 s between runs, and the evidence for the progress wait is a fake server with a fixed 7 s null window. The progress stream reaching a client through the mux is read from the code, not exercised end to end. The 40 s ceiling is a choice, not a measurement; a window above it still ends in the null error.'
---

# BUG: a cold rust-analyzer answers documentSymbol with null for 53 s on this repo, so references fails after the 5 s cold-start budget; the nested linked worktrees are what make the window long

## Summary

On a cold rust-analyzer, `references` (and anything else that calls `LspClient::document_symbols`) fails with "answered textDocument/documentSymbol with null … on every attempt for 5000 ms". The 5 s budget is shorter than the real null window on this repo, which measured 53.5 s. About 276,000 files of linked worktrees sit inside the repo root, and the scan of them is what stretches the window.

## Class fit

Work deferred to first use is charged to the first caller, who cannot tell "cold" from "broken": here the first `references` call after a cold start pays a rust-analyzer workspace scan. It differs from the class's other members because the caller is told honestly ("the server has not answered"), so the result is not a false negative. The defect is that the wait ceiling is far below the real warm-up time.

## Symptom (Effect)

Verbatim, from `references(symbol="per_user_runtime_dir", path="src/socket_discovery.rs")`, 2026-10-23:

```
{
  "ok": false,
  "error": "the language server answered textDocument/documentSymbol with null for /home/marius/work/claude/codescout/src/socket_discovery.rs on every attempt for 5000 ms",
  "hint": "null means the server has not answered, not that the file has no symbols — rust-analyzer sends it while it reloads its workspace. Retry shortly."
}
```

The call took 5243 ms (`tool_done tool=references duration_ms=5243`). The same call, repeated 104 s after rust-analyzer started, returned 9 references.

## Reproduction

Two levels.

1. Through codescout: start a session whose rust-analyzer is cold (no rust mux for this root), then call `references` on any symbol in the first seconds. Observed once, 2026-10-23 10:07:35Z to 10:07:41Z, instance `88ca`.
2. Without codescout, repeatable: `ra_null_window.py` speaks LSP to rust-analyzer 1.97.1 over stdio, sends `didOpen` for `src/socket_discovery.rs`, then polls `documentSymbol` every 200 ms (codescout's retry interval) and logs the answer kind and `$/progress`. The script was kept in the session scratchpad only, not in the repo.

## Environment

Linux 7.2.8-zen, 64 cores, load average about 9, 47 GB available. rust-analyzer 1.97.1 (pinned by `rust-toolchain.toml`), reached through codescout's mux. Branch `experiments`, HEAD `2acc553ad`. codescout passes no `initializationOptions` to rust-analyzer (no `initialization_options` or `targetDir` match in `src/lsp`).

## Root cause

Mechanism, measured 2026-10-23 with `ra_null_window.py`, one run per condition:

- rust-analyzer answers `documentSymbol` correctly at 0.02 s, turns `null` at 0.62 s, and stays `null` until 54.1 s. That is a 53.5 s window.
- The window matches two "Roots Scanned 0/451" scans: 0.52 s to 36.66 s, then 39.87 s to 53.51 s. Symbols return at 54.10 s, right after the second scan ends.
- `LspClient::not_answered_budget` (`src/lsp/client.rs:1287-1294`) allows 5 s inside the cold-start window (`in_cold_start_window`, 5 minutes, `src/lsp/client.rs:788-792`). `request_until_answered` (`src/lsp/client.rs:1307-1326`) then returns `None` and `document_symbols` raises the error above. So the code does what it was written to do. The budget is the defect.
- Why the window is long: the repo root holds 233,056 files in `.claude/worktrees` (55 dirs) and 42,696 in `.worktrees` (4 dirs), against 14,909 files in the main tree (measured by `find`, 2026-10-23). With those two directories excluded through `initializationOptions`, the same probe had two short windows (0.64 s to 5.06 s and 5.26 s to 8.26 s) and symbols returned for good at 8.26 s. The root scans took about 4.1 s and 3.1 s, against 36 s and 13.6 s.

Replicated 2026-10-23 (two more runs per condition, run one at a time, load average 9 to 14): default options returned symbols at 53.9 s and 33.6 s, so the default window is 33 to 54 s in three runs. With the worktrees excluded, symbols returned at 7.3 s and 7.4 s after one unbroken `null` window of about 6.5 s each. So excluding the worktrees shortens the window by 5 to 7 times, but it stays ABOVE the 5 s budget in two of three runs.

Not established: why the archived measurement (2026-09-24) saw 0.4 s to 0.7 s windows on the same 451 roots. The worktree count then is unknown, and machine load differs.

## Evidence

Probe run 2 (default options), answer kind changes:

```
{"ev": "answer_kind_changed", "kind": "symbols:5", "t": 0.021}
{"ev": "answer_kind_changed", "kind": "null", "t": 0.623}
{"ev": "answer_kind_changed", "kind": "symbols:5", "t": 54.103}
```

Probe run 3 (`files.excludeDirs` = `.claude/worktrees`, `.worktrees`):

```
{"ev": "answer_kind_changed", "kind": "symbols:5", "t": 0.038}
{"ev": "answer_kind_changed", "kind": "null", "t": 0.639}
{"ev": "answer_kind_changed", "kind": "symbols:5", "t": 5.055}
{"ev": "answer_kind_changed", "kind": "null", "t": 5.255}
{"ev": "answer_kind_changed", "kind": "symbols:5", "t": 8.261}
```

`.codescout/debug.log`, instance `88ca`: the rust mux and rust-analyzer started at 10:07:34Z (`ps` start time 13:07:34 local); the failing `references` call ran 10:07:35.811Z to 10:07:41.055Z with about 26 `documentSymbol` replies of 4 bytes (`null`); the same call at 10:09:18Z got a 1613-byte reply at once.

## Hypotheses tried

1. **Hypothesis:** the file was not open in rust-analyzer (mux dedup of `didOpen`). **Test:** the preceding `symbols` call on the same file got a 1613-byte reply 2 ms after its request. **Verdict:** rejected for this event.
2. **Hypothesis:** a crate-graph swap of 0.2 s to 0.7 s (the archived mechanism) caused it. **Test:** probe run 2 timed the window. **Verdict:** rejected as the size of the window; the swap exists but lasts 53 s here.
3. **Hypothesis:** the nested worktrees lengthen the scan. **Test:** probe run 3 with both directories excluded. **Verdict:** supported, replicated (three runs per condition): the exclusion cuts the window from 33 to 54 s to about 7 s. It is a contributing cause, not the whole cause, because about 7 s still exceeds the 5 s budget.

## Fix

Applied 2026-10-24 in three commits (below; the three steps are described in this section). The replicates showed that excluding the worktrees alone does not clear the 5 s budget (about 6.5 s unbroken `null` in two of three runs), so the first option needs the second or a longer budget beside it. Options, none chosen:

- Pass `files.excludeDirs` for linked worktrees that sit inside the project root as `initializationOptions` (codescout already lists them in its "Reads are resolving against" warning).
- Raise or replace the 5 s cold-start budget, for example by waiting on rust-analyzer's `Roots Scanned` progress instead of a fixed time.
- Make the hint say that a repo with many nested worktrees can take about a minute.

**Assessment 2026-10-24 (read from the code; nothing built).** Both real options reach the same two places:

- There are TWO `initialize` sites: `LspClient::initialize` (`src/lsp/client.rs:980`) and the mux handshake (`src/lsp/mux/process.rs:168`). The mux one is a hand-written JSON literal with no `initializationOptions` and no `window.workDoneProgress` capability. An `excludeDirs` option set at one site only would work in one mode only.
- Nothing in `src/` reads `$/progress` or `window/workDoneProgress/create`, and nothing sends `initializationOptions` (grep over `src`, 2026-10-24). So "wait on `Roots Scanned`" is a new subsystem: advertise the capability at both sites, parse progress in the reader, and forward it through the mux to every connected client.
- Smallest change that would clear the measured 6.5 s window: `files.excludeDirs` at both sites plus a cold-start budget above 8.3 s (the measured time to first symbols with the exclusion). The budget is a guess without the progress signal. A budget of about 15 s trades a failed first call for a call that blocks up to 15 s.

Recommended order, confidence medium: (1) the hint change now (no protocol change, safe); (2) `excludeDirs` at both sites with a live-server test per mode; (3) the progress wait only if (2) leaves failures. This needs a plan and live rust-analyzer tests, so it is not started in the session that filed the bug.


**Step 2 applied 2026-10-24 (see the commit that cites this file): partial, measured.** New `src/lsp/init_options.rs` sends rust-analyzer `initializationOptions` `files.excludeDirs` at BOTH `initialize` sites (`LspClient::start`, and the mux handshake in `src/lsp/mux/process.rs`). Other servers get none. Wire tests record the `initialize` params a fake server receives, for both sites; with the options dropped at either site the matching test fails with `left: Null`.

The first version excluded each registered nested worktree (`list_git_worktrees`; 43 of 50 once the mutation-probe slots it skips are removed). Measured with the probe on a cold rust-analyzer, same file, no other load from this session: the null window was still **31.3 s** (0.893 s to 32.198 s), no better than the default (33.6 to 54.1 s). Cause, from the directory contents and not re-measured per directory: `.claude/worktrees` and `.worktrees` hold trees that are not registered, and the skipped `mutation-slot-N` trees, and those are most of the files.

The rule was changed: a registered worktree whose parent directory is named `worktrees` or `.worktrees` is excluded as that parent, whole; any other parent (`crates`, `vendor`) never is. For this repo that is `['.claude/worktrees', '.worktrees']`. Measured with that list: **15.3 s** (0.689 s to 16.0 s) and **7.6 s** (0.769 s to 8.343 s), n = 2. That is a cut of 2x to 7x against the default, and **both runs are still above the 5 s cold-start budget**. The spread between the two runs is large and its cause is not found.

So this bug stays open: `references` and `symbols` can still fail on a cold start in a repo like this one. What remains is step 3 of the assessment above (wait on rust-analyzer's `Roots Scanned` progress instead of a fixed 5 s), or a longer cold budget, which costs latency for a symbol that truly has no references. Not started.


**Step 3 applied 2026-10-24 (see the commit that cites this file).** Design from the progress stream of three cold probe runs (`$/progress` events, same repo): (a) events can be 17.7 s apart inside the null window, so "recent activity" ends the wait too soon; (b) a token (`Indexing`, cache priming) stays open long after symbols work, so "all tokens ended" is far too strict, which is harmless only because the wait runs while the answer is still `null`; (c) there was a 0.28 s moment between two scans with no token open, so a token that closed under 1 s ago still counts. The rule is title-agnostic: **while a `documentSymbol` answer is `null`, inside the cold-start window, keep re-asking as long as some progress token is open or closed less than 1 s ago, up to a 40 s ceiling** (below the 60 s tool timeout). An empty array is never extended. New `src/lsp/progress.rs` (`ProgressTracker`, `keep_waiting`); the client reads `$/progress` on both transports (the mux already broadcasts server notifications to its clients, read in `handle_server_message`); rust-analyzer alone is sent `window.workDoneProgress` at both `initialize` sites (Kotlin and other servers keep their capabilities); the error now states the time actually waited.

Evidence. Deterministic: a fake server that answers null for 7 s while it announces a progress token succeeds past the 5 s budget; the same 7 s of null with no progress still ends at 5 s with the old error (`a_null_answer_is_re_asked_past_the_budget_while_the_server_reports_progress` and `a_null_answer_still_ends_at_the_budget_when_the_server_reports_no_progress`); wire tests for the capability at both sites; unit tests of the tracker and of `keep_waiting`. Live, through codescout's own client against real rust-analyzer on this repo (`live_cold_document_symbols_smoke_test_on_this_repo`, ignored, manual): answered after 6.2 s with the wait on, and after 4.0 s with it switched off. **That live test does not discriminate**: the window varies run to run (4.0 s and 6.2 s back to back; 7.6 s and 15.3 s in the earlier probe with the exclusions) because the file cache warms, so a short window passes either way. A first version of that test asked at once and answered in 12 ms, because rust-analyzer answers symbols for its first ~0.7 s before its first reload; it now waits 1.5 s.

Not established: the progress stream reaching a client through the mux is read from the code and not exercised end to end (only the mux's capability is tested); the 40 s ceiling is a choice, not a measurement; no run has shown a window above 5 s failing before this change and passing after, on real rust-analyzer.

## Tests added

In `src/lsp/progress.rs`: the tracker (begin, end and the 1 s debounce, a report for a token never seen to begin, string versus number tokens, non-progress messages ignored) and `keep_waiting` (inside the budget, past it, an empty array, the 40 s ceiling). In `src/lsp/client.rs`: `a_null_answer_is_re_asked_past_the_budget_while_the_server_reports_progress` (a fake server null for 7 s while announcing progress succeeds past the 5 s budget; it failed first, with the old 5000 ms error) and `a_null_answer_still_ends_at_the_budget_when_the_server_reports_no_progress` (the same 7 s of null with no progress still ends at 5 s); `initialize_excludes_nested_worktrees_for_rust_analyzer` and `initialize_sends_no_options_to_another_server` (the `initialize` params on the wire: exclusions and the progress capability for rust-analyzer, neither for another server). In `src/lsp/mux/process.rs`: `the_mux_handshake_excludes_nested_worktrees_for_rust_analyzer` (the same for the mux handshake). In `src/lsp/init_options.rs`: eight unit tests of the decision. Dropping the options at either `initialize` site makes the matching wire test fail with `left: Null` (checked by mutation). Manual and not in the gate: `live_cold_document_symbols_smoke_test_on_this_repo`, which does not discriminate (see Fix).

`./scripts/gate.sh` exited 0 on a clean detached worktree of `1b208ad79`: lean 5278 passed, default 7937 passed, Python 1274 passed, 0 failed. `scripts/build-windows.sh clippy --all-targets -- -D warnings` exits 0.

## Workarounds

Wait and retry: `references` worked 104 s after rust-analyzer started. Or exclude the worktree directories in the rust-analyzer configuration.

## Resume

Closed, with the standing caveat in `unverified`. To reopen: a run of codescout's own client on real rust-analyzer in which a window above 5 s still ends in the null error (the 40 s ceiling is the first suspect), or a client joining a running mux that never sees its progress.


## Fix provenance

- **SHA:** `1b208ad79e522da6de8d970e0edd7e48e3340b33` (`experiments`)
- **patch-id:** `1421dd4bbc60e6b6cb6183ef82c958fc1443c48c`

The fix is three commits on `experiments`; the two earlier ones are `86e7b1fbdaed0cf16c2c3832d9486697c92ca516` (the hint, patch-id `777cb7943742cda70150278777f68c03c21f6a51`) and `a9e198d07a5ada42fe24d0259e3dccbf1ba21a1e` (the exclusion of nested worktrees, patch-id `3cf0f9a0384ce5399541e841f5eb81d190ba844c`).

## References

- `docs/issues/archive/2026-08-27-references-symbol-not-found-while-lsp-warms.md` (the earlier null-answer fix, `e26da0b2`)
- `src/lsp/client.rs:788-792`, `src/lsp/client.rs:1287-1294`, `src/lsp/client.rs:1307-1326`, `src/lsp/client.rs:1332-1429`
- `docs/trackers/issue-clusters/IC-7-lazy-warmup-bills-the-first-caller.md`
