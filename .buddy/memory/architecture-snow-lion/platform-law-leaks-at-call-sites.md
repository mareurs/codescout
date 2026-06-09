---
specialist: architecture-snow-lion
scope: project
slug: platform-law-leaks-at-call-sites
created: 2026-06-09
updated: 2026-06-09
tags: [platform, cross-cutting-law, coupling, drop-impl, spawn, verification]
---

**Lesson:** When a cross-cutting law is newly declared in `src/platform/` (a canonical builder, a no-spawn kill, a default), the highest-risk leak is an *existing* call site that predates the law and keeps the old inline pattern — especially in another file's `Drop` impl or the highest-traffic path. These leaks are invisible to a diff of `src/platform/` because they live elsewhere in the tree.

**Why:** The VDI reliability work stream leaked twice in one session. (1) Task 5 declared "kill processes via `platform::terminate_process` — no spawn" (Win32 `TerminateProcess`), yet `BackgroundKillGuard::drop` in `src/tools/run_command/inner.rs` still shelled out to `taskkill` — and in a *cancellation Drop*, where a stalled `CreateProcess` under EDR blocks cleanup, the worst place for the anti-pattern. (2) `shell_command_configured` was declared the single shell-spawn builder, yet the foreground `run_command` path built its `Command` inline on both platforms, re-duplicating sh -c / cmd raw_arg / GIT_PAGER / stdin / process_group / SIGPIPE. Both leaks sat outside `src/platform/`; the foreground one was the most-travelled spawn site. Found only by reading the call graph, not the platform diff.

**How to apply:** After declaring or changing a `platform::` function, grep the **whole tree** (not just `src/platform/`) for the *old* pattern it replaces — the raw syscall / shell-out / inline construction: `taskkill`, `tasklist`, `Command::new("sh")`, `Command::new("cmd")`, `std::process::Command` spawns, `raw_arg`. `Drop` impls and timeout/cancel paths are the usual stragglers. Confirm every call site routes through the new abstraction before calling the consolidation complete. Second standing note: `platform::terminate_process` is **semantically asymmetric** — SIGTERM (catchable) on unix, forced `TerminateProcess` on Windows — so a caller wanting a hard, unignorable kill (e.g. an orphan-cleanup guard) must use `libc::kill(SIGKILL)` directly on unix, not the platform fn. Don't assume cross-OS parity behind a shared signature. See [[outputguard-cross-cutting-law]] for the sibling "this is a law, not a helper" pattern.
