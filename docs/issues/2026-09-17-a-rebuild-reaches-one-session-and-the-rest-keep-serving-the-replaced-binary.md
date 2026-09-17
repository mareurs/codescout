---
id: '0db9597a451ba41e'
kind: bug
status: open
title: a rebuild reaches the rebuilding session only; every other server keeps the replaced inode and nothing reports it
owners:
- marius
tags:
- cluster/transient-shared-state-lies-to-readers
opened: 2026-09-17
related: []
severity: medium
---

## Summary

`cargo rb` replaces `target/release/codescout`, which every session's MCP server runs via
the `~/.cargo/bin/codescout` symlink. A running process keeps its **original inode**
mapped after the file is replaced, so it serves the old code until that session restarts
its server with `/mcp`.

One session rebuilds and reconnects. Every other session keeps the pre-rebuild binary,
indefinitely, and **no instrument anywhere reports the split** — not the rebuilding
session, not the sessions still on old code, not `/mcp`, not the gate.

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

Not yet fixed.

## Tests added

None yet.

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
