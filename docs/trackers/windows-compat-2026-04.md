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
| CI `windows-latest` matrix job | 🔴 open |

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
  branch (`#[cfg(windows)]` at `:1106`) spawns `cmd /C` but has no
  process-group equivalent. Spawned grand-children orphan on timeout.
- **Existing warning:** `child_pgid` unused in the Windows path — surface
  symptom of this gap.
- **Fix options:**
  - Win32 Job Objects (`CreateJobObject` + `AssignProcessToJobObject` +
    `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`). Cleanest; tokio doesn't expose
    it natively, would need `windows`/`windows-sys` crate.
  - `taskkill /T /F /PID <pid>` on timeout. Simpler but shells out and
    races with already-exited children.
- **Status:** 🔴 open. Pre-existing — not introduced by mux work.

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

### W8. Hardcoded `/tmp` as write root 🔴 open
- **Location:** `src/util/path_security.rs` write-root validator.
- **Issue:** `/tmp` is a Unix-ism. Windows uses
  `%LOCALAPPDATA%\Temp` (returned by `std::env::temp_dir()`).
- **Action:** Replace hardcoded `/tmp` check with
  `std::env::temp_dir()` ancestor check. Already partially right via
  `platform::temp_dir()`; audit all sites.
- **Status:** 🔴 open.

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

### W11. `local-embed` won't build for `x86_64-pc-windows-gnu` 🚫 blocked
- **Cause:** `ort-sys` (ONNX Runtime FFI) ships prebuilts for
  `x86_64-pc-windows-msvc` only. The `gnu` toolchain is unsupported
  upstream.
- **Workarounds:**
  - Use MSVC target on actual Windows. `cargo build` on a
    `windows-latest` runner (or with `xwin` cross-build on Linux) should
    succeed.
  - Drop `local-embed` from the Windows feature set; users rely on
    `remote-embed` (Ollama, OpenAI, etc.) for semantic search.
  - Switch backend (`ort-tract`, `candle`); each carries its own cost.
- **Recommendation:** ship Windows with `local-embed` off by default;
  document `remote-embed` as the supported semantic-search path. Revisit
  if MSVC cross-build proves easy enough to ship in CI.
- **Status:** 🚫 blocked on upstream `ort-sys` packaging.

### W12. `tikv-jemallocator` global allocator on Windows 🟡 partial
- **Location:** `src/main.rs:5`.
- **Note:** `tikv-jemallocator` claims Unix-focus but compiles cleanly
  for Windows in this audit (cross-check passes). Whether it actually
  benefits Windows or quietly degrades to system allocator is untested.
- **Action:** measure on real Windows; consider `cfg(unix)` gating the
  global allocator if Windows behavior is degraded.
- **Status:** 🟡 unverified.

## Hardware / system probes

### W13. `hardware.rs` Windows RAM probe missing 🔴 open
- **Location:** `src/hardware.rs:174-195`.
- **Issue:** Linux reads `/proc/meminfo`; macOS shells `sysctl`. Windows
  branch is absent — returns 0. Heuristics that scale by RAM see a
  zero-RAM machine.
- **Fix:** call `GlobalMemoryStatusEx` via `windows-sys` (kernel32). One
  function; small surface.
- **Status:** 🔴 open. Non-fatal — falls through to defaults.

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
  Hashing for `socket_path_for_workspace` uses raw `PathBuf` hashing
  via `DefaultHasher` — case-different drive letters yield different
  workspace hashes, so a single workspace can fork into multiple mux
  instances.
- **Fix:** lower-case the workspace root on Windows before hashing in
  `mux::workspace_hash`.
- **Status:** 🔴 open.

## CI / tooling

### W19. CI `windows-latest` matrix job 🔴 open
- **Action:** add a `windows-latest` runner to the existing GitHub
  Actions workflow. Run `cargo build --no-default-features --features
  remote-embed,http,dashboard` and `cargo test --lib` at minimum.
  `local-embed` deferred until W11 is resolved.
- **Status:** 🔴 open.

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
- **Note:** `tokio::signal::unix` is Unix-only. Search for any uses;
  if found, replace with `tokio::signal::ctrl_c` for Windows.
- **Action:** grep audit.

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
| Process management | 1 | 0 | 1 | 0 |
| Path / filesystem | 1 | 1 | 2 | 0 |
| Embedding | 0 | 1 | 0 | 1 |
| Hardware | 0 | 0 | 1 | 0 |
| Shell | 2 | 0 | 0 | 0 |
| Security / paths | 1 | 0 | 2 | 0 |
| CI / tooling | 0 | 0 | 2 | 0 |
| Unaudited | 0 | 0 | 5 | 0 |
| **Totals** | **7** | **4** | **13** | **1** |

## Next moves (ordered by leverage)

1. **W19** — CI windows-latest job. Cheapest way to keep regressions out.
2. **W11 decision** — ship Windows with `local-embed` off OR adopt MSVC
   cross-build via `xwin`. Rest of plan branches off this.
3. **W5** — process group cleanup (Job Objects). Largest correctness gap
   for users running shell pipelines.
4. **W18, W17, W7** — path/security correctness pass. All small, can
   bundle into one commit with `dunce` adopted under W20.
5. **W11 runtime gates** — actual Windows runtime test for mux + at least
   one LSP. Without this we have hypothesis, not a port.

## References

- Phase A commit: `3b6f8c3` — concentrate IPC behind `transport` module.
- Phase B commit: `39e5317` — Windows named-pipe impl + cfg(unix) ungate.
- Snow Lion architectural review (April 2026 conversation) — origin of
  the transport seam refactor.
- `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md` — related
  but Unix-specific; flagged for cross-check after W11 lands.
