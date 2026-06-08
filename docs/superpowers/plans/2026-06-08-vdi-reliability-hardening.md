# VDI Reliability Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every Windows process-spawn surface in codescout reliable on an EDR-monitored VDI by routing them through the proven-safe pattern already used by foreground `run_command`.

**Architecture:** Add one shared `platform::shell_command_configured()` builder (verbatim `cmd /C "<cmd>"` via `raw_arg` on Windows; `sh -c` + process-group + SIGPIPE reset on Unix) and migrate the background and interactive `run_command` paths onto it; replace the `taskkill`/`tasklist` shell-outs with direct Win32 API calls; harden LSP spawn; scrub the ambiguous `find` coreutil from guidance.

**Tech Stack:** Rust, tokio::process, `windows-sys` (Win32 FFI), libc (Unix).

---

## Environment notes for the implementer (this VDI)

- **Build/install dance (Windows, running MCP server locks the exe):** the live server holds `target\release\codescout.exe`. To rebuild: `move /Y target\release\codescout.exe target\release\codescout.prev.exe`, then `cargo build --release` **in background** (`run_in_background: true`) writing to a log file, poll the log with `read_file` (Windows can rename a running exe; the new build lands in the freed path), then the user runs `/mcp` to load it. Do **not** rely on a `~/.cargo/bin` symlink — there is none on this machine; the MCP config points directly at `target\release\codescout.exe`.
- **Unit tests** build a separate test binary (no exe lock): `cargo test --lib <name>` works directly, but spawns compiler children — run it in background + read the log if foreground times out.
- **Windows-gated tests** (`#[cfg(windows)]`) cannot run in the Linux CI. The pure `build_windows_cmdline` test is the cross-platform anchor. Mark Windows-only tests clearly and verify them manually on this host.

---

## File structure

| File | Responsibility | Change |
|---|---|---|
| `src/platform/mod.rs` | platform dispatch + pure helpers | add `build_windows_cmdline` (pure, tested) + `shell_command_configured` dispatch |
| `src/platform/windows.rs` | Windows impls | add `shell_command_configured`; rewrite `terminate_process`/`process_alive` via Win32 |
| `src/platform/unix.rs` | Unix impls | add `shell_command_configured` |
| `src/tools/run_command/inner.rs` | run_command core | migrate `spawn_background_command` onto builder + stdin-null |
| `src/tools/run_command/interactive.rs` | interactive mode | migrate spawn onto builder (keep stdin piped) |
| `src/lsp/client.rs` | LSP spawn | abs-path resolution + bounded spawn/init timeout (Task 7) |
| `Cargo.toml` | deps | add `windows-sys` as a `cfg(windows)` dependency |
| `src/prompts/source.md`, guides | guidance | scrub `find` suggestions |

---

## Task 1: Pure `build_windows_cmdline` + `shell_command_configured` builder

**Files:**
- Modify: `src/platform/mod.rs`
- Modify: `src/platform/windows.rs`
- Modify: `src/platform/unix.rs`

- [ ] **Step 1: Write the failing test (cross-platform, in `mod.rs`)**

In `src/platform/mod.rs`, inside (or adding) `#[cfg(test)] mod tests`:

```rust
#[test]
fn windows_cmdline_wraps_in_outer_quotes() {
    // cmd /C with a leading quote strips the first+last quote of the whole
    // line and runs the remainder verbatim, so the command — including its own
    // inner quotes — must be wrapped in exactly one outer pair.
    assert_eq!(
        build_windows_cmdline(r#"py -c "print(1)""#),
        r#"/C "py -c "print(1)"""#
    );
    assert_eq!(build_windows_cmdline("git --version"), r#"/C "git --version""#);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib windows_cmdline_wraps_in_outer_quotes`
Expected: FAIL — `cannot find function build_windows_cmdline`.

- [ ] **Step 3: Add the pure function to `mod.rs`**

In `src/platform/mod.rs` (module body, not behind `cfg`):

```rust
/// Build the verbatim command-line tail handed to `cmd /C` on Windows.
/// Wrapped in an outer quote pair so cmd's `/C` quote rule consumes exactly
/// that pair and runs the inner command — including its own quotes — verbatim.
/// Pure + cross-platform so it is testable on the Linux CI.
pub fn build_windows_cmdline(cmd: &str) -> String {
    format!("/C \"{cmd}\"")
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib windows_cmdline_wraps_in_outer_quotes`
Expected: PASS.

