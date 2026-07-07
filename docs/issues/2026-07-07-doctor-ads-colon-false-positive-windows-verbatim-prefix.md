---
status: fixed
opened: 2026-07-07
closed: 2026-07-07
severity: medium
owner: marius
related: [2026-07-07-activate-hint-home-path-not-forward-slash-normalized.md, 2026-07-07-upstream-try-build-runtime-stray-arg-compile-break.md]
tags: [windows, cross-platform, path-separator, librarian, doctor]
kind: bug
---

# BUG: `librarian(doctor)` false-positives every Windows-indexed artifact as `ads_colon_in_abs_path`

## Summary
`doctor`'s `ads_colon_in_abs_path` check exempts only a bare drive-letter
prefix (`C:/...`) from its NTFS-alternate-data-stream colon scan. On
Windows, `artifact.abs_path` is stored in the extended-length ("verbatim")
path form (`//?/C:/Users/...` — forward-slash rendering of `\\?\C:\...`
from `std::fs::canonicalize`), so the real drive-letter colon sits at byte
5, past the 4-byte `//?/` marker. The check misread this as an ADS
selector, flagging effectively every artifact indexed on this machine.

## Symptom (Effect)
```
{
  "check": "ads_colon_in_abs_path",
  "artifact_id": "d3d816200939d5b6",
  "path": "CHANGELOG.md",
  "detail": "colon at byte position 5 (outside drive prefix)"
}
```
Repeated for ~2000+ artifacts (effectively every row in the catalog) when
running `librarian(action="doctor")` on this Windows workspace.

## Reproduction
1. Commit: `experiments` @ `1a3b6fc2` (or later)
2. Index/onboard a project on Windows so `artifact.abs_path` rows are
   populated via `std::fs::canonicalize` (extended-length form).
3. `librarian(action="doctor")` → every row's `abs_path` triggers
   `ads_colon_in_abs_path` at "byte position 5".

## Environment
- OS: Windows, PowerShell 7
- codescout v0.15.0, `experiments` branch, `librarian`/`server-stack` feature

## Root cause
[src/librarian/tools/doctor.rs:249](../../src/librarian/tools/doctor.rs#L249)
`check_ads_colon()` — `starts_with_drive` only checks `bytes[0..2]` for the
`X:` shape. It never accounts for a leading `//?/` verbatim-prefix marker,
so on a path like `//?/C:/Users/...` bytes[0..2] are `//` (not a letter+`:`
pair), the drive-prefix exemption never fires, and the drive colon at byte 5
is treated as a bare, unexplained colon.

## Evidence
`workspace(activate)` in this session returned
`project_root: "//?/C:/Users/MAILINCA.BRN.002/..."`. Counting bytes in
`//?/C` gives position 5 for the following `:` — exactly matching every
violation's reported "colon at byte position 5 (outside drive prefix)".

## Hypotheses tried
1. **Hypothesis**: the colon really is an ADS selector (corrupted path).
   **Test**: checked one flagged path (`CHANGELOG.md`, stripped display
   form) against the full stored `abs_path` implied by the project root —
   `//?/C:/Users/.../codescout/CHANGELOG.md`. No `:` appears anywhere except
   the drive-letter position (shifted by the verbatim prefix).
   **Verdict**: rejected — false positive, not real corruption.

## Fix
[src/librarian/tools/doctor.rs](../../src/librarian/tools/doctor.rs) —
`check_ads_colon()` now strips a leading `//?/` marker (if present) before
applying the drive-letter exemption, and adjusts the reported byte position
by the stripped prefix length. Added two regression tests:
`check_ads_colon_exempts_verbatim_prefix_drive_colon` and
`check_ads_colon_flags_ads_colon_after_verbatim_prefix` (confirms a genuine
post-verbatim-prefix ADS colon is still caught).

## Tests added
- `check_ads_colon_exempts_verbatim_prefix_drive_colon` — `src/librarian/tools/doctor.rs`
- `check_ads_colon_flags_ads_colon_after_verbatim_prefix` — `src/librarian/tools/doctor.rs`

## Workarounds
None needed — false positives only, no data corruption; safe to ignore
prior `doctor` reports on Windows workspaces pending this fix.

## Resume
N/A — fixed, `cargo test --lib -- doctor`: 14 passed, 0 failed.

## References
- [src/librarian/tools/doctor.rs:249-273](../../src/librarian/tools/doctor.rs#L249-L273) — the fixed check
- [docs/issues/2026-07-07-upstream-try-build-runtime-stray-arg-compile-break.md](2026-07-07-upstream-try-build-runtime-stray-arg-compile-break.md) — sibling defect found in the same debugging pass (stray `lsp:` field in `src/librarian/tools/mv.rs` test fixture, also fixed this session)
