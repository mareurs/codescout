---
status: open
opened: 2026-05-20
closed:
severity: medium
owner: marius
related: ["docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md"]
tags: [lsp, error-message, rustup-component, agentic-surface, diagnostics]
kind: bug
---

# BUG: `edit_code` / `symbols` return opaque "LSP server disconnected" when rust-analyzer rustup component is missing

## Summary

When the rustup shim `rust-analyzer` is in `$PATH` but the `rust-analyzer`
component is not installed for the active toolchain, every `edit_code` and
`symbols` call against Rust source files fails with the single-line error
`"LSP server disconnected"`. The actual root cause — rustup printing
`error: Unknown binary 'rust-analyzer' in official toolchain
'stable-x86_64-unknown-linux-gnu'` to stderr on launch and exiting — is
swallowed by codescout's LSP launch path. The agent has no way to diagnose
this from the error text alone; a working session looks identical to one
where the binary is unreachable.

## Symptom (Effect)

```
mcp__codescout__edit_code(path="src/tools/markdown/edit_markdown.rs",
                          symbol="compute_section_end",
                          action="insert", position="after",
                          body="<helper>")
```

```
{
  "error": "LSP server disconnected"
}
```

Identical error on `symbols(path="src/tools/edit_file/mod.rs")` and any
other LSP-requiring tool call. Persists across retries — the LSP never
recovers because it was never running. No log, no hint, no pointer to
`rustup component add rust-analyzer`.

## Reproduction

```
git rev-parse HEAD
# 3ae6095d0b2c0f8e7bf4057b6e0f4a3c85a59a4d (branch: experiments)

# Setup: ensure rust-analyzer component is NOT installed but the rustup
# shim IS in PATH (the default state on a fresh rustup install).
rustup component remove rust-analyzer 2>/dev/null

# Confirm the shim is reachable and the error reproduces:
which rust-analyzer
# /usr/lib/rustup/bin/rust-analyzer
rust-analyzer --version 2>&1
# error: Unknown binary 'rust-analyzer' in official toolchain 'stable-x86_64-unknown-linux-gnu'.

# Now any LSP-backed codescout tool call returns the opaque error.
mcp call codescout symbols '{"path": "src/lib.rs"}'
# "LSP server disconnected"
```

Workaround that restores the session:
```
rustup component add rust-analyzer
# (re-run the failing call — succeeds)
```

## Environment

- OS: Linux 7.0.9-zen1-1-zen
- Rust: stable (toolchain `stable-x86_64-unknown-linux-gnu`)
- Rustup: present; `/usr/lib/rustup/bin/rust-analyzer` is the shim
- MCP transport: stdio
- Project: codescout, branch `experiments` @ `3ae6095d`
- Affected tools: `edit_code`, `symbols(name=...)`, `references`, `call_graph`,
  any tool that requires a live LSP. Tree-sitter-only tools (`grep`, `tree`,
  `read_file`) are unaffected.

## Root cause

Unknown — under investigation. Likely path:

The LSP manager (`src/lsp/manager.rs`) spawns rust-analyzer as a child process
via `std::process::Command` / `tokio::process::Command`. The rustup shim
exits non-zero with the "Unknown binary" message on stderr the moment it
is invoked. The manager observes the process die during the initialize
handshake and reports it to callers as `LSP server disconnected` — the
generic terminal-state for any LSP that exits unexpectedly.

The opaque error is correct at the LSP-protocol layer (the server did
disconnect) but lossy at the agent-facing layer: the stderr capture that
would have shown `Unknown binary` is dropped before the error reaches
the caller.

Working hypothesis to confirm in code:
1. The launch path discards stderr (or only consumes it for the LSP
   protocol stream, never surfacing the launch-time stderr).
2. The error type returned to `edit_code` / `symbols` is a flat string
   without a `cause` chain that could carry the captured stderr line.

Both should be confirmed by reading `src/lsp/manager.rs` + the launch
codepath that builds the `Command` for rust-analyzer.

## Evidence

### E1 — `rust-analyzer --version` reproducing the rustup error

```
$ rust-analyzer --version
error: Unknown binary 'rust-analyzer' in official toolchain 'stable-x86_64-unknown-linux-gnu'.
$ echo $?
1
```

The shim exits 1 with a single-line stderr message. No other output.

### E2 — `pgrep` confirms LSP not running during the opaque-disconnect window

While `edit_code` was failing repeatedly in this session:
```
$ pgrep -fa rust-analyzer
# (no output)
$ pgrep -fa codescout-lsp
# (no output)
```

The "disconnected" message was accurate — the server process did not
exist. The session was not in a "started then crashed" state; the start
itself failed.

