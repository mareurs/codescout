---
id: '9391e97eeb937e90'
kind: bug
status: open
title: 'BUG: LspClient::rename decodes a legal null answer into an opaque serde error, and edit_code rename passes it on'
owners:
- marius
tags:
- cluster/unclassified
- lsp
opened: 2026-11-12
related:
- '8117d5eac05e5723'
severity: low
unverified: 'No real `edit_code rename` call is known to have hit the null answer: the repo''s usage.db (149893 rows, 2026-10-16 to 2026-11-14) has none, among at least 79 renames, but 548 edit_code rows have no recorded input, rows of dropped writes are missing, and no other project''s database was read. The test failure was a direct `LspClient::rename` call, 2 of 15 suite runs. Whether the error reaches the agent as an error result or as a recoverable one was read from the code (`?` in `do_rename`), not run. Why rust-analyzer answered null was not established; the documentSymbol hint says it sends null while it reloads its workspace, and that was not measured for rename. Whether the null answers still happen in the 20 clean iterations after the test helper started retrying them was not measured, because the retry hides them.'
---

# BUG: LspClient::rename decodes a legal null answer into an opaque serde error, and edit_code rename passes it on

## Summary

`textDocument/rename` may answer `null`: in `lsp-types` 0.97.0 `request::Rename::Result` is `Option<WorkspaceEdit>`. `LspClient::rename` (`src/lsp/client.rs`) decodes the answer with `serde_json::from_value(result)?` into a plain `WorkspaceEdit`, so a `null` becomes the error `invalid type: null, expected struct WorkspaceEdit`. `edit_code rename` (`src/tools/symbol/edit_code.rs`, `do_rename`) passes that error on with `?`, so the agent would see a serde message with no hint about what happened or that asking again may work.

## Class fit

No reserved class fits: the client handles only part of the answer type that the protocol defines, and shows the gap as a decode error instead of a named condition. `cluster/unclassified` keeps that uncertainty, because no other record in this class is known to share the mechanism.

## Symptom (Effect)

In a full-suite stress run the test `lsp::client::tests::rename_edit_does_not_land_on_another_callee_after_a_same_size_write_outside_codescout` failed in 2 of 15 iterations, after 0.55 s and 0.74 s:

```
thread '…rename_edit_does_not_land_on_another_callee_after_a_same_size_write_outside_codescout' panicked at src/lsp/client.rs:4106:27:
rename failed: invalid type: null, expected struct WorkspaceEdit
```

## Reproduction

Not on demand. `scripts/with-slot.sh cargo nextest run --workspace --no-fail-fast --stress-count 15` on `experiments` at `d516da4cb`, no extra load job: iterations 8 and 14 failed with the message above. The four rename tests passed in two earlier attempts: 18 of 18 iterations alone under a load job of one busy process per core, and 12 of 12 iterations with all 71 `lsp::client` tests in parallel beside that load (see `8117d5eac05e5723`).

## Environment

Linux, rust-analyzer started by the test, stdio transport (no mux), `experiments` at `d516da4cb`, a host shared with other sessions.

## Root cause

Two parts.

1. Known, read in the code: `LspClient::rename` (`client.rs`, about line 1817) returns `Ok(serde_json::from_value(result)?)` with the return type `Result<lsp_types::WorkspaceEdit>`. A JSON `null` cannot be decoded into that type, so it becomes an `anyhow` error whose text is the serde message. The protocol type is `Option<WorkspaceEdit>` (`lsp-types-0.97.0/src/request.rs`, `impl Request for Rename`, `type Result = Option<WorkspaceEdit>`).
2. Unknown: why the server answered `null` in those two runs. The test asks for a rename right after an outside write and a resync of the changed file, on a host running the whole suite. "The server was still working through the change" fits the timing and is not measured.

## Evidence

