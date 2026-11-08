---
id: '7b7154df28e2978c'
kind: bug
status: open
title: create_file's shrink-guard refusal discards its input, so the caller resends the whole payload with force
tags:
- cluster/unclassified
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
None measured. The `usage.db` family for this refusal has not been counted, so the size of the cost is unknown.

## Hypotheses tried
None.

## Fix
Not done. Candidate: mint a handle here too, with the existing-path marker (`OverwriteGuard`) and an input that carries `overwrite=true, force=true` on replay. The refusal's text must say how much would be lost, which `report.describe()` already does. See the sibling fix: `docs/issues/archive/2026-11-08-create-file-existing-path-refusal-makes-the-agent-resend-the-whole-write.md`.

## Tests added
None. This file records the defect only.

## Workarounds
Resend with `force=true`, or use `edit_file`.

## Resume
Start from `CodeScoutServer::replay_ack` in `src/server.rs` and `src/server/overwrite_ack_tests.rs` for the test shape.

## References
`src/tools/create_file.rs`; `src/tools/output_buffer.rs`; `src/server.rs`.
