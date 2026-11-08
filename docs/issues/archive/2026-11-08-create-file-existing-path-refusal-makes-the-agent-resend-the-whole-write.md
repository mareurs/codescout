---
id: f9a4cd9ba331916e
kind: bug
status: fixed
title: create_file refuses an existing path and discards its input, so the caller resends the whole payload with overwrite
tags:
- cluster/unclassified
closed: 2026-11-08
opened: 2026-11-08
---

# BUG: create_file refuses an existing path and discards its input, so the caller resends the whole payload with overwrite

## Summary
`create_file` on an existing path refuses with `file already exists: <path>` and the hint "pass overwrite: true to replace". The refusal keeps nothing, so the caller sends the full `content` again with `overwrite: true`.

## Class fit
No roster class fits without forcing it. The refusal is a correct decision; the defect is that it discards the input it refused, so recovery costs a full resend. That is a cost of recovery, not a wrong gate decision, and the roster has no class for it.

## Symptom (Effect)
```
file already exists: /home/marius/.cache/cc-tmp/claude-1000/.../scratchpad/mutate.py
hint: Use edit_file to modify, or pass overwrite: true to replace. create_file is for new files only.
```

The text is built in `src/tools/create_file.rs` and normalised to the `target_already_exists` family by `src/usage/db.rs` (`msg.starts_with("file already exists:")`).

## Reproduction
1. `create_file(path=<existing file>, content=<large>)`. It refuses.
2. `create_file(path=<same>, content=<same large>, overwrite=true)`. The payload crosses the wire twice.

Not re-run for this file; the symptom is taken from `usage.db` rows.

## Environment
codescout on branch `experiments`, 2026-11-08. In this project's `usage.db` the sampled rows are scratchpad scripts, so overwrite is the usual intent.

## Root cause
`CreateFile::call` checks `!overwrite && resolved.exists()` and returns `RecoverableError::with_hint(..)` at `src/tools/create_file.rs:78-84` (read this session). It keeps no copy of the input. Inferred from the source; not measured at runtime.

## Evidence
Measured 2026-11-08 over every `usage.db` under `~/work` larger than 1 MB and modified after 2026-10-01, same method as the sibling files: failed write-tool calls with input over 1000 bytes whose next call of the same tool in the same session (within 6 calls) had an input length within 30% of the failed one.

| family | failed | resent | resent bytes |
|---|---|---|---|
| `target_already_exists` | 100 | 93 | 437,729 |

Limits: the match is by length, so an edited resend counts the same as an identical one, and an agent that correctly switched to `edit_file` with a similar-sized diff is counted too. Upper bound on avoidable bytes, not a measured waste.

## Hypotheses tried
1. **Hypothesis:** callers could use `edit_file` instead. **Verdict:** not tested. For a script rewritten from scratch the whole content is the change.

## Fix

`create_file` keeps the input when it refuses an existing path. In `CreateFile::call` (`src/tools/create_file.rs`) the refusal stores the input under an `@ack_*` handle (`OutputBuffer::store_overwrite_replayable_write`) and its hint names `create_file(ack="@ack_…")`, the existing file's size and its age. The handle is minted inside the tool, where the resolved path and the input are both at hand, so no error text is parsed.

`ack` replays through `CodeScoutServer::replay_ack` (renamed from `replay_pin_ack`; it serves three refusals now). The handle carries an `OverwriteGuard` (path, size, modification time) beside `pin_root`, because an overwrite is not a pin. The replay inserts `overwrite=true` only while the file still has the size and modification time it had at the refusal. Otherwise it is refused and the file is left as it is.

Residual: a replay that then meets the shrink guard (a reduction of more than 50% over a file of at least 200 bytes) is refused again, with the old text that asks for `force=true` and a resend. That is a second refusal with the same cost shape, and it has no handle.

## Tests added

`src/server/overwrite_ack_tests.rs`, all through `call_tool_inner`:
- `an_existing_path_refusal_keeps_the_write_and_an_ack_replaces_the_file`: the refusal names the handle and the size, nothing is written, and the replay carries only the handle and writes the kept content verbatim.
- `an_ack_does_not_replace_a_file_that_changed_since_the_refusal`: the replay is refused and the changed file is untouched.
- `an_overwrite_ack_cannot_be_replayed_through_another_tool`.

The first test failed once on its own assertion, which looked for `ack="@ack_` in a JSON body where the quotes are escaped. The code was right, and the assertion was fixed.

## Workarounds
Resend with `overwrite: true`, or use `edit_file`.

## Resume

Done. Open follow-up: the shrink-guard refusal after a replay (see Fix, Residual).

## Fix provenance

- **SHA:** `8936f3924` (`experiments`)
- **patch-id:** `519483e90bcc`

## References
`src/tools/create_file.rs`; `src/usage/db.rs`; `src/tools/core/write_ack.rs`; `docs/issues/archive/2026-11-08-read-only-refusal-makes-the-agent-resend-the-whole-write.md`.
