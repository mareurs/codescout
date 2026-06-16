# VDI Reliability Hardening — Design Spec

- **Date:** 2026-06-08
- **Status:** approved (design)
- **Author:** marius + Claude
- **Scope:** Windows process-spawn reliability on constrained/EDR-monitored VDI
- **Workstream:** Reliability tier 1 of 3 (footprint profile + holistic "VDI mode" are deferred follow-ons)

## Problem

codescout runs poorly on a corporate Windows VDI where an EDR/AV product
injects into spawned processes and process creation is slow. The session that
prompted this work hit a hard `run_command` hang: any command that spawned a
child/grandchild (git, every Python interpreter launch) never returned, because
the Windows branch used `cmd /C` + `.arg()` (MSVC-CRT quoting mangled embedded
quotes) + `.output()` (waits on stdout pipe EOF, which a grandchild inheriting
the pipe handle holds open forever). That specific bug is **fixed**
(`docs/issues/2026-06-08-windows-run-command-child-process-hang.md`,
commit `2d0de46e`) for the **foreground** `run_command` path.

This spec hardens the **remaining** process-spawn surfaces so the whole tool —
not just foreground `run_command` — is reliable on this class of machine.

### Empirical scoping (gathered live, on the fixed binary)

The foreground fix turned out to resolve more than expected:

- **Git's MSYS2 coreutils now work** through `run_command`: `cat`, `tail -3`,
  `head -3`, `sed -n 1,2p` all return correct output instantly. They were
  hanging *before* purely because of the stdin/pipe-EOF/quoting bug. **The
  buffer-query class is therefore already fixed** and is NOT part of this spec.
- `git --version`, `py -c "..."`, quoted exe paths, pipes, `&` chains: all work.

What still breaks or is at risk (this spec's scope):

| # | Gap | Location / evidence |
|---|---|---|
| A | `run_command` **background** path uses `.arg()` (no raw_arg / stdin-null / capture-wait) | `src/tools/run_command/inner.rs::spawn_background_command` |
| B | `taskkill`/`tasklist` shell out via PATH `.output()` — EDR can hang/slow them; PATH-hijack risk | `src/platform/windows.rs::terminate_process`, `process_alive` |
| C | `find` resolves to Git's **Unix** `find` (shadows cmd's); cmd-syntax → runaway `/c` traversal → effective hang | hit live this session |
| D | LSP spawn on Windows: abs-path resolution + bounded spawn/init timeout | open bug `docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md`; cold-start budget already exists in `src/lsp/client.rs` |
| E | Default 30s `run_command` timeout may be too tight under EDR-slowed spawns | cross-cutting |

## Non-goals (deferred to follow-on specs)

- Resource-footprint / low-resource profile (indexing, embedding, drift, LSP memory/disk).
- The holistic "VDI mode" switch and documented install workflow.
- A full central process-execution abstraction (approach B). We borrow only the
  one shared *builder* from it; we do not reroute every spawn site.
- Replacing cmd.exe with direct exec / a different shell (approach C) — loses
  pipes/redirects/`&` the agent relies on.

## Guiding principle

Every process codescout spawns on Windows must use the proven-safe pattern:
command passed **verbatim** (`cmd /C "<cmd>"` via `raw_arg`), **stdin = NUL**,
output **captured to files**, **wait on the process** (not pipe EOF),
`kill_on_drop`. The foreground path already does this; the work is extending it
to the other surfaces and making future drift impossible via one shared builder.
**Non-Windows code paths are untouched.**

## Design

### Component 2a — Shared shell-command builder (anti-drift)

Replace `platform::shell_command(&str) -> (&'static str, Vec<String>)` with:

```rust
// src/platform/{windows,unix}.rs
pub fn shell_command_configured(cmd: &str) -> tokio::process::Command
```

- **Windows:** `Command::new("cmd")` + `raw_arg(build_windows_cmdline(cmd))`,
  `env("GIT_PAGER", "cat")`.
- **Unix:** `Command::new("sh").arg("-c").arg(cmd)`, `process_group(0)`,
  `pre_exec` SIGPIPE→SIG_DFL, `env("GIT_PAGER", "cat")` (preserves current
  foreground semantics exactly).

Factor the pure string part out for testability on any OS:

```rust
// returns the verbatim command-line tail handed to cmd.exe, e.g. /C "py -c \"x\""
pub fn build_windows_cmdline(cmd: &str) -> String  // -> format!("/C \"{cmd}\"")
```

Both foreground and background `run_command` call `shell_command_configured`;
each sets its own stdio afterward. The foreground tempfile-capture + process-wait
logic (already in `inner.rs`) is unchanged in behavior but now sources its
`Command` from the shared builder, so the Windows `raw_arg`/wrap rule lives in
exactly one place.

### Component 2b — Background path hardening (Gap A)

