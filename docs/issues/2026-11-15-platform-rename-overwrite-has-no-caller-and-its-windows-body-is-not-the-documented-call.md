---
id: df45ce3cd83e2693
kind: bug
status: open
title: 'BUG: platform::rename_overwrite has no caller and its docs promise MOVEFILE_REPLACE_EXISTING while the Windows body is remove_file then rename'
owners:
- marius
tags:
- cluster/unclassified
- platform
opened: 2026-11-15
severity: low
unverified: 'Read from the code, not run on Windows. Established: no caller exists in the *.rs files under src/ and tests/. Not established: whether a script or an external crate calls it.'
---

# BUG: platform::rename_overwrite has no caller and its docs promise MOVEFILE_REPLACE_EXISTING while the Windows body is remove_file then rename

## Summary
`crate::platform::rename_overwrite` (`src/platform/mod.rs:124-129`) is documented as "Platform-aware rename that overwrites the destination. On Windows this uses `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING`." The Windows body (`src/platform/windows.rs:334-339`) does not call `MoveFileExW`. It calls `remove_file(to)` when `to` exists, then `rename(from, to)`. The Unix body (`src/platform/unix.rs:340-342`) is `std::fs::rename`. No code calls the function: `grep -rn rename_overwrite` over the `*.rs` files finds the three definitions and the wrapper, nothing else.

## Class fit
cluster/unclassified: a doc comment the code contradicts, on a function nothing calls. No existing class names dead code with a false contract.

## Symptom (Effect)
None today, because nothing calls it. A future caller that trusts the doc comment gets, on Windows, a destination that is deleted before the rename. A failure of the rename after the delete leaves neither file at the destination. It is not an atomic replace, and the doc says it is.

## Reproduction
Read path only. `rg rename_overwrite` over `src/` and `tests/`; read the two bodies.

## Environment
Branch `experiments` at `1c6ec18e3`. Found while scouting the no-clobber publish plan (`docs/superpowers/plans/2026-11-15-no-clobber-publish.md`, Finding 3).

## Root cause
The Windows body was written as a portable fallback and the doc comment describes the intended call, not the one made. (inferred from the code, not measured)

## Fix
Decide first whether the function is wanted. If not, delete it with its two bodies. If it is, give the Windows body `MoveFileExW(from, to, MOVEFILE_REPLACE_EXISTING)` (`windows-sys` is already a dependency) or `std::fs::rename`, which already replaces on Windows, and make the doc comment true. Do not use it as the no-clobber step: it replaces by design. The no-clobber step is `crate::util::fs::rename_no_clobber`.

## Tests added
None. The bug is dead code with a wrong comment.

## Resume
Open. Not started. First decision: delete the function or fix its Windows body.

## References
- `src/platform/mod.rs:124-129`, `src/platform/unix.rs:340`, `src/platform/windows.rs:334`
- `docs/superpowers/plans/2026-11-15-no-clobber-publish.md` (Finding 3)
