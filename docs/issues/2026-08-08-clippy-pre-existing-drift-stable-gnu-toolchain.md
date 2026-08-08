---
status: open
opened: 2026-08-08
closed:
severity: low
owner: marius
related: []
tags: [clippy, toolchain, ci]
kind: bug
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
Not attempted in this session — out of scope for Task 1 (pure embedder
trait-object refactor; brief names only `src/retrieval/{embedder,client,
search,sync}.rs`). Candidate one-line fixes if pursued: rewrite the
`thread_local!` block per whatever clippy 0.1.96 actually wants, add
`.wait()` (or an explicit rationale) after the test-only `spawn()` in
`windows.rs:362`, and drop the `return` in `path_security.rs:1485`.

## Tests added
N/A — no fix attempted; this file documents a pre-existing gate
failure discovered while verifying Task 1.

## Workarounds
Run `cargo clippy` (without `--all-targets`) to skip `#[cfg(test)]`
code, or scope clippy to the changed files/crate when validating a
specific change.

## Resume
Confirm whether the pinned `1.97.1-msvc` toolchain (once linkable, or
on a host with MSVC available) reproduces these 3 lints. If it does
not, this is toolchain-substitute drift and the fix is either pinning
CI to match, or applying the 3 one-line fixes described in `## Fix`.

## References
- `.superpowers/sdd/2026-08-08-local-onnx-embedder/task-1-brief.md`
- memory `gotchas` — MSVC linker unavailable, `+stable-x86_64-pc-windows-gnu` substitute
