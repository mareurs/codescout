---
id: '177695780d080014'
kind: bug
status: mitigated
title: 'BUG: /mcp can replace a server without closing the old one''s stdin, so the old process lives on — and stale-servers.sh calls it current'
tags:
- cluster/selector-narrower-than-its-population
- harness
- mcp-reconnect
- stale-servers
closed: 2026-09-25
opened: 2026-09-25
owner: marius
severity: low
unverified: The leak itself is the harness's (Claude Code) and is unaddressed. An upstream report is drafted in § Upstream report and NOT filed, pending the operator. Only the codescout-side instrument half is fixed.
---

# BUG: `/mcp` can replace a server without closing the old one's stdin, so the old process lives on — and `stale-servers.sh` calls it `current`

## Summary

A `/mcp` reconnect spawns a new codescout server but does not always close the previous server's stdio
connection. The old process keeps running with nobody talking to it: measured 2026-09-24/25, **2 of 7**
servers replaced in one session were left alive (the sixth and seventh replacements, 2026-09-25 07:53 and 09:15, closed cleanly), each still `ESTAB` to the parent `claude` process on its
stdin socket. codescout behaves correctly (a stdio server stays up while its stdin is open); the leak is the
harness's. The part codescout owns is the instrument: `scripts/stale-servers.sh` flags a server `STALE`
only when its executable inode has been unlinked, so the orphan on a live binary is reported `current`, and
its printed remedy — reconnect with `/mcp` — is the act that produced the orphan.

## Symptom (Effect)

Children of one `claude` process (pid 2834158, `.claude-sdd`, Claude Code 2.1.282), every server spawned by a
`/mcp` the operator typed:

| server | started (local) | replaced by `/mcp` at | alive now? | `claude` still holds its stdin peer |
|---|---|---|---|---|
| 3304378 | 09-24 22:17:50 | 22:55:11 | no | — |
| **3543619** | 22:55:11 | 23:56:55 | **yes** — 320 MB RSS, 68 threads, binary `(deleted)` | **fd 16** |
| **2826596** | 23:56:55 | 09-25 00:00:06 | **yes** — 180 MB RSS, 68 threads, current binary | **fd 21** |
| 3190735 | 00:00:06 | 00:02:32 | no | — |
| 3474525 | 00:02:32 | 00:07:52 | no | — |
| 4080390 | 00:07:52 | 07:53:04 | no, and fd 18 is closed | — |
| 2291837 | 07:53:04 | 09:15:52 | no, closed cleanly | — |
| 3533331 | 09:15:52 | (current) | yes, and `stale-servers.sh` `CONN` reads `live` | fd 16, a number reused after 3543619's socket closed |

`scripts/stale-servers.sh` the same hour: 3543619 `STALE` (right, for the wrong reason — its binary was
rebuilt), **2826596 `current`** (wrong: nothing will ever send it a request), 4080390 `current` (right).

## Reproduction

Not deterministic: 5 of 7 replacements closed the old connection. Observe, per `/mcp`: list
`ps -o pid= --ppid <claude-pid>` before and after, and for each surviving codescout child resolve its stdin
peer: `ino=$(readlink /proc/<pid>/fd/0 | tr -dc 0-9); ss -xpn | awk -v i=$ino '$6==i || $8==i'`. A leaked
server's peer row names `"claude",pid=<claude-pid>`.

## Environment

Linux, Claude Code 2.1.282, profile `~/.claude-sdd`, codescout `start --debug` over stdio; several peer
sessions rebuilding `target/release/codescout` in the same hour.

## Root cause

**Harness side, observed not read:** the old connection is never closed on some reconnects. The socket-peer
check is what attributes it — the other end is held by the `claude` process itself, `ESTAB`, so the old
server is not failing to notice EOF; there is no EOF to notice.

