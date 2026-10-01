---
id: '0db9597a451ba41e'
kind: bug
status: mitigated
title: a rebuild reaches the rebuilding session only; every other server keeps the replaced inode and nothing reports it
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
closed: 2026-09-27
opened: 2026-09-17
related: []
severity: medium
unverified: 'The librarian has no stale-binary write guard, so a server on a replaced binary still runs old code for doc(move) and every other librarian write. Adding one is an open policy decision for the operator: right after a rebuild it would refuse most sessions'' writes.'
---

## Summary

`cargo rb` replaces `target/release/codescout`, which every session's MCP server runs via
the `~/.cargo/bin/codescout` symlink. A running process keeps its **original inode**
mapped after the file is replaced, so it serves the old code until that session restarts
its server with `/mcp`.

One session rebuilds and reconnects. Every other session keeps the pre-rebuild binary,
indefinitely, and **no instrument anywhere reports the split** — not the rebuilding
session, not the sessions still on old code, not `/mcp`, not the gate.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `8e274b32`) — still live, claim narrowed.**
- **Still live.** `scripts/stale-servers.sh` (read-only) was run by the verifier and then by the coordinator, with
  identical counts, against a binary rebuilt 2026-09-24 23:50:57: `servers=30 stale=25 current=5`, `muxes=2 stale=0`.
  Control: all 5 current servers started after the rebuild, and every stale one before it (the oldest 2026-09-18
  11:02:59).
- **This bug's shape.** 23 of the 25 stale servers sit under 20 parents that hold no current server:
  - Three of those parents are `codex` processes (`4057660` alone holds 4 stale servers), which `/mcp` guidance
    never reaches.
  - One parent (`1180549`) no longer exists, so its server outlived its session.
- **Sibling `177695780d080014`'s shape.** The other 2 stale servers sit under parents that also hold a current server:
  a replaced server lingering after `/mcp`. One of those parents, `2834158`, is the coordinating session's own
  `claude` process. Related, not merged: that bug's trigger is this bug's remedy.
- **Narrowed: "no instrument anywhere reports the split" was false when this file was written.**
  - `workspace(action="status")` has returned `server.exe_deleted` and `build_id` since `fbd7f348` (2026-08-28),
    which is § candidate 1.
  - `scripts/stale-servers.sh` predates this file.
  - What is missing is push: `scripts/rb.sh` still runs no fleet check (candidate 2).
- **The move exposure is still unguarded.** `guard_stale_binary` (`src/retrieval/sync.rs:94`) refuses index sync from
  a stale server, but nothing under `src/librarian` checks `exe_deleted`, so § *Why this is more than…*'s
  `doc(action="move")` exposure has no guard.
