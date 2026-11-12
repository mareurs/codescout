---
id: a73c49456056fb92
kind: bug
status: fixed
title: A file changed outside codescout stays stale in the LSP server unless a documentSymbol call resyncs that same file (symbol_at, and references into other open files)
tags:
- cluster/unclassified
- navigation
closed: 2026-11-11
opened: 2026-11-11
owner: marius
related:
- '4e7ec7bfc047ac7b'
- '58a45860b802a477'
- '489e013ddfd6c3fc'
severity: medium
unverified: 'Measured on two tiny crates (one per transport), rust-analyzer only, one run per case. The automated tests cover stdio only; the mux transport was verified once end to end with a driver script that is not in the repo. The hover and goto_definition tests (2026-11-12) were run once each on the fixed code and once each with the resync disabled; they cover the changed file on stdio only. Not done or not measured: prepare_call_hierarchy with a raw position was not run; the cost of the per-request stat and how often a rename after a resync is cancelled by the server are not measured; whether the server can notice a changed file by itself was not examined; a language other than Rust. The call_graph omission of other.rs::use_it that I saw is a different defect (call_graph serves a stale edge cache), filed as 489e013ddfd6c3fc.'
---

# BUG: a file changed outside codescout stays stale in the LSP server unless a documentSymbol call resyncs that same file (symbol_at, and references into other open files)

## Summary

`LspClient::resync_if_drifted` re-sends a file to the language server when its on-disk `(len, mtime)` differs from the signature recorded at the last send. Its only caller is `LspClient::document_symbols`, so only the file that a `documentSymbol` request names is repaired. Two consequences, both measured:

- `symbol_at` (hover, goto_definition) calls only `did_open`, which returns early for an open file. After an external edit it asks the server about content that no longer exists.
- `references` into a file that is open, changed on disk and not named in the call returns a location taken from the old layout. The label is the current text of a different line.

No error is raised in either case. One `symbols(path)` on the changed file repairs it.

## Class fit

The nearest class is `cluster/gate-keyed-on-unobservable-event`, the class of the archived `2026-08-11-symbols-stale-after-external-git-checkout`: a drift check that fires on one operation and not on its siblings. I use `cluster/unclassified` and not that class, because I did not read that class's definition against this case. Here the check exists and works for `document_symbols`; the defect is the operations that take a raw position and never reach it. It differs from the original bug in that the repair already exists and is only not wired to these calls.

## Symptom (Effect)

After a file is edited outside codescout, `symbol_at(path, line, col)` at the **current** position of a symbol returns "no definition resolvable". Called at the **old** position, it returns hover text for the symbol that used to be there, and a `def` snippet taken from the current disk bytes, which is a different line. One `symbols(path)` call on the changed file repairs it. For a file the call does not name, `references` shows the same staleness (see the second round under Reproduction).

## Reproduction

Scratch crate `stale` (two files: `Cargo.toml`, `src/lib.rs` with `alpha`, `target_fn`, `omega`), pinned with `workspace=<crate>`, rust-analyzer, 2026-11-11, session `5f715d8f`. The mutator rewrites the file from a shell, outside codescout.

| # | Step | Result |
|---|---|---|
| 1 | `symbols(path)`, then `symbols(name="target_fn", include_body=true)` | `target_fn` at L5-8, body correct |
| 2 | External rewrite: prepend 4 comment lines (size changes). `symbols(name="target_fn", path, include_body=true)` | L9-12, body correct. The drift check works. |
| 3 | Same, `symbols(name="target_fn")` with no path | L9, body correct |
| 4 | External rewrite: same byte length (151), blocks moved, `target_fn` now at line 12. `symbol_at(line=12, col=9)` | `def`: "no definition resolvable"; `hover`: "no hover info". **Stale.** |
| 5 | `symbol_at(line=9, col=9)`, the OLD position | `def` shows `// b` (the current disk text of line 9); `hover` shows `pub fn target_fn(x: u32) -> u32`. **The server still holds the old layout.** |
| 6 | Control: `symbols(path)`, then `symbol_at(line=12, col=9)` | Correct: `def` `pub fn target_fn(x: u32) -> u32 {`, hover correct |
| 7 | External rewrite: prepend 4 lines again (size changes; `target_fn` now at line 16). `symbol_at(line=16, col=9)` first, with no `symbols` call | "no definition resolvable". **Stale, for a size change too.** |

Steps 4, 5 and 7 show that the staleness does not depend on the same-size blind spot. Any external change that is not followed by a `document_symbols` call leaves the server stale for `symbol_at`.

### Measured 2026-11-11, second round: the stale view reaches `references` through OTHER files

