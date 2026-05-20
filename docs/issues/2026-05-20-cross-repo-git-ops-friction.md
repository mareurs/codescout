---
status: fixed
opened: 2026-05-20
closed: 2026-05-20
severity: low
owner: marius
related: []
tags: [tool-friction, sandbox, companion-hook]
kind: bug
---

# BUG: Cross-repo git ops require workspace switch — sandbox + Bash-block sandwich

## Summary

When working in codescout (or any project where the companion plugin is active) and the agent needs to perform git operations in a *sibling* repo (e.g. `~/work/claude/claude-plugins/`), both paths to the shell are closed by default. The agent has to either workspace-switch (polluting shared MCP state) or escalate to the user. Documented nowhere; rediscovered per session.

## Symptom (Effect)

Two consecutive blocks when trying to commit in `~/work/claude/claude-plugins/` while the active codescout workspace is `~/work/claude/code-explorer`:

1. Native `Bash` call refused by companion hook:

   ```
   This call is blocked because codescout offers a leaner path for shell work.
   Command: cd /home/marius/work/claude/claude-plugins && git status --short ...
   ```

2. codescout `run_command` with `cwd` parameter refused:

   ```
   {"ok": false, "error": "cwd '/home/marius/work/claude/claude-plugins' escapes project root",
    "hint": "The cwd must be a subdirectory within the project, or a path under the platform temp directory."}
   ```

Net: no shell path available for the sibling repo without a workspace switch.

## Reproduction

```
Active codescout workspace: ~/work/claude/code-explorer
Edit a file under ~/work/claude/claude-plugins/ via Edit tool.
Try to commit:
  - Bash("cd ~/work/claude/claude-plugins && git ...") → blocked by companion hook
  - run_command(command="git ...", cwd="/home/marius/work/claude/claude-plugins") → blocked by sandbox
```

## Environment

- Host: linux, bash shell
- Active workspace at time of friction: codescout (`/home/marius/work/claude/code-explorer`)
- Companion plugin: `codescout-companion@sdd-misc-plugins` enabled
- Companion hook config: default (Bash blocked, source-file Read/Grep/Glob blocked)
- Date: 2026-05-20

## Root cause

Two independent guard layers compose into a closed system:

1. **Companion `PreToolUse` hook on `Bash`** (`~/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh`) — denies all `Bash` calls, redirects to codescout `run_command`. Project-agnostic deny; does not check whether the target path is inside the codescout project.
2. **codescout `run_command` sandbox** (cwd-escape check in `src/tools/run_command/`) — rejects any `cwd` outside the active project root, which is the right safety property for the tool but leaves no escape hatch for legitimate sibling-repo work.

Result: the only path to a sibling-repo shell is `workspace(action="activate", path=...)` to switch projects — which violates Iron Law 4 unless the agent remembers to restore, and pollutes shared MCP state for any concurrent session.

## Evidence

### codescout sandbox refusal

Direct tool output, this session 2026-05-20:

```
run_command(command="git status --short", cwd="/home/marius/work/claude/claude-plugins")
→ {"ok": false,
   "error": "cwd '/home/marius/work/claude/claude-plugins' escapes project root",
   "hint": "The cwd must be a subdirectory within the project, or a path under the platform temp directory."}
```

### Companion hook refusal

```
Bash("cd /home/marius/work/claude/claude-plugins && git status --short && ...")
→ "This call is blocked because codescout offers a leaner path for shell work."
```

## Hypotheses tried

None — friction is observable and reproducible on demand; no investigation required to confirm it.

## Fix

Shipped as **`claude-plugins:958c0b9`** (codescout-companion 1.11.1).

The pre-tool-guard Bash branch now extracts an effective cwd from a leading `cd <dir>` (absolute / quoted / tilde-expanded / relative), canonicalizes via `realpath -m`, and exits 0 (allow) when the result lies outside the announced `$CWD` subtree. Symmetric with the existing Read/Edit/Grep/Glob/Write workspace-scoping pattern. `is_in_workspace` could not be reused because `WORKSPACE_ROOT` is empty by default — fail-closed there is correct for source-file scoping but wrong for cross-repo cd.

Change lives at `codescout-companion/hooks/pre-tool-guard.sh` (Bash branch, ~13 new lines after `CMD=$(...)`).

**Option 2 (codescout `run_command` external_cwd opt-in) deferred** — not needed for the common shape (`cd /path && cmd` via native Bash now bypasses the companion hook cleanly). Reopen if someone needs codescout's smart-summary / `@cmd_*` buffer routing for sibling-repo work; without that, plain Bash is sufficient.

**Note on archival:** the fix shipped to `claude-plugins/main`, not `code-explorer/master`. The standard CLAUDE.md archive rule ("move to `docs/issues/archive/` AFTER the fix ships to master") was written for in-repo fixes. This file stays in `docs/issues/` until a comparable cross-repo archival convention is settled.
## Tests added

`codescout-companion/hooks/pre-tool-guard.test.sh` — 9-case matrix following the `il3-deny-hook.test.sh` shape:

| Case | Command | Expected |
|---|---|---|
| cd-sibling-abs | `cd /home/marius/work/claude/claude-plugins && git status` | allow |
| cd-sibling-quoted | `cd "/home/marius/work/mirela/backend-kotlin" && git status` | allow |
| cd-sibling-tilde | `cd ~/work/claude/claude-plugins && git log -1` | allow |
| cd-sibling-relative | `cd ../claude-plugins && git status` | allow |
| cd-tmp | `cd /tmp && ls` | allow |
| bare-cargo-test | `cargo test` | deny |
| cd-subdir-in-ws | `cd src && ls` | deny |
| cd-abs-back-into-ws | `cd $CWD/src && ls` | deny |
| grep-on-source | `grep foo src/main.rs` | deny |

Run: `bash codescout-companion/hooks/pre-tool-guard.test.sh`. All 9 pass.

The test sources the real `pre-tool-guard.sh` and lets `detect-tools.sh → detect.py` resolve the environment from the announced `$CWD` (code-explorer in this matrix). No env stubs — black-box.
## Workarounds

Pick the right shape for the situation:

- **One-off commit in sibling repo:** ask the user to run the command, or use the `!` prefix to suggest the user run it inline.
- **Multi-step work in sibling repo:** `workspace(action="activate", path="/path/to/sibling")` then restore at end of turn with `workspace(action="activate", path="/home/marius/work/claude/code-explorer")`. Per Iron Law 4, restore is mandatory.
- **Reading docs in sibling repo:** Edit/Read on `.md` files works (companion hook only blocks source-file reads); no shell needed.

## Resume

N/A — fixed in `claude-plugins:958c0b9`. If the friction recurs (third datapoint), escalate to option 2: add an `external_cwd` parameter to codescout's `run_command` so the smart-summary / `@cmd_*` buffer machinery works for sibling-repo shells too. Source location: `src/tools/run_command/` (cwd-escape check).
## References

- `~/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh` — companion Bash deny hook.
- `src/tools/run_command/` — codescout sandbox enforcement.
- CLAUDE.md → "Companion Plugin: codescout-companion" — current documentation of the hook (does not mention cross-repo).
- CLAUDE.md → Iron Law 4 (workspace restore discipline).