- **§ Workarounds miscounts.** Its one-liner counts muxes as servers: 7 current today, which is 5 servers plus 2 muxes.
- **SIGTERM immunity re-verified, respawn not.** Re-verified the same day on the coordinating session's own two
  leaked servers (3543619 and 2826596, killed at the operator's instruction; see `177695780d080014`). Both were
  still alive 3 s after SIGTERM, and SIGKILL reaped them. The respawn-on-current claim was not re-tested, since
  it needs a killed server whose session is still in use.

## Symptom (Effect)

A fix is "shipped" from its author's vantage point and absent for everyone else. Each of
the four signals available to the author is satisfied identically in both worlds:

- `cargo rb` exits 0 and the binary's mtime is fresh — true regardless of who serves it.
- `/mcp` reports `Reconnected to codescout` — true of the reconnecting session only.
- The gate is green — it tests the source tree, never a running server.
- Tool calls succeed — the old binary works fine; it is merely old.

## Reproduction

Measured 2026-09-17T19:56:41+03:00, immediately after a `cargo rb` whose binary mtime was
`19:49:05`:

```
processes whose exe is .../codescout/target/release/codescout : 19
  ... without "(deleted)"  -- running the CURRENT binary        :  2   started 19:55:18
  ... with    "(deleted)"  -- holding the REPLACED inode        : 17
old-binary servers with a LIVE `claude` parent                  : 17
old-binary servers that were orphans                            :  0
```

Three of the seventeen had a parent whose cwd is this checkout. The two current ones are
the reconnecting session's own.

The discriminator is `readlink /proc/<pid>/exe` — the kernel appends ` (deleted)` once the
inode is unlinked, which is exactly the condition *"not the binary now on disk"*.

**The control that makes the count a measurement rather than a broken filter:** the two
processes WITHOUT the marker started at `19:55:18`, *after* the `19:49:05` mtime, and all
seventeen with it started before. A filter matching nothing and a fleet that is fully
updated would both print `0`; they are told apart by the two that legitimately match.

Measured 2026-09-18, after SIGKILLing the seventeen stale servers:

```
09:57:40  after the kill   servers alive: 1 (mine, stale)
10:01:15  after /mcp here  mine respawns on the CURRENT inode
10:01:21  peer 40130's server reappears,   CURRENT inode
10:01:46  peer 3818904's server reappears, CURRENT inode
10:02:03  fleet: stale=0 current=4
```

**A killed server comes back, and it comes back on the current binary.** Nothing was done
to those two peers; their servers reappeared on their own. That materially cheapens the
second remedy above — `rb.sh` reaping stale servers after a build needs no follow-up action
from any operator, because the reap IS the update.

**Confidence, stated because the obvious reading is not the only one.** This cannot fully
separate *auto-respawn on the session's next codescout call* from *the operator ran `/mcp`
on two of the three*. The discriminator pointing at the former is the session that did
**not** come back: peer `3288266` had no server at all at `10:02:03`. An operator
reconnecting sessions would plausibly have covered it; a use-triggered respawn would not,
because that session was idle and had made no call. Suggestive, not settled — the clean
experiment is to kill one idle session's server and watch whether it returns before that
session is touched.

**Two things this does NOT weaken.** The seventeen ran stale for roughly fourteen hours
without being killed, so respawn repairs nothing on its own — it is triggered by the
process dying, never by the binary changing. And SIGTERM did not end them: all seventeen
survived it with identical pids and start times, and only `kill -9` worked. So any reaper
must use SIGKILL, which is safe only against an idle server; all seventeen parents were
`idle` and all seventeen processes were sleeping when this was done, and a reaper would
have to check the same thing rather than assume it.

## Environment

codescout `experiments`, Linux, one checkout shared by several sessions across profiles;
`~/.cargo/bin/codescout` a symlink into `target/release/`.

## Root cause

POSIX unlink semantics, plus a per-session restart requirement nothing enforces or
observes. `CLAUDE.md` § *Development Commands* says *"after it, run `/mcp` to reconnect"*,
which is correct **for the session that ran the build** and silent about the other N−1.
The guidance is a policy addressed to one party; the state it governs is shared by all.

## Why this is more than "restart to get new code"

The exposure is proportional to what the rebuild FIXED. This was observed while shipping
`0370c1ab`, where the old code cascade-deletes a ledger's durable outgoing `entry_cite`
rows on `doc(action="move")` — the archive route this repo *mandates* over `git mv`. So
for as long as a peer serves the old binary, the documented-safe archive path keeps
destroying data on that session, while the bug file reads `status: fixed` and cites a SHA
and a patch-id.

Observed directly rather than inferred: the `doc(action="move")` calls made minutes before
the rebuild returned a `history_grafted` block carrying only the four pre-fix fields.

## Root-cause candidates for the fix

Not settled; recorded so the next session does not re-derive them.

- **Make the server report its own build.** A `build_id` (compile-time commit SHA) on
  `workspace(action="status")` turns an unobservable event into a readable value, and makes
  *"am I on the fix?"* answerable without walking `/proc`.
- **Have `rb.sh` enumerate the fleet after a successful build** and print which live
  sessions still hold the replaced inode, by the `/proc/<pid>/exe` test above. It already
  refuses when HEAD is behind origin, so it is the surface that exists and the party who
  knows a rebuild just happened.
- **Notify the affected sessions.** The socket registry makes each addressable. Weakest of
  the three: it costs a round trip per session, a peer can decline, and on a machine this
  size "tell everyone plausible" is not a bounded action. It informs; it does not close.

The first two are complements, not alternatives: one lets a reader ask, the other tells
the builder who needs asking.

## Fix

**The reporting half is fixed as of 2026-09-27; the split itself cannot be.** A replaced inode stays mapped until
its process exits, which is POSIX, and only each session's operator can type `/mcp`. So this file is `mitigated`.

- **Candidate 1 (a `build_id` on `workspace(action="status")`)** predates this file (`fbd7f348`, 2026-08-28), as the
  re-verification above records.
- **Candidate 2 (`rb.sh` enumerates the fleet): BUILT.** After a SUCCESSFUL build, `scripts/rb.sh` prints
  `scripts/stale-servers.sh --sessions`: every Claude Code session whose LIVE server is on the replaced binary,
  listed by socket and launch directory, with the builder's own row marked. Superseded servers (`/mcp` does not reap
  them) and servers under non-Claude parents (`/mcp` does not reach them) are counted apart and never listed as
  sessions to reconnect. It is addressed to the operator, because `/mcp` is a slash command only they can type.
  - The report runs only after a successful build, since a failed one replaced nothing.
  - The build's own exit status is passed through. `rb.sh` no longer `exec`s the build, so that is now its job.
  - A failing report never changes the exit status.
  - The report is overridable (`CODESCOUT_RB_FLEET_CMD`), for the same reason the build is.
  - `CLAUDE.md` § Development Commands now says so, where the `/mcp` instruction sits.
