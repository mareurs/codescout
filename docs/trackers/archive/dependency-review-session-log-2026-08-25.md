---
id: 228a7a2f4dc2378d
kind: tracker
status: archived
title: Dependency review session log
tags:
- dependencies
- build-leanness
- session-log
---

# Dependency review session log

> Work-stream log for the dependency / build-leanness review started
> 2026-07-25 (`git2` investigation → full manifest audit). Frictions (F-N)
> and wins (W-N) from sessions touching dependency graph, feature gating,
> and build cost.
>
> **Archived 2026-08-25** — the work stream wrapped same-day (2026-07-25):
> all five frictions were fixed-verified in-session (plan edits landed
> before any subagent ran), and neither win's `Promote-when` criterion has
> fired. Compacted per the tracker-hygiene skill's distill-then-archive
> procedure; full prose history is in git (pre-compaction blob) and in the
> citing plan/ADR listed below.
>
> Category conventions and the full Status vocabulary are pinned in
> [`docs/templates/session-log.md`](../templates/session-log.md).

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-25 | med | codescout-tool | fixed-verified | Inferred `#[cfg]` gating from grep line-adjacency; modules were ungated |
| F-2 | 2026-07-25 | med | rust-cargo | fixed-verified | `cargo tree -p X --no-default-features` retargets the flag to X, invalidating subtree closures |
| F-3 | 2026-07-25 | high | plan-drift | fixed-verified | Task 1.3 gates `pub mod client;` — 14 ungated consumers, and it excludes Task 1.0's invariant test from every build config |
| F-4 | 2026-07-25 | med | plan-drift | fixed-verified | "the four OpenAI wire structs (`:92-117`)" is five structs; two belong to the retained sparse leg |
| F-5 | 2026-07-25 | med | plan-drift | fixed-verified | Task 1.4's `any(server-stack, remote-embed)` body gate contradicts Task 1.5's `dep:rustls` placement |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-07-25 | med | A/B the real manifest (patch → measure → restore) for any dep-cost claim | Review would have shipped "one-line change saves 48 crates" — the one-line version does not compile | validated |
| W-2 | 2026-07-25 | high | Re-scout a plan's cited shapes in the NEW session, and enumerate consumers for every proposed `#[cfg]` gate | Stage 1 would have dispatched with a wall of E0433 across 12 files and a load-bearing invariant "pinned" by a test that compiles in zero configurations | validated |

## Outcomes (archive digest)

- **F-1 … F-5:** all `fixed-verified` same day — corrected mid-review or landed as plan
  edits before any subagent ran. No code touched; no follow-up owed. Full repro/fix detail
  lived in each entry's body (pre-compaction; see git history on this file, or the citing
  plan `docs/plans/2026-07-25-embedding-transport-consolidation.md` Stage 1 tasks
  1.0/1.3/1.2/3.1/1.4/1.5/3.2, which carry the corrected claims directly).
- **W-1 — unfired `Promote-when`:** "A second session uses patch→measure→restore for a
  build-config claim and it catches a wrong assertion" → promote to CLAUDE.md as
  *"dependency and feature-cost claims are measured by A/B-ing the manifest, never read
  off `cargo tree -p`."* Sitting at 1 datapoint (this session). Not promoted.
- **W-2 — unfired `Promote-when`** (explicitly two separate rules, each at 1 datapoint,
  per this entry's own correction of an earlier miscount):
  1. *Session-scoped verification claims do not transfer* — promote to the
     `reconnaissance` codescout memory on a second stale-verification-claim case, in any
     work stream.
  2. *Enumerate the consumer set before accepting a `#[cfg]`-gate task* — promote to the
     `reconnaissance` codescout memory on a second consumer-set case (hit or miss).
- Nothing rehomed: no friction is `open`, so nothing needed a bug-file promotion or a
  handoff to a successor work stream.

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     artifact(action="update", id=<this id>, patch={body_edits: [{
         heading: "## Template for new entries",
         action: "insert_before",
         content: "## F-N — title\n..."}]})
     Also update the matching Index / Wins Index table row at the top.
     Templates + Status vocabulary: docs/templates/session-log.md -->

