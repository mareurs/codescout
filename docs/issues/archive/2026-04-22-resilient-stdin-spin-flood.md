---
status: fixed
opened: 2026-04-22
closed: 2026-04-22
severity: critical
owner: marius
related: ["BUG-033"]
tags: ["resilient-stdin", "logging", "disk-exhaustion", "regression"]
kind: bug
---

# BUG: `ResilientStdin` spinning `Poll::Pending` flooded log files to 268 GB

## Summary

The `ResilientStdin` wrapper (added 2026-03-21 to absorb transient `EAGAIN` from `tokio::io::Stdin` so rmcp would not close the stdio transport) entered a CPU busy-loop on stdin `EAGAIN` and logged every iteration at `WARN` level with no rate-limiting or size cap. Two log files (`.codescout/debug.log` and `.codescout/diagnostic-*.log`) grew to 268 GB each, exhausting the disk on the `mirela/deployment` project host.

## Symptom (Effect)

- Two log files reached 268 GB each before disk exhaustion.
- High CPU on the codescout server process while idle.
- Massive volume of `WARN` lines repeating the same EAGAIN message at scheduler rate.

## Reproduction

Observed in practice when Claude Code's Node.js runtime briefly set the stdin pipe to `O_NONBLOCK` under I/O pressure. If that state persisted past a single poll cycle, the spin began. No standalone repro authored; the regression test `resilient_stdin_absorbs_would_block` was updated to mirror the new backoff state machine.

## Environment

- Project: `mirela/deployment`
- Date: 2026-04-22
- Codescout server running under Claude Code stdio transport

## Root cause

A regression introduced by BUG-033's fix. The `ResilientStdin` wrapper had two compounding flaws:

1. **Spinning `Poll::Pending` (CPU busy-loop).** When `tokio::io::Stdin::poll_read` returned `Ready(Err(WouldBlock))`, the waker was *not* registered with epoll (the inner returned `Ready`, not `Pending`). To avoid a task hang, the code called `cx.waker().wake_by_ref()` before returning `Pending`. `wake_by_ref()` immediately re-schedules the task — an external event never fires the waker, the task itself does. The task re-polled, got EAGAIN again, woke itself again, at tokio scheduler rate. This is the canonical "spinning pending" async anti-pattern: `Poll::Pending` requires the waker be called by an external event (I/O readiness, timer, channel), not by the pending future itself.
2. **`WARN`-level log inside the spin.** A `tracing::warn!` fired on every poll, so two non-blocking tracing writers (debug.log at DEBUG, diagnostic-*.log at INFO) each received millions of lines per second.
3. **No size-based log rotation.** `rotate_logs()` in `src/logging.rs` ran only at startup and capped by count (3 backups), not size. Runtime floods had no gate.

## Evidence

Two log files at 268 GB each; ground truth was disk-full alarm on the host.

## Hypotheses tried

*N/A — migrated from compact form; original investigation not recorded as a hypothesis list. Root cause was confirmed by inspecting `ResilientStdin::poll_read` and the `wake_by_ref` call.*

## Fix

Applied 2026-04-22:

- **`src/server.rs` — `ResilientStdin` truthful `Pending`.** On EAGAIN, arm a 1 ms `tokio::time::Sleep` and poll it; this registers the waker via tokio's timer reactor so the task resumes after the delay, not immediately. Struct gains `backoff: Option<Pin<Box<Sleep>>>`. `WARN` → `TRACE`.
- **`src/logging.rs` — `SizeRotatingFile` defense-in-depth.** New `Write` wrapper caps each log at 50 MiB with 3 numbered backups, rotating on the non-blocking log-writer thread. Both `debug.log` and `diagnostic-*.log` routed through it. Guards against any future runtime log-flooding bug.

**Upstream still pending:** `rmcp::transport::AsyncRwTransport::receive()` converts *any* IO error to `None`. It should distinguish transient (`WouldBlock`, `Interrupted`) from fatal (`BrokenPipe`, EOF). File an issue with rmcp so this wall is no longer needed.

## Tests added

- `size_rotating_file_rotates_on_cap_exceeded`
- `size_rotating_file_caps_total_growth_at_keep_plus_one`
- `size_rotating_file_single_write_under_cap_does_not_rotate`
- `numbered_appends_suffix`
- `resilient_stdin_absorbs_would_block` (updated to mirror new backoff state machine)

## Workarounds

N/A — fix shipped same day.

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-047** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Regression of: BUG-033 (`ResilientStdin` originally added 2026-03-21).
- Upstream issue: rmcp `AsyncRwTransport::receive()` IO error handling.
