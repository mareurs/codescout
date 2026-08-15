---
kind: bug
status: fixed
tags:
- clippy
- toolchain
- ci
closed: 2026-08-15
opened: 2026-08-08
owner: marius
related: []
severity: low
---

# BUG: `cargo clippy --all-targets -- -D warnings` fails on 3 pre-existing lints under the `stable-x86_64-pc-windows-gnu` substitute toolchain

## Summary
On this Windows host, the pinned `rust-toolchain.toml` toolchain
(`1.97.1-msvc`) cannot link (no MSVC linker available), so all cargo
commands here use `+stable-x86_64-pc-windows-gnu` instead (documented
gotcha, not new). Under that substitute toolchain, `cargo clippy
--all-targets -- -D warnings` fails on 3 lints in code untouched by
Task 1 of the local-onnx-embedder plan. Confirmed pre-existing via
`git stash` — the same 3 errors reproduce on a clean tree with none of
this session's changes applied.

## Symptom (Effect)
```
error: initializer for `thread_local` value can be made `const`
   --> src\librarian\tools\event_create.rs:423:5
error: spawned process is never `wait()`ed on
   --> src\platform\windows.rs:362:21
error: unneeded `return` statement
    --> src\util\path_security.rs:1485:17
error: could not compile `codescout` (lib test) due to 3 previous errors
```
All 3 sites are inside `#[cfg(test)]` modules.

## Reproduction
```
git rev-parse HEAD   # cdac68f9 (branch feat/local-onnx-embedder) at time of observation
cargo +stable-x86_64-pc-windows-gnu clippy --all-targets -- -D warnings
```
Reproduces on a clean tree (verified via `git stash` / `git stash pop`
during Task 1 work — no code changes needed to trigger it).

## Environment
Windows 11, `clippy 0.1.96 (ac68faa20c 2026-05-25)` / `rustc 1.96.0`
via `+stable-x86_64-pc-windows-gnu` (the pinned `1.97.1-msvc` cannot
link on this host — see memory `gotchas`). Project: codescout, branch
`feat/local-onnx-embedder`.

## Root cause
Unknown — see Hypotheses tried. Likely lint-version drift between the
pinned toolchain (`1.97.1-msvc`, presumably what the code was last
cleaned against) and the `1.96.0`-gnu substitute forced by this host's
missing MSVC linker.

*Measured 2026-08-08: `git stash` then `cargo +stable-x86_64-pc-windows-gnu
clippy --all-targets -- -D warnings` on the clean tree reproduces all 3
errors verbatim → confirms the drift predates and is unrelated to Task 1's
`CodeEmbedder` trait-object refactor.*

## Evidence
### `event_create.rs:423` — lint fires despite an existing `const { }` block
```rust
thread_local! {
    pub(super) static INJECT_FAIL_AFTER_EVENT_INSERT: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}
```
The initializer is already wrapped in `const { }`, which is clippy's
own documented fix for this lint — suggests a lint false-positive or a
different expectation in this clippy version.

## Hypotheses tried
1. **Hypothesis:** These are new warnings introduced by Task 1's edits.
   **Test:** `git stash`, re-ran clippy on the clean tree.
   **Verdict:** rejected — same 3 errors on the clean tree.
2. **Hypothesis:** Toolchain-version drift (`1.97.1-msvc` pinned vs
   `1.96.0`-gnu substitute) changed which lints fire.
   **Verdict:** deferred — plausible given the `const{}` false-positive
   in Evidence, not confirmed against the pinned MSVC toolchain (can't
   link it on this host to compare).

## Fix

Shipped in `2f76136e`. The filing's framing — "pre-existing lints on a Windows
host, out of scope here" — was wrong on both halves.

**They reproduce on Linux.** `x86_64-pc-windows-gnu` is an installed rustup
*target*, so `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D
warnings` lints the Windows cfg from this machine. No Windows host required.
That single command is what turned this from a triage note into a fix.

**Only 2 of the 3 fire.** `event_create.rs`'s `thread_local` lint is gone under
clippy 1.97 — as this file already suspected, the initializer was wrapped in
`const { }`, which *is* that lint's documented fix, so it was a version
artifact. Nothing to change there.

1. **`path_security.rs` — `needless_return`.** Not really about the `return`.
   Every assertion in `symlink_to_denied_path_is_caught_on_read` lived inside
   `#[cfg(unix)]`, so on Windows the body reduced to "look up `$HOME/.ssh`, then
   stop": a test that ran, asserted nothing, and reported `ok`. The `return`
   became needless *because everything after it compiled out* — the lint was
   pointing at the empty test. Gating the whole test `#[cfg(unix)]` removes the
   thing the lint was complaining about and drops six now-redundant inner `cfg`
   attributes. Same shape as the `server-stack` lane
   (`tests/feature_lanes.rs`): coverage that looks present because a name is in
   the test list.
2. **`windows.rs` — `zombie_processes`.** A real leak: an unwaited `Child` holds
   the process handle open for the life of the test binary. Liveness is now
   sampled into a local *before* the reap, so the assertion observes exactly
   what it did before.

**Systemic half.** The `windows-gnu` CI lane built and tested that target but
never linted it — so the host Clippy job was the only lint gate, and it cannot
see `#[cfg(windows)]` code at all. `scripts/build-windows.sh` gains a `clippy`
mode (so the local loop and CI run the identical command) and the lane now runs
`scripts/build-windows.sh clippy --all-targets -- -D warnings`.
## Tests added

No new test — the gate *is* the regression guard, and it is a CI lane rather
than a `#[test]` because the invariant is "this target lints clean", which no
host-target test can assert.

Verified locally, both directions:

- Before: `cargo clippy --target x86_64-pc-windows-gnu --all-targets -- -D
  warnings` → 2 errors (`windows.rs:362` zombie_processes,
  `path_security.rs:1480` needless_return).
- After: same command → clean. Via the new script mode
  (`scripts/build-windows.sh clippy --all-targets -- -D warnings`) → clean.
- Linux unaffected: `cargo clippy --workspace --all-targets -- -D warnings`
  clean, and `symlink_to_denied_path_is_caught_on_read` still runs and passes
  there (it is unix-gated, not deleted).
## Workarounds
Run `cargo clippy` (without `--all-targets`) to skip `#[cfg(test)]`
code, or scope clippy to the changed files/crate when validating a
specific change.

## Resume

Closed. The reusable lesson is narrower and more useful than the bug:
**"reproduces only on platform X" deserves one check of `rustup target list
--installed` before it is believed.** Cross-target `clippy` needs no linker, no
emulator, and no second machine — it type-checks and lints another platform's
cfg from wherever you are. This file sat scoped-to-a-host for a week behind a
command that takes 40 seconds.
## References
- `.superpowers/sdd/2026-08-08-local-onnx-embedder/task-1-brief.md`
- memory `gotchas` — MSVC linker unavailable, `+stable-x86_64-pc-windows-gnu` substitute
