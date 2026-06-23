---
status: open
opened: 2026-06-19
closed:
severity: high
owner: marius
related: []
tags:
  - memory
  - oom
  - mcp-server
  - stability
kind: bug
---

# BUG: codescout MCP server leaked to 68 GB RSS and triggered a kernel OOM-kill, taking down the host

## Summary
A long-running `codescout start --debug` MCP server process grew to **68.7 GB
resident (≈168 GiB virtual)**, exhausted all 4 GB of system swap, and was killed
by the kernel global OOM-killer at **2026-06-19 16:23:33 EEST** on host `ripper`.
The runaway drove the machine into a swap-thrash spiral that made the desktop
unusable for ~20 min and collaterally SIGKILLed `systemd-journald`. One
occurrence in the last 7 days. Root cause (which operation/codepath leaked) is
**not yet identified** — no coredump survived and the offending instance's debug
log was lost (see Evidence).

## Symptom (Effect)
Verbatim kernel log (host journal):

```
Jun 19 16:23:32 ripper systemd[5971]: app-org.kde.konsole-10418.scope: The kernel OOM killer killed some processes in this unit.
Jun 19 16:23:33 ripper kernel: chromium invoked oom-killer: gfp_mask=0x140cca(GFP_HIGHUSER_MOVABLE|__GFP_COMP), order=0, oom_score_adj=200
Jun 19 16:23:33 ripper kernel: Free swap  = 104kB
Jun 19 16:23:33 ripper kernel: Total swap = 4194300kB
Jun 19 16:23:33 ripper kernel: oom-kill:constraint=CONSTRAINT_NONE,nodemask=(null),cpuset=user.slice,mems_allowed=0,global_oom,task_memcg=/user.slice/user-1000.slice/user@1000.service/app.slice/app-org.kde.konsole-10418.scope/tab(2336326).scope,task=codescout,pid=2621226,uid=1000
Jun 19 16:23:33 ripper kernel: Out of memory: Killed process 2621226 (codescout) total-vm:176941204kB, anon-rss:68726128kB, file-rss:2708kB, shmem-rss:0kB, UID:1000 pgtables:136028kB oom_score_adj:200
Jun 19 16:23:33 ripper systemd[1]: systemd-journald.service: Main process exited, code=killed, status=9/KILL
```

- **Victim:** pid 2621226, comm `codescout`, **anon-rss 68,726,128 kB ≈ 65.5 GiB**,
  total-vm 176,941,204 kB ≈ 168.7 GiB, pgtables 136 MB. The 68 GB is *anonymous*
  heap (not file/shmem) → an unbounded Rust-side allocation.
- `chromium invoked oom-killer` is only the trigger (it made the allocation that
  hit the wall); the kernel chose **codescout** to kill because it was by far the
  largest process and carried `oom_score_adj=200`.
- Swap fully drained (Free swap 104 kB of 4 GB). Global OOM (`CONSTRAINT_NONE`).
- The 68 GB freed instantly on kill → system recovered. PSI/vmstat at 17:08 showed
  zero pressure, confirming a one-shot spike, not chronic.
- Collateral: `systemd-journald` SIGKILLed (auto-restarted); `systemd-coredump@`
  for pid 2621226 logged `Failed with result 'timeout'` — no usable core written.

## Reproduction
**Not yet reproducible — best lead:** identify which tool call / operation the
pid-2621226 server was servicing in the minutes before 16:23. It ran as
`~/.cargo/bin/codescout start --debug` (→ `target/release/codescout`) as the MCP
server for a Claude Code session in a Konsole tab (cgroup
`app-org.kde.konsole-10418.scope/tab(2336326)`). Candidate unbounded-allocation
paths to scrutinise: semantic index build, a tool buffering an entire large
output/file into memory, retrieval/embedding batch, or the mux/LSP bridge.

