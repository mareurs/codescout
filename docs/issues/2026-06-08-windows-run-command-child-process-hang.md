---
status: fixed
opened: 2026-06-08
closed: 2026-06-08
severity: high
owner: marius
related: []
tags: [windows, run_command, process, quoting]
kind: bug
---

# BUG: run_command hangs on Windows for any command that spawns a child process, and mangles embedded double quotes

## Summary
On Windows, `run_command` hung until timeout (no output, no exit code) for any
command that spawns a child/grandchild process — `git --version`, every real
Python interpreter launch (`py -c …`, `python.exe …`, `py script.py`). Self-
contained single-process exes (`cargo --version`) and cmd builtins (`echo`,
`ver`, `type`) worked fine. Separately, embedded double quotes were escaped to
`\"` before reaching cmd.exe, which cmd does not understand — so
`py -c "print(1)"` was mangled and Python dropped into its (stdin-blocked) REPL.
Both are fixed and verified live.

## Symptom (Effect)
Two distinct, observed failures (same code site):

1. **Hang.** Any interpreter/launcher launch returned:
   ```
   {"timed_out": true, "stderr": "Command timed out after 30 seconds", "exit_code": null}
   ```
   Confirmed hangs: `py -c print(2+2)`, `py -c "print(2+2)"`, `py -c pass`,
   `C:\...\python.exe --version`, `py scratch.py`, `git --version`.
   Worked instantly: `cargo --version`, `echo`, `ver`, `type`, `ping`,
   `py --version` / `py -0p` (launcher answers from metadata, no interpreter spawned).
   Redirecting child output to a file (`> out.txt 2>&1`) did **not** help.

2. **Quote mangling.** A quoted path came back as:
   ```
   '"C:\Program Files\Python311\python.exe"' is not recognized as an internal or external command
   ```
   i.e. the `"` arrived at cmd.exe as `\"`.

## Reproduction
- HEAD `0930e3a6` (experiments), Windows 11 (10.0.26200).
- `run_command(command="git --version")` → timed out.
- `run_command(command="py -c \"print(1)\"")` → timed out (REPL, stdin-blocked).
- `run_command(command="cargo --version")` → returned instantly (control).

## Environment
- OS: Windows 11 Enterprise 10.0.26200, corporate domain machine (likely EDR/AV present).
- Shell spawned by run_command: `cmd /C <command>`.
- Python: launcher `py` 3.11.9 (`C:\Program Files\Python311\python.exe`) + a uv-managed 3.14.5.
- MCP transport: stdio (Claude Code).

## Root cause
Both failures lived in the `#[cfg(windows)]` branch of
`src/tools/run_command/inner.rs::run_command_inner` (pre-fix ~line 405).

1. **Hang.** The branch used `tokio::process::Command::new("cmd").arg("/C")
   .arg(cmd)…​.output()`. `.output()` waits for **stdout/stderr pipe EOF**, which
   occurs only when *every* handle to the pipe's write end is closed. When the
   direct child spawns a grandchild (git's mingw helper exe; the `py` launcher's
   python child; a process Python itself spawns; or — most likely here — an
   EDR/AV DLL injected into the child) that grandchild **inherits the pipe write
   handle** and can hold it open after our direct child exits → EOF never arrives
   → `.output()` never returns. `cargo` is a single static process so its pipe
   closes cleanly; that's why it worked. The existing timeout hint already named
   this exact failure: "output() never gets EOF".

   The branch also never set `stdin`, so the child inherited the MCP server's
   stdin pipe (never closes). Any child that reads stdin — notably a Python that
   fell into its REPL — blocked forever.

