# Tool Misbehaviours — Living Log

**Reader:** developers (including Claude) working on codescout's own MCP tools.

**Purpose:** catch unexpected behaviour from codescout's tools (`edit_file`, `replace_symbol`, `find_symbol`, `semantic_search`, `edit_markdown`, `run_command`, …) *before* it gets normalised as "just how the tool works". Log it while the context is fresh; fix it or let it inform future work.

**Scope:** bugs, silent failures, misleading errors, corrupt output. Not feature requests — those go to GitHub issues.

## Before starting any task

Skim the "Mitigated quirks" section below so you know which sharp edges still exist. If you hit a new one, add an entry **before continuing**.

## Adding an entry

Use the template at the bottom. Keep it one entry per observation, even if you think it's a duplicate — historians decide that later. Mention commits and tests where possible.

## Mitigated quirks (live caveats)

These are fixed in the happy path but still have edge cases worth knowing about. Full write-ups in `docs/archive/bug-reports/2026-03-to-2026-04-tool-misbehaviors.md`.

### BUG-030 — `replace_symbol` on `mod tests` can eat an adjacent function body

- **Mitigation (2026-03-20):** `validate_symbol_position` guard detects stale LSP positions and surfaces a `RecoverableError`. Happy path works.
- **Still watch for:** stale LSP positions on large files mid-edit — if `replace_symbol` ever reports "symbol not found" after a big write, `/mcp` reconnect re-indexes.

### BUG-032 — `remove_symbol` can leave orphaned `impl` block code after enum removal

- **Mitigation (2026-03-20):** same `validate_symbol_position` guard catches the stale-position case.
- **Still watch for:** adjacent/nested `impl Trait for Type` next to inherent `impl Type` — range computation may still grab the wrong brace set (also noted on BUG-037). Workaround: `create_file` for those cases.

### BUG-021 — partial state after parallel `edit_file` calls (by design)

- **Crash fixed** by rmcp 1.2.0 cancellation-race fix.
- **Still applies:** never dispatch parallel write tool calls. Two independent writes have no transaction semantics; if one is denied by the permission dialog and the other succeeds, files end up half-applied.

## Open

*(none at time of archive — 2026-04-22)*

## Archive

Fixed / superseded entries: `docs/archive/bug-reports/`.


### BUG-047 — `ResilientStdin`: spinning `Poll::Pending` floods log files to 268 GB

**Date:** 2026-04-22
**Severity:** Critical (disk exhaustion)
**Status:** Fixed (2026-04-22)

**What happened:**
The codescout server for the `mirela/deployment` project hit a busy-loop on stdin
`EAGAIN` and logged every iteration at `WARN` level with no rate-limiting or size
cap. Two log files (`.codescout/debug.log` and `.codescout/diagnostic-*.log`)
grew to **268 GB each** before the disk was exhausted.

**Root cause — a regression introduced by BUG-033's fix:**

The `ResilientStdin` wrapper (added on 2026-03-21 to absorb transient `EAGAIN` so
rmcp would not close the stdio transport) had two compounding flaws:

1. **Spinning `Poll::Pending` (CPU busy-loop).** When `tokio::io::Stdin::poll_read`
   returned `Ready(Err(WouldBlock))`, the waker was *not* registered with epoll
   (the inner returned `Ready`, not `Pending`). To avoid a task hang, the code
   called `cx.waker().wake_by_ref()` before returning `Pending`. `wake_by_ref()`
   immediately re-schedules the task — an external event never fires the waker,
   the task itself does. The task re-polls, gets EAGAIN again, wakes itself again,
   at tokio scheduler rate. `Poll::Pending` requires the waker be called by an
   external event (I/O readiness, timer, channel), not by the pending future
   itself. This is the canonical "spinning pending" async anti-pattern.

2. **`WARN`-level log inside the spin.** A `tracing::warn!` fired on every poll,
   so two non-blocking tracing writers (debug.log at DEBUG, diagnostic-*.log at
   INFO) each received millions of lines per second.

3. **No size-based log rotation.** `rotate_logs()` in `src/logging.rs` ran only at
   startup and capped by count (3 backups), not size. Runtime floods had no gate.

**Fix (2026-04-22):**

- **`src/server.rs` — `ResilientStdin` truthful `Pending`.** On EAGAIN, arm a 1ms
  `tokio::time::Sleep` and poll it; this registers the waker via tokio's timer
  reactor so the task resumes after the delay, not immediately. Struct gains
  `backoff: Option<Pin<Box<Sleep>>>`. `WARN` → `TRACE`.
- **`src/logging.rs` — `SizeRotatingFile` defense-in-depth.** New `Write` wrapper
  caps each log at 50 MiB with 3 numbered backups, rotating on the non-blocking
  log-writer thread. Both `debug.log` and `diagnostic-*.log` routed through it.
  Guards against any future runtime log-flooding bug, not only this one.
- Tests: `size_rotating_file_rotates_on_cap_exceeded`,
  `size_rotating_file_caps_total_growth_at_keep_plus_one`,
  `size_rotating_file_single_write_under_cap_does_not_rotate`,
  `numbered_appends_suffix`, plus `resilient_stdin_absorbs_would_block` updated
  to mirror the new backoff state machine.

**Upstream still pending:** `rmcp::transport::AsyncRwTransport::receive()` converts
*any* IO error to `None`. It should distinguish transient (`WouldBlock`,
`Interrupted`) from fatal (`BrokenPipe`, EOF). File an issue with rmcp so this
wall is no longer needed.

**Reproduction hint:**
Observed in practice: Claude Code's Node.js runtime briefly sets the stdin pipe
`O_NONBLOCK` under I/O pressure. If that state persists past a single poll cycle,
the spin begins.

**Observed at:** `mirela/deployment` project, 2026-04-22.
## Template for new entries

```markdown
### BUG-XXX — <tool>: <one-line symptom>

- **Observed:** YYYY-MM-DD
- **Tool:** `tool_name`
- **Severity:** Low / Medium / High (Low = cosmetic; High = data loss or crash)
- **What I did:** minimal repro, with actual args.
- **Expected:** …
- **What happened:** …
- **Probable cause / root cause:** (leave blank if unknown — investigation can be a separate entry)
- **Workaround:** …
- **Fix:** commit or test name, or "open"
- **Status:** Open / Fixed (YYYY-MM-DD, commit `abcdef0`) / Mitigated
```
