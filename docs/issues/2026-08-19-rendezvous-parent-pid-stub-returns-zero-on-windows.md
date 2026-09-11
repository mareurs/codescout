---
status: fixed
opened: 2026-08-19
closed: 2026-08-19
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
Implemented direction (a): real Windows PPID detection via a
`CreateToolhelp32Snapshot` + `Process32FirstW`/`Process32NextW` walk
(`windows-sys`'s `Win32_System_Diagnostics_ToolHelp` bindings — Windows has
no `getppid()`, so the parent PID has to be read off this process's own
`PROCESSENTRY32W` entry in a full-process snapshot).

`Cargo.toml`: added the `Win32_System_Diagnostics_ToolHelp` feature to the
existing `[target.'cfg(windows)'.dependencies] windows-sys` entry.

`src/tools/rendezvous.rs:159-193` (replacing the old `0` stub):
```rust
#[cfg(windows)]
fn parent_pid() -> u32 {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let pid = std::process::id();
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return 0;
    }
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut ppid = 0u32;
    unsafe {
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                if entry.th32ProcessID == pid {
                    ppid = entry.th32ParentProcessID;
                    break;
                }
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }
    ppid
}
```
The `snapshot == INVALID_HANDLE_VALUE` / "this process not found in its own
snapshot" paths still return `0`, preserving the original stub's safe
"never matched" degradation for the failure case — only the success path
changed from always-`0` to an actual PPID.

Fixed on `experiments`, base commit `66ed27dea7f48557ddfa25886527f5d6c1a7ccaa`
(fast-forward branch — no separate master SHA needed, per this repo's
CLAUDE.md git-workflow section).
## Tests added
No new test — the existing
`tools::rendezvous::tests::publish_records_the_parent_pid_the_hook_matches_on`
(`src/tools/rendezvous.rs:240-247`) is the regression test; it now exercises
real Windows PPID detection instead of failing against the old stub.

Verified 2026-08-19 (Windows, `1.97.1-x86_64-pc-windows-gnu`, release +
`server-stack`):
```
test tools::rendezvous::tests::publish_records_the_parent_pid_the_hook_matches_on ... ok
```
## Workarounds
None; the rendezvous PID-hook match silently never fires on Windows
(degrades to "never matched" per the stub's own design intent, not a crash).

## Resume
Fixed. N/A.
## References
- `src/tools/rendezvous.rs:153-163` (parent_pid, both platform branches)
- `src/tools/rendezvous.rs:240-247` (the failing test)
- commit `87e85bf2` ("feat(rendezvous): publish a pid-keyed slot for the companion to stamp")
