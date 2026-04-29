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

### W5. `run_command` timeout cleanup is Unix-only 🔴 open

- **Location:** `src/tools/run_command.rs:1057-1175`.
- **Issue:** Unix path uses `libc::killpg`, `libc::signal(SIGPIPE)`, and
  `process_group(0)` to kill an entire shell pipeline on timeout. Windows
  branch had no equivalent — spawned grand-children orphaned on timeout.
- **First-pass fix (commit pending):** `TaskkillGuard` mirrors the Unix
  `PgidKillGuard`. On future drop or timeout, runs
  `taskkill /T /F /PID <pid>` to walk the process tree. Still uses
  `kill_on_drop(true)` for the immediate child as belt-and-braces.
- **Status:** 🟡 partial. Job Objects (cleaner, kernel-managed) deferred
  as brainstorm-level. `taskkill` race window: a grand-child that
  exits-and-respawns between our `taskkill` and `wait_with_output`
  return could escape — unlikely in shell-pipeline contexts but worth
  a Job Objects upgrade later.
### W6. `terminate_process` already abstracted ✅ done
- **Location:** `src/platform/{unix,windows}.rs`.
- **Note:** Already does `taskkill /PID /F` on Windows for top-level
  process termination (LSP shutdown path). W5 is about *grandchildren*
  during shell pipeline timeouts.

## Path handling / filesystem

### W7. Symlink validation Unix-only 🔴 open
- **Location:** `src/util/path_security.rs:915,953`.
- **Issue:** Symlink-escape tests guarded `#[cfg(unix)]`. Windows
  symlinks (require admin or Developer Mode) are not validated.
- **Action:** Extend symlink resolution to canonicalize Windows paths
  through `dunce::canonicalize` or `std::fs::canonicalize` (which on
  Windows returns UNC paths — caller must normalize).
- **Status:** 🔴 open.

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

### W17. UNC path handling 🔴 open
- **Issue:** Windows `std::fs::canonicalize` returns UNC paths
  (`\\?\C:\…`). `path_security` and various string-comparison sites may
  fail to match equal-but-textually-different paths.
- **Action:** consider `dunce` crate or hand-rolled UNC stripper at
  validation boundaries.
- **Status:** 🔴 open.

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
### W20. `dunce`-style canonicalize helper 🔴 open
- **Action:** decide whether to take a dep on `dunce` (small, pure-Rust
  UNC normaliser) or hand-roll a stripper in `util/fs.rs`. Required
  before W7, W17 can ship.
- **Status:** 🔴 open.

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
| Process management | 1 | 1 | 0 | 0 |
| Path / filesystem | 3 | 1 | 0 | 0 |
| Embedding | 1 | 1 | 0 | 0 |
| Hardware | 1 | 0 | 0 | 0 |
| Shell | 2 | 0 | 0 | 0 |
| Security / paths | 1 | 0 | 2 | 0 |
| CI / tooling | 1 | 0 | 1 | 0 |
| Signals (W22) | 1 | 0 | 0 | 0 |
| Unaudited | 0 | 0 | 5 | 0 |
| **Totals** | **14** | **5** | **8** | **0** |
## Next moves (ordered by leverage)

Low-hanging fruit landed (W5 first-pass, W8, W11 decision + W19 CI swap,
W12, W13, W18, W22). Remaining items need brainstorming or a real
Windows machine:

1. **W7, W17, W20** — path/security pass: dunce dep adoption, Windows
   symlink validation, UNC normalisation. One bundled commit.
2. **W5 upgrade** — swap `taskkill` for Win32 Job Objects
   (`CreateJobObject` + `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`). Removes
   the `taskkill` race window.
3. **W8a (optional)** — extend `SYSTEM_PATHS` in
   `src/embed/preflight.rs` with Windows equivalents. UX polish.
4. **W21–W25** — actual Windows runtime tests. Until these run, the
   port is hypothesis. First Windows CI run (W19) is the first real
   signal.
## References

- Phase A commit: `3b6f8c3` — concentrate IPC behind `transport` module.
- Phase B commit: `39e5317` — Windows named-pipe impl + cfg(unix) ungate.
- Snow Lion architectural review (April 2026 conversation) — origin of
  the transport seam refactor.
- `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md` — related
  but Unix-specific; flagged for cross-check after W11 lands.