**codescout side, read at `scripts/stale-servers.sh:136-139`:** `STALE` means `/proc/<pid>/exe` ends in
`" (deleted)"` — the process runs an unlinked inode — and anything else is `current`. (Not a start-time
against build-time comparison: this file's first draft said so from the output alone, and the script's own
header, `:40-42`, is explicit that it is an inode test.) That selects "running a replaced binary" and names
it "should be recycled", a narrower set than the population the remedy is for: an orphan on a live inode is
excluded, and the remedy it prints (`/mcp`) does not reap anything when the harness keeps the old connection. Also refines
`docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`,
whose *"a clean client disconnect DOES reap"* was right about a disconnect that happened and silent about
a reconnect that does not disconnect.

## Evidence

The table above; `ss -xpn` rows, e.g. `u_str ESTAB … 289175551 … 289175550 users:(("claude",pid=2834158,fd=16))`
paired with `289175550 … users:(("codescout",pid=3543619,fd=0))`.


### 2026-09-25 07:53: a sixth replacement closed cleanly, and the two leaks survived it

The operator rebuilt the binary (`target/release/codescout` mtime 07:52:02, HEAD `6eae6b74`) and ran `/mcp`. The
observations below are `ps`, and `ss -xpn` resolved per stdin inode, with the new server as the control:

- **Clean close.** The server that was answering, 4080390, exited, and `claude`'s fd 18 no longer exists.
- **Control.** The new server 2291837 is `ESTAB` with `claude` fd 26. Its stdin inode matches no `claude` fd
  directly, because each end of a socketpair has its own inode, so resolving the PEER inode is what makes the
  check discriminate.
- **Both leaks persist.** 3543619 is still `ESTAB` with fd 16, and 2826596 with fd 21, both `Sl+`.
  - 3543619 has now outlived four later reconnects, and 2826596 three.
  - So a leaked connection is not closed by a later reconnect either. No reconnect has closed one yet.
- **The instrument is right for the wrong reason.** `stale-servers.sh` now reports 2826596 `STALE`, only because
  the binary under it was rebuilt at 07:52. Its orphan status did not change; an unrelated rebuild moved its label.

## Hypotheses tried

None tested. One correlation, recorded so it is not re-derived as a finding: during 3543619's life the harness
moved a long-running MCP call to the background (a gate run); nothing comparable ran on 2826596, so that alone
does not explain both.

## Fix

Two halves, independent.

1. **Harness (not ours): close the replaced server's stdio on reconnect.** Not fixable here. The upstream report
   below is drafted; filing it is outward-facing, and waits on the operator.
2. **Instrument (ours): BUILT 2026-09-25.** `scripts/stale-servers.sh` has a second axis, `CONN`, computed over the
   whole population by one pure `mark_superseded`:
   - `live`: the NEWEST server under a Claude Code session;
   - `SUPERSEDED`: an older one with a newer sibling under the same session;
   - `NO-PARENT`: its parent process no longer exists;
   - `-`: a mux, or a parent that is not a Claude Code session.

   Its remedy has its own `SUPERSEDED` branch: `/mcp` does not reap such a server, SIGTERM does not either, and
   killing it is the operator's call. The `SERVERS` remedy now counts only the servers a `/mcp` can help. Design
   choices, each written into the script's header:
   - A session is recognised by its cc-socks socket, never by `comm`: a version-pinned install has
     `comm=2.1.258`.
   - Starts are compared as numbers of clock ticks.
   - The rule is not applied to codex parents: one has been seen running four servers, and nothing says it uses
     only one.
   - "The session talks to the newest" holds by construction of `/mcp`, and was observed: `workspace(status)`
     named the newest of three.

   **The same gap was in `scripts/peer-sessions.sh`, and is fixed in the same commit.** It kept whichever codescout
   child the lexical `/proc` glob visited last, so session 1194273 read `cs REPLACED` while its live server was
   current. It now reports the newest child by start time, plus `+N superseded`.

Fix SHA: `59c10059` (experiments; the instrument half)
Patch-id: `2df2498830a27ec70f56658b71b4fac5b51ff524`


## Upstream report (draft, NOT filed)

For `anthropics/claude-code`. It is written to stand alone for a reader outside this repo, with no internal paths
or session ids. Filing it publishes it, so it waits on the operator.

