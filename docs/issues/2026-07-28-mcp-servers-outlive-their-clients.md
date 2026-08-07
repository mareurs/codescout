---
id: f3c9339e7e7a6822
kind: bug
status: open
title: codescout MCP servers outlive their clients — 18 orphaned `start --debug` processes accumulated over 3 days
tags:
- mcp-server
- lifecycle
- resource-leak
- gpu
topic: process lifecycle
---

# BUG: codescout MCP servers outlive their clients

## Summary

`codescout start --debug` processes are not reaped when the Claude Code session that
spawned them ends. 18 were live simultaneously, the oldest running 2 days 19 hours. They
are individually cheap — ~20 threads and 15–24 fds each, 0.3 GiB RSS in total — so this
is not a memory emergency. It matters for a different reason: each newly spawned server
can trigger a session-start index refresh against the **shared GPU embedder**, so the
spawn rate, not the resident set, is the cost.

## Symptom (Effect)

```
$ pgrep -cf 'codescout start --debug'
18
```

By start date:

| day | processes |
|---|---|
| Sat Jul 25 | 4 |
| Sun Jul 26 | 1 |
| Mon Jul 27 | 2 |
| Tue Jul 28 | 11 |

Oldest: PID 5018, `Sat Jul 25 18:08:52`, elapsed `2-19:48:26`, RSS 2200 KB (largely paged
out), 15 fds, 20 threads. Newest: PID 116918, `Tue Jul 28 13:53:41` — the live one, RSS
29228 KB, 24 fds, 20 threads. Six of the 18 hold open TCP sockets.

## Reproduction

```
git rev-parse HEAD    # ab15cd2f, branch experiments
```

1. Open a Claude Code session with codescout configured as an MCP server.
2. End the session (or `/mcp` reconnect, which spawns a replacement).
3. `pgrep -cf 'codescout start --debug'` — the count climbs and does not fall.

Three concurrent Claude Code profiles on this host (`~/.claude`, `~/.claude-sdd`,
`~/.claude-kat`) multiply the rate, which is why 11 accumulated in a single day.

## Environment

Linux, codescout `experiments` @ `ab15cd2f`. Servers launched as
`/home/marius/.cargo/bin/codescout start --debug` by Claude Code's MCP client. GPU idle at
measurement time (0 %, 771 MiB, two resident llama-servers).

## Root cause

Not investigated. The shape of the evidence points at the server not treating stdio EOF /
client disconnect as a shutdown signal, so it parks instead of exiting. That is a
hypothesis, not a finding — nothing in this entry traced the shutdown path.

What *is* established: the accumulation is not an artifact of one bad session. The spread
across four distinct days rules out a single crash-loop, and the 11-in-one-day figure
matches roughly one orphan per session start rather than a rare race.

## Evidence

### Orphan status confirmed by env drift

PID 3830582 (`11:27:56`) still carries `CODESCOUT_QUERY_PREFIX` in `/proc/<pid>/environ`,
a setting removed from all three profiles' `.claude.json` earlier in this work. The live
server PID 116918 (`13:53:41`) does not. An orphan therefore keeps serving a
configuration that no config file on disk still describes — which is how it was noticed:
sampling `pgrep … | tail -1` returned an orphan and made a completed config change look
like it had failed.

### The GPU connection is the spawn, not the residency

