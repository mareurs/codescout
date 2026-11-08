---
kind: bug
status: mitigated
title: 'BUG: first references call after start returns a partial result with a warning; an immediate retry returns the full one'
owners:
- marius
tags:
- cluster/lazy-warmup-bills-the-first-caller
closed: 2026-11-08
opened: 2026-11-07
severity: medium
unverified: Reproduced on the base and not on `ec450d195`, but only under Wine 11.21 (a re-implementation of the Windows API, not Windows), with a synthetic project (standard library plus six files), pyright 1.1.413, 3 cold runs per binary. Not run on a real Windows host, not run on the reporter's project, and not run with the real CPU load of a user session. Every unit test still runs against the mock LSP provider, including the client swap after a mux disconnect. The first attempt is not bounded by the settle deadline, the MCP client's own tool-call timeout was not checked, concurrent first calls each wait, and which servers answer an empty list for ever was not measured. The gate does not cover the heavy Python suites, server-stack, librarian code in the lean lane or the ONNX weight tests.
---

# BUG: first references call after start returns a partial result with a warning; an immediate retry returns the full one

## Summary
On a cold pyright, the first `references` call returns only the definition file's hits and a
`completeness_warning`; the same call repeated immediately returns the complete set. The tool already
detects the gap but hands the retry to the caller.

## Class fit
Work deferred to first use is billed to the first caller, who gets a well-formed answer that is a
subset of the true one. It differs from the class's other members in that the guard fires and names
the problem, so the answer is not a silent zero; the open point is that the tool detects the gap and
still returns the partial set instead of waiting.

## Symptom (Effect)
Reported once by a peer session on a Python project, home project activated moments earlier,
`references(symbol="build_chain", path=...)`:
1. first call: 2 hits, both in the definition file, with the warning "LSP returned 0 references outside the
   definition file, but `build_chain` appears as a whole word in 3+ other source file(s) ... the reference
   index may still be warming after a reindex. Re-run, ...";
2. same call, repeated: "17 references in 3 files".

## Reproduction

Not reproduced by the filer: a `references` call on the same symbol returned the 17 hits on the first call (index already warm).

Reproduced 2026-11-08 under Wine 11.21 against a real pyright 1.1.413 (setup and scripts: codescout memory `wine-windows-test-suite`, `tools/wine-repro/`). Project: the 2169 `.py` files of the Python 3.11.9 standard library plus a package `pkg` (`pkg/core.py` defines `build_chain`, six files import and call it). A fresh Windows `codescout.exe` (`--no-default-features --features http`) per run, driven over HTTP from Linux, `references(symbol="build_chain", path="pkg/core.py")` called twice, first call on a cold server.

| Binary | First call | Second call, at once |
|---|---|---|
| `267d8cd01` (no fix), 4 runs | partial, 1.8 s, 1.3 s, 1.0 s, 1.3 s: the definition file only, with the `LSP returned 0 references outside the definition file` warning | partial in 2 runs (0.4 s, 0.5 s); complete in 2 runs, after a 12.7 s and an 11.2 s wait |
| `5cf72a154` (with `ec450d195`, the 45 s settle wait), 3 runs | complete, 8.0 s, 8.3 s, 8.3 s: 19 references in 7 files, no warning | complete, 4.3 s, 6.7 s, 4.6 s |

So the first call is partial on the base in 4 of 4 cold runs and complete with the settle wait in 3 of 3. The report's claim that an immediate retry returns the full set held in 2 of 4 base runs here; in the other 2 the retry answered at once and was still partial.

A first attempt with `textwrap.dedent` was NOT a reproduction and is not evidence: `from textwrap import dedent` resolves to the typeshed stub `textwrap.pyi`, so pyright reports 3 references on Linux and under Wine alike, however long it runs (a Linux control on the same files gave the same 3). Use a symbol whose module name is not a standard-library name.

## Environment
Windows, pyright 1.1.413, Python 3.11, `pyright-langserver.cmd` from the npm shim.

## Root cause
Not established. The warning is raised in `References::call` (`src/tools/symbol/references.rs`) by the
external-refs-zero branch, after `corroborate_zero_references` finds the symbol as a whole word in other files.

## Evidence
The two outputs above. Related: `docs/issues/archive/2026-08-27-references-symbol-not-found-while-lsp-warms.md`.

## Hypotheses tried
None.

## Fix

The wait lives in the tool, not the client. `LspClient::references` re-asks only an EMPTY answer (`request_until_answered`), so a non-empty but partial answer (the reported 2 hits, both in the definition file) is invisible to it; only `References::call` can see the gap, by comparing the answer with the text scan.

The first `references` call served by a client waits (blocking) for the server to settle, once per client, with one wall-clock deadline (`FIRST_CALL_SETTLE_WAIT` = 45 s, poll interval `SETTLE_POLL_INTERVAL` = 500 ms) in `References::call_with_settle` (`src/tools/symbol/references.rs`). The per-client state is `LspClientOps::references_warmup_done` / `mark_references_warmup_done`, required methods with no defaults.

Two signals count as unsettled: (a) no locations at all, on any server and in any window state; (b) no location outside the definition file or a build directory, the corroboration scan finds the symbol as a whole word in another file, and the client is inside its cold-start window (`LspClientOps::cold_start_window_open`, 5 minutes), on any server. An incomplete scan and the call-hierarchy completeness warning are not warming signals.

