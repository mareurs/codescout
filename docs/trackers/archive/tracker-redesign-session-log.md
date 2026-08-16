---
status: archived
---
# Session Log — Tracker Management Redesign

> **Purpose:** Two-sided observation log for a multi-session work stream.
> Captures frictions (F-N) and wins (W-N) that the session producing it
> wants to preserve so future sessions inherit the lesson.
>
> **How to use:** Copy this file to `docs/trackers/<topic>-session-log.md`
> in the active project on first reconnaissance pass. Append F-N / W-N
> entries via `edit_markdown(action="insert_before", heading="## Template
> for new entries", content=...)`. Add a row to the Index / Wins Index
> table for each new entry — the indexes are the eval surface, the
> sections are the evidence.
>
> **Lifecycle:**
> - Created at the start of a multi-session work stream.
> - Appended-to across every session that touches the work.
> - Entries with `Status: open` carry forward across sessions.
> - Promotion to permanent surfaces (CLAUDE.md, ADRs, formal bug
>   trackers) happens when the entry's `Promote-when` / `Fix idea`
>   criteria fire.
> - File archived (moved to `docs/trackers/archive/`) when the work
>   stream wraps.

---

## Index

| ID | Date | Severity | Category | Status | Title |
|----|------|---------:|----------|--------|-------|
| F-1 | 2026-07-17 | med | plan-vs-reality | fixed-verified | Stage-1 plan assumed push triggers must be built; hygiene nudge already exists, dormant on missing bootstrap file |
## Wins Index

| ID | Date | Impact | Status | Title |
|----|------|-------:|--------|-------|
| W-1 | 2026-07-17 | med | validated | Pre-plan recon of all four Stage-1 seams prevented duplicate-machinery design |
---

## Category conventions

Use a short kebab-case category to group similar frictions. Prior
sessions have used:

| Category | When to use |
|---|---|
| `codescout-tool` | Friction in a codescout MCP tool (`grep`, `read_file`, `edit_markdown`, etc.) |
| `subagent` | Subagent produced unexpected output or diverged from instructions |
| `plan-prose` | Plan document had drift vs reality (wrong file paths, fictional code, mismatched counts) |
| `architectural` | Discovered structural property of the system that the plan / docs didn't surface |
| `self-friction` | Predicted a friction that turned out to be a false alarm — recorded for transparency |
| `<language>-<library>` | Language- / library-specific footgun (`rust-serde`, `python-typing`) |
| `release-pipeline` | Deployment-time gap (release binary missing, MCP reload needed, etc.) |

Add a new category by writing it as a kebab-case string; no central registry needed.

---

## F-N entry template

Copy this block when appending a new friction. Allocate the next free
ID. Add a matching row to the Index table.

```markdown
## F-N — <one-line title>

**Observed:** <date, session task>

**When:** <what you were trying to do>

**Expected:** <what plan / docs / prior session said>

**Got:** <actual observed reality>

**Probable cause:** <one sentence>

**Workaround:** <what you did to proceed>

**Severity:** low | med | high

**Status:** open | wontfix-false-alarm | fixed-verified | mitigated | promoted-to-bug-tracker | pinned-as-eval-baseline

**Fix idea / Pointer:** <issue # in formal tracker, plan task ID, or "TBD">

---
```

## W-N entry template

Copy this block when appending a new win. A win without a
**Counterfactual** is marketing — name what would have happened
without the pattern, with at least one piece of evidence.

```markdown
## W-N — <one-line title>

**Observed:** <date, session task>

**Pattern:** <the practice that worked>

**Counterfactual:** <what would have happened without the pattern, with evidence>

**Confirming data points:** <list of session moments validating the pattern; aim for ≥2>

**Impact:** low | med | high

**Promote-when:** <criterion for graduating into permanent docs (CLAUDE.md, ADR, etc.)>

**Status:** validated | promoted-to-permanent-docs | archived

---
```

---

## Status vocabulary

Codified so the Index column means the same thing across sessions.

### Friction statuses

| Status | Meaning |
|---|---|
| `open` | Observed, not yet resolved. Default for new entries. |
| `wontfix-false-alarm` | Initial observation was wrong; documented for transparency rather than deleted. |
| `mitigated` | Workaround in place; root cause not fully resolved. |
| `fixed-verified` | Code / process fix landed AND empirically confirmed. (`fixed` alone is too weak — verification is part of the status.) |
| `promoted-to-bug-tracker` | Moved to a formal tracker (`docs/issues/*`, `docs/TODO-*`, GitHub issue). The session log keeps the pointer; the formal tracker owns the lifecycle. |
| `pinned-as-eval-baseline` | Kept verbatim as a reference point for measuring later improvements. Do NOT close — its job is to remain comparable. |