- [ ] **Step 5: Add `shell_command_configured` to `mod.rs` dispatch**

In `src/platform/mod.rs`, next to the existing `shell_command` dispatcher:

```rust
/// Build a fully-configured shell `tokio::process::Command` for `cmd`.
/// Windows: `cmd /C "<cmd>"` via raw_arg (no MSVC-CRT quote mangling).
/// Unix: `sh -c <cmd>` in a fresh process group with SIGPIPE reset.
/// Sets `GIT_PAGER=cat`. The caller sets cwd, stdio, and kill_on_drop.
pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    imp::shell_command_configured(cmd)
}
```

- [ ] **Step 6: Implement the Windows builder**

In `src/platform/windows.rs`:

```rust
pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    use std::os::windows::process::CommandExt;
    let mut std_cmd = std::process::Command::new("cmd");
    std_cmd
        .raw_arg(super::build_windows_cmdline(cmd))
        .env("GIT_PAGER", "cat");
    tokio::process::Command::from(std_cmd)
}
```

- [ ] **Step 7: Implement the Unix builder**

In `src/platform/unix.rs`:

```rust
pub fn shell_command_configured(cmd: &str) -> tokio::process::Command {
    let mut c = tokio::process::Command::new("sh");
    c.arg("-c").arg(cmd).env("GIT_PAGER", "cat").process_group(0);
    // SAFETY: pre_exec runs post-fork, pre-exec; signal() is async-signal-safe.
    unsafe {
        c.pre_exec(|| {
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
            Ok(())
        });
    }
    c
}
```

- [ ] **Step 8: Verify it compiles**

Run: `cargo build --lib` (background + poll log on this VDI if it times out)
Expected: builds clean (no new warnings).

- [ ] **Step 9: Commit**

```bash
git add src/platform/mod.rs src/platform/windows.rs src/platform/unix.rs
git commit -m "feat(platform): shell_command_configured builder + pure build_windows_cmdline"
```

---

## Task 2: Migrate the background `run_command` path onto the builder

**Files:**
- Modify: `src/tools/run_command/inner.rs` (`spawn_background_command`, ~line 87-143)
- Test: `src/tools/run_command/tests.rs`

- [ ] **Step 1: Write the failing test (`#[cfg(windows)]`)**

In `src/tools/run_command/tests.rs`:

```rust
#[cfg(windows)]
#[tokio::test]
async fn background_command_with_quotes_captures_output() {
    // Regression: the background path used .arg() → MSVC-CRT quote mangling →
    // a quoted -c argument dropped Python into its stdin-blocked REPL.
    let ctx = crate::tools::test_support::test_ctx().await; // existing helper
    let res = run_command_inner(
        r#"py -c "print('bg-ok', 2+2)""#,
        r#"py -c "print('bg-ok', 2+2)""#,
        30, false, None, false, /*run_in_background=*/ true,
        ctx.root(), ctx.security(), &ctx,
    )
    .await
    .unwrap();
    let ref_id = res["output_id"].as_str().unwrap().to_string();
    // Poll the bg log buffer until the line appears (warm-up returns fast).
    let mut found = false;
    for _ in 0..50 {
        let out = run_command_inner(
            &format!("type {ref_id}"), &format!("type {ref_id}"),
            10, false, None, false, false, ctx.root(), ctx.security(), &ctx,
        ).await;
        if let Ok(v) = out {
            if v["stdout"].as_str().unwrap_or("").contains("bg-ok 4") { found = true; break; }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(found, "background command output not captured");
}
```

> Note: adapt `test_ctx`/`test_support` to the existing test harness in `tests.rs` (check how sibling tests build `ToolContext`). If no async ctx helper exists, model it on the nearest existing `run_command_inner` test.

- [ ] **Step 2: Run the test to verify it fails (Windows host)**

Run: `cargo test --lib background_command_with_quotes_captures_output`
Expected: FAIL (on the pre-change binary the quoted `-c` is mangled → no `bg-ok 4`).

- [ ] **Step 3: Migrate `spawn_background_command`**

In `src/tools/run_command/inner.rs`, replace the spawn block:

