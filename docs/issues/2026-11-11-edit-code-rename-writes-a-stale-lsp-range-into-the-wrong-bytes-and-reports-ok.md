---
id: '58a45860b802a477'
kind: bug
status: open
title: edit_code rename writes a stale LSP range into the wrong bytes of another file and reports ok
tags:
- cluster/unclassified
- edit-code
opened: 2026-11-11
owner: marius
related:
- a73c49456056fb92
- '4e7ec7bfc047ac7b'
severity: high
unverified: Tests cover the stdio transport only and check the server's edit, not the tool's write; the mux transport was verified once, end to end, with a driver script that is not in the repo. A stale landing inside a string literal was not run. How often the server cancels a rename after a resync is not measured. Severity is my proposal.
---

# BUG: edit_code rename writes a stale LSP range into the wrong bytes of another file and reports ok

## Summary

`edit_code(action="rename")` applies the `WorkspaceEdit` that the language server computes from its own copy of each file. If a file other than the one named in the call changed on disk outside codescout, and the server still holds the old copy, the edit for that file lands at the old line and column of the new bytes. The call returns `status: "ok"` and the real occurrence is not renamed. In case E the stale range hit the call of a different function and rebound it to the renamed function; the project still compiled and a test failed. The only guard is a regex heuristic that looks for the new name right after an alphanumeric character, so a landing after a space or punctuation raises no warning.

## Class fit

This is the write-side twin of `a73c49456056fb92`, whose symptom is a stale read. The nearest class is the archived `cluster/gate-keyed-on-unobservable-event` case of `2026-08-11-symbols-stale-after-external-git-checkout`, which I did not read against this case. I use `cluster/unclassified`. It differs from the read-side bug because the stale answer is written to disk and the response claims success.

## Symptom (Effect)

After a rename, a comment or other text in a second file is overwritten with the new name, and the real call keeps the old name. In one run the overwritten text was `tail text` (9 bytes). The response shows `status: ok`, `files_changed: 2` and no `corruption_warning`.

## Reproduction

Two scratch crates, rust-analyzer, 2026-11-11, session `5f715d8f`, HEAD `5bccb5bdf`. `lib.rs` holds the function; `other.rs` holds `use_it`, which calls it. Both files were synced with `symbols(path)` and `references`. Then `other.rs` was rewritten from a shell, outside codescout, and `edit_code(action="rename", symbol=<fn>, path="src/lib.rs", new_name=<n>)` was the first call after the rewrite. Each result was read from the files with `diff`, not from the response.

| # | Transport | Disk after the external rewrite | Rename | Result on disk | Response |
|---|---|---|---|---|---|
| A | stdio | 66 bytes, same length as before; real call on line 2; line 5 is `// y` | `target_fn` → `renamed_fn` | line 5 `// y` became `// yrenamed_fn`; the real call `crate::target_fn(5)` was NOT renamed | `ok`, 2 files, 2 edits, **with** `corruption_warning` (preview `// yrenamed_fn`) |
| B | stdio | line index 1 (0-based) is `// abcdefg tail text here`; the real call is on line 4 | `renamed_fn` → `nn_fn` | `// abcdefg tail text here` became `// abcdefg nn_fnhere` (10 bytes `tail text ` overwritten); the real call `crate::renamed_fn(5)` was NOT renamed | `ok`, 2 files, 2 edits, **no warning** |
| C | socket (mux) | same layout as B | `target_fn` → `nn_fn` | `// abcdefg tail text here` became `// abcdefg nn_fn here` (9 bytes `tail text` overwritten); the real call `crate::target_fn(5)` was NOT renamed; the 3 edits in `lib.rs` were right | `ok`, 2 files, 4 edits, **no warning** |
| D | socket (mux), control | same layout as C, but `symbols(other.rs)` was called after the rewrite and before the rename | `nn_fn` → `mm_fn` | the comment is intact and the real call became `mm_fn(5)` | `ok`, 2 files, 4 edits |

D differs from C only by the `symbols(other.rs)` call, so the corruption comes from the stale server view and not from the rename itself. The server position for the call was line 1, column 11 (0-based) in B and C; on disk that position is inside the comment, and the character before it is a space.

### Case E: the stale edit lands on code and the project still compiles

Same stdio crate `stale2`, reset. `lib.rs`: `target_fn(x) = (x+1)*2`, `helper_fn(x) = x`, `pub mod other;` and a test `use_it_is_five` that asserts `crate::other::use_it() == 5`. `other.rs`: `use_it` calls `crate::target_fn(5)` (51 bytes). Both files and `references(target_fn)` were synced first.

| Step | Result |
|---|---|
| External rewrite of `other.rs`: `crate::target_fn(5)` becomes `crate::helper_fn(5)`. Both names have 9 characters, so the file stays 51 bytes; only the mtime differs | disk now calls `helper_fn` |
| `cargo test --offline` | `use_it_is_five ... ok` (1 passed) |
| `edit_code(rename, symbol="target_fn", path="src/lib.rs", new_name="nn_fn")`, first call after the rewrite | `status: ok`, 2 files, 2 edits, **no warning** |
| `diff` of `other.rs` | line 2 `crate::helper_fn(5)` became `crate::nn_fn(5)`; `helper_fn` is still defined in `lib.rs` and now unused |
| `cargo test --offline` | compiles; `use_it_is_five ... FAILED`, `left: 12, right: 5` |

The real file had no call to `target_fn`, so a correct rename had nothing to change in `other.rs`. The stale range (line 1, column 11, 0-based) fell on the call of a different function with a name of the same length, and the tool rebound that call to the renamed function. The build is green; the behavior changed (5 became 12). The `:` before the name also keeps the heuristic silent.