### Win statuses

| Status | Meaning |
|---|---|
| `validated` | Pattern confirmed by ≥1 counterfactual data point. Default for entries with evidence. |
| `promoted-to-permanent-docs` | Moved into CLAUDE.md, an ADR, a skill, or another permanent surface. Session log keeps the pointer. |
| `archived` | Pattern no longer load-bearing — either the underlying system changed or the discipline became automatic. |

---

## F-1 — Stage-1 plan assumed push triggers must be built; the hygiene nudge already exists, dormant

**Observed:** 2026-07-17, reconnaissance before writing the Stage-1 plan
(`docs/plans/2026-07-17-tracker-lifecycle-stage1-plan.md`) for TMR-3/TMR-6
(`docs/trackers/tracker-management-redesign.md`).

**When:** Scouting the "push-based maintenance" seam before drafting plan tasks.

**Expected (plan sketch):** TMR-3 requires building SessionStart push triggers for
hygiene/doctor/refresh from scratch.

**Got (scouted reality):** `claude-plugins:codescout-companion/hooks/session-start.mjs:115-128`
already implements a tracker-hygiene overdue nudge — it reads
`docs/trackers/tracker-hygiene-log.md`, parses a `next-sweep-due: YYYY-MM-DD` frontmatter
line, and injects `TRACKER HYGIENE: sweep overdue` when due. It has been dormant in
codescout the whole time because the bootstrap file was never created (the survey's
"hygiene never ran here" finding). The mechanism works; the seed is missing.

**Probable cause:** Trigger design coupled activation to a file only the skill's *first run*
creates — a chicken-and-egg: the nudge that should cause the first run waits on the first
run's output.

**Workaround:** Stage-1 plan reframed: Task = bootstrap `tracker-hygiene-log.md` (+ seed
`next-sweep-due`) instead of building a new trigger; new-trigger work reserved for the
decay detector only.

**Severity:** med — an unscouted plan would have tasked a subagent with re-implementing an
existing hook (duplicate mechanism, merge conflict with the dormant one, and a second
source of truth for sweep cadence).

**Status:** fixed-verified — plan not yet written; reframe landed before any task was drafted.

**Fix idea / Pointer:** Also a design datapoint for TMR-3 itself: push triggers must not
depend on artifacts their own action creates. Cite in the plan's design notes.

## W-1 — Pre-plan recon of all four Stage-1 seams prevented duplicate-machinery design

**Observed:** 2026-07-17, same recon pass as F-1.

**Pattern:** Before writing an implementation plan that names tool/hook surfaces, scout every
surface the plan will extend: `list_stale` (`src/librarian/catalog/augmentation.rs:350-394`,
signature `(cat, threshold_iso, limit, abs_path_prefix)` — no kind filter), doctor's per-check
function pattern (`src/librarian/tools/doctor.rs:493-547`), `ProjectStatus::call`
(`src/tools/config/mod.rs:251-399`), and the hygiene skill's D1–D9 detector table with its
v2 growth path.

**Counterfactual:** Without the scout, the plan would have (a) re-implemented the SessionStart
trigger (F-1), (b) designed a decay detector as a *new skill* instead of a new detector row in
the existing D-table with its established human-gating and HY-N ledger, and (c) guessed
`list_stale`'s signature (the plan needs a kind-aware variant — the real extension point is the
SQL in `augmentation.rs`, not a new query fn). Estimated cost: ≥2 subagent task rejections and
one architectural rework mid-execution.

**Confirming data points:**
1. F-1 (this session) — dormant-trigger discovery reframed a whole plan task.
2. Pending: next plan touching companion-plugin hooks.

**Impact:** med — one plan task deleted, one redesigned, extension points pinned to real
signatures before any subagent dispatch.

**Promote-when:** A second pre-plan recon across repo boundaries (Rust + plugin hooks) catches
an existing-mechanism duplication. Then promote to memory `reconnaissance` as: "before planning
new automation, grep the companion plugin's hooks for the mechanism first."

**Status:** validated — single datapoint.
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