```rust
// BEFORE:
let (shell, shell_args) = crate::platform::shell_command(resolved_command);
let child = tokio::process::Command::new(shell)
    .args(&shell_args)
    .current_dir(work_dir)
    .env("GIT_PAGER", "cat")
    .stdout(std::process::Stdio::from(log_file))
    .stderr(std::process::Stdio::from(log_stderr))
    .spawn()?;

// AFTER:
let mut cmd = crate::platform::shell_command_configured(resolved_command);
let child = cmd
    .current_dir(work_dir)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::from(log_file))
    .stderr(std::process::Stdio::from(log_stderr))
    .spawn()?;
```

(`GIT_PAGER` now comes from the builder; drop the inline `.env`.)

- [ ] **Step 4: Run the test to verify it passes (Windows host)**

Run: `cargo test --lib background_command_with_quotes_captures_output`
Expected: PASS.

- [ ] **Step 5: Verify the full build + existing tests**

Run: `cargo build --lib` then `cargo test --lib run_command`
Expected: builds clean; existing run_command tests still pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/run_command/inner.rs src/tools/run_command/tests.rs
git commit -m "fix(run_command): harden background path via shell_command_configured + stdin null"
```

---

## Task 3: Migrate the interactive `run_command` path onto the builder

**Files:**
- Modify: `src/tools/run_command/interactive.rs` (~line 67-76)

- [ ] **Step 1: Migrate the spawn block**

In `src/tools/run_command/interactive.rs`, replace:

```rust
// BEFORE:
let (shell, shell_args) = crate::platform::shell_command(command);
let mut child = Command::new(shell)
    .args(&shell_args)
    .current_dir(&work_dir)
    .env("GIT_PAGER", "cat")
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;

// AFTER:
let mut cmd = crate::platform::shell_command_configured(command);
let mut child = cmd
    .current_dir(&work_dir)
    .stdin(std::process::Stdio::piped())   // interactive needs real stdin
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()?;
```

(Interactive keeps `stdin(piped)` — it drives stdin deliberately. The fix here is the verbatim arg passing.)

- [ ] **Step 2: Verify it compiles**

Run: `cargo build --lib`
Expected: clean. (The local `use ... Command` may now be unused — remove the import if clippy flags it.)

- [ ] **Step 3: Run interactive tests**

Run: `cargo test --lib interactive`
Expected: existing interactive tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/tools/run_command/interactive.rs
git commit -m "fix(run_command): harden interactive path via shell_command_configured"
```

---

## Task 4: Remove the now-unused `shell_command` tuple builder

**Files:**
- Modify: `src/platform/mod.rs`, `src/platform/windows.rs`, `src/platform/unix.rs`

- [ ] **Step 1: Confirm no remaining callers**

Run: `grep -rn "shell_command(" src/ | grep -v shell_command_configured | grep -v shell_command_mode`
Expected: only the `pub fn shell_command` definitions (and possibly a `shell_command_uses_sh` test). No call sites.

- [ ] **Step 2: Remove `shell_command` from all three files and update/remove its test**

Delete the `pub fn shell_command(cmd: &str) -> (&'static str, Vec<String>)` from `mod.rs`, `windows.rs`, `unix.rs`. If a `shell_command_uses_sh` test exists, replace it with a `shell_command_configured` smoke test:

```rust
#[cfg(unix)]
#[test]
fn shell_command_configured_uses_sh() {
    let cmd = crate::platform::shell_command_configured("echo hi");
    assert_eq!(cmd.as_std().get_program(), "sh");
}
```

- [ ] **Step 3: Verify build is clean (no dead-code warnings)**

Run: `cargo build --lib` then `cargo clippy --lib`
Expected: no `shell_command` references, no new warnings.

- [ ] **Step 4: Commit**

```bash
git add src/platform/
git commit -m "refactor(platform): drop unused shell_command tuple builder"
```

---

## Task 5: Replace `taskkill`/`tasklist` with direct Win32 API

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/platform/windows.rs` (`terminate_process`, `process_alive`)
- Test: `src/platform/windows.rs` tests

- [ ] **Step 1: Add the `windows-sys` dependency**

In `Cargo.toml`, add (version matches the 0.61 already resolved in `Cargo.lock`):

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61", features = [
    "Win32_Foundation",
    "Win32_System_Threading",
] }
```

- [ ] **Step 2: Write the failing test (`#[cfg(windows)]`)**

In `src/platform/windows.rs` `#[cfg(test)] mod tests`:

```rust
#[test]
fn win32_terminate_and_liveness() {
    // Spawn a long sleeper, confirm alive, terminate, confirm dead.
    let child = std::process::Command::new("cmd")
        .args(["/C", "ping -n 30 127.0.0.1 >nul"])
        .spawn()
        .unwrap();
    let pid = child.id();
    assert!(process_alive(pid), "sleeper should be alive");
    terminate_process(pid).unwrap();
    // Give the OS a moment to reap.
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(!process_alive(pid), "sleeper should be dead after terminate");
}

#[test]
fn win32_liveness_false_for_dead_pid() {
    // A PID that almost certainly does not exist.
    assert!(!process_alive(0xFFFF_FFF0));
}
```

- [ ] **Step 3: Run to verify it fails (Windows host)**

Run: `cargo test --lib win32_terminate_and_liveness`
Expected: compiles against the *old* shell-out impl and likely passes — so this test mainly guards the *new* impl. Proceed to rewrite, then re-run.

- [ ] **Step 4: Rewrite `terminate_process` and `process_alive` via Win32**

In `src/platform/windows.rs`, replace both functions:

```rust
use windows_sys::Win32::Foundation::{CloseHandle, FALSE, STILL_ACTIVE};
use windows_sys::Win32::System::Threading::{
    GetExitCodeProcess, OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_TERMINATE,
};

pub fn terminate_process(pid: u32) -> std::io::Result<()> {
    // SAFETY: OpenProcess returns null on failure (checked); handle is closed.
    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
        if handle.is_null() {
            // Process already gone (or no rights) — treat "gone" as success.
            return Ok(());
        }
        let ok = TerminateProcess(handle, 1);
        CloseHandle(handle);
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

pub fn process_alive(pid: u32) -> bool {
    // SAFETY: handle checked for null and closed; exit_code is out-param.
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let got = GetExitCodeProcess(handle, &mut exit_code);
        CloseHandle(handle);
        got != 0 && exit_code == STILL_ACTIVE as u32
    }
}
```

> Verify the exact import paths/types against the resolved `windows-sys` 0.61 API (e.g. `STILL_ACTIVE` is in `Win32::System::Threading` in some versions, `Win32::Foundation` in others; `FALSE` is `windows_sys::Win32::Foundation::FALSE` = `0`). Fix imports if the compiler disagrees.

- [ ] **Step 5: Run tests to verify they pass (Windows host)**

Run: `cargo test --lib win32_terminate_and_liveness win32_liveness_false_for_dead_pid`
Expected: PASS.

- [ ] **Step 6: Verify LSP consumers still compile**

Run: `cargo build --lib` (consumers: `src/lsp/client.rs:1459,2120,2132`)
Expected: clean — signatures `terminate_process(u32)->io::Result<()>` and `process_alive(u32)->bool` are unchanged.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/platform/windows.rs
git commit -m "fix(platform): Win32 OpenProcess/TerminateProcess instead of taskkill/tasklist (no spawn under EDR)"
```

---

## Task 6: Scrub `find` from guidance + add a Windows coreutil note

**Files:**
- Modify: `src/prompts/source.md` (and any `get_guide` body that suggests `find`)
- Modify: `src/server.rs` test (the prompt-surface guard), if extending it

- [ ] **Step 1: Find any `find`-as-command suggestions in prompt/guide surfaces**

Run: `grep -rn "\bfind \" src/prompts/ src/lsp/ src/tools/ | grep -vi "find_symbol\|find symbol\|find the\|find a "`
Expected: a short list of human-facing hint strings (if any) that suggest shell `find`.

- [ ] **Step 2: Replace each `find` suggestion with `grep`/`tree`/`read_file`**

For each hit that suggests shelling `find`, rewrite to a codescout-native equivalent (`tree(glob=...)` for file discovery, `grep(pattern=...)` for content). On Windows `find` resolves to Git's Unix `find` and a cmd-style `find "x" file` becomes a runaway `/c` filesystem traversal.

- [ ] **Step 3: (Optional) Extend the prompt-surface guard test**

In `src/server.rs` (near `prompt_surfaces_reference_only_real_tools`), add a check that the sliced surfaces contain no ` find ` shell suggestion. Keep it narrow to avoid matching `find_symbol`/prose.

- [ ] **Step 4: Verify**

Run: `cargo test --lib prompt`
Expected: PASS. (No `ONBOARDING_VERSION` bump needed — this is the `server_instructions` surface, live on next connect; if you edited the `onboarding_prompt` surface, bump `ONBOARDING_VERSION` in `src/tools/onboarding.rs`.)