- **Candidate 3 (notify the sessions):** not built, still the weakest. It costs a round trip per session and
  informs without closing anything, and the operator is who acts.

**Still open, and deliberately not decided here: a stale server runs OLD code for librarian writes.**
`guard_stale_binary` (`src/retrieval/sync.rs:94`) refuses index sync from a replaced binary, and nothing under
`src/librarian` checks `exe_deleted`, so the `doc(action="move")` exposure in § *Why this is more than…* is
unguarded. Adding that guard is a policy decision, not a drive-by: right after any rebuild almost every server is
stale (26 of 27 on 2026-09-25), so it would refuse most sessions' librarian writes until they reconnect. It is left
to the operator.


## Fix provenance

- **SHA:** `775181db78996c54ca7924b0e3a69f015f110ac0` (`experiments`)
- **patch-id:** `e597166601ff9b1396b32b766cbaa3c6a0ecdd01`

This is the reporting half only (`scripts/rb.sh` printing the fleet after a successful build). The split itself cannot be fixed, and the librarian stale-binary write guard is an open operator decision, so neither has a commit.


## Tests added

- **`tests/rb-guard.sh` (40/0).** New rows:
  - the fleet report runs after a successful build;
  - it does not run after a failed build, which keeps its own exit code (7);
  - it does not run on a refusal;
  - a failing report leaves the exit code 0;
  - with the seam empty, the default path prints the real `FLEET:` line.

  Every allow row sets a SILENT fleet marker, which keeps the "completely silent" row hermetic.
- **`tests/stale-servers.sh` (45/0).** `--sessions-report`:
  - two stale sessions are listed by socket;
  - the self marker lands on its row only;
  - `/mcp` and "operator" appear in the remedy;
  - a current live server lists nothing, and its superseded sibling is counted apart;
  - non-Claude and parentless servers are counted, never listed;
  - a stale mux is not a server;
  - a two-live-servers tie is one session.

  The live `--sessions` prints a FLEET line on any machine.

Nine mutations via `scripts/mutation-probe.sh`, each killed by its named row:

| mutation | killed by |
|---|---|
| `exit 0` in place of the build's code | the exit-code row |
| report on failure | the failed-build row |
| a failing report fails the build | the still-exits-0 row |
| no default report | the FLEET line |
| self marker on every row | the other-row check |
| any CONN value | the current-live and neither-listed rows |
| any flag | the current-live row |
| mux counted as a server | the non-Claude count and mux rows |
| no dedupe | the tie row |

## Workarounds

After any `cargo rb` that fixes behaviour other sessions rely on, check the fleet:

```sh
for p in $(pgrep -x codescout); do readlink /proc/$p/exe; done | sort | uniq -c
```

A ` (deleted)` line is a server on replaced code. Each such session must run `/mcp`
itself — it cannot be done for them.

## References

- `CLAUDE.md` § *Development Commands* — the `cargo rb` + `/mcp` guidance this extends
- `scripts/rb.sh` — the guarded wrapper, and the natural home for a fleet report
- Found by a post-rebuild reconnaissance of
  `docs/issues/archive/2026-09-17-graft-cascade-deletes-the-source-ledgers-outgoing-entry-citations.md`,
  whose fix is the one seventeen sessions did not receive.