Same scratch crate. `src/lib.rs` holds `alpha`, `target_fn`, `frst_caller_`, `omega`, `late_caller` and `pub mod other;`. `src/other.rs` holds `use_it`, which calls `crate::target_fn(5)` on line 2. Both files were synced with `symbols(path)`.

| # | Step | Result |
|---|---|---|
| 8 | Baseline: `references(symbol="target_fn", path="src/lib.rs")` | 4 references: `lib.rs` 15, 23, 9 and `other.rs:2  crate::target_fn(5)` |
| 9 | External rewrite of `other.rs` only: 3 comment lines prepended, so the real call is on line 5. Then `references(target_fn, lib.rs)` first, with no `symbols` call | 4 references; the `other.rs` entry is `2  // y`. Line 2 on disk is `// y`, which has no `target_fn`. **Stale and wrong, for a file the call never asked about.** |
| 10 | Control: `symbols(path="src/other.rs")`, then the same `references` call | `other.rs:5  crate::target_fn(5)`. Correct. |
| 11 | By-name calls on the file that was changed: `references(target_fn, lib.rs)` after an external +4-line and new-caller edit of `lib.rs`; `call_graph(target_fn, callers)` after a same-length rename in `lib.rs` | Both correct (15, 23, 9; `frst_caller_`, `late_caller`). The repair covers the file named in the call. |

So the resync is per file: a file is repaired only when a `documentSymbol` request goes to it. Any other file that the server has open and that changed on disk is answered from the old content. `references` is a by-name tool that agents trust, and it returned a location whose label is a different line of the current file.

`call_graph(target_fn, callers)` listed 2 edges (both in `lib.rs`) in the stale state (step 9 setup, `other.rs` rewritten again) and in the control state. It never listed `other.rs::use_it`, which `references` finds. Because it shows the same output in both states, it neither shows nor rules out the staleness. Why it omits `use_it` was not investigated and is not claimed to be a defect.

The tool-level callers of raw positions that I found: `symbol_at` (`goto_definition`, `hover`: `symbol_at.rs:138`, `:272`), `References` (`references.rs:649` `references`, `:793` `prepare_call_hierarchy`), `resolve_one_hop` and `resolve_via_ts` (`call_edges/resolver.rs:69`, `:213`), and `call_graph/identity.rs:320` (`goto_definition` on an edge file). For the by-name ones, the position comes from `document_symbols` of the named file, which resyncs. Positions inside OTHER files come from the server's own answers.

No `didChangeWatchedFiles` appears anywhere in `src` (`grep`, 0 matches). Whether the language server finds a changed file on its own was not examined: in this run it did not.

### Measured 2026-11-11, third round: the same on the stdio transport

Second crate `stale2`, with `[lsp.rust] mux = false` in its `.codescout/project.toml` (`src/config/project.rs`, `LspLangOverride.mux`). Same layout: `lib.rs` (`alpha`, `target_fn`, `pub mod other;`) and `other.rs` (`use_it` calls `crate::target_fn(5)` on line 2, 51 bytes).

**The transport was checked in the process tree, not inferred.** `rust-analyzer` for the first crate has as parent `codescout mux --socket /run/user/1000/codescout-rust-mux-….sock` (socket). `rust-analyzer` for `stale2` has as parent `codescout start --debug` itself (stdio). `readlink /proc/<pid>/cwd` tied each process to its crate.

| # | Step | Result |
|---|---|---|
| 12 | Baseline, both files synced: `references(target_fn, lib.rs)`; `symbol_at(other.rs, line=2, col=17)` | `other.rs:2  crate::target_fn(5)`; def `lib.rs:5`, hover `pub fn target_fn(x: u32) -> u32`. Correct. |
| 13 | External rewrite of `other.rs` only: 3 comment lines prepended (66 bytes; the call is on line 5). `references(target_fn, lib.rs)` first | `other.rs:2  // y`. **Stale and wrong**, same as step 9 on the socket transport. |
| 14 | Same state: `symbol_at(other.rs, line=5, col=17)` | **LSP error** `-32603 Invalid offset LineCol { line: 4, col: 16 } (line index length: 51)`. 51 is the byte length of the OLD `other.rs`, so the server still holds it. On the socket transport the same kind of call returned "no definition resolvable"; here it surfaces as a raw error with no hint. |
| 15 | Control: `symbols(other.rs)` (L4-6), then both calls again | `other.rs:5  crate::target_fn(5)`; def `lib.rs:5`, hover correct. |

So the defect does not depend on the transport. What differs is how the stale position is reported.

### Measured 2026-11-11, two more stdio cases (`stale2`)