- [ ] **Step 5: Commit**

```bash
git add src/prompts/source.md src/server.rs
git commit -m "docs(prompts): drop ambiguous shell find suggestion (Git Unix find shadows cmd on Windows)"
```

---

## Task 7: LSP spawn hardening (riskiest — splittable)

> If this task grows beyond "verify abs-path + add a bounded timeout," STOP and split it into its own spec/plan; ship Tasks 1-6 first.

**Files:**
- Modify: `src/lsp/client.rs` (spawn site, ~line 367 per the bug doc)
- Reference: `src/platform/windows.rs::lsp_binary_name`, `src/lsp/servers/mod.rs`
- Closes: `docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md`

- [ ] **Step 1: Verify abs-path resolution is wired through the spawn**

Read `src/lsp/client.rs` around the `Command::new(&config.command).spawn()` site and confirm `config.command` is produced via `platform::lsp_binary_name(base)` (which probes `.exe`/`.cmd`/`.bat` on PATH). If it still uses a hardcoded extension, route it through `lsp_binary_name`.

- [ ] **Step 2: Write a failing test for binary resolution (if not already covered)**

`lsp_binary_name_with` already has unit tests in `windows.rs`. If the *client* doesn't call it, add a test asserting the resolved command for a dual-packaged server prefers the present extension. Model on `pyright_prefers_exe_when_only_exe_present`.

- [ ] **Step 3: Add a bounded spawn + initialize timeout**

Wrap the LSP `spawn()` + `initialize` handshake in `tokio::time::timeout(...)`. On timeout, log and return the existing tree-sitter-fallback error path (the cold-start retry budget in `client.rs` already handles query-time fallback; this guards the spawn itself). Reuse the project's `tool_timeout_secs` or a dedicated LSP spawn timeout const.

- [ ] **Step 4: Verify build + LSP tests**

Run: `cargo build --lib` then `cargo test --lib lsp`
Expected: clean; existing LSP tests pass.

- [ ] **Step 5: Close the bug file + commit**

Update `docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md` status → `fixed`, then:

```bash
git add src/lsp/client.rs docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md
git commit -m "fix(lsp): resolve binary via abs-path probe + bounded spawn timeout (Windows/EDR)"
```

---

## Task 8: Docs + bug-file bookkeeping

**Files:**
- Modify: `docs/manual/src/configuration/project-toml.md` (or troubleshooting)
- Modify: `docs/issues/2026-06-08-windows-run-command-child-process-hang.md` (Resume/follow-ups)

- [ ] **Step 1: Document `tool_timeout_secs` for EDR-slowed VDIs**

Add a short note in the project-toml/troubleshooting docs: on EDR-monitored machines, raise `tool_timeout_secs` (e.g. 60) if commands that spawn many children approach the default 30s.

- [ ] **Step 2: Update the run_command bug file Resume**

Mark the background/interactive/find/taskkill follow-ups (originally listed in `2026-06-08-windows-run-command-child-process-hang.md` Resume) as addressed by this plan, citing the new commits.

- [ ] **Step 3: Commit**

```bash
git add docs/
git commit -m "docs: VDI tool_timeout_secs note + run_command follow-up bookkeeping"
```

---

## Ship sequence

Per CLAUDE.md Standard Ship Sequence, after each task's commit on `experiments` is verified (build + tests on a Windows host), cherry-pick to `master` and record the master-side SHA. Suggested order: Task 1 → 2 → 3 → 4 → 5 → 6 → 8, with Task 7 (LSP) last or split out.

## Self-review (completed)

- **Spec coverage:** 2a→Task 1(+4); 2b→Task 2; (interactive, discovered during planning)→Task 3; 2c→Task 5; 2d→Task 7; 2e→Task 8; Gap C→Task 6. All spec components map to tasks.
- **Placeholders:** none — every code step shows full code; Windows-API import caveat flagged explicitly for verification.
- **Type consistency:** `shell_command_configured(&str) -> tokio::process::Command` used identically in Tasks 1/2/3; `terminate_process(u32)->io::Result<()>` and `process_alive(u32)->bool` signatures preserved (Task 5) so LSP consumers compile unchanged.
- **New discovery:** the interactive path (`interactive.rs:68`) shares the same `.arg()` quoting bug as the background path — added as Task 3 (not in the original spec's enumerated gaps, but within "all spawn surfaces").
