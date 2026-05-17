---
title: Archive-tracker rule — eval
parent: src/prompts/source.md (`### Artifact & Tracker Routing` → "Archive workflow")
created: 2026-05-17
version: v0.1
baseline: 4/5 PASS (1 case-design defect)
---

# Archive-tracker rule — eval

This eval grades the **archive-tracker rule** in `src/prompts/source.md`
(slot: `### Artifact & Tracker Routing` → "Archive workflow") against 5
representative cases. Re-score before any future description change. Treat
<4/5 as a regression — refine the rule before shipping.

## Method

Dispatch a fresh subagent (general-purpose, no session context). Provide:

- The rule text verbatim (from `src/prompts/source.md`).
- The 5 cases below.
- The rubric (every criterion must PASS for case PASS — partial credit FAILs).

The agent answers each case (verdict + procedure) and self-grades strictly.
Pattern threshold: **≥4/5 cases PASS**.

## Cases

### Case 1 — Easy positive, unindexed

**Shape:** Friction-log tracker at `docs/trackers/<name>.md`. Enumeration
shows 16/16 entries closed (mix of `fixed-verified`, `mitigated`,
`wontfix-false-alarm`, `promoted-to-bug-tracker`). File has no librarian
frontmatter `id:`.

**Criteria (all must PASS for case PASS):**

- C1a: Verdict is "archive"
- C1b: Names the librarian-index check (`artifact(action="find", ...)`) as
  step 1
- C1c: Names that the move + inbound-ref rewires go in ONE atomic commit
- C1d: Names the verification clause (100% rename + symmetric ins/del)

### Case 2 — Easy positive, librarian-indexed

**Shape:** Plan tracker at `docs/trackers/<name>.md`. All 3 phases `[x]`.
Frontmatter contains `id: <hex>` (librarian artifact).

**Criteria (all must PASS for case PASS):**

- C2a: Verdict is "archive"
- C2b: Chooses `artifact(action="move", ...)` — **NOT** `git mv`
  (load-bearing — `git mv` would not update the librarian index)
- C2c: Names that the move + rewires go in ONE atomic commit
- C2d: Names the verification clause

### Case 3 — Easy negative, mixed state

**Shape:** Tracker with 12 entries: 7 `fixed-verified`, 3 `mitigated`,
**2 still `open`**.

**Criteria (all must PASS for case PASS):**

- C3a: Verdict is "do-not-archive"
- C3b: Enumerates which entries are still open (the 2 open ones)
- C3c: Proposes a resolution path (close the 2 first, OR split the tracker)

### Case 4 — Edge: blocked-on-external (gap test)

**Shape:** Tracker with 6 entries, all status `blocked-on-external` (waiting
on an upstream library bug-fix). Closed from this team's side but not
resolved.

**Criteria (all must PASS for case PASS):**

- C4a: Names that the rule's status list (`fixed-verified | mitigated |
  wontfix | promoted to <other>`) does NOT include `blocked-on-external`
- C4b: Verdict is "ask-user" (do NOT silently archive nor silently keep open)

### Case 5 — Edge: wrong path

**Shape:** Tracker at `docs/archive/old-trackers/bug-tracker.md` (note: `docs/issues/`, not
`docs/trackers/`). Zero-open per its enumeration.

**Criteria (all must PASS for case PASS):**

- C5a: Notices the path mismatch — file is NOT in `docs/trackers/`
- C5b: Does **NOT** blindly move to `docs/trackers/archive/`; either routes
  to `docs/issues/archive/` OR asks the user about the destination

## Rubric & threshold

| Case | Max criteria | PASS threshold |
|---|---|---|
| 1 | 4 | 4/4 |
| 2 | 4 | 4/4 (C2b load-bearing — `artifact(move)` MUST be chosen) |
| 3 | 3 | 3/3 |
| 4 | 2 | 2/2 (gap MUST be named) |
| 5 | 2 | 2/2 (sibling-archive routing MUST be named) |

**Per-case verdict:** PASS only if every criterion is PASS. Partial = FAIL.

**Pattern threshold:** ≥4/5 cases PASS = rule ships / stays shipped. <4/5 =
refine rule, re-run eval, do not ship.

## Baseline runs

### v0.1 — 2026-05-17 — 4/5 PASS

Subagent: general-purpose, fresh context. Dispatched mid-session with the
rule text + 5 cases + rubric.

| Case | Result | Notes |
|---|---|---|
| 1 | **PASS** (4/4) | All criteria named verbatim |
| 2 | **PASS** (4/4) | C2b hit — agent explicitly said "Do NOT use `git mv`" citing index preservation |
| 3 | **FAIL** (2/3) | C3b: agent gave count ("2 open") not enumeration. Strict self-grade |
| 4 | **PASS** (2/2) | C4a: agent named the status list excludes `blocked-on-external` verbatim. C4b: asked user, refused silent pick |
| 5 | **PASS** (2/2) | C5a: caught the `docs/issues/` mismatch. C5b: asked user, surfaced `docs/issues/archive/` as candidate |

**Case 3 failure analysis:** Case-design defect, not rule defect. C3b asked
the agent to "enumerate which entries are still open" but the case shape did
not provide entry IDs. The agent gave the count and a qualitative
description; strict self-grade marked FAIL. **v0.2 candidate:** amend Case 3
to provide IDs (e.g. "F-7 and F-11 are still open"), then re-score. Expected
post-refinement: 5/5.

## Version history

- **v0.1** (2026-05-17): initial 5-case set. Baseline 4/5 PASS at threshold.
  Rule shipped. Case 3 case-design defect logged for v0.2 refinement.