2. **Quote mangling.** `.arg(cmd)` passes the whole command as one argument;
   Rust's std `Command` on Windows quotes/escapes arguments per the **MSVC CRT**
   convention, turning embedded `"` into `\"`. cmd.exe uses different rules and
   does not unescape `\"`, so the quoted token was corrupted. For `py -c "code"`
   this broke the `-c` argument, dropping Python into the REPL (→ feeds failure #1).

## Evidence
Discrimination matrix gathered live this session (pre-fix):

| Command | Result |
|---|---|
| `echo`, `ver`, `type`, `ping` | ✅ instant (cmd builtins / single proc) |
| `cargo --version` | ✅ `cargo 1.96.0` (single static exe) |
| `py --version`, `py -0p`, `py -V -c pass` | ✅ (launcher metadata, no interpreter spawned) |
| `git --version` | ❌ timeout |
| `py -c print(2+2)` / `py -c "print(2+2)"` / `py -c pass` | ❌ timeout |
| `C:\...\python.exe --version` (uv) | ❌ timeout |
| `"C:\Program Files\Python311\python.exe" -c "…"` | ❌ `'\"C:\Program Files\…\"' is not recognized` (quote mangling) |

## Hypotheses tried
1. **Hypothesis:** nested double-quotes in the `-c` string. **Test:** ran
   `py -c print(2+2)` with no quotes. **Verdict:** rejected — still hung.
2. **Hypothesis:** the `py` launcher specifically. **Test:** ran the real
   `python.exe` directly. **Verdict:** rejected — direct interpreter also hung;
   `git --version` (no python) hung too.
3. **Hypothesis:** stdout pipe never gets EOF (grandchild holds it). **Test:**
   redirected child output to a file (`> out.txt`). **Verdict:** confirmed/refined
   — redirect alone didn't fix it, so we must wait on the *process*, not the pipe,
   and also give the child a closed stdin.
4. **Hypothesis:** quote escaping is cmd-incompatible. **Test:** quoted a path
   with spaces. **Verdict:** confirmed — `"` arrived as `\"`.

## Fix
Rewrote the `#[cfg(windows)]` branch of `run_command_inner`
(`src/tools/run_command/inner.rs`, ~lines 405–470):

- **`raw_arg(format!("/C \"{cmd}\""))`** (via `std::os::windows::process::CommandExt`)
  passes the command to cmd.exe **verbatim** — no MSVC-CRT `\"` escaping. The
  command is wrapped in an outer quote pair so cmd's `/C` quote rule (per
  `cmd /?`: leading-quote → strip first+last quote, run remainder verbatim)
  consumes exactly that pair, leaving commands that themselves start with a
  quoted path (`"C:\Program Files\…\python.exe" …`) intact. Fixes #2.
- **`stdin(Stdio::null())`** so stdin-reading children get immediate EOF instead
  of blocking on the inherited MCP stdin.
- **Capture stdout/stderr to temp files and `child.wait().await`** on the
  *process*, then read the files into a synthesized `std::process::Output` —
  instead of `.output()` waiting on pipe EOF. A lingering grandchild holding a
  file handle no longer blocks us. Fixes #1.
- Temp files cleaned via `TmpfileGuard` (drops on both normal completion and
  future-drop / timeout / cancellation). `kill_on_drop(true)` preserved.

**Live verification (post-`/mcp` restart with the new binary):**

| Command | Before | After |
|---|---|---|
| `py -c "print('hello from python', 2+2)"` | ❌ timeout | ✅ `hello from python 4` |
| `git --version` | ❌ timeout | ✅ `git version 2.54.0.windows.1` |
| `"C:\Program Files\Python311\python.exe" -c "import sys; …"` | ❌ not recognized | ✅ `quoted path ok` + version |
| `py -c "print(\"nested double quotes\", 1+1)"` | ❌ timeout | ✅ `nested double quotes 2` |
| `echo one two three \| findstr two` | (pipe) | ✅ matched |
| `git -C . log --oneline -1` | ❌ timeout | ✅ commit line |
| `py -c "…" & echo chained-ok` | ❌ | ✅ both lines |

Build: `cargo build --release` clean (only pre-existing dead-code warnings).
`cargo fmt` clean.

**Install note (Windows-specific):** a running MCP server locks
`target\release\codescout.exe`, so `cargo build` fails the final replace with
`os error 5: Access is denied`. Workaround used: `move` the running exe aside
(Windows allows renaming a running exe — the process keeps its open handle),
rebuild into the freed path, then `/mcp` restart to load the new binary.

Master-side SHA: pending (not yet committed/shipped).

## Tests added
No automated test yet. A Windows-only regression is awkward in the Linux CI: it
needs a helper exe that spawns a detached grandchild inheriting the pipe handle,
then asserts `run_command` returns promptly (not after the timeout). Live
verification matrix above stands in for now; automated test tracked in Resume.

## Workarounds
Pre-fix: use `run_in_background: true` (detaches via a log file, returns after the
warm-up window) and read the log file directly with `read_file` (not `cat`/`tail`
— those resolve to Git's coreutils, which are themselves grandchild-spawning and
hang). Post-fix: none needed.

## Resume
Fix is live and verified. Remaining follow-ups:
1. Add a `#[cfg(windows)]` regression test (helper exe spawning a pipe-inheriting
   grandchild; assert prompt return).
2. Apply the same `raw_arg` + outer-quote treatment to the background path
   (`src/platform/windows.rs::shell_command` / `spawn_background_command`), which
   still uses `.arg`.
3. Cherry-pick to master; record the master-side SHA here and in Fix.

## References
- `src/tools/run_command/inner.rs` — `run_command_inner` Windows branch (the fix).
- `src/platform/windows.rs::shell_command` — sibling `cmd /C` construction used by
  the background path (uses `.arg`, not `raw_arg`; candidate for the same fix).