| # | Step | Result |
|---|---|---|
| 16 | External rewrite of the NAMED file `lib.rs` (4 lines prepended; `target_fn` now at line 9). `symbols(name="target_fn", path="src/lib.rs", include_body=true)` | L9-12, body correct. `symbols` resyncs on stdio too. |
| 17 | External rewrite of the SECOND file `other.rs`, **same byte length** (66 bytes, blocks moved so the call is on line 2 and line 5 is `// y`). `references(target_fn, lib.rs)` first | `other.rs:5  // y`. Stale and wrong, so a size change is not needed. |

### Measured 2026-11-11, fourth round: the stale view reaches a WRITE (`edit_code rename`)

The same state as steps 13 and 9 (a second file open in the server, changed on disk outside codescout), then `edit_code(action="rename")` on a symbol that the second file uses. The edit for the second file landed at the server's old position and overwrote other bytes; the response was `ok`. Full table and the control in `58a45860b802a477`. In short: on stdio and on the socket transport, `// abcdefg tail text here` became `// abcdefg nn_fnhere` (stdio) and `// abcdefg nn_fn here` (socket) while the real call kept the old name; with `symbols(other.rs)` called before the rename, the result was correct. This makes the read-side defect a write-side defect too, so the severity question below is open.

## Environment

Linux 7.2.15-zen1, btrfs, codescout at HEAD `5bccb5bdf`, rust-analyzer. The scratch files are under the session scratchpad and are not durable: `stale/src/lib.rs`, `stale-mutate.py` (modes `prepend4`, `samesize_move`), `mtime-probe.py`.

## Root cause

Read at the bytes, `src/lsp/client.rs`:

- `resync_if_drifted` (L1379) has one caller, inside `document_symbols` (L1495, call at L1512). `references` to `resync_if_drifted` returned exactly those two lines.
- `hover` (L1710) calls `self.did_open(path, language_id)` and then the request. `did_open` (L1390) returns `Ok(())` early when the canonical path is already in `open_files`, on the stdio transport.
- The archived bug's own text says the check runs "before the `documentSymbol` request goes out". It did not cover the position-based calls.

NOT established: which tool-level function sits between `symbol_at` and `LspClient::hover` / `goto_definition`, and whether some other layer resyncs. I inferred the path from the output (`def` and `hover` sections) and from the client code, not from a trace. `references` was not run as a raw-position call.

## Evidence

Tool outputs in the table above, copied from the session. The blind-spot probe in `mtime-probe.py` measured 0 of 2000 same-length rewrites with an identical mtime on this machine (median gap between the two mtimes 48.9 µs), so the documented `(len, mtime)` blind spot is not reachable here.

## Hypotheses tried

- The stale `symbols` ranges in `4e7ec7bfc047ac7b` come from the same-size blind spot: **not supported on this machine** (0/2000, above).
- `symbols` fails to resync after an external size change: **refuted** in steps 2 and 3. It resyncs.
- `symbol_at` does not resync: **confirmed** in steps 4, 5 and 7.

## Fix

Done 2026-11-11 in `src/lsp/client.rs`, commit `43c4ca9df` (see `## Fix provenance`). `LspClient::request` now calls `resync_all_drifted` before every request except `initialize` and `shutdown`. It replaces `resync_if_drifted`, which had one caller (`document_symbols`). `resync_all_drifted` asks `SyncedSignatures::drifted` for every recorded file whose `(len, mtime)` differs from the last delivered bytes and sends each one a `didChange`. `document_symbols` still calls it before `did_open`, so its ordering rule (check before the open) is unchanged.

Two details that carry the correctness:

- The record now keeps the path a file was SENT under (`Delivered.sent_as`), next to the canonical key. The server knows a document by the URI of that path; a resync that used the canonical key would name a document the server never opened when the workspace is reached through a symlink (`/var` is a link on macOS). Unit test `drifted_names_a_changed_file_by_the_path_it_was_sent_under`.
- The hook sits in `request` and not in each operation, because every operation passes through it (`grep` of `request_with_timeout(` and `.request(` in `src/lsp`: only `initialize` bypasses it, and it runs before any file is open). A new LSP operation cannot forget it.

Decision record (architecture-snow-lion):

