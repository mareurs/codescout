---
kind: bug
status: investigating
tags:
- cluster/blast-radius-exceeds-visibility
- multi-account
- coordination
- peer-discovery
- concurrency
- shared-checkout
opened: 2026-08-31
owner: marius
related: []
severity: high
---

# Agents on different accounts cannot see each other, while writing the same checkout

## Measured 2026-08-31 21:27

One host, one Unix user, three Claude Code profiles in daily use (`~/.claude`,
`~/.claude-kat`, `~/.claude-sdd`).

| quantity | value |
|---|---|
| live sessions (socket pid alive) | **15** |
| peers `ListAgents` reported to a `~/.claude-sdd` session | **2** |
| live peers it did not report | **12** |
| sessions with cwd = the codescout checkout | **6**, across **3** profiles |

The two peers it did report were both `~/.claude-sdd` — the reporting session's own
profile. Three `.claude-sdd` sessions were live; it saw the other two and nothing else.
Discovery is scoped to `CLAUDE_CONFIG_DIR`.

Method: every socket in `/run/user/1000/cc-socks/` is named for its owning pid, so
liveness is `kill -0`, and the owning profile is `CLAUDE_CONFIG_DIR` in
`/proc/<pid>/environ`. 22 sockets, 15 live, 7 stale.

## Re-measured 2026-09-24 19:20Z — still live, and the population grew a profile

Same method (socket enumeration, `kill -0` via `/proc/<pid>`, profile from `CLAUDE_CONFIG_DIR`), run by
sessionId `e4fbc7ef-27b7-4707-8469-ccdffa8e4e92` from a `~/.claude-sdd` session, with `ListAgents` called
in the same minute.

| quantity | value |
|---|---|
| live sessions | **28** (27 peers plus me) across **4** profiles: `.claude` 11, `.claude-sdd` 11, `.claude-kat` 4, `claude-cfg` 2 |
| peers `ListAgents` reported | **10**, every one `.claude-sdd` (= that profile's 11 minus me) |
| live peers it did not report | **17** |
| sessions with cwd = this checkout | **7** (6 peers plus me) across 2 profiles; the 2 `.claude-kat` ones are invisible to `ListAgents` |
| sessions with cwd in a `codescout.worktrees/` tree | 2, profile `claude-cfg` — also invisible |

**`claude-cfg` is a fourth config dir that did not exist in the 2026-08-31 measurement**, so the
shortfall is not a fixed offset a reader can correct for. `ListAgents` itself is unchanged: its
scope is still exactly the caller's profile.

What has moved since filing, stated at the grain it was checked: need (1) host-scoped discovery exists
only as `/codescout-companion:reaching-peer-sessions` Step 1 — a mechanism for whoever runs it, not a
change to `ListAgents`; need (3) is served by that table's `CWD` column, which
`docs/issues/archive/2026-09-09-cwd-answers-who-is-alive-and-is-read-as-who-is-working-where.md`
(`b8adf790124afd40`) shows answers launch directory, not occupancy. Needs (2), (4) and (5) were **not**
re-tested in this pass.

## What it cost, concretely

At 21:25:35 a session on `~/.claude-kat` (pid 2900182, cwd
`/home/marius/work/claude/prompt-engineering`) committed `b7c54fe` to a checkout this
session was about to run a measurement campaign in. This session had listed its peers
**ninety seconds earlier** and been told there were two, neither in that repo.

That is not a near-miss. The whole preceding evening was spent managing a hazard this
makes unmanageable: `prompt-engineering`'s eval provenance is a **git subtree hash**, so a
commit landing mid-campaign splits the pool and silently makes rounds unpoolable. The
operating discipline that emerged was "freeze the tree, wait for the campaign, then fix" —
a discipline that assumes you can enumerate who else might write. On this host you cannot:
the enumeration is 2 of 14.

The failure is silent in the worst direction. Nothing errors. The rounds complete, the
numbers look ordinary, and the only signal is a provenance mismatch discovered later — if
anyone thinks to pool them.

## The transport is already shared; only discovery is scoped

`/run/user/1000/cc-socks/` is **user-scoped, not profile-scoped**. All 15 sockets from all
three profiles sit in one directory this session can read, with the pid in the filename.
Nothing about the substrate prevents cross-account discovery — a session can enumerate
every live peer on the host in one `ls` plus a `kill -0`, which is exactly what the table
above is.

So the gap is a **filter**, not a missing channel. Whatever scopes `ListAgents` to the
caller's `CLAUDE_CONFIG_DIR` is discarding information that is already present and already
readable by the same user.

codescout has its own peer layer that is account-agnostic by construction —
`src/peer/{registry,server}.rs`, `src/tools/peer.rs`, `.codescout/peers.toml` — keyed on a
**workspace root** hash (`codescout-peer-<hash>.sock`), never on a Claude profile. That is
the right key for this problem: what agents collide over is a *checkout*, not an account.

## What is needed

1. **Host-scoped peer discovery.** Enumerate live sessions across every profile on the
   host, reporting each one's profile and cwd. The data is already on disk; this is a
   filter change plus a display field, not a new protocol.
2. **Cross-account messaging.** Once a peer is addressable, `SendMessage` should reach it.
   The socket is readable by the same Unix user, so the boundary is policy rather than
   permission — which means it needs a deliberate decision, not an accident.
3. **Checkout-scoped presence, keyed on the workspace root.** "Who else is working in
   `/home/marius/work/claude/prompt-engineering` right now?" is the question that actually
   matters, and it has no answer today. codescout's workspace-hash socket naming is the
   existing precedent.
4. **An advisory claim for a checkout under measurement.** Not a lock — a soft, readable
   declaration ("a campaign is running here, provenance is frozen until it drains") that a
   peer in that checkout can see before committing. The eval harness already writes a
   `PROVENANCE.env` per round; the missing half is making it visible to a peer that never
   looked at the round directory.
5. **Stale-socket reaping,** or at least not counting them. 7 of 22 sockets were dead, some
   five days old. Any discovery built on this directory has to distinguish a live peer from
   a leftover, or it will over-report as badly as it currently under-reports.

## Boundary this must not cross

Cross-account visibility is not cross-account authority. A peer on another account can be
*seen* and *messaged*; it must never be able to grant permission, approve a prompt, or act
as its operator's consent. The existing permission-laundering rule applies unchanged and
gets *more* important once the peer set widens beyond one profile.

## Not in scope

Whether the three profiles should exist at all. They are deliberate (separate accounts,
separate rate limits, separate plugin sets) and this issue takes that as given. The defect
is that agents inside them are blind to each other while sharing a filesystem.
