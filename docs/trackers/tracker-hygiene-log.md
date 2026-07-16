---
id: '7e498b6dcb45b924'
kind: tracker
status: active
title: Tracker hygiene log
tags:
- hygiene
- skill-meta
- lifecycle
next-sweep-due: 2026-07-17
sweep-interval-days: 30
---

# Tracker hygiene log

Per-project ledger for the `codescout-companion:tracker-hygiene` skill.
Two kinds of entries live here:

- **Sweep entries** (`## Sweep YYYY-MM-DD`) — one per sweep: per-detector
  findings/verdicts, every reject's reason, fixes applied with commit SHA.
- **HY-N meta-entries** (`## HY-N — <title>`) — observations about the
  *skill itself*: detector hits, misses, false-positive patterns, and
  SKILL.md change proposals. Monotonic per project; never reuse an ID.

The frontmatter `next-sweep-due:` field is read by the companion's
SessionStart hook — an overdue date produces a one-line nudge at session
start. Every sweep entry ends by updating it to
`sweep date + sweep-interval-days`.

## Detector trust state

Batch-approval graduation is per detector, earned from this table.
A detector enters `batch` after **two consecutive advancing sweeps** — a sweep
advances only if the detector fired and every finding was approved (zero rejects,
zero defers). Any reject resets to `individual`; a no-finding or deferred sweep is
neutral (streak unchanged).

| Detector | Mode | Consecutive zero-reject sweeps | Last reject (sweep, reason) |
|----------|------|-------------------------------|------------------------------|
| D1 index-drift | individual | 0 | — |
| D2 terminal-not-archived | individual | 0 | — |
| D3 stale-active | individual | 0 | — |
| D4 frontmatter-catalog-mismatch | individual | 0 | — |
| D5 canonical-conflict | individual | 0 | — |
| D9 augmentation-stale | individual | 0 | — |
| D10 session-log-decay | individual | 0 | — |

## Template for new entries

<!-- Insert new sweep entries and HY-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## Sweep YYYY-MM-DD\n...")
     Update frontmatter in the same call:
     edit_markdown(..., frontmatter={set: {"next-sweep-due": "YYYY-MM-DD"}}) -->