- The two panic lines above, in the log of the 15-iteration run (`@bg_00000024`, session `5f715d8f`).
- `edit_code.rs` lines 748 to 750: `let edit = client.rename(&full_path, sym.start_line, sym.start_col, new_name, &lang).await?;`. No branch handles a null answer.
- A grep for `expected struct WorkspaceEdit` and `invalid type: null` over `docs/` and `src/` found no earlier record and no handling.


Production exposure, measured 2026-11-14 (session `5f715d8f`) in the repo's own `.codescout/usage.db`, read-only, which holds 149893 rows from 2026-10-16 to 2026-11-14 (UTC stamps):

- No row in the whole table has an `error_msg` containing `invalid type: null` or `expected struct WorkspaceEdit`. That search does not depend on `input_json`.
- 79 `edit_code` calls had `action` `rename` (`json_extract(input_json, '$.action')`); all 79 succeeded. They are spread over 21 days, 1 to 14 a day.
- 548 `edit_code` rows (423 in October, 125 in November) have a NULL `input_json`, so the action of those calls cannot be told, and some may be renames. The 79 is therefore a lower bound of the renames.
- This is one project's database. Rows of calls whose write was dropped (see `6767d5eaafa320c7`) are missing. I did not read the `usage.db` of any other project.
- The test flake hit `LspClient::rename` in 2 of 15 suite runs. A rate of that size per call would make 0 failures in 79 calls very unlikely (about 1 in 80,000: 0.867 to the power 79), so real `edit_code rename` calls, made at human pace and not right after an outside write on a loaded host, do not meet the null answer at that rate. This says nothing about the rate under the conditions of the test.

## Hypotheses tried

None beyond the two parts of Root cause. That a real `edit_code rename` call can hit the null answer: not tried.

## Fix

Not fixed in the product. The tests that call `LspClient::rename` directly now retry this one error for up to 20 s (`4603d77cf`, bug `8117d5eac05e5723`); that removes the test flake and changes nothing for `edit_code rename`. Two ways to fix the product, the owner's choice, each touching the `LspClientOps::rename` trait and its mocks (not counted):

- return `Result<Option<WorkspaceEdit>>` and let `do_rename` say "the server returned no edit";
- map `null` inside `LspClient::rename` to a `RecoverableError` with a hint, so sibling tool calls survive and the agent knows asking again may work.


Precedent in the same file, read 2026-11-14: `LspClient::document_symbols` (`src/lsp/client.rs`, about lines 1596-1620) already treats a `null` answer as "the server has not answered". It retries for a window, then returns a `RecoverableError` named `the language server answered textDocument/documentSymbol with null for <path> on every attempt for <n> ms`, with a hint that says rust-analyzer sends null while it reloads its workspace. `normalize_err_family` (`src/usage/db.rs`, about line 750) gives that message its own family. The usage database has 22 `edit_code` rows with that message, from 2026-10-24 to 2026-11-01. So the second fix above has a sibling in the code base. The hint text is a claim in a source comment; nobody measured it for `rename`.

## Tests added

None. N/A: not fixed in the product, and a test needs a server that answers `null` on demand, which has not been found.

## Workarounds

Ask for the rename again. Not verified in a real `edit_code rename` call.

## Resume

Decide with the owner between the two fixes above. Before that, find out whether a real `edit_code rename` call, right after an outside write, can hit the null answer, and what the agent sees. The measurement that would settle it: run `edit_code rename` through the MCP server in a loop beside a load, and log the raw answer in `LspClient::rename` when it is null.


Update 2026-11-14: the production measurement above found 0 null decode errors in 79 renames, so the bug stays low severity and is not urgent. The open choice is unchanged: a `null`-aware return type or a `RecoverableError` as `document_symbols` does. A reopen trigger for severity: any row with `invalid type: null` in `usage.db` from a real `edit_code rename` call.

## References

- `src/lsp/client.rs`: `LspClient::rename`
- `src/tools/symbol/edit_code.rs`: `do_rename`
- `8117d5eac05e5723` (the flaky test)
