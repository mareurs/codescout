---
title: Windows Compatibility Tracker — April 2026
kind: tracker
status: active
topic: windows-port
tags: [windows, port, compatibility, lsp, mux]
owners: [ailinca.marius@gmail.com]
created_at: 2026-04-27
---

# Windows Compatibility Tracker — April 2026

Tracks every known Windows-port issue surfaced during the April 2026 audit on
branch `worktree-windows-compat`. Includes items that are currently
**unsolvable** (upstream-blocked) so they stay visible — silence is not
resolution.

What "Windows works" means here: a `cargo build` on `x86_64-pc-windows-msvc`
produces a binary that launches, serves MCP, drives at least one real LSP
session through the mux, and passes the bulk of `cargo test`.

## Status legend

| Code | Meaning |
|------|---------|
| ✅ done | Landed on `worktree-windows-compat` (or master) |
| 🟡 partial | Compiles but unverified at runtime, or fix narrows scope |
| 🔴 open | Identified, not started |
| 🚫 blocked | Blocked on upstream / external dependency |

## Validation gates

| Gate | Status |
|------|--------|
| `cargo check --target x86_64-pc-windows-gnu --lib` (no `local-embed`) | ✅ done — Phase B (`39e5317`) |
| `cargo check --target x86_64-pc-windows-gnu --bin codescout` (no `local-embed`) | ✅ done — Phase B |
| `cargo build --release` on actual Windows | 🔴 open |
| `cargo test` on actual Windows | 🔴 open |
| MCP smoke test (Claude Code → codescout via stdio) | 🔴 open |
| Mux runtime test (Kotlin or Java LSP via mux) | 🔴 open |
| CI `windows-latest` matrix job (windows-baseline + no-features rows) | ✅ done |

## Mux / LSP transport

### W1. Mux IPC transport seam ✅ done
- **Issue:** UnixListener/UnixStream/`remove_file`/`0o600` perms scattered
  across `mux/process.rs`, `client.rs`, `manager.rs`. No portable seam.
- **Fix:** `src/lsp/mux/transport/{mod,unix,windows}.rs`. Unix uses UDS;
  Windows uses tokio named pipes (`\\.\pipe\codescout-<lang>-mux-<hash>`).
  Caller code is platform-agnostic.
- **Commits:** `3b6f8c3` (Phase A), `39e5317` (Phase B).
- **Status:** ✅ done — compile-only verification. Runtime on Windows
  pending W11.

### W2. Mux `cfg(unix)` gates removed ✅ done
- **Issue:** `LspManager::get_or_start_via_mux`, `LspClient::connect`, and
  the dispatch site in `do_start` were whole-function-gated `cfg(unix)`.
- **Fix:** Gates dropped now that transport is portable. `build_mux_args`
  also un-gated (it was always pure string manipulation).
- **Commit:** `39e5317`.
- **Status:** ✅ done.

### W3. `client_reader_task` reads named-pipe halves on Windows 🟡 partial
- **Issue:** Generic over the read half via `transport::ServerReadHalf`
  type alias. On Unix that's `tokio::net::unix::OwnedReadHalf`; on Windows
  it's `tokio::io::ReadHalf<NamedPipeServer>`. The latter is split via
  `tokio::io::split` (no native `into_split` on `NamedPipeServer`).
- **Status:** compiles. Not runtime-tested. May exhibit different
  back-pressure or close-detection semantics than UDS owned halves.
- **Action:** verify under runtime W11.

### W4. Pipe-busy retry loop in `transport::windows::connect` 🟡 partial
- **Issue:** `ClientOptions::open` returns `ERROR_PIPE_BUSY` (231) when
  all instances are busy. We retry 20× with 100ms backoff (≈2s total).
- **Risk:** during cold-start of a slow LSP (Kotlin/Java) the retry budget
  may be insufficient. The existing Unix retry on `LspClient::start` is
  300ms × 5; Windows-side retry is layered below that, so the effective
  budget is product-of-both. Likely fine — flag for runtime test.
- **Status:** unverified.

## Process management

### W5. `run_command` timeout cleanup via Job Objects ✅ done

- **First pass (earlier commit):** added `TaskkillGuard` to mirror the
  Unix `PgidKillGuard` Drop pattern using `taskkill /T /F /PID`.
  Functional but carried a PID-reuse race window between guard
  observation and signal.
- **Upgrade (2026-04-29):** replaced `TaskkillGuard` with
  `JobObjectGuard`. After spawning the child, codescout now creates a
  Win32 Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
  and assigns the child via `AssignProcessToJobObject`. When the job
  handle is dropped (timeout, future cancel, normal completion) the
  kernel atomically terminates every process still in the job — the
  closest semantic match to Unix `killpg(SIGKILL)`. No more PID-reuse
  race.
