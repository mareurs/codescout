---
status: fixed
opened: 2026-08-19
closed: 2026-08-19
severity: low
owner: marius
related: []
tags: [clippy, windows, test-only]
kind: bug
---

# BUG: `INJECT_FAIL_AFTER_EVENT_INSERT` test thread_local missing the scoped clippy allow its sibling already documents

## Summary
`src/librarian/tools/event_create.rs`'s test-only `INJECT_FAIL_AFTER_EVENT_INSERT`
thread_local (line 481) already uses the correct `const { ... }` initializer
but was missing the `#[allow(clippy::missing_const_for_thread_local)]`
scoped allow that its sibling `GENERATOR` thread_local (line 250-252, in
`next_monotonic_id`) already carries with an explanatory comment for the
exact same known clippy false-positive on this Windows toolchain. Blocked
`cargo clippy --all-targets -- -D warnings`.

## Symptom (Effect)
```
error: initializer for `thread_local` value can be made `const`
   --> src\librarian\tools\event_create.rs:481:5
    = note: `-D clippy::missing-const-for-thread-local` implied by `-D warnings`
error: could not compile `codescout` (lib test) due to 1 previous error
```

## Reproduction
1. `git checkout experiments` (any commit from before this fix)
2. `cargo +1.97.1-x86_64-pc-windows-gnu clippy --release --features server-stack --all-targets -- -D warnings`
3. Observe the error above.

## Environment
Windows 11 Enterprise 10.0.26200 (VDI), `1.97.1-x86_64-pc-windows-gnu`
toolchain. `codescout` repo, `experiments` branch. Not Windows-specific in
principle, but the sibling comment notes this false-positive is specific to
the Windows toolchain's clippy build.

## Root cause
Pre-existing, unrelated to any of the 2026-08-19 Windows-test-suite fixes —
confirmed via `git status` that `event_create.rs` was untouched by any of
those. Simply an oversight: when the `GENERATOR` thread_local
(`next_monotonic_id`, line 259-263) got its scoped allow for this exact
clippy false-positive, the test-module's `INJECT_FAIL_AFTER_EVENT_INSERT`
thread_local (added separately) did not receive the same treatment.

## Evidence
Discovered running the final `cargo clippy --all-targets -- -D warnings`
verification gate after landing 5 unrelated bug fixes on this branch — the
gate was otherwise clean until this surfaced.

## Fix
Added the same `#[allow(clippy::missing_const_for_thread_local)]` (with a
comment cross-referencing the sibling) directly above the `thread_local! {`
block at `src/librarian/tools/event_create.rs:481`, matching the existing
pattern at line 250-252.

Fixed on `experiments`, base commit `66ed27dea7f48557ddfa25886527f5d6c1a7ccaa`
(fast-forward — no separate master SHA needed).

## Tests added
N/A — clippy-only, no runtime behavior change. `cargo clippy --all-targets -- -D warnings` clean afterward is the confirmation.

## Workarounds
N/A — fixed immediately on discovery.

## Resume
Fixed. N/A.

## References
- `src/librarian/tools/event_create.rs:250-252` (the sibling pattern this copies)
- `src/librarian/tools/event_create.rs:481` (the fix site)