This session's own `SessionStart` hook reported `INDEX: Refreshing in background (9
commits behind HEAD)`, and a matching index lock appeared at 13:28:01 holding PID 28839
with hash `ee26de4c61f6f20e` = `sha256("codescout")[..16]`. So a server start does kick
off an embedder-backed index refresh. Eleven starts in one day is eleven such refreshes
against a GPU embedder shared with every other project — the mechanism behind
"GPU at 100% with no obvious cause", which was investigated three separate times today
and had a different proximate cause each time.


### 2026-08-07 census — the leak is ongoing and now has a cost figure

```
codescout processes (all)   : 22      oldest 336430s = 93.5h    total RSS 1037 MiB
`codescout start --debug`   : 18      oldest 93.5h   mean 46.0h
```

Measured 10 days after the original observation (`ps -o pid=,etimes=,rss=,lstart= -C codescout`).
Two things this pins that the original entry could not:

- **Nothing is being reaped.** The count is still 18 and the oldest process has aged from 2d19h
  (≈67 h) to **93.5 h** — the same process, ~26 h older. So the residency is unbounded, not
  long-but-finite, and no existing mechanism collects them.
- **Aggregate cost is ~1 GiB of RSS**, which the file previously described only as "individually
  cheap". Individually cheap and collectively a gigabyte are both true; the second is the one that
  argues for a fix.

A mean age of 46 h against an oldest of 93.5 h means the population is not one stuck outlier but a
steady accumulation — consistent with one orphan per ended session over roughly four days.

Note for the eventual fix: the mux next to them already solves this, and the census shows it
working. Every `codescout mux` process carries `--idle-timeout` (180 s for rust, 300 s for kotlin)
and there were exactly 4 alive — one per live project/language pair, none stale — while 18
`start --debug` processes with no such flag had piled up over four days. The two populations sit in
the same process table under the same binary, which makes the missing-mechanism argument directly
observable rather than inferred.
## Hypotheses tried

1. **Hypothesis:** the high process count is stale `ps` output or includes the LSP mux.
   **Test:** `ps -o pid=,lstart=,args= -C codescout`, filtered to `start --debug`; the mux
   (`codescout mux --socket …`) is a separate argv and was excluded.
   **Verdict:** rejected — 18 distinct `start --debug` processes, each with its own start
   timestamp.
2. **Hypothesis:** they are cheap enough to ignore entirely.
   **Test:** summed RSS (0.3 GiB / 17 sampled), counted threads (20 each) and fds (15–24).
   **Verdict:** partially confirmed for memory, rejected as a whole — the per-start index
   refresh makes the cost GPU-side and invisible to a memory measurement.

## Fix

**2026-08-06 — shutdown path now READ, and the ordering above is wrong. Direction 1 is already implemented.**

`src/server.rs:1408-1428` is the stdio serve path:

```rust
let (stdin, stdout) = rmcp::transport::stdio();
let service = server.serve((ResilientStdin::new(stdin), stdout)).await?;
tokio::select! {
    result = service.waiting() => { /* logs service_exit */ }
    reason  = shutdown_signal()  => { /* logs service_exit */ }
}
lsp.shutdown_all().await;
```

`ResilientStdin::poll_read` (`src/server.rs:1238`) intercepts **only** `ErrorKind::WouldBlock`, arming a 1 ms backoff; every other outcome passes through unchanged via `other => other`. A true EOF in tokio's `AsyncRead` is `Poll::Ready(Ok(()))` with zero bytes filled — it is not an error, so it is **not** absorbed. It reaches rmcp, which reports `QuitReason::Closed`, `service.waiting()` returns, and the process runs `lsp.shutdown_all()` and exits.

So "exit on stdio EOF" is not a missing feature. Which means the orphans are not failing to *notice* EOF — **they are never sent one.** EAGAIN and EOF are distinct signals: `EAGAIN` means "no data yet, peer still attached", while a closed peer yields a 0-byte read. A process that never sees EOF has a client whose write end is still open — the parent died without closing it, or the fd was inherited by a surviving descendant. `shutdown_signal()` does not help either, because nothing signals these processes.

**Therefore direction 2 (idle timeout) is the fix, and direction 1 should be struck.** The precedent named in the original list still holds: the LSP mux in this same process tree already runs `--idle-timeout 180`.

**Blocked on one decision, deliberately not taken here:** what counts as idle, and how long. A timeout that fires on a live-but-quiet session is worse than the leak — it would kill an MCP server mid-conversation while the user is reading. Needs an explicit value plus a definition of idle (last tool call? last successful stdin read?) before implementation. Direction 3 (suppress the session-start index refresh when another server holds the project's index lock) is independent, carries none of that risk, and remains the cheapest safe win.

**Still reproducing, worse than filed.** Live count 2026-08-06: **16** `codescout ... start` processes, oldest `etime=252846s` (**2 days 22 h**), combined RSS ~1.05 GB:

```
35497   etime=252846s rss=32992KB
903598  etime=219368s rss=9380KB
961164  etime=218504s rss=50000KB
...
3790736 etime=4878s   rss=65900KB
```

Not mitigated by anything in the interim. Note for whoever works this: **do not bulk-kill by pattern** — the list always includes the server for the session doing the killing.

Not implemented, and the shutdown path is unread. Directions, cheapest first:

1. **Exit on stdio EOF.** If the server is stdio-transport, a closed stdin is an
   unambiguous client-gone signal and the smallest correct fix.
2. **Idle timeout**, mirroring what the LSP mux already does — the mux in this same
   process tree runs with `--idle-timeout 180`. Reusing that mechanism for the server
   would be consistent with an established pattern in the codebase.
3. **Suppress the session-start index refresh when another server already holds the
   project's index lock.** This does not fix the leak but removes its GPU cost, and the
   lock is already there to consult.

Direction 1 or 2 fixes the leak; direction 3 fixes the part that is actually expensive,
and the two are independent.

## Tests added

None — no fix. Whatever lands wants an assertion on the *exit*, not on a process count: a
test that closes the client end and asserts the server process terminates within a bound.
Counting processes would pass on a machine that happened to be clean.

## Workarounds

`pkill -f 'codescout start --debug'` reaps them, but it also kills the live server for
every open session, so it needs every Claude Code window closed first. Individually:
`kill <pid>` for each PID whose start time predates the current session.

Until then, **never identify "the current server" with `pgrep … | tail -1`** — PID order
is not start order, and the orphans are indistinguishable by name. Match on start time
against the session's own start, or read `/proc/<pid>/environ` for a setting known to have
changed.

## Resume

The census is refreshed (see Evidence, 2026-08-07): still 18 processes, oldest now 93.5 h, ~1 GiB
aggregate RSS, nothing reaped in 10 days. That is the measurement side done; what remains is a code
read plus one decision.

Read the `start` subcommand's shutdown path and determine whether it watches stdin at all — an MCP
stdio server's client closing stdin is the canonical shutdown signal, and if `start` ignores it,
that is the whole bug. Then compare against the mux's `--idle-timeout` implementation, which solves
the same problem one process over in the same tree and is *observably* working: the census found 4
mux processes, one per live project/language pair with none stale, beside 18 un-timed-out `start`
processes.

**The decision that blocks a fix is what "idle" means for `start --debug`**, and it is the
maintainer's: an MCP server legitimately sits idle between user turns for hours, so the mux's 180 s
is far too aggressive to copy directly. Options are stdin-EOF (precise, no timeout needed, but
depends on the client actually closing it), a long idle timeout measured from last request, or a
parent-death watch. Pick the signal before writing code — the wrong one kills live sessions, which
is worse than the leak.
## References

- `src/bin/` — `codescout start` entry point (shutdown path unread)
- The LSP mux's `--idle-timeout 180`, visible in the argv of the sibling `codescout mux`
  process — an existing solution to the same lifecycle question
- `docs/issues/2026-07-28-edit-code-target-base-from-stale-lsp-range.md` — filed in the
  same session; unrelated mechanism, but both were surfaced by measuring rather than
  trusting a first reading