Bounds: the deadline is clamped to the configured `tool_timeout_secs` minus the call-hierarchy bound (`CALL_HIERARCHY_BOUND`, 5 s) and a margin; every attempt after the first runs under `timeout(remaining)` on the live client (after a mux disconnect the loop continues on the replacement client); a later empty answer never replaces a partial one already in hand; a call dropped mid-wait still spends the client's wait. The call-hierarchy cross-check is skipped when the wait ran out, bounded to 5 s after a wait that settled, and bounded by the remaining tool budget when the call never waited. On exhaustion the partial answer keeps its existing warning plus a note that the call already waited N s.

45 s, not 60 s: the server wraps every tool, `references` included, in `tool_timeout_secs` (default 60; `src/server.rs`, `tool_skips_server_timeout`), and `src/lsp/manager.rs` (`MUX_PEER_STARTUP_WAIT`) already sizes its wait below that limit for the same reason.

This replaces the 5 s re-ask of `9f78d028e` (`reask_until_outside_hit`, `PARTIAL_REASK_BUDGET`, `has_outside_hit`), which is removed in its own commit; two wait layers would multiply (PR 33 measured 62 s). The author of `9f78d028e` confirmed nothing else relied on it.

The comments in `LspClient::request` that said the cold-start retry costs about 45 s now state the arithmetic: delays of 3 s x (1+..+N), 165 s on Linux, plus up to (N+1) x 30 s of request timeouts (comment-only commit).

## Fix provenance

- **SHA:** `ec450d195a2d527a1b9d68770ac82a895af6bbd5` (`experiments`)
- **patch-id:** `f14fe2e3c335bed791fe167ff8fda90a18c66596`

Supporting commits on `experiments`: `c7d077f2f9aae74d2536b012b7653bf77fdb132f` (removes the re-ask of `9f78d028e`, patch-id `f49507725ab798ee4473a2eb0e09454796a064c9`) and `f4162fbf101c865925c4deab83206a91c0bfca85` (comment-only, patch-id `6116806c5caa2510e6b01e5a9e48f16fb27b86e6`). Superseded: `9f78d028e27d8b0e38b34963218b357c9c9c13e5` (patch-id `c6901954325299c0012bb0225f89e41da3c4b141`).

## Tests added

`src/tools/symbol/references_settle_tests.rs` (24 test functions), driven through the seam `call_with_settle` with millisecond deadlines, plus paused-time tests that call `Tool::call` with the production constants. They cover: a cold-then-warm server answered by one call with no warning; a server that never settles returns at the deadline with the waited note; a second call, and a call after a settled first call, never wait; a used file-local symbol on a client past its window and a symbol named in no other file return at once; an incomplete scan does not trigger the wait; a no-locations answer waits whatever the window; a later empty answer does not replace the partial one; a build-directory location is not an outside hit; transient and deterministic errors on a later attempt; attempts after a mux disconnect start on the replacement client; a configured short `tool_timeout_secs` shortens or removes the wait; a call dropped mid-wait spends the client's wait; the 45 s wait plus the cross-check bound fits under the default tool timeout; `LspClient`'s flag starts false.

Mutants run and reverted, each turning at least one of these red: delete the loop; never mark done; ignore the done flag; ignore or invert the window gate; gate no-locations on the window; count a complete scan with no hits or an incomplete scan as unsettled; bare await instead of `timeout(remaining)`; remove the cross-check bound or the skip after a wait that ran out; remove the tool-timeout clamp; remove the partial-answer guard; compute `external_refs` from the unfiltered answer; disable the drop guard; swap or zero the constants in `Tool::call`. Not killable: running the corroboration scan once per call instead of once per attempt (identical results, no counter). Three independent Opus reviews of the combined change found, in turn: a wait equal to the server tool timeout, a loop that restarted on a dead client after a mux disconnect, untested time bounds, a loop that ignored a configured `tool_timeout_secs`, and a later empty answer that erased a partial one; each was fixed with a test before the merge.

Gate 2026-11-08: `./scripts/gate.sh --check` on `7275cb7b2`, whose `src/`, `crates/`, `Cargo.toml` and `Cargo.lock` are identical to those of `f4162fbf1` (`git diff` empty): FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0. It does not cover the heavy Python suites, server-stack, librarian code in the lean lane or the ONNX weight tests. The tests of `9f78d028e` were removed together with its code.

## Workarounds
Run `references` twice, or corroborate with `grep` / `call_graph(direction="callers")` as the warning says.

## Resume

Status stays `mitigated`: the first call now waits, but the defect was never reproduced against a real language server, so it is not shown to cure the report, and every test runs against the mock. To close: reproduce on a project like the reporter's (Windows, pyright 1.1.413, a slower first index) and confirm the first call returns the full set within the deadline.

Known costs, accepted by the user: a first call on a client waits up to 45 s (less under a short `tool_timeout_secs`) when the server answers nothing at all, on any server; and when the answer holds only the definition file, the text names the symbol elsewhere and the client is inside its 5-minute window, which includes a used file-local symbol with a generic name on a warm server and a fresh mux connection to a long-warm server. Known gaps: the first attempt is not bounded by the deadline (the client's own cold-start retry budget is 165 s of delays on Linux), so a call can still reach the server's tool timeout regardless of this change; concurrent first calls each wait; the cold window is anchored at construction on the mux path (`docs/issues/2026-10-30-references-treats-every-new-mux-connection-as-a-cold-server-and-re-asks-for-5-s.md`); the MCP client's own tool-call timeout was not checked.

## References
- `docs/trackers/issue-clusters/IC-7-lazy-warmup-bills-the-first-caller.md`