- **Fallback:** if Job Object setup fails (e.g. parent in a job that
  disallows nested-job assignment without breakaway), the future
  installs a `TaskkillFallback` that reproduces the prior
  `taskkill /T /F` behaviour. Belt-and-braces only.
- **windows-sys features added:** `Win32_System_JobObjects`,
  `Win32_System_Threading`.
- **Send marker:** `JobObjectGuard` carries a `HANDLE` (`*mut c_void`)
  which is `!Send`. A Win32 job handle has no thread affinity and is
  safe to close from any thread, so an `unsafe impl Send` is sound and
  documented in the source.
- **Status:** ✅ done.
### W6. `terminate_process` already abstracted ✅ done
- **Location:** `src/platform/{unix,windows}.rs`.
- **Note:** Already does `taskkill /PID /F` on Windows for top-level
  process termination (LSP shutdown path). W5 is about *grandchildren*
  during shell pipeline timeouts.

## Path handling / filesystem

### W7. Symlink validation routed through platform helper ✅ done

- **Action (2026-04-29):** all production `std::fs::canonicalize` call
  sites that participate in path comparison or are forwarded to LSP
  servers now route through `crate::platform::canonicalize` /
  `canonicalize_or` (W20 helper). On Windows this strips verbatim UNC
  prefixes via `dunce`, so symlink-resolved paths compare equal to
  project roots and `extra_write_roots`.
- **Sites updated:** `path_security::best_effort_canonicalize`,
  `embed/preflight::{classify_path, scan}`,
  `library/auto_register::{find_node_source, find_python_source}`,
  `lsp/client::{did_open, did_close, did_change}`,
  `agent::{Agent::new, ActiveProject::activate}`,
  `tools/library::register`, `server::from_parts`.
- **Deferred:** Windows symlink-escape unit tests need a runner with
  Developer Mode enabled (or admin) to call
  `std::os::windows::fs::symlink_dir/symlink_file`. Picked up under
  W21–W25 once we have a real Windows machine.
- **Status:** ✅ done (production code path); 🟡 Windows symlink test
  coverage deferred to runtime-validation phase.
### W8. Hardcoded `/tmp` as write root ✅ done

- **Audit (2026-04-29):** the original concern (a hardcoded `/tmp`
  write-root in `path_security.rs`) is no longer present.
  `validate_write_path` already routes through
  `crate::platform::temp_dir()` (file:src/util/path_security.rs around
  L308). All other `"/tmp"` literals under `src/` are inside `#[cfg(test)]`
  fixtures.
- **Side finding:** `src/embed/preflight.rs::SYSTEM_PATHS` (L55) is a
  Unix-only list (`/`, `/usr`, `/etc`, `/var`, `/tmp`, `/root`, `/opt`,
  `/proc`, `/sys`, `/home`) used to warn when a user roots codescout on
  a "broad" filesystem location. On Windows this list never matches, so
  it produces no false positives — but also no warning for `C:\`,
  `C:\Windows`, `C:\Program Files`, `%USERPROFILE%`. UX polish, not a
  security regression. Tracked as a follow-up below; not blocking the
  Windows port.
- **Status:** ✅ done.
- **Follow-up (W8a, optional):** extend `SYSTEM_PATHS` with Windows
  equivalents under `#[cfg(windows)]`, or replace with a platform-aware
  classifier in `crate::platform`.
### W9. `atomic_write` exec-bit preservation Unix-only 🟡 partial
- **Location:** `src/util/fs.rs:52-68`.
- **Issue:** `#[cfg(unix)]` block reads + restores Unix mode (preserves
  exec bit). Windows has no equivalent semantic; safe degradation.
- **Status:** 🟡 acceptable — Windows files don't carry the exec bit.
  Test `atomic_write_preserves_exec_bit` already `#[cfg(unix)]`-only.

### W10. Endpoint naming via `transport::endpoint_path` ✅ done
- **Location:** `src/lsp/mux/mod.rs::socket_path_for_workspace`.
- **Fix:** Now delegates to `transport::endpoint_path`. Unix returns
  `<per_user_dir>/codescout-<lang>-mux-<hash>.sock`. Windows returns
  `\\.\pipe\codescout-<lang>-mux-<hash>` (kernel namespace, not a
  filesystem path).
- **Commit:** `39e5317`.

## Embedding stack

### W11. `local-embed` Windows build path ✅ decided

- **Cause:** `ort-sys` (ONNX Runtime FFI) ships prebuilts for
  `x86_64-pc-windows-msvc` only. The `gnu` toolchain is unsupported
  upstream — Microsoft does not publish MinGW binaries.
