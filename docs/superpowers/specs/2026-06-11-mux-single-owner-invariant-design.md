# Mux Single-Owner Invariant — Design Spec

**ADR:** [[2026-06-11-mux-single-owner-invariant]] (architectural decision +
rationale). This spec elaborates that decision into an implementable design.

**Status:** Draft — 2026-06-11, `experiments`.

## Problem

The kotlin/rust LSP is fronted by a per-workspace mux so N codescout processes
share one LSP (`src/lsp/mux/mod.rs:15`, `src/lsp/manager.rs:563`,
`src/lsp/mux/process.rs:66`). The genuinely exclusive resource is the kotlin-lsp
**RocksDB index** (`src/lsp/servers/mod.rs:73`), which takes an exclusive
per-directory lock on the RocksDB-migrated build we run. The architecture's
invariant —

> at most one process holds a workspace's RocksDB index, and that process is the
> live mux

— is currently *hoped for*, not *enforced*, and leaks at three seams (all
observed on `backend-kotlin`, 2026-06-11):

- **S1** mux-ownership lock and RocksDB index lock never check each other.
- **S2** `kill_on_drop(true)` (`process.rs:93`) skips under SIGKILL → orphaned
  JVM squats the index forever (read-confirmed: no `setsid`/process-group/handler
  in `process.rs:66-135`).
- **S3** `get_or_start`'s third mux-failure branch sets `config.mux = false` and
  spawns a competing direct LSP on the same index.

## Goals / Non-goals

**Goals:** enforce the invariant causally via three mechanisms (M1–M3 below);
keep the mux as the single-owner boundary.

**Non-goals (explicit):**
- Upgrading kotlin-lsp / dropping the faketime hack — separate work-stream,
  [[2026-06-11-kotlin-lsp-upgrade-decoupled-from-sharing]].
- A per-user LSP daemon — rejected in the ADR (rule-of-three).
- Changing the rust mux path beyond what M1/M3 touch generically (rust-analyzer
  has no RocksDB index; M2 is kotlin-specific by guard).

## Design

### M1 — Mux owns its LSP as a process group; reaps on signalled exit (S2)

- In `process.rs::run`, before spawning the LSP child, put the **mux** into its
  own session/process group (`setsid` via a `pre_exec` hook on the child, or
  set the child's process group so a group-kill reaches it). Target shape: the
  LSP child and any grandchildren share a killable group id.
- Install a SIGTERM/SIGINT handler (tokio `signal`) in `run`'s event loop that,
  on receipt, kills the child's process group, then returns so `Drop` also runs.
- `kill_on_drop(true)` stays — it is the in-process net for normal scope exit.
- **SIGKILL is uncatchable** — M1 cannot cover it. The SIGKILL case is covered by
  M2's reap-before-spawn. This is the boundary between M1 (graceful) and M2
  (catastrophic) and must be stated in the test plan.

### M2 — Ownership acquisition verifies and reaps the index lock (S1)

- After a codescout process wins the mux-ownership `flock`
  (`get_or_start_via_mux`, the `need_spawn` branch), and before/at mux child
  startup, probe the RocksDB index `LOCK` using the existing
  `kotlin_index_lock_held` (`manager.rs:236`) → `posix_write_lock_is_held`
  (`manager.rs:198`).
- If the index lock is **held**, classify the holder by **reading process/lock
  state** (the lock file's PID line + whether a mux socket for this workspace is
  alive) — NOT by issuing a second-client probe (that recreates the contention;
  recorded miss R-23). A holder whose mux socket is dead is an **orphan**.
- If orphan → reap it (SIGTERM, escalate to SIGKILL after a grace window), wait
  for the lock to release, then proceed to spawn.
- If the holder is a **live** mux's child → this process should not have reached
  the spawn branch (it should have connected); surface the existing actionable
  contention error rather than reaping a live server.
- Probe by **directory glob** (`…/rocks/*/LOCK`), never the hardcoded `v492`
  segment (it changes across builds — ADR alternative #2).

### M3 — Remove the silent direct-LSP fallback for mux languages (S3)

- In `get_or_start`, the mux-failure `Err(e)` arm currently ends with
  `config.mux = false` (fall through to direct spawn). Gate that fall-through:
  for `mux: true` languages it must NOT fire silently. Replace with an explicit
  default-off opt-in used only when `current_exe()` is not the codescout binary
  (the test-runner case the comment already calls out).
- When the fallback is suppressed, return the existing/última actionable
  `RecoverableError::with_hint` — preserve the `fuser … LOCK` + retry guidance
  (agentic surface).

## Interfaces touched

- `src/lsp/mux/process.rs::run` — process group setup + signal handler (M1).
- `src/lsp/manager.rs::get_or_start_via_mux` — reap-before-spawn hook (M2).
- `src/lsp/manager.rs::get_or_start` — fallback gate (M3).
- Reuse: `kotlin_index_lock_held`, `posix_write_lock_is_held`,
  `mux_failure_is_index_contention`, `mux_failure_report`.
- Likely new: a `reap_orphan_index_holder(workspace_root, language) -> Result<bool>`
  helper (returns whether a reap happened), and an `is_test_runner_exe() -> bool`
  predicate for M3's opt-in.

## Test strategy

- **M1 graceful:** spawn a mux against a fixture LSP, send SIGTERM, assert the
  child process group is gone (no orphan PID). Cheap-LSP fixture (e.g. `sleep`
  stand-in) to avoid a real JVM.
- **M1 SIGKILL boundary:** SIGKILL is uncatchable, so the test asserts the **M2
  net** instead — see below. Document explicitly that there is no direct M1
  SIGKILL test (Testing Snow Leopard referral).
- **M2 reap (three-query sandwich):** (1) start holder A on the index; (2) kill
  A's *parent mux* with SIGKILL leaving A orphaned (mutate state outside the
  normal path); (3) assert a fresh `get_or_start_via_mux` first *observes* the
  orphan lock (stale/contended); (4) trigger reap-before-spawn; (5) assert the
  new mux acquires the index (fresh). The stale-assertion in step 3 is what makes
  it a regression test.
- **M2 live-holder safety:** index held by a *live* mux's child → assert the
  spawn branch refuses to reap and returns the contention error (no live server
  killed).
- **M3:** mux language + simulated mux startup failure that is NOT index
  contention → assert NO direct LSP is spawned and an actionable error is
  returned; assert the test-runner opt-in path still falls back (so existing
  `current_exe()`-is-test-runner tests keep working).

## Risks

- **Reaping the wrong process.** Mitigated by the live-vs-orphan classification
  reading state, plus the directory-glob LOCK probe and the PID line in the lock
  file. The live-holder safety test guards this.
- **Signal-handler scope.** Enumerate every exit path of `run` (idle timeout,
  init failure, client-drop) before wiring the handler so it composes with the
  existing teardown rather than racing it.
- **Cross-platform.** `setsid`/process-group is `#[cfg(unix)]` — the mux module
  is already `#[cfg(unix)]` (`mux/mod.rs`), so this is consistent; no Windows
  path to regress.

## Open questions

1. Does `run` already select on any signal/shutdown channel we can extend, or is
   a fresh `tokio::signal` arm needed? (Scout `event_loop`, `process.rs:301`.)
2. Grace window before SIGKILL-escalation in M2 reap — 2s? Tie to the existing
   cold-start grace (`COLD_START_GRACE`)?
3. Should M2's reap also clear a stale mux **socket** file left by the orphan, or
   does connect-retry already handle it?