## Environment

Linux 7.2.15-zen1, btrfs, codescout HEAD `5bccb5bdf`, rust-analyzer. Transport checked in the process tree: `stale` under `codescout mux --socket …`, `stale2` under `codescout start --debug` (stdio). Scratch files: `scratchpad/stale/`, `scratchpad/stale2/`, `*.before*.rs` snapshots.

## Root cause

Read at the bytes, `src/tools/symbol/edit_code.rs` (`EditCode/do_rename`):

- After the edits, a heuristic builds `corruption_hints`: when `new_name.len() >= 4`, the regex `[a-zA-Z0-9]<new_name>` is run on each changed file (lines 837-864); a hit sets `corruption_warning` (lines 1005-1012). It runs after the write, and it cannot see an edit that lands after a space, a punctuation mark or a line start. In B and C the new name had 5 characters and was preceded by a space, and no warning was raised.
- The staleness itself comes from `a73c49456056fb92`: `resync_if_drifted` has one caller, in `document_symbols`, so only the file named in the call is repaired.

Read at the bytes: `plan_path` in `do_rename` reads the file from disk as the pre-image and calls `apply_text_edits(&pre_image, &plain_edits)` (`src/symbol/edit.rs:921`) with the ranges from the server. `apply_text_edits` logs a warning for an edit whose start line is past the end of the file and for overlapping edits. It does not compare the text at a range with the old name. So a range that the server computed from an old copy is applied to the new bytes. Case E shows that this can compile; I did not run a landing inside a string literal.

## Evidence

The `diff` output of each case against its snapshot, taken in the session; the responses quoted in the table.

## Hypotheses tried

- The rename path resyncs every file it edits: **refuted** by A, B and C.
- The `corruption_warning` catches a stale landing: **partly**. It fired in A (preceded by `y`) and not in B and C (preceded by a space).

## Fix

Done 2026-11-11, commit pending, by the fix of `a73c49456056fb92`: `LspClient::request` resyncs every recorded file whose signature drifted before every request (`resync_all_drifted`, `src/lsp/client.rs`), so the server's copy of every file is current when it computes the rename edit. Cases B, C and E no longer reproduce (client-level tests, and an end-to-end run of the new binary on the mux transport). Design and rejected alternatives are in `a73c49456056fb92` `## Fix`.

**Not done, on purpose:** a range check in `do_rename` (refuse an edit whose range does not cover the old name). It would also catch a stale copy for a cause I have not seen, and it sits at the write site. Rejected for now because other languages' servers may return edit ranges that are not the bare old name (for example for an import alias or a constructor), so the check could refuse a legitimate rename; I did not measure that. The `corruption_warning` heuristic is unchanged and still misses a landing after a non-alphanumeric character.

**Cost seen:** the `didChange` that now precedes a rename can make the server cancel it; the client does not retry a non-idempotent request, so the caller gets a `RecoverableError` and re-issues. Seen once in a 468 s full-suite run under load; not measured in real use.

## Tests added

In `src/lsp/client.rs`, stdio transport, real `rust-analyzer` (skipped when it is missing). Shared helpers: `create_two_file_project`, `synced_two_file_client`, `disk_text_at`, `edit_ranges_in`, `references_until_other_rs_appears`, `rename_retrying_when_cancelled`, `rename_after_an_external_write`. The invariant is fix-agnostic: every rename edit range for `other.rs` must cover `target_fn` in the file as it is on disk. Each test has a control twin that differs only by one `document_symbols(other.rs)` call.

| Test | Case | State |
|---|---|---|
| `rename_edit_for_another_file_reflects_a_prepend_made_outside_codescout` | B, three lines prepended | was red, now green (`#[ignore]` removed) |
| `rename_edit_does_not_land_on_another_callee_after_a_same_size_write_outside_codescout` | E, same-size swap to `helper_fn` | was red, now green |
| `rename_edit_for_another_file_is_current_after_a_prepend_once_it_is_resynced` | control of B | green |
| `rename_edit_does_not_land_on_another_callee_after_a_same_size_write_once_resynced` | control of E | green |

Before the fix the three red tests failed with the stale ranges (`""` and `"helper_fn"` where `target_fn` was expected); after it `cargo test --lib -- lsp::` has 259 passed, 0 failed, and `./scripts/gate.sh` is green. The read-side tests and the mux end-to-end run are in `a73c49456056fb92` `## Tests added`.

Not covered: a stale landing inside a string literal; the `edit_code` tool layer (the tests check the edit the server returns, not what `do_rename` writes); the socket transport has no automated test.

## Workarounds

Call `symbols(path)` on each file that may have changed outside codescout before `edit_code rename`. After any rename, run the build and `git diff`.

## Resume

Fixed in the working tree, commit pending. After the commit: add `## Fix provenance` (SHA and patch-id) and set `fixed`.

Severity `high` is my proposal: a successful-looking write put text in the wrong bytes of the user's source, and case E changed program behavior while the build stayed green. The owner decides.

Open, none blocking: whether to add the `do_rename` range check as a second guard (it needs a measurement across languages first); a stale landing inside a string literal; how often the server cancels a rename that follows a resync.

Related: `a73c49456056fb92` (read side), `4e7ec7bfc047ac7b`, `bug-fix-session-log:F-235`.

## References

- `src/tools/symbol/edit_code.rs`: `EditCode/do_rename`, lines 837-864 and 1005-1012
- `src/lsp/client.rs`: `LspClient::resync_if_drifted`, `document_symbols`
- `docs/issues/2026-11-11-symbol-at-and-hover-answer-from-stale-lsp-content-after-an-external-edit.md`
