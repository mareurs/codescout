---
id: '177695780d080014'
kind: bug
status: open
title: 'BUG: /mcp can replace a server without closing the old one''s stdin, so the old process lives on — and stale-servers.sh calls it current'
tags:
- cluster/selector-narrower-than-its-population
- harness
- mcp-reconnect
- stale-servers
closed: null
opened: 2026-09-25
owner: marius
severity: low
---

# BUG: `/mcp` can replace a server without closing the old one's stdin, so the old process lives on — and `stale-servers.sh` calls it `current`

## Summary

A `/mcp` reconnect spawns a new codescout server but does not always close the previous server's stdio
connection. The old process keeps running with nobody talking to it: measured 2026-09-24/25, **2 of 5**
servers replaced in one session are still alive, each still `ESTAB` to the parent `claude` process on its
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
| 4080390 | 00:07:52 | (current) | yes | fd 18 |

`scripts/stale-servers.sh` the same hour: 3543619 `STALE` (right, for the wrong reason — its binary was
rebuilt), **2826596 `current`** (wrong: nothing will ever send it a request), 4080390 `current` (right).

## Reproduction

Not deterministic — 3 of 5 replacements closed the old connection. Observe, per `/mcp`: list
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

## Hypotheses tried

None tested. One correlation, recorded so it is not re-derived as a finding: during 3543619's life the harness
moved a long-running MCP call to the background (a gate run); nothing comparable ran on 2826596, so that alone
does not explain both.

## Fix

Not designed. Two halves, and they are independent:

1. **Harness (not ours):** close the replaced server's stdio on reconnect. Report upstream with the socket-peer
   evidence.
2. **Instrument (ours):** teach `stale-servers.sh` a second axis — a server whose parent's current connection
   is a different child is an orphan whatever its build — and make its remedy say that `/mcp` may not reap it.
   The discriminator is observable without cooperation (`ss -xpn` + the parent's other children).

## Tests added

None yet.

## Workarounds

Kill an orphan only after confirming its stdin peer is its parent `claude` AND that parent has a newer codescout
child: that pair is what makes it unreachable. Never kill on a `STALE` flag alone — a deleted inode says the
binary was replaced, not that nobody is talking to the process, and that is how a live server of another
session gets shot.

## Resume

Decide whether (2) is worth building now; it is the only half this repo can change.

## References

- `docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md`
- `docs/issues/2026-09-17-a-rebuild-reaches-one-session-and-the-rest-keep-serving-the-replaced-binary.md`
- `scripts/stale-servers.sh`