### E3 — Workaround confirms the diagnosis

```
$ rustup component add rust-analyzer
info: downloading component rust-analyzer
$ rust-analyzer --version
rust-analyzer 1.95.0 (5980761 2026-04-14)
$ # Subsequent edit_code calls in the same session succeeded immediately.
```

The session went from "every LSP tool fails" to "every LSP tool works"
the moment the component was installed. No codescout restart needed.

## Hypotheses tried

1. **Hypothesis:** rust-analyzer crashed mid-session.
   **Test:** `pgrep -fa rust-analyzer` during the failure window.
   **Verdict:** Rejected — no process. The server never started.
   **Evidence:** E2.

2. **Hypothesis:** rust-analyzer binary not in `$PATH`.
   **Test:** `which rust-analyzer`.
   **Verdict:** Rejected — `/usr/lib/rustup/bin/rust-analyzer` was on `$PATH`;
   the rustup shim was reachable but the component behind it was missing.
   **Evidence:** E1.

3. **Hypothesis:** codescout's LSP-launch path captures stderr but drops it
   before producing the agent-facing error.
   **Test:** Deferred — needs a read of `src/lsp/manager.rs` launch + error
   path. Not done in this session.
   **Verdict:** Unconfirmed.

## Fix

Two layers, both worth doing.

**Fix-1 — Diagnose at launch time.** When the LSP child exits before the
initialize handshake completes, capture the last ~1 KB of stderr and
include it in the error returned to the caller. The current
`LspError::ServerDisconnected` (or whatever the type is) becomes
`LspError::LaunchFailed { stderr: String }` for the
"died-before-initialize" subcase. Caller-facing message becomes:

```
LSP server failed to launch.
rust-analyzer exited (code 1) before completing the initialize handshake.
Last stderr:
  error: Unknown binary 'rust-analyzer' in official toolchain 'stable-x86_64-unknown-linux-gnu'.
Hint: run `rustup component add rust-analyzer` if you use rustup.
```

The rustup-specific hint is gated on detecting the rustup-shim path
substring or the literal `Unknown binary` text in stderr. Default hint
for non-rustup launch failures: `Verify rust-analyzer is on PATH and
executable.`

**Fix-2 — Pre-flight check.** On first LSP-requiring call per session per
language, invoke `rust-analyzer --version` (timeout 2s) before the full
launch. If it exits non-zero, fail fast with the same actionable error
**without** trying to maintain a doomed LSP child. This is cheaper than
re-attempting the full launch on every failed tool call.

Snow Lion note: Fix-1 alone would close the agentic-surface gap
(`[[agentic-surface-as-moat]]`) — the error becomes self-documenting.
Fix-2 is an optimization on top of Fix-1, not a substitute for it.

## Tests added

None yet. Pre-fix, the test fixture would need:
- A `RUSTUP_TOOLCHAIN` override that points at a toolchain without
  `rust-analyzer`, OR
- A mock `Command` that returns the rustup error on `--version` /
  on spawn.

Either is feasible. Defer test authoring until the fix lands.

## Workarounds

- **User-side immediate fix:** `rustup component add rust-analyzer`.
- **Long-term:** install rust-analyzer via the official VS Code extension
  or a direct binary download (`rust-analyzer.github.io`) so the rustup
  shim is never on `$PATH` first.
- **CI / fresh-machine:** add `rustup component add rust-analyzer` to
  any project setup script that expects codescout's Rust LSP tools to
  work.

## Resume

1. Read `src/lsp/manager.rs` launch path. Find where the LSP child
   process is spawned and where its exit-before-initialize is detected.
2. Trace the error type that flows back to `edit_code` / `symbols`.
   Confirm whether stderr is captured at any layer.
3. Implement Fix-1 (capture-stderr-on-launch-failure). Add the rustup
   hint detection.
4. Write a regression test using `Command` mock or
   `RUSTUP_TOOLCHAIN` override.
5. Bump `ONBOARDING_VERSION`? **No** — this is a runtime error path,
   not a prompt surface change. No version bump needed.

## References

- `src/lsp/manager.rs` — LSP child-process lifecycle (entry point for Fix-1).
- `docs/issues/2026-03-24-kotlin-lsp-concurrent-instances.md` — related LSP
  lifecycle bug (Kotlin multi-instance). Different root cause but same family.
- F-5 in `docs/trackers/codescout-lessons-2026-05-20-session-log.md` — the
  session log entry that captured this friction during the F-3 fix work.
- Rustup component docs: `https://rust-lang.github.io/rustup/concepts/components.html`
