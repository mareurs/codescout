---
status: open
opened: 2026-08-19
closed:
severity: medium
owner: marius
related: []
tags: [windows, rendezvous, hooks]
kind: bug
---

# BUG: `parent_pid()` is a hardcoded `0` stub on Windows, breaking the rendezvous PID-hook match on every Windows host

## Summary
`src/tools/rendezvous.rs`'s Windows implementation of `parent_pid()` always
returns `0` — it was never implemented, only stubbed with a comment framing
`0` as a safe "never matched" degradation. The unit test added in the same
commit asserts the recorded PPID is nonzero, so it fails deterministically.
This is not specific to this VDI or sandbox: the stub is `#[cfg(windows)]`,
so any Windows host hits the same `0`.

## Symptom (Effect)
```
thread 'tools::rendezvous::tests::publish_records_the_parent_pid_the_hook_matches_on' panicked at src\tools\rendezvous.rs:246:9:
assertion `left != right` failed: ppid must be recorded
  left: 0
 right: 0
```

## Reproduction
1. `git checkout experiments` at `5b54848fd2a4e7fe5da6bf277dc85de39958ff27`
2. `cargo +1.97.1-x86_64-pc-windows-gnu test --release --features server-stack --lib tools::rendezvous::tests::publish_records_the_parent_pid_the_hook_matches_on -- --nocapture`
3. Observe the panic above.

## Environment
Windows 11 Enterprise 10.0.26200 (VDI), `1.97.1-x86_64-pc-windows-gnu`
toolchain (host toolchain forced to gnu since this VDI has no MSVC C++
Build Tools installed — see
`docs/issues/archive/2026-08-08-cyberark-epm-blocks-ort-sys-build-script.md`
for the unrelated reason `local-embed`/`local-embed-dynamic` are also off
in this build). `codescout` repo, `experiments` branch.

## Root cause
`src/tools/rendezvous.rs:158-163`:
```rust
#[cfg(windows)]
fn parent_pid() -> u32 {
    // No getppid here. The hook walks ancestry itself, so a zero degrades to
    // "never matched" rather than to a WRONG match — the safe direction.
    0
}
```
versus the Unix branch (`:153-156`) which calls `libc::getppid()`. The
Windows PPID path was never implemented — this always returns `0`
unconditionally, regardless of caller. The companion test
(`src/tools/rendezvous.rs:240-247`) asserts `ppid != 0`, a correctness
assumption the Windows stub can never satisfy.

*Measured 2026-08-19: reproduced via `cargo test` through the
run_command→bash→cargo chain AND via direct invocation of the compiled test
binary (`target/release/deps/codescout-*.exe <test> --exact --nocapture`) —
identical `left: 0 right: 0` panic both ways, ruling out process-nesting
as the cause. The stub returns 0 regardless of caller.*

## Evidence
### Subagent investigation (2026-08-19)
```
git log confirms parent_pid() and this exact test were both introduced
together in commit 87e85bf2 ("feat(rendezvous): publish a pid-keyed slot
for the companion to stamp"), dated 2026-08-18 — brand-new code from the
experiments fast-forward, not long-standing.
```

## Hypotheses tried
1. **Hypothesis:** The `0` is a process-nesting artifact of this session's
   `run_command` → Git Bash → cargo → test-binary spawn chain, not a real
   bug.
   **Test:** Ran the test both through the normal `cargo test` wrapper and
   by directly invoking the compiled test binary, bypassing bash/cargo.
   **Verdict:** rejected — identical panic both ways.
   **Evidence link:** Evidence section above.
2. **Hypothesis:** `parent_pid()` on Windows is an intentional, permanent
   stub (per its own comment) and the test is simply wrong to assert
   nonzero.
   **Test:** Read `src/tools/rendezvous.rs:158-163` directly.
   **Verdict:** confirmed as the proximate cause — the stub is real and
   unconditional. Whether the *right* fix is "implement real Windows PPID
   detection" or "adjust the test to accept 0 as valid on Windows" is
   still open — see Fix.

## Fix
Not yet implemented. Two candidate directions:
(a) Implement real Windows PPID detection (`CreateToolhelp32Snapshot` +
`PROCESSENTRY32` walk, or the `sysinfo` crate if already a dependency) so
the hook-matching feature actually works on Windows.
(b) If the feature is intentionally POSIX-only for now, gate the test
`#[cfg(unix)]` and document that the Windows rendezvous hook-match is a
known no-op until (a) lands.

## Tests added
N/A — not yet fixed.

## Workarounds
None; the rendezvous PID-hook match silently never fires on Windows
(degrades to "never matched" per the stub's own design intent, not a crash).

## Resume
Decide (a) vs (b) above with the person who owns the rendezvous/companion
hook feature, then either implement Windows PPID detection in
`src/tools/rendezvous.rs:158-163` or add `#[cfg(unix)]` to the test at
`src/tools/rendezvous.rs:240-247` and document the Windows gap.

## References
- `src/tools/rendezvous.rs:153-163` (parent_pid, both platform branches)
- `src/tools/rendezvous.rs:240-247` (the failing test)
- commit `87e85bf2` ("feat(rendezvous): publish a pid-keyed slot for the companion to stamp")