## Environment
- Host `ripper`, Arch Linux (kernel 7.0.x-zen), 125 GiB RAM + **only 4 GiB swap**.
- codescout binary: `~/.cargo/bin/codescout` → `/home/marius/work/claude/codescout/target/release/codescout`, launched `start --debug`.
- Repo at investigation time: **`915867df`** on branch **`experiments`**
  (`git -C ~/work/claude/codescout rev-parse HEAD`). The dead instance's actual
  build commit is unknown (it was a pre-existing long-lived server).
- Transport: stdio MCP server under a Claude Code session.
- ~28 codescout MCP servers were running concurrently; **only this one** ballooned
  (all others were 20–200 MB), so this is a per-instance runaway, not aggregate
  pressure.

## Root cause
**Unknown — under investigation.** The process accumulated ~68 GB of anonymous
heap; total-vm (168 GiB) ≫ rss (65.5 GiB) also suggests large reserved mappings
(big `Vec`/buffer `reserve`/capacity, or repeated growth). Needs the offending
instance's diagnostic log to identify the last operation before the climb — but
that log was not recoverable (see Evidence → "codescout's own logs").

## Evidence

### Kernel OOM (authoritative). How to retrieve:
```bash
journalctl -k --since "2026-06-19 16:20" --until "2026-06-19 16:25" \
  | grep -iE "invoked oom-killer|Out of memory|Killed process|Free swap|task_memcg"
# enumerate ALL oom kills in a window (recurrence check):
journalctl --since "7 days ago" | grep "Out of memory: Killed process"
```
Only one OOM kill in the last 7 days — this one.

### No coredump
```bash
coredumpctl list | grep codescout   # → empty
```
`systemd-coredump@…` logged `Failed with result 'timeout'` at 16:23:33 — a 68 GB
core could not be written in time. Note: OOM delivers **SIGKILL**, so codescout's
Rust panic hook never runs → **no `.codescout/crash.log`** is produced for an OOM.

### codescout's own logs — where they live, and why this instance's are gone
From `src/logging.rs`: with `--debug`/`--diagnostic`, codescout writes under
**`<server-cwd>/.codescout/`**:
- `.codescout/debug.log`            (DEBUG, `SizeRotatingFile`, `MAX_LOG_BYTES`, shared per project-cwd)
- `.codescout/diagnostic-<id>.log`  (per-instance)

both via `tracing_appender::non_blocking` (buffered). To locate the instance that
died, search for logs touched in the kill window:
```bash
find /home/marius -maxdepth 6 -type f -path '*/.codescout/*.log' \
  -newermt "2026-06-19 16:00" ! -newermt "2026-06-19 16:25" 2>/dev/null
```
**Result of running this during investigation: nothing.** Why the instance log
was unrecoverable:
1. **SIGKILL + non-blocking appender** → the final buffered lines (the ramp-up and
   last tool call) were never flushed to disk.
2. The **shared `debug.log`** for that project kept being appended/rotated by
   sibling instances afterward, so the 16:23 lines rotated out (grepping all
   `debug.log`s for a `16:2x` June-19 timestamp returned only stale June-04 lines).
3. The per-instance `diagnostic-<id>.log` for pid 2621226 is gone / in an unknown
   project cwd.

→ **Actionable gap:** an OOM (SIGKILL) currently leaves no codescout-side trace.
See Fix item 3.

## Hypotheses tried
1. **Hypothesis:** Chronic/recurring leak. **Test:** `journalctl --since "7 days ago" | grep "Out of memory: Killed"`. **Verdict:** rejected as chronic — 1 occurrence in 7 d (still a real bug). **Evidence:** Kernel OOM section.
2. **Hypothesis:** Another process (e.g. chromium/brave) was the real hog and codescout was killed only for its high `oom_score_adj`. **Test:** read the kernel victim line's `anon-rss`. **Verdict:** rejected — codescout's own anon-rss was 68.7 GB (kernel-measured), unambiguously the largest single consumer. **Evidence:** Kernel OOM victim line.

