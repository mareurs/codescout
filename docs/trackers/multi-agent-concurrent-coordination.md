---
kind: tracker
status: draft
title: Multi-Agent Concurrent Coordination
owners: []
tags:
  - multi-agent
  - scoping
---

# Multi-Agent Concurrent Coordination

**Status:** Scoping — two concrete instances on file, conventions
shipped, structural fix deferred  
**Origin:** `docs/trackers/archive/i1-session-friction.md` F-13 (mitigated
2026-05-17 first session) + F-14 (mitigated 2026-05-17 second session)  
**Pattern:** shared resource accessed by ≥2 concurrent agents with no
transaction between observation and action.  
**Decision-by:** next time a third concrete instance of the same fault
line lands.

## Fault line

When two Claude Code sessions (or any two agents on the same repo) act
concurrently, they share resources that lack atomic read-act
transactions:

1. **Git HEAD** (filesystem-level pointer in `.git/HEAD`).
2. **Friction-log F-N / W-N namespace** (sequential ID allocator in
   a markdown file).
3. *(latent)* Any sequential allocator over shared filesystem state —
   tracker IDs, artifact IDs in non-librarian dirs, draft-document
   slugs.

The fault is structural: **observation and mutation are separate
operations, and the resource can move between them.** Every fix on
this fault line must either (a) close the gap atomically, or (b)
shape the work so the gap doesn't matter.

## Confirming data — two concretes

### F-13 — `git reset --soft HEAD~1` on stale HEAD

**Mechanism:** Agent A reads `git log`, plans `git reset --soft HEAD~1`
to amend the previous commit. Between read and reset, Agent B commits
T-13. Agent A's reset evaluates `HEAD~1` *at execution time*, which is
now Agent A's own commit (Agent B's commit is the new HEAD). The reset
silently traverses Agent B's commit. Recovery only via `git reflog`
catching the SHA before retention expires.

**Severity:** high — silent destruction of another agent's work.

**Convention shipped:** CLAUDE.md § Git Workflow → Concurrent-Work
Rules (added 2026-05-17). Rule: never `git reset HEAD~N` during
concurrent work; always quote the explicit SHA after re-reading
`git reflog -10` *in the same command*.

### F-14 — F-N namespace collision in friction log

**Mechanism:** Agent A reads the friction log, sees F-1..F-10, plans
to allocate F-11. Between read and write, Agent B writes F-11. Both
sessions end up referencing "F-11" but for different frictions; the
later writer's F-11 silently renumbers to F-12 in the index but cross-
references (W-N → F-N chains) keep the old number.

**Severity:** med — failure mode is silent; broken references read
plausibly until someone tries to follow them. Cost grows with cross-
reference depth.

**Convention shipped:** manual editorial pass after-the-fact. When a
parallel session is detected, re-read the index and update any
W-N → F-N references whose target has renumbered. Works for the
current low-collision rate but doesn't prevent the silent-corruption
window.

## Why two concretes warrants a design artifact

Per the **two-concretes threshold** (Snow Lion OP-4, loaded as project
memory `two-concretes-threshold`): *a new kind / column / layer /
interface earns its keep only after 2+ concrete instances from the
same root cause. One concrete = decoration; two concretes from same
root cause = design.*

F-13 and F-14 are two concretes from the same root cause. The
shipped conventions are local mitigations (per-resource: git, F-N
allocator) — they do not address the structural problem (shared-state
mutation without read-act transactions). A third concrete will fire
on whatever shared resource isn't yet protected by a convention.

## Options for the structural fix

### Option A — Session-prefixed allocators

For every sequential allocator over a shared markdown surface, prefix
IDs with a session token (e.g. `S1-F-1`, `S2-F-1`). Decentralized,
zero infra, immediately namespaced.

**Coupling change:**
- Trackers (F-N / W-N / U-N / H-N / T-N / etc.) get a session-prefix
  convention.
- Cross-references promote across sessions only after a manual
  consolidation pass (`Sk-F-1` → permanent `F-N` in a per-tracker
  re-numbering).

