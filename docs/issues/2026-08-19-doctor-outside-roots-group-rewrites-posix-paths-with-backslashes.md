---
status: open
opened: 2026-08-19
closed:
severity: low
owner: marius
related: []
tags: [windows, librarian, doctor, path-handling]
kind: bug
---

# BUG: `outside_roots_group()` silently rewrites POSIX-style artifact paths with backslashes on Windows, breaking both its display string and a map-key lookup

## Summary
`src/librarian/tools/doctor.rs`'s `outside_roots_group()` builds its return
value with `std::path::PathBuf`, which renders with native `\` separators on
Windows regardless of the input's separators. Artifact paths are always
POSIX-style strings by contract, so on Windows this function silently
corrupts the "reporting key" it's documented to produce — both directly (the
displayed group string) and indirectly (a map keyed by the same function's
output no longer matches a POSIX-style lookup key). Same defect class as the
already-fixed bug `c8902da802c0117d` (symbols glob overview `Path::display()`
on Windows).

## Symptom (Effect)
```
# outside_roots_group_uses_the_project_prefix_before_docs
left:  "\home\u\work\proj"
right: "/home/u/work/proj"
panicked at src\librarian\tools\doctor.rs:2541

# outside_roots_by_project_counts_elided_rows_too
looked up entry for "/elsewhere/alpha" -> Null (expected Number(13))
```

## Reproduction
1. `git checkout experiments` at `5b54848fd2a4e7fe5da6bf277dc85de39958ff27`
2. `cargo +1.97.1-x86_64-pc-windows-gnu test --release --features server-stack --lib librarian::tools::doctor::tests::outside_roots_by_project_counts_elided_rows_too librarian::tools::doctor::tests::outside_roots_group_uses_the_project_prefix_before_docs -- --nocapture`
3. Observe the failures above.

## Environment
Windows 11 Enterprise 10.0.26200 (VDI), `1.97.1-x86_64-pc-windows-gnu`
toolchain (host toolchain forced to gnu — this VDI has no MSVC C++ Build
Tools; see `docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`).
`codescout` repo, `experiments` branch.

## Root cause
`src/librarian/tools/doctor.rs:831-843`:
```rust
fn outside_roots_group(path: &str) -> String {
    let p = std::path::Path::new(path);
    let mut prefix = std::path::PathBuf::new();
    for comp in p.components() {
        if comp.as_os_str() == "docs" {
            return prefix.to_string_lossy().into_owned();
        }
        prefix.push(comp);
    }
    ...
}
```
The function's own doc comment calls it "deliberately a *reporting* key, not
a managed-root lookup" — i.e. it should be pure string manipulation over a
POSIX-contract path, not real filesystem-path semantics. `PathBuf::push` +
`to_string_lossy` round-trip transparently on POSIX hosts but re-serialize
with Windows' native `\` separator regardless of the input's separators —
exactly the `Path::display()`-on-POSIX-data mistake `c8902da802c0117d`
already fixed once, in a different call site.

The map-key failure (`outside_roots_by_project_counts_elided_rows_too`) is
the same bug one level removed: `.entry(outside_roots_group(&v.path))` at
`doctor.rs:264` inserts under a backslash-separated key on Windows, so a
POSIX-style lookup key (`"/elsewhere/alpha"`) misses.

*Measured 2026-08-19 (subagent investigation): reproduced via
`cargo +1.97.1-x86_64-pc-windows-gnu test ... -- --nocapture`, actual vs
expected strings captured above.*

## Evidence
### Subagent investigation (2026-08-19)
Both `outside_roots_group()` (doctor.rs:831-843) and its two unit tests
were introduced together, fresh, in commit `27309362` ("fix(doctor): make
the outside-roots sample stable and its remainder reachable", 2026-08-14) —
part of the experiments fast-forward, never previously exercised on a
Windows host/toolchain.

## Hypotheses tried
1. **Hypothesis:** Windows `PathBuf` round-tripping silently normalizes
   separators, breaking a function contractually meant to stay POSIX-style.
   **Test:** Read `outside_roots_group`'s implementation and doc comment;
   compared actual vs expected test output.
   **Verdict:** confirmed.
   **Evidence link:** Root cause section above.

## Fix
Not yet implemented. `outside_roots_group` should operate on the path as a
plain string (split on `/` directly, or normalize with the project's
existing `to_forward_slash` helper — see `src/util/fs.rs`) instead of
routing through `std::path::Path`/`PathBuf`, since the function's own
contract says it isn't doing real filesystem-path work.

## Tests added
N/A — not yet fixed. The two existing tests (`doctor.rs:2482-2508`,
`doctor.rs:2540-2551`) already cover this once corrected.

## Workarounds
None needed for correctness on POSIX hosts (the bug is Windows-only); on
Windows, the "outside roots" doctor report's grouping/counts are unreliable
until fixed.

## Resume
Rewrite `outside_roots_group` (`src/librarian/tools/doctor.rs:831-843`) to
do string-only prefix extraction (no `std::path::Path`/`PathBuf`), matching
the pattern the project's `to_forward_slash` helper already uses elsewhere.
Re-run the two cited tests to confirm.

## References
- `src/librarian/tools/doctor.rs:831-843` (`outside_roots_group`)
- `src/librarian/tools/doctor.rs:264` (map-key call site)
- `src/librarian/tools/doctor.rs:2482-2508`, `:2540-2551` (the two failing tests)
- `c8902da802c0117d` (fixed, archived) — prior instance of the same defect class
- commit `27309362` ("fix(doctor): make the outside-roots sample stable and its remainder reachable")
