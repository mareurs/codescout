---
id: 41ee521667ef3437
kind: bug
status: fixed
title: create_file's shrink-guard refusal discards its input, so the caller resends the whole payload with force
tags:
- cluster/unclassified
closed: 2026-11-09
opened: 2026-11-08
---

# BUG: create_file's shrink-guard refusal discards its input, so the caller resends the whole payload with force

## Summary
`create_file(overwrite=true)` over a file of at least 200 bytes refuses when the new content cuts the file by more than 50%. The hint asks for `force=true`, and the refusal keeps nothing, so the caller sends `content` again. Same cost shape as the existing-path refusal, which now has an `ack` replay.

## Class fit
No roster class fits without forcing it. The refusal is a correct decision; the defect is that it discards the input, so recovery costs a full resend.

## Symptom (Effect)
```
shrink guard: overwriting '<path>' <report>
hint: Read the existing file before replacing it. If the reduction is intentional, repeat create_file(path=..., content=..., overwrite=true, force=true).
```

## Reproduction
Not run for this file. Read in `src/tools/create_file.rs` (`CreateFile::call`, the `overwrite && resolved.exists() && !force` branch) on 2026-11-08, while fixing the existing-path sibling.

## Environment
codescout on branch `experiments`, 2026-11-08.

## Root cause
The branch returns `RecoverableError::with_hint(..)` and keeps no copy of the input. Inferred from the source; not measured at runtime. It also covers an `ack` replay of the existing-path refusal: a replay over a file that the content would shrink by more than 50% meets this refusal, with no handle.

## Evidence

Measured 2026-11-08 in this project's `.codescout/usage.db` only: `select err_family, count(*) from tool_calls where tool_name='create_file' and outcome<>'success' group by 1`.

| family | failed calls |
|---|---|
| `worktree_activate_required` | 34 |
| `target_already_exists` | 29 |
| `write_scope_denied` | 5 |
| (no family) | 4 |
| `shrink_guard_blocked` | 2 |
| `missing_required_param` | 1 |

So this refusal is about 1/15 of the existing-path one in this database. Limits: one project's database, not the all-databases sweep the sibling files used; the table counts refusals, not resends, so it does not say how many were followed by a resend. Low observed cost, so the fix is not urgent.

## Hypotheses tried
None.

## Fix

Fixed in `a52d68284` on `experiments` (patch-id `730f0d94a45e`).

The shrink-guard branch of `CreateFile::call` now builds an `OverwriteGuard` of the file, sets its new `force` flag, and stores the input with `store_overwrite_replayable_write`. The refusal text keeps its old wording and adds the handle. `replay_ack` inserts `overwrite=true`, and `force=true` when the guard's `force` is set. The replay is refused when the file's size or modification time changed since the refusal. An `ack` replay of the existing-path refusal that meets the shrink guard now mints its own handle, which closes the gap that Root cause named.

Verified live on a rebuilt server on 2026-11-09. A 400-byte file, `create_file(content="tiny", overwrite=true)`: refused with a handle. `create_file(ack=...)` with the pin and nothing else wrote `tiny` (4 bytes). With the file changed to 500 bytes between the refusal and the replay, the replay was refused with "changed since create_file refused to overwrite it" and the file stayed at 500 bytes.

## Tests added

In `src/server/overwrite_ack_tests.rs`: `a_shrink_guard_refusal_keeps_the_write_and_an_ack_applies_it_with_force`, `an_existing_path_ack_that_meets_the_shrink_guard_mints_a_second_handle`, `a_shrink_guard_ack_does_not_apply_to_a_file_that_changed_since_the_refusal`. With the `force` insert in `replay_ack` disabled, the first two fail; the third passes, as it should. Gate green on 2026-11-09: LEAN 6287, DEFAULT 9162, Python 1764 passed, clippy clean.

## Workarounds
Resend with `force=true`, or use `edit_file`.

## Resume

Nothing left to do: the fix is committed, verified live and the gate is green.

## Fix provenance

- **SHA:** `a52d68284` (`experiments`)
- **patch-id:** `730f0d94a45e`

## References
`src/tools/create_file.rs`; `src/tools/output_buffer.rs`; `src/server.rs`.
