---
id: aab4cfc52a5e7504
kind: bug
status: fixed
title: 'BUG: doctor''s check_ads_colon flags every Windows-indexed abs_path as an ADS-colon violation (verbatim-prefix drive-letter miss)'
owners:
- marius
tags:
- windows
- cross-platform
- librarian-doctor
- path-normalization
topic: null
time_scope: null
closed: '2026-07-07'
opened: '2026-07-07'
related:
- docs/issues/2026-07-07-artifact-get-full-body-silent-truncation.md
severity: medium
---

## Summary

`librarian(action="doctor")`'s `check_ads_colon` catalog-drift check flagged every Windows-indexed artifact's `abs_path` with `ads_colon_in_abs_path`, because it only recognized a bare 2-byte drive-letter prefix (`C:`) as exempt from the NTFS-alternate-data-stream-colon check. On Windows, `abs_path` is stored in the extended-length verbatim form `fs::canonicalize` produces (`\\?\C:\Users\...`), rendered via this repo's `to_forward_slash` normalization (`src/util/fs.rs:106-107`) as `//?/C:/Users/...`. The real drive-letter colon then sits at byte 5, past the 4-byte `//?/` marker — a false positive on every single Windows-indexed row.

## Symptom (Effect)

`librarian(action="doctor")` reported an `ads_colon_in_abs_path` violation, with detail `"colon at byte position 5 (outside drive prefix)"`, for every artifact indexed on a Windows checkout.

## Reproduction

```rust
// src/librarian/tools/doctor.rs, pre-fix
check_ads_colon("a1", "//?/C:/Users/marius/foo.md")
// → Some(Violation { check: "ads_colon_in_abs_path", detail: "colon at byte position 5 (outside drive prefix)", .. })
// Expected: None (this is a legitimate Windows path, not an ADS selector)
```

`git rev-parse HEAD` at time of fix: `a92c734fde3e5901e37b57904f5dd16f1cfc2113` (branch `experiments`, uncommitted working-tree change).

## Environment

Windows only (any Windows checkout with `fs::canonicalize`-derived `abs_path` values in the catalog). Not reproducible on Linux/macOS, where `abs_path` never carries the `//?/` marker.

## Root cause

`check_ads_colon` (`src/librarian/tools/doctor.rs:249`, pre-fix) computed:

```rust
let bytes = abs_path.as_bytes();
let starts_with_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
```

This only exempts a drive letter sitting at byte offset 0..2. It has no awareness of the Windows verbatim-prefix marker (`\\?\`, forward-slash-rendered as `//?/`) that `fs::canonicalize` prepends on Windows — a pattern this codebase already has a correct reference implementation for in `src/util/path_security.rs:150-176` (`is_denied::normalize`, added for an unrelated deny-list-bypass bug, with the exact same root-cause comment about `\\?\` shifting byte positions). `check_ads_colon` was never updated to match that precedent.

A second call site with the identical `starts_with_drive` fragment exists at `check_abs_path_must_be_absolute` (`doctor.rs:296-322`, now line ~296) — audited as part of this fix and confirmed **not** buggy: it's OR'd with `bytes.first() == Some(&b'/')`, and `//?/C:/...` already starts with `/`, so it's correctly treated as absolute regardless of the drive-letter check. No fix needed there.

## Evidence

Pre-fix reproduction (see Reproduction section) and the 12 pre-existing `doctor.rs` unit tests (none of which covered the verbatim-prefix shape — `check_ads_colon_exempts_drive_prefix` only asserted the bare `C:/Users/...` form).

Cross-reference: `src/util/path_security.rs:151-156`'s comment documents the identical mechanism (`fs::canonicalize` verbatim form, component-wise `starts_with` mismatch) for a different bug (deny-list bypass), confirming this is a recurring, previously-solved class of issue in this codebase that `check_ads_colon` simply didn't inherit.

## Hypotheses tried

1. **Hypothesis:** The bug also affects `check_abs_path_must_be_absolute`, which shares the same `starts_with_drive` code fragment.
   **Test:** Traced the function's full logic — `starts_with_posix_root || starts_with_drive` — and confirmed `//?/C:/...` satisfies `starts_with_posix_root` (leading `/`) independent of the drive check.
   **Verdict:** rejected — not buggy, no fix applied.
   **Evidence link:** Root cause section, second paragraph.
2. **Hypothesis:** `gather.rs::guard_relative_path`'s identical-looking `starts_with_drive` fragment has the same bug.
   **Test:** Read the function — it's a relative-path guard that additionally blanket-rejects any colon (`path.contains(':')`), and is never fed a `//?/`-prefixed absolute path (it exists to reject absolute-looking input outright).
   **Verdict:** rejected — different context, not exposed to the buggy shape.

## Fix

Implemented, uncommitted on `experiments` (`a92c734fde3e5901e37b57904f5dd16f1cfc2113` + working-tree diff). `check_ads_colon` (`src/librarian/tools/doctor.rs:249-273`) now strips a leading `//?/` marker (4 bytes) before computing `starts_with_drive`, and adjusts the reported byte offset to account for the stripped prefix:

```rust
let verbatim_prefix_len = if abs_path.starts_with("//?/") { 4 } else { 0 };
let rest = &abs_path[verbatim_prefix_len..];
let bytes = rest.as_bytes();
let starts_with_drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
let tail = if starts_with_drive { &rest[2..] } else { rest };
tail.find(':').map(|pos_in_tail| {
    let absolute_pos = pos_in_tail + verbatim_prefix_len + if starts_with_drive { 2 } else { 0 };
    // ...
})
```

No other call site required a fix (see Hypotheses tried).

## Tests added

- `check_ads_colon_exempts_verbatim_prefix_drive_colon` (`src/librarian/tools/doctor.rs:395`) — `//?/C:/Users/marius/foo.md` must not be flagged.
- `check_ads_colon_flags_ads_colon_after_verbatim_prefix` (`src/librarian/tools/doctor.rs:402`) — `//?/C:/foo.txt:stream` must still be flagged, with `detail` reporting the correct absolute byte position (14).

`cargo test --lib librarian::tools::doctor::tests` → 14 passed (was 12; +2 new). Full `cargo test --lib` → 2954 passed, 0 failed, 6 ignored. `cargo fmt` and `cargo clippy --lib -- -D warnings` both clean.

## Workarounds

None needed once fixed. Prior to the fix, Windows users could ignore `ads_colon_in_abs_path` violations from `librarian(action="doctor")` for rows whose reported byte position was exactly 5 with a drive-letter-shaped path around it (heuristic, not reliable for genuine ADS-colon rows on Windows).

## Resume

N/A — fixed and verified. If cherry-picked to `master`, update this file's `closed:`/`status:` fields per the archive convention and archive after `git branch --contains <master-sha>` confirms `master` is present.

## References

- `src/util/path_security.rs:150-176` (`is_denied::normalize`) — pre-existing correct handling of the same `\\?\`/`//?/` verbatim-prefix shape, used as the reference pattern for this fix.
- `src/util/fs.rs:106-107` (`to_forward_slash`) — confirms the exact string shape (`\\` → `/`) that produces `//?/C:/...` from `fs::canonicalize`'s `\\?\C:\...`.
- `docs/issues/2026-07-07-artifact-get-full-body-silent-truncation.md` — unrelated codescout-tool bug noticed while filing this fix's session-log entry.
- `docs/trackers/bug-fix-session-log.md` (id `2dd9d90bc83f9f49`) — F-28/W-21 entries covering this session's reconnaissance findings.