## Fix
Plan (not yet implemented):
1. **Find the leak.** Recover the offending operation. Since the live log was lost,
   add lightweight always-on instrumentation first (Fix 3) and reproduce, or
   bisect suspected unbounded buffers (large tool outputs read fully into memory;
   index/embedding batches; retrieval result accumulation).
2. **Bound the offending allocation** — stream/chunk instead of buffering whole;
   enforce a max on whatever grew.
3. **Make OOM observable** (defense-in-depth): periodic RSS heartbeat line to the
   diagnostic log (so the ramp survives even when the SIGKILL'd tail is lost), and
   optionally a soft self-limit that logs+aborts cleanly before the host OOMs.
4. **Blast-radius cap** (ops mitigation): run codescout MCP servers under a
   `MemoryMax=`/`MemorySwapMax=0` cgroup so a future runaway is cgroup-killed in
   isolation instead of taking down the desktop. Also review why the server runs
   with `oom_score_adj=200`.

**Update (2026-06-23) — Fix 3 (make OOM observable) shipped, and reframed.** Recon found a
30s RSS heartbeat *already existed* (`src/server.rs`, `--debug`-gated, `tracing::info!` → the
non-blocking appender). The gap was never "no heartbeat" — it was that the existing one's data
was **lost on SIGKILL** (non-blocking worker buffer), **undiscoverable** (per-instance log in an
unknown server-cwd), **gated on `--debug`**, and **never recorded the in-flight operation**.
Shipped in a new `src/heartbeat.rs`: an **always-on durable sink** that *synchronously* appends +
flushes one `rss_kb=…` line per 30s tick to a **central, predictable** path
`~/.cache/codescout/heartbeats/<pid>.log` (mirrors `logging::sync_append`, the panic hook's
SIGKILL-proof write). Each line carries `op=<tool> op_age_s=<n>` — the in-flight tool, captured at
the single dispatch chokepoint `CodeScoutServer::call_tool_inner` — so the ramp's top names the
leaking operation. A startup header records pid/version/**git_sha**/cwd (so a dead instance's build
is known); stale files are pruned to the 16 most-recent on startup. **Verified:** a `kill -9`'d test
server's header survived on disk at the central path. The 'optionally abort' soft self-limit and the
cgroup blast-radius cap (Fix 4) remain deferred.
## Tests added
N/A — not yet fixed (root cause unidentified).

## Workarounds
- Cap a codescout MCP server's memory so a runaway dies alone, not with the host:
  ```bash
  systemd-run --user --scope -p MemoryMax=20G -p MemorySwapMax=0 \
    codescout start --debug
  ```
- Raising swap only delays the thrash; the cgroup cap is the real mitigation.

## Resume

Fix 3 (a durable, discoverable, always-on, `op=`-tagged RSS heartbeat) **shipped** — see the Fix
Update above; lives in `src/heartbeat.rs`, wired at `src/server.rs` (`run` spawn + `call_tool_inner`).

**Next concrete action (Fix 1 — the leak, still open):** watch `~/.cache/codescout/heartbeats/`
across the fleet. When an instance next ramps, its `<pid>.log` shows the RSS climb *and* the `op=`
tool in flight at the top of the ramp — that finally names the leaking operation. Check whatever
`op=` reports against the two magnitude-plausible audit leads: `embed_queue` full materialization
(`src/librarian/indexer.rs:80,207`) and `run_command` stdout pre-materialization
(`src/tools/run_command/inner.rs:482`). The build commit is now in every heartbeat header, so the
dead instance's build is known. Optional hardening still open: the soft self-limit abort and the
cgroup `MemoryMax=`/`MemorySwapMax=0` blast-radius cap (Fix 4).
## References
- Host journal: `journalctl -k --since "2026-06-19 16:20" --until "2026-06-19 16:25"`
- Logging impl: `src/logging.rs` (debug/diagnostic file layers, `.codescout/` dir, non-blocking appender, panic-hook crash.log)
- Investigated from host `ripper`, 2026-06-19 ~17:10–17:30 EEST.