- **Decision (2026-04-29):** ship the Windows release artifact as
  `x86_64-pc-windows-msvc`. Drop `windows-gnu` from the supported set.
  ORT's `download-binaries` feature handles the rest — no Rust code
  changes. Build via the `windows-latest` GitHub Actions runner (native
  MSVC) or `cargo-xwin` from Linux.
- **Why MSVC over alternatives:** `ort` prebuilts give us first-class
  perf with zero build engineering. `candle` (pure Rust) is the
  documented fallback if we ever need a SDK-licence-free path; it
  builds on both Windows triples but is ~3× slower on CPU and requires
  attention-mask/mean-pool correctness work. `tract` is a similar
  fallback. Neither is needed today.
- **Status:** ✅ done (decision recorded; CI swap tracked under W19).
- **Follow-up:** if MinGW users surface, revisit candle. For now,
  `windows-gnu` is unsupported.
### W12. `tikv-jemallocator` global allocator on Windows 🟡 partial

- **Location:** `src/main.rs:5`, `Cargo.toml`.
- **Fix (commit pending):** Both the dependency declaration and the
  `#[global_allocator]` are now `cfg(not(windows))`-gated. Windows uses
  the system Win32 heap. Avoids any latent `tikv-jemallocator` Windows
  oddities and saves a build dep.
- **Status:** ✅ done.
## Hardware / system probes

### W13. `hardware.rs` Windows RAM probe missing 🔴 open

- **Location:** `src/hardware.rs:171-228`.
- **Fix (commit pending):** `#[cfg(windows)]` branch calls
  `GlobalMemoryStatusEx` via `windows-sys` (newly-added Windows-only
  dep, feature `Win32_System_SystemInformation`). Returns total physical
  RAM in GiB, falling through to `0` on probe failure.
- **Status:** ✅ done.
## Shell / process invocation

### W14. Shell command branching abstracted ✅ done
- **Location:** `src/platform/{unix,windows}.rs::shell_command`.
- **Note:** Returns `cmd /C` on Windows, `sh -c` on Unix. Already in
  place pre-audit; no Windows action needed.

### W15. `shell_tokenize` differs per platform ✅ done
- **Location:** `src/platform/{unix,windows}.rs::shell_tokenize`.
- **Note:** Unix handles `'`, `"`, `\` escape. Windows handles `"` only
  (matches `cmd.exe` parsing rules). Already in place; flagged here for
  visibility — runtime testing under W11 should exercise this.

## Denied-paths / security

### W16. Per-OS denied path lists differ ✅ done
- **Location:** `src/platform/{unix,windows}.rs` denied prefix lists.
- **Note:** Unix has 29 entries (`/etc/shadow`, `/etc/sudoers`, etc).
  Windows has 19 (`%USERPROFILE%\.aws`, `%USERPROFILE%\.pypirc`, etc).
  Lists are intentionally different per OS. ✓

### W17. UNC path handling ✅ done

- **Action (2026-04-29):** resolved by W20 — `crate::platform::canonicalize`
  on Windows uses `dunce::canonicalize`, which strips verbatim UNC
  prefixes (`\\?\C:\foo` → `C:\foo`) when the underlying path does not
  require them. All production canonicalize call sites that compare or
  forward paths now go through this helper, so prefix mismatches that
  would have broken `starts_with` are eliminated.
- **Test:** `src/platform/windows.rs::tests::canonicalize_strips_verbatim_unc_prefix`
  asserts the UNC prefix is gone after canonicalization (cfg(windows)).
- **Status:** ✅ done.
### W18. Drive-letter case sensitivity 🔴 open

- **Issue:** Windows treats `C:\foo` and `c:\foo` as the same path.
  Hashing for `socket_path_for_workspace` used raw `PathBuf` hashing
  via `DefaultHasher` — case-different drive letters yielded different
  workspace hashes, forking a single workspace into multiple mux
  instances.
- **Fix (commit pending):** `mux::workspace_hash` ASCII-lowercases the
  workspace root on Windows before hashing. Unix path unchanged.
  `#[cfg(windows)]` test `mixed_case_drive_letter_collapses_on_windows`
  pins the contract.
- **Status:** ✅ done.
## CI / tooling

### W19. CI `windows-latest` matrix job ✅ done

- **Done (2026-04-29):** removed `windows-latest` excludes for `default`
  and `local-embed`; removed the `windows-baseline` carve-out row.
  All three feature combos now run on every OS. `windows-latest` is
  MSVC, so `ort`'s `download-binaries` resolves prebuilts.