`spawn_background_command` switches from
`Command::new(shell).args(shell_args)` to `shell_command_configured(cmd)` and
adds `.stdin(Stdio::null())` before setting the log-file stdout/stderr. Direct
fix for the path that never got the foreground treatment. Warm-up + detach +
`BackgroundKillGuard` logic unchanged.

### Component 2c — taskkill/tasklist → Win32 API (Gap B)

Rewrite `windows.rs::terminate_process(pid)` and `process_alive(pid)` to call
Win32 directly via the `windows-sys` crate — **no process spawning**:

- `terminate_process`: `OpenProcess(PROCESS_TERMINATE)` → `TerminateProcess` →
  `CloseHandle`. `OpenProcess` failure with "not found" ⇒ `Ok(())` (already
  reaped). Other failures ⇒ `io::Error`, best-effort (logged, non-fatal).
- `process_alive`: `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` +
  `GetExitCodeProcess` == `STILL_ACTIVE`, or open-fail ⇒ not alive.

Eliminates the entire EDR-hang risk for process control (used by the background
kill-guard, interactive-session cleanup, and process liveness checks) and closes
the PATH-hijack note. New direct dependency: `windows-sys` (already in the tree
transitively; add `[target.'cfg(windows)'.dependencies] windows-sys`).

### Component 2d — LSP spawn hardening (Gap D)

- Verify `lsp_binary_name`'s `.exe`/`.cmd`/`.bat` PATH probing is wired through
  the spawn in `src/lsp/client.rs`; close
  `docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md`.
- Wrap the LSP spawn + `initialize` handshake in a bounded timeout so an
  EDR-stalled server falls back to the existing tree-sitter path instead of
  blocking. LSP stdio stays piped (JSON-RPC needs it) — unchanged.

**Risk flag:** this is the least-understood surface. If it grows beyond
"verify + bound," it splits into its own spec/plan and the rest of this spec
ships without it.

### Component 2e — Guidance (Gap C) + timeout note (Gap E)

- Scrub `find` from hints/guides/prompt surfaces (it's ambiguous on Windows due
  to Git's Unix `find`). Keep `cat`/`tail`/`head`/`grep`/`sed` (verified working
  post-fix). codescout's own code never calls `find` (git is libgit2-only).
- Document raising `tool_timeout_secs` in `project.toml` for EDR-slowed spawns
  (no code change; `researcher` already uses 60). Optionally raise the Windows
  default later — out of scope here.

## Data flow

```
run_command (fg & bg) ─► platform::shell_command_configured(cmd)
                          ├─ fg: + stdin(null) + stdout/stderr→tempfiles
                          │       + kill_on_drop → wait(process) → read files → Output
                          └─ bg: + stdin(null) + stdout/stderr→logfile
                                  → detach after warm-up (BackgroundKillGuard)
process kill / liveness ─► Win32 OpenProcess/TerminateProcess/GetExitCodeProcess
LSP ───────────────────► spawn via abs path, bounded init → tree-sitter on timeout
```

## Error handling

- Background spawn failure → `RecoverableError` (unchanged).
- Win32 calls → `io::Result`; best-effort, non-fatal; not-found = success.
- LSP spawn/init timeout → existing tree-sitter fallback (`RecoverableError`).

## Testing

| Test | Gate | Asserts |
|---|---|---|
| `build_windows_cmdline` | cross-platform (Linux CI) | `cmd /C "<cmd>"` shape; embedded quotes preserved |
| background quoted-arg capture | `#[cfg(windows)]` | bg log captures correct output for `py -c "..."` |
| Win32 kill + liveness | `#[cfg(windows)]` | spawn sleeper → `process_alive` true → `terminate_process` → false |
| prompt-surface guard | cross-platform | hints/guides contain no `find` suggestion |

`build_windows_cmdline` is the cross-platform anchor (the part most prone to
regression). Windows-only tests are documented as CI-gated / manual since Linux
CI can't exercise them.

## Component isolation

- `shell_command_configured` / `build_windows_cmdline` — single purpose (build
  the hardened shell command); the string builder is pure and tested.
- Win32 process control — isolated in `platform/windows.rs`.
- Each component is independently shippable via the Standard Ship Sequence
  (separate commits, cherry-pick to master). Suggested order: 2a→2b (drift fix +
  background), 2c (highest-leverage), 2e (cheap), 2d (last / splittable).

## References

- Fix that prompted this: `docs/issues/2026-06-08-windows-run-command-child-process-hang.md` (commit `2d0de46e`)
- Open LSP bug: `docs/issues/2026-06-06-windows-lsp-binary-hardcoded-cmd-extension.md`
- Foreground impl: `src/tools/run_command/inner.rs` (`#[cfg(windows)]` branch)
- Shell abstraction: `src/platform/windows.rs`, `src/platform/unix.rs` (`shell_command`)
- Process control: `src/platform/windows.rs` (`terminate_process`, `process_alive`)