> **`/mcp` reconnect sometimes leaves the replaced stdio MCP server connected and running**
>
> **Environment:** Claude Code 2.1.281 and 2.1.282, Linux, one stdio MCP server (a long-running Rust binary started
> as `<server> start`).
>
> **What happens:** running `/mcp` to reconnect a stdio server spawns a new server process, but on some reconnects
> the previous one is not shut down, and its stdin socket is not closed. The old process stays alive indefinitely,
> because a stdio server has no reason to exit while its stdin is open. It is still `ESTAB` to the `claude` process:
>
> ```
> ss -xpn  # the old server's fd 0 and its peer
> u_str ESTAB ... 289175550 ... 289175551 users:(("<server>",pid=3543619,fd=0))
> u_str ESTAB ... 289175551 ... 289175550 users:(("claude",pid=2834158,fd=16))
> ```
>
> **Measured in one session, 2026-09-24/25:**
> - 7 reconnects; 5 closed the old connection cleanly and 2 did not.
> - The 2 leaked connections were never closed by any LATER reconnect. One outlived four more reconnects, holding
>   ~320 MB RSS.
> - Killing the leaked process with SIGKILL makes `claude` close its end of the socket immediately, and the live
>   server is unaffected. So the harness appears to keep the fd only because nothing closes it.
> - On the same machine, one other session (Claude Code 2.1.281) showed the same shape at two readings hours
>   apart: a live server plus an older, still-running sibling of the same stdio server.
>
> **Not established:** what distinguishes a leaking reconnect from a clean one. It was not deterministic, and no
> correlation was found. One leaked server's life included an MCP call the harness moved to the background, and the
> other's did not.
>
> **Expected:** on reconnect, close the replaced server's stdio (and terminate it after a grace period), so exactly
> one server process per configured stdio server remains.
>
> **Repro sketch:** run `/mcp` several times against a stdio server. After each, list the `claude` process's
> children, and for each server child resolve its stdin peer with `ss -xpn`. A leaked child's peer is the `claude`
> process itself.

## Tests added

- **`tests/stale-servers.sh` § *the second axis: CONN*** (32/0). It drives `--conn` and the three-argument
  `--remedy`, which are the same functions the live loop calls. Six mutations, each killed by the case written for it:
  - a lexical start compare, by the 999/1000 fixture;
  - the session gate dropped, by the codex-parent case;
  - the kind filter dropped, by the newer-mux case;
  - the gone branch dropped, by the NO-PARENT case;
  - the SUPERSEDED remedy branch removed, by its 3 cases;
  - "last row wins", by input order.
- **`tests/peer-sessions.sh` § *a session with two codescout children*** (22/0). A stand-in session with two
  real `codescout`-named children, run through the real `scan_cs_children`.
  - Removing the superseded count is killed, and so is "first visited wins".
  - **"Last visited wins" SURVIVES, by construction, and the fixture says so.** The newer child also has the
    higher pid, so both rules pick it. Only pid wrap produces the field case, and no test can arrange that.

## Workarounds

Kill an orphan only after confirming its stdin peer is its parent `claude` AND that parent has a newer codescout
child: that pair is what makes it unreachable. Never kill on a `STALE` flag alone — a deleted inode says the
binary was replaced, not that nobody is talking to the process, and that is how a live server of another
session gets shot.


**Applied 2026-09-25 about 08:05 +03:00, at the operator's instruction, to 3543619 and 2826596.** Both conditions were
re-checked first against live state:
- the start times were unchanged, so the PIDs had not been reused;
- each stdin peer was still `claude` 2834158 (fds 16 and 21);
- 2834158's newer child 2291837 was `ESTAB` on fd 26.

SIGTERM ended neither: both were still `Sl+` after 3 s. SIGKILL, behind a start-time guard, reaped both. `claude`
then closed fds 16 and 21 by itself, and the live server 2291837 kept serving: the next tool call went through
it. So killing a leaked server does not disturb the harness's current connection. The SIGTERM result matches
`0db9597a451ba41e`'s claim for stale servers generally.

## Resume

Decide whether (2) is worth building now; it is the only half this repo can change.

## References

- `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`
- `docs/issues/2026-09-17-a-rebuild-reaches-one-session-and-the-rest-keep-serving-the-replaced-binary.md`
- `scripts/stale-servers.sh`
