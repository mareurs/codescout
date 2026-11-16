---
id: '9391e97eeb937e90'
kind: bug
status: fixed
title: 'BUG: LspClient::rename decodes a legal null answer into an opaque serde error, and edit_code rename passes it on'
owners:
- marius
tags:
- cluster/unclassified
- lsp
closed: 2026-11-16
opened: 2026-11-12
related:
- '8117d5eac05e5723'
severity: low
unverified: STANDING Why rust-analyzer answers null for rename was not established, and no real `edit_code rename` call is known to have hit it. The new RecoverableError routing through `do_rename` was read in the code, not run.
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

`LspClient::rename` now decodes the answer through `decode_rename_answer` (`src/lsp/client.rs`). A `null` answer becomes a `RecoverableError`: "the language server answered textDocument/rename with null: it returned no edit", with a hint to ask again. This is the second option of the earlier text, and it follows the `document_symbols` precedent. The `LspClientOps::rename` trait and its mocks did not change. `do_rename` still passes the error on with `?`, and the recoverable routing of that error was read in the code, not run. The test helper `rename_retrying_while_the_server_is_busy` now matches the new message in place of `invalid type: null`.

## Fix provenance

- **SHA:** `41ba4adfa` (`experiments`)
- **patch-id:** `d14f50d7d19bd8cdde5b54d7b8b508b3e77c5f3b`

## Tests added

Two unit tests in `src/lsp/client.rs`: `a_null_rename_answer_is_a_recoverable_error_that_names_the_method` and `a_real_rename_answer_still_decodes_to_the_edit`. Measured 2026-11-16: the first test fails when the `null` branch is disabled, with the message `invalid type: null, expected struct WorkspaceEdit`, and passes with the fix. The 73 `lsp::client` tests pass. A test with a live server that answers `null` was not written, because none was found.

## Workarounds

Ask for the rename again. Not verified in a real `edit_code rename` call.

## Resume

Fixed by `41ba4adfa`. The full `./scripts/gate.sh` has not run on the fix. Archive after a green gate. Why rust-analyzer answers `null` is still not established.

## References

- `src/lsp/client.rs`: `LspClient::rename`
- `src/tools/symbol/edit_code.rs`: `do_rename`
- `8117d5eac05e5723` (the flaky test)
