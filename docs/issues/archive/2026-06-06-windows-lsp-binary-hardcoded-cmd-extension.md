---
status: fixed
opened: 2026-06-06
closed: 2026-06-06
severity: high
owner: marius
related: []
tags: [windows, lsp, pyright]
kind: bug
---

# BUG: Windows LSP launch hardcodes `.cmd`, breaking non-npm pyright installs

## Summary
On Windows, codescout could not start pyright for a Python project — every
symbol/LSP query failed with `Failed to start LSP server:
pyright-langserver.cmd`. `lsp_binary_name` hardcoded a `.cmd` suffix for
pyright (and four other servers), which only exists for npm-installed shims.
A pip/pipx/standalone install ships `pyright-langserver.exe`, so codescout
named a file that did not exist and the spawn failed. Affects any Windows
machine without an npm-installed copy of these servers.

## Symptom (Effect)
`symbols("app/server.py")` on a Python project returned:

```
Failed to start LSP server: pyright-langserver.cmd
```

Pyright never started; all LSP-backed navigation on Python files was dead.

## Reproduction
- Commit: `0930e3a6fc03edd23bfca0d8eccccc791566d278` (experiments), Windows.
- Install pyright via pip/pipx/standalone (gives `pyright-langserver.exe`,
  NOT the npm `.cmd` shim). Confirm: `where pyright-langserver.cmd` → not
  found; `where pyright-langserver.exe` → found.
- Activate a Python project and run `symbols(<a .py file>)` → spawn fails.

## Environment
Windows 11; codescout MCP (stdio); pyright installed at
`C:\Users\…\.local\bin\pyright-langserver.exe` (no `.cmd` on PATH);
branch `experiments`.

## Root cause
`src/platform/windows.rs::lsp_binary_name` unconditionally appended `.cmd`
for `typescript-language-server | vscode-json-language-server |
yaml-language-server | bash-language-server | pyright-langserver`. That
assumes npm packaging (which generates `.cmd` shims). The spawn site
`src/lsp/client.rs:367` does `Command::new(&config.command).spawn()` and
relies on PATH resolution, so a name with no matching file on PATH fails at
spawn with the literal command string in the error.

## Evidence

PATH probe on the affected machine:

```
> where pyright-langserver.cmd
INFO: Could not find files for the given pattern(s).
> where pyright-langserver.exe
C:\Users\MAILINCA.BRN.002\.local\bin\pyright-langserver.exe
```

Spawn site (`src/lsp/client.rs:367,379`):

```rust
let mut cmd = Command::new(&config.command);
...
.with_context(|| format!("Failed to start LSP server: {}", config.command))?;
```

## Hypotheses tried
1. **Hypothesis:** the space in the project dir name (`proj X`) or the `\\?\`
   extended-length root prefix breaks the LSP. **Test:** read the spawn
   error; it names `pyright-langserver.cmd` and fails before any rootUri is
   sent. **Verdict:** rejected — failure is at process spawn, not path
   handling.
2. **Hypothesis:** `lsp_binary_name` names a `.cmd` that does not exist.
   **Test:** `where pyright-langserver.cmd` (absent) vs `.exe` (present).
   **Verdict:** confirmed.

## Fix
`src/platform/windows.rs` — `lsp_binary_name` now probes `PATH` for the
dual-packaged servers and returns whichever variant exists, preference
`.cmd` → `.exe` → `.bat`, falling back to `.cmd` when none resolve (keeps
the historical default and error message; `.exe` for all other servers).
Logic factored into `lsp_binary_name_with(base, exists)` + `find_on_path`
for testability.

Master-side SHA: *pending cherry-pick* (committed on experiments first; update
this line with the master SHA after `git cherry-pick` + `git rev-parse HEAD`).

## Tests added
`src/platform/windows.rs::tests` — 5 unit tests:
`pyright_prefers_exe_when_only_exe_present` (the regression),
`pyright_prefers_cmd_when_npm_shim_present`,
`pyright_prefers_cmd_when_both_present`,
`dual_packaged_falls_back_to_cmd_when_absent`,
`non_dual_packaged_server_uses_exe`.

## Workarounds
`npm i -g pyright` puts a `pyright-langserver.cmd` shim on PATH, which the
old hardcoded name finds. The fix removes the need for this.

## Resume
N/A — fixed. After shipping to master, replace the "pending cherry-pick"
line in `## Fix` with the master SHA and `git mv` this file to
`docs/issues/archive/`.

## References
- `src/platform/windows.rs` (`lsp_binary_name`, `find_on_path`)
- `src/lsp/client.rs:362-379` (spawn site)
- `src/lsp/servers/mod.rs:23-30` (python LspServerConfig)