- **Change scenarios absorbed:** (1) an external edit (`git checkout`, `cargo fmt`, an editor) of any file the server has open, followed by any operation; (2) a future LSP operation added to the client.
- **Alternatives rejected:** resync inside each of the six position-based methods only (covers `symbol_at`, not steps 9 and 13, and each new operation can forget it); a range check in `do_rename` (a wall at the write site; it would also refuse legitimate renames whose edit range is not the bare old name in other languages, which I did not measure; kept as a possible second guard, not done); `workspace/didChangeWatchedFiles` (the client does not watch files; a new moving part for the same result).
- **Costs:** one `stat` per recorded file on every request (not measured; rust-analyzer's own request time dominates), and a `didChange` right before a request, which can make the server cancel a non-idempotent request such as rename. The client does not retry that and returns a `RecoverableError` asking the caller to re-issue it. I saw one such cancellation in a 468 s full-suite run under heavy load (see Tests added); I did not measure how often it happens in real use.
- **Revisit when:** `didChangeWatchedFiles` support is added to the client, or the per-request `stat` shows up in a profile.
- **Confidence:** medium-high for the read side; the transports are verified below.

## Fix provenance

- **SHA:** `43c4ca9df` (`experiments`)
- **patch-id:** `0e4db3459478`

## Tests added

In `src/lsp/client.rs`, stdio transport, real `rust-analyzer` (skipped when it is missing): `references_into_another_file_reflect_a_write_made_outside_codescout` (was red, now green, `#[ignore]` removed), its control `references_into_another_file_are_current_once_that_file_is_resynced`, the three rename tests listed in `58a45860b802a477`, and the unit test `drifted_names_a_changed_file_by_the_path_it_was_sent_under` (unix only). The invariant: after the NAMED file is resynced the way the by-name tools do it, every location and rename edit the client returns for ANOTHER file must cover `target_fn` in the file as it is on disk.

Verified 2026-11-11: `cargo test --lib -- lsp::` 259 passed, 0 failed; the group of 11 passed 4 of 4 runs with 8 threads; `./scripts/gate.sh` green (FMT, CLIPPY, LEAN, DEFAULT all 0). One full `cargo test --lib` run under load had one failure: the control `rename_edit_does_not_land_on_another_callee_after_a_same_size_write_once_resynced`, because the server cancelled `textDocument/rename` (`LSP cancelled non-idempotent request`). The test now asks again on exactly that error through `rename_retrying_when_cancelled`. I cannot prove that no flake remains; I saw it once.

**Mux transport, verified outside the unit tests.** The tests cover stdio only. I built the new binary, ran it over MCP stdio against a fresh crate on the default transport (process tree: `rust-analyzer` under `codescout-new mux --socket …`) with a driver script, and replayed steps 9 and C: after the external write `references` reports `other.rs:4` (correct); after the external write `edit_code rename` leaves the comment intact and renames the real call. The same driver on the previous release binary shows the defect (`// abcdefg nn_fn here`, real call not renamed). The driver is in the session scratchpad and is not in the repo.

Not covered: the socket transport has no automated test.

**`hover` and `goto_definition` on the changed file, 2026-11-12.** In `src/lsp/client.rs`, stdio transport, real `rust-analyzer`: `goto_definition_follows_a_write_made_outside_codescout` and `hover_follows_a_write_made_outside_codescout`. Each reads a baseline at the call's line 1, prepends three lines to `other.rs` outside codescout, then asks ONCE at the new position (line 4), with no intermediate `document_symbols` and no polling, because rust-analyzer watches files itself and polling could let it mask a stale answer. Only an error is retried (`retrying_on_error`), never an empty answer. Both pass on the fixed code. With the resync in `request` disabled (`if false && ...`, restored afterwards and checked) both fail, each after ten tries, with `LSP error (code -32603): Invalid offset LineCol { line: 4, col: 11 } (line index length: 51)`: the server still held the old 51-byte file. So the stale state shows as an error here and not as an empty answer. One run each, on this machine.

Verified 2026-11-12 on the shared tree: `./scripts/gate.sh` FMT 0, CLIPPY 0, LEAN 0 (9538 passed), DEFAULT 0 (9632 passed), Python light lane 1836 passed.

## Workarounds

Call `symbols(path)` on the file after any edit made outside codescout, then use `symbol_at`. Take line and column from that result.

## Resume

Fixed by `43c4ca9df` (see `## Fix provenance`). Archive after the owner confirms, with the fix verified on `experiments`: the gate was green at that tree and the tests above are the regression tests.

`4e7ec7bfc047ac7b` (the `symbols` report, never reproduced for `symbols` itself) is now `zombie`, with a re-open trigger.

Open questions, none blocking: the cost of the per-request `stat` on a large session; how often a rename right after an external edit is cancelled by the server; `prepare_call_hierarchy` with a raw position was not run; a language other than Rust.

Link: `58a45860b802a477`, `bug-fix-session-log:F-235`.

## References

- `src/lsp/client.rs`: `LspClient::resync_if_drifted`, `did_open`, `hover`, `document_symbols`
- `docs/issues/archive/2026-08-11-symbols-stale-after-external-git-checkout.md`
- `docs/issues/2026-11-10-symbols-stale-ranges-after-source-formatting.md`
