---
id: '7e498b6dcb45b924'
kind: tracker
status: active
title: Tracker hygiene log
tags:
- hygiene
- skill-meta
- lifecycle
next-sweep-due: 2026-08-16
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
| D10 session-log-decay | individual | 1 | — |

## Sweep 2026-07-17

**Scope:** docs/trackers/ (D10-focused inaugural sweep) | **Files inventoried:** 21 session-logs (13 live + 8 archived) | **Convention sources:** docs/TAXONOMY.md, docs/trackers/archive-cadence-policy.md (archive dir: docs/trackers/archive/, staleness default 45d; D10 threshold fixed 21d)

| Detector | Findings | Approved | Rejected | Deferred |
|----------|----------|----------|----------|----------|
| D1 index-drift | not run | - | - | - |
| D2 terminal-not-archived | 1 (observed, not gated) | 0 | 0 | 0 |
| D3 stale-active | not run | - | - | - |
| D4 frontmatter-catalog-mismatch | not run | - | - | - |
| D5 canonical-conflict | not run | - | - | - |
| D9 augmentation-stale | not run | - | - | - |
| D10 session-log-decay | 4 | 4 | 0 | 0 |

**D10 findings (all approved → distill-then-archive):**
- **codescout-lessons-2026-05-20** (36d) → archived. F-7/F-8/F-9/F-12 rehomed to the TMR redesign tracker `3e01d4fe6de9d69b` as TMR-5 evidence (status-enum drift + kind:unknown vocabulary gap were independently observed here before the multi-repo survey).
- **dzo-legibility** (32d) → archived. W-4's promote-when was MET, but the lesson is **superseded by its own F-9 correction + ADR** `docs/adrs/2026-06-13-drop-name-collision-defect.md` ("the collision was never a real edit_code block") → NOT promoted, to avoid propagating a corrected-wrong rule. W-5 already promoted-to-ADR.
- **usage-analysis-improvements** (36d) → archived. Open frictions (I-2 redesign follow-up, Kotlin-LSP tracker) preserved in git body.
- **vdi-reliability** (35d) → archived. W-5 (pure platform logic → `platform::mod`) promote-when MET but promotion **DECLINED** — low-impact convention; `language-patterns` memory is a curated top-5 list and a low-impact rule would bloat it. Criterion recorded here.

**Compaction note:** bodies were NOT rewritten. Archived files retain full prose in git; this avoids the librarian body-loss anti-pattern for files that are already out of live view. Decay goal met via catalog archive + value-salvage decisions above.

**D2 observed (not gated):** `docs/trackers/archive/tool-friction-reduction-session-log.md` sits in archive/ with catalog `status=active` — resurfaces next sweep.

**link_scan (write=true, post-move edge repair):** edges_added=38, edges_pruned=0 — the 4 moves' cascade-dropped `cites` edges healed.

**Fixes applied:** this commit.

**Detector trust updates:** D10 session-log-decay individual 0→1 (fired, 4/4 approved, zero rejects/defers). Others neutral (not run this sweep).

**Next sweep due:** 2026-08-16 (frontmatter updated in this edit).

## HY-1 — D10's inaugural run caught exactly the survey-predicted stale logs; surfaced two promotion-nuance cases

**Verdict:** hit

**Sweep:** 2026-07-17 (this ledger).

**Observation:** D10 fired on 4 session logs and correctly excluded the fresh work streams (worktree-overlay, tracker-redesign) and the working-tree-modified `pi-integration` (27d by commit but actively edited) and the 20-day `structural-edit-gate` (under threshold). Two apply-time nuances that the bare detector doesn't capture but the distill procedure surfaced: (1) a win whose promote-when is MET can still be non-promotable if a later correction/ADR **superseded** it (dzo W-4); (2) a fired win can be legitimately **declined** to protect a curated durable surface from low-impact bloat (vdi W-5).

**Proposal:** D10 step-1 ("promote wins") should explicitly say: check for a superseding correction/ADR before promoting a fired win, and weigh curated-surface bloat (defer low-impact rules to the archived body, don't force them into a capped memory). Promote-when firing is necessary, not sufficient.

**Promote-when:** same shape (superseded-fired-win, or curated-surface-decline) recurs in a second sweep → PR the step-1 refinement into `tracker-hygiene/SKILL.md`.
## Template for new entries

<!-- Insert new sweep entries and HY-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## Sweep YYYY-MM-DD\n...")
     Update frontmatter in the same call:
     edit_markdown(..., frontmatter={set: {"next-sweep-due": "YYYY-MM-DD"}}) -->