**Tradeoffs:**
- **Now easier:** zero coordination cost; works offline.
- **Now harder:** longer IDs; prefix lookup needed when promoting
  to permanent docs; doesn't help with non-allocator collisions (git
  HEAD).
- **Scope:** F-14-only. Does not address F-13.

### Option B — Coordinator-allocated IDs via a librarian artifact

Add an "allocator" artifact per tracker (or one global) that hands
out the next free ID atomically (claim via SQL transaction in the
librarian DB).

**Coupling change:**
- New artifact kind: `id_allocator` (or extend an existing tracker
  artifact's schema with a `next_id` counter).
- Trackers transition from "scan markdown for max ID + write" to
  "call allocator, get ID, write".
- Requires the librarian to be reachable from all sessions
  (already true — librarian-mcp is shared).

**Tradeoffs:**
- **Now easier:** allocator-level collisions impossible; ID space is
  consistent across sessions.
- **Now harder:** infra dependency for tracker writes (offline /
  scripted writes need a different path); allocator becomes a
  coupling point for all trackers; doesn't help with non-allocator
  shared state (git HEAD).
- **Scope:** F-14-only. Does not address F-13.

### Option C — Worktree-per-session (closes both concretes)

Each concurrent session works in its own `git worktree`. Friction
logs live per-worktree; merges to the canonical tracker are explicit
+ reconciled at integration time. Git HEAD lives per-worktree too —
F-13's race becomes impossible because the two sessions share no
HEAD.

**Coupling change:**
- Session start: `git worktree add .worktrees/<session-id>
  <branch>`.
- Each session writes its trackers into the worktree.
- Merging into `experiments` becomes a reconciliation step (rebase
  or cherry-pick), not a concurrent append.

**Tradeoffs:**
- **Now easier:** both F-13 and F-14's races become structurally
  impossible (different HEAD, different friction-log file paths).
  The fault line closes once for both concretes plus future
  allocator-collisions.
- **Now harder:** reconciliation work moves from "during the work"
  to "at integration". Disk cost is N copies of the repo (mitigated
  by git's worktree shared-objects-dir). Tooling around CLAUDE.md
  needs to know about worktree layout.
- **Scope:** F-13 + F-14 + future allocator collisions. Addresses
  the *fault line*, not just the symptoms.
- **Revisit-when:** sustained ≥2 concurrent sessions become common
  enough that reconciliation cost is lower than today's
  observe-and-react cost.

## Decision criteria

| Concurrent-session rate | Recommended |
|---|---|
| <1 / week, 1 collision / month | Conventions only (current state) |
| 1–3 / week, 1 collision / week | Option A (F-N namespace only) |
| Daily, multiple collisions / week | Option C (worktree-per-session) |
| Allocator-only collisions, no git HEAD races | Option B is enough |

**Today's state:** 1 concurrent session pair this calendar quarter,
2 collisions both recovered. Conventions are sufficient.

## Promote-when (graduate this tracker to an ADR + plan)

Promote out of `scoping` when **any** of:

- A third concrete instance of the same fault line lands (whatever
  the shared resource is — not necessarily git or F-N).
- A collision causes work loss that the conventions can't recover
  (e.g. reflog retention exceeded; manual editorial pass missed a
  cross-reference for >24h).
- Concurrent-session rate exceeds 1 per day, sustained over
  ≥1 week.
- A new sequential allocator is introduced on a shared markdown
  surface (e.g. a new tracker kind with N-numbered entries) — that's
  a third allocator concrete pre-emptively.

## Stale-when

This tracker becomes wrong when **any** of:

- Option A, B, or C ships. At that point: archive this tracker, link
  the ADR / plan that replaced it.
- The codescout MCP server gains intrinsic session-scoping (e.g.
  every tracker write goes through an MCP tool that auto-prefixes
  with the cc_session_id) — the F-14 concrete dissolves without an
  explicit fix.
- Concurrent multi-agent work stops for ≥3 months (the fault line
  becomes hypothetical; archive as `wontfix-rare`).

## Status

Scoping — conventions shipped for both concretes; structural fix
deferred pending the Promote-when criteria. Two concretes is the
threshold to *write this tracker*, not the threshold to ship code.