- **Status:** ✅ done. First Windows CI run is the validation gate;
  if `local-embed` fails on `windows-latest`, revisit W11 (candle
  fallback).
### W20. `dunce` canonicalize helper ✅ done

- **Done (2026-04-29):** added `dunce = "1"` (small pure-Rust dep) and
  introduced two helpers in `crate::platform`:
  - `canonicalize(&Path) -> io::Result<PathBuf>` — fallible, mirrors
    `std::fs::canonicalize` semantics on Unix; on Windows uses
    `dunce::canonicalize`.
  - `canonicalize_or(&Path) -> PathBuf` — best-effort, returns the
    input unchanged on failure.
- **Why a dep, not hand-rolled:** `dunce` correctly handles edge cases
  where the verbatim form *is* required (long paths, paths with `\\`
  components). A hand-rolled stripper would either be over-eager
  (corrupting valid UNC paths) or under-eager (missing forms). Dep is
  ~200 LOC, no transitive dependencies.
- **Status:** ✅ done.
## Unaudited surfaces (need a pass before claiming Windows works)

### W21. `notify` / `fs2` / `fs4` runtime behavior 🔴 open
- **Note:** all three deps claim cross-platform support but were not
  exercised on Windows during this audit. Lock-file flock semantics
  (`manager.rs::get_or_start_via_mux`) are particularly load-bearing.
- **Action:** runtime-test mux ownership transfer on Windows (kill mux
  child, second mux acquires lock, etc.).

### W22. Tokio signals 🔴 open

- **Audit result:** `src/server.rs:799-823` `shutdown_signal` already
  uses `#[cfg(unix)]` for `tokio::signal::unix` (SIGTERM, SIGHUP) and
  `#[cfg(not(unix))]` for `tokio::signal::ctrl_c` only. No other uses
  of `tokio::signal::unix` in the tree.
- **Status:** ✅ done (was already correct — audit confirmed).
### W23. LSP server discovery (PATHEXT) 🔴 open
- **Note:** `platform::lsp_binary_name` adds `.exe`/`.cmd` on Windows.
  Untested whether Windows PATH lookup actually finds the binary —
  tokio's `Command::new` should handle PATHEXT via `CreateProcessW`,
  but Node-based servers shipped as `.cmd` shims have known invocation
  quirks (need `cmd /C` wrapper or `UseShellExecute`).
- **Action:** runtime-test each LSP server we ship support for.

### W24. `gh` CLI invocations 🔴 open
- **Note:** Several tools shell out to `gh`. Untested on Windows where
  `gh` is `gh.exe` and may have different stderr handling.

### W25. Claude Code MCP integration on Windows 🔴 open
- **Note:** Claude Code itself runs on Windows; whether it can launch
  codescout via stdio MCP transport on Windows is untested. May
  intersect with line-ending handling in transport.

## Roll-up

| Bucket | ✅ done | 🟡 partial | 🔴 open | 🚫 blocked |
|--------|--------|------------|---------|-----------|
| Mux / LSP transport | 2 | 2 | 0 | 0 |
| Process management | 2 | 0 | 0 | 0 |
| Path / filesystem | 4 | 1 | 0 | 0 |
| Embedding | 1 | 1 | 0 | 0 |
| Hardware | 1 | 0 | 0 | 0 |
| Shell | 2 | 0 | 0 | 0 |
| Security / paths | 2 | 0 | 1 | 0 |
| CI / tooling | 2 | 0 | 0 | 0 |
| Signals (W22) | 1 | 0 | 0 | 0 |
| Unaudited | 0 | 0 | 5 | 0 |
| **Totals** | **18** | **4** | **6** | **0** |
## Next moves (ordered by leverage)

Low-hanging fruit + design-tier items landed (W5 Job Objects, W7+W17+W20
path/UNC pass, W8, W11 decision + W19 CI swap, W12, W13, W18, W22).
Remaining items need brainstorming or a real Windows machine:

1. **W8a (optional)** — extend `SYSTEM_PATHS` in
   `src/embed/preflight.rs` with Windows equivalents. UX polish.
2. **W21–W25** — actual Windows runtime tests. Until these run, the
   port is hypothesis. First Windows CI run (W19) is the first real
   signal. Symlink-escape coverage and Job Object kill-on-close
   verification fold into this phase.
## References

- Phase A commit: `3b6f8c3` — concentrate IPC behind `transport` module.
- Phase B commit: `39e5317` — Windows named-pipe impl + cfg(unix) ungate.
- Snow Lion architectural review (April 2026 conversation) — origin of
  the transport seam refactor.
- `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md` — related
  but Unix-specific; flagged for cross-check after W11 lands.
