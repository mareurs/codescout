---
status: fixed
opened: 2026-06-26
closed: 2026-06-26
severity: medium
owner: marius
related: [2026-06-19-mcp-server-oom-68gb]
tags: [heartbeat, oom, instrumentation, retention]
kind: bug
---

# BUG: heartbeat prune (LRU-by-mtime) evicts the OOM victim's file first

## Summary
The durable heartbeat's startup prune (`prune_stale`, keep the 16 newest files
by mtime) deletes the **oldest-mtime** files first. A SIGKILLed OOM victim stops
writing, so its mtime freezes at time-of-death and it always sorts as the oldest
— making the victim's log the *first* thing pruned on the next server startup.
The retention policy actively targets exactly the forensic payload the heartbeat
exists to preserve. Benign on a 1–2 server machine; actively biting here, where
~16 codescout servers run concurrently (3 profiles × worktrees).

## Symptom (Effect)
With ≥`KEEP_FILES` concurrently-alive servers, a dead instance's heartbeat file
can be deleted on the next live server's startup, before a post-mortem reads it.
Observed live state (`~/.cache/codescout/heartbeats/`, 2026-06-26):

```
17 *.log files on disk (cap KEEP_FILES=16 → ceiling is 16+1=17; see Root cause)
16 of them with mtime inside a ~90s window (12:21:55–12:23:29) = 16 ALIVE servers
 1 header-only file (pid 3382007, last write 08:10) = a server that died
```

The fleet currently sits exactly at the cap. One more concurrent server start
prunes the oldest-mtime file. For a *dead* server that is by construction the
oldest, so a victim log is one startup away from deletion.

## Reproduction
`git rev-parse HEAD` = `e559c8a8` (heartbeat shipped in `c7d90cff`).
1. Start `KEEP_FILES` (16) codescout servers so 16 live heartbeat files exist.
2. `kill -9` one of them. Its file's mtime now freezes (oldest among the set).
3. Start one more codescout server (any `/mcp` reconnect or subagent).
4. On startup it runs `prune_stale(dir, 16)`, keeps the 16 freshest (all still
   *alive* and ticking), and deletes the dead one — the file you needed.

## Environment
Linux, codescout `0.15.0` `git_sha=e559c8a8`, MCP stdio. Multi-profile machine:
`~/.claude`, `~/.claude-sdd`, `~/.claude-kat`, each potentially serving several
projects/worktrees → routinely ~16 concurrent servers (observed). Single-server
or small-fleet machines do not hit this.

## Root cause
- `stale_to_remove` (`src/heartbeat.rs:136-143`) sorts entries newest-first by
  mtime and drops everything past `keep` — i.e. an LRU-by-mtime eviction.
- `prune_stale` (`src/heartbeat.rs:148-162`) feeds it every `*.log` in the dir.
- A SIGKILLed process writes no more lines, so its file mtime is frozen at death
  while every living server bumps its mtime each `INTERVAL` (30s) tick. The dead
  file is therefore *always* the oldest → first to be pruned.
- `spawn_durable` (`src/heartbeat.rs:188-233`) calls `prune_stale(&dir, KEEP_FILES)`
  **before** creating its own file, so the on-disk ceiling is `KEEP_FILES + 1`
  (= 17), which explains the 17-vs-16 count. That part is cosmetic; the eviction
  *ordering* is the real defect.

The policy optimizes for the wrong thing: it treats "least-recently-written" as
"least valuable," but for OOM forensics dead == most valuable.

## Evidence
### Live directory state (2026-06-26 12:23)
17 `*.log`; 16 with mtimes in a ~90s window (alive, ticking every 30s); one
header-only dead file from 08:10 (`pid=3382007`, project `southpole/MRV-poc`).
Peak RSS across all 16 data-bearing files was 0.68 GB (`2596197.log`), a healthy
plateau (warmup 43 MB→660 MB in first 3.3h, then flat 645–681 MB band for 43h) —
i.e. **no OOM recurrence and no leak signature** in this 2.5-day window. The fix
is otherwise working: durable, op-tagged, build-stamped, surviving SIGKILL.

### Why the dead file is the prune target
The 08:10 dead file is already the oldest mtime in the set; the 16 living servers
all have mtimes within 90s of each other and of "now". `stale_to_remove` keeps
those 16 and would delete the dead one on the next startup.

## Hypotheses tried
1. **Hypothesis:** 17-vs-16 is the bug. **Test:** read `spawn_durable`.
   **Verdict:** rejected — prune-before-create gives a benign `keep+1` ceiling;
   the eviction *ordering* is the actual defect.
2. **Hypothesis:** mtime-LRU is safe because a ramping victim ticks until death so
   its file stays fresh. **Test:** reason about post-death + host-recover.
   **Verdict:** rejected — after SIGKILL the mtime freezes; on a host-wide OOM the
   restarting fleet each prunes on startup, and the victim (now oldest) is the
   prime target within minutes.

## Fix

**Implemented** on `experiments` 2026-06-26 (age-floor approach, option 2).

`stale_to_remove` (`src/heartbeat.rs`) now takes `now: SystemTime` and
`min_age: Duration`: it retains the `keep` most-recent by mtime **and** any file
whose age (`now − mtime`) is below `min_age`, so a recent crash victim — whose
mtime froze at death and sorts oldest — is never selected for deletion.
`prune_stale` threads `min_age` through and passes `SystemTime::now()`;
`spawn_durable` calls `prune_stale(&dir, KEEP_FILES, RETAIN)`.

Tuning: `KEEP_FILES` 16 → 64 (a 3-profile × worktree machine runs ~16 servers
concurrently — observed 2026-06-26); new `RETAIN = 7 days` age floor. A file
dated in the future (clock skew) is kept (`duration_since` errs → not pruned).

The benign `keep + 1` on-disk ceiling (prune runs before the new file is
created) is left as-is — cosmetic, not the defect.

Master cherry-pick **pending** (master is protected; ship via the standard
sequence). master-side SHA to be recorded here after cherry-pick.
## Tests added

`stale_to_remove_age_floor_retains_recent_victim` (`src/heartbeat.rs`, in
`mod tests`): constructs an oldest-mtime-but-recent "victim" file alongside
genuinely old logs and asserts the victim survives while the old logs past
`keep` are pruned. Existing `stale_to_remove_*` / `prune_stale_*` tests updated
for the new signature (pass `Duration::ZERO` to exercise the count-only path).

Full gate (2026-06-26): clippy `-D warnings` clean; `cargo test` 2925 passed,
0 failed, 43 ignored; 8/8 heartbeat unit tests pass.
## Workarounds
- Before a post-mortem, copy the suspect `~/.cache/codescout/heartbeats/<pid>.log`
  out of the dir immediately — any new server start may prune it.
- Temporarily reduce concurrent server count if hunting a known victim.

## Resume

Cherry-pick the fix to `master` via the standard ship sequence
(`docs/RELEASE.md`), then record the master-side SHA in **Fix** and archive
this file to `docs/issues/archive/`. No further code work needed.
## References
- `src/heartbeat.rs` — `stale_to_remove`, `prune_stale`, `spawn_durable`, `KEEP_FILES`
- `docs/issues/2026-06-19-mcp-server-oom-68gb.md` — the OOM this instrumentation serves
- Heartbeat shipped: `c7d90cff`; cached_capabilities sibling bug: `e559c8a8`
