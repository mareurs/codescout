# Session Log — Perf + Windows Work Stream

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
| F-1 | 2026-07-02 | med | plan-prose | fixed-verified | WIN-26 zombie-open: lite stack Phases 0-4 shipped to master but tracker said "Phases 1-3 designed" |
| F-2 | 2026-07-02 | med | librarian-artifact | open | windows tracker augmentation missing; body cites nonexistent artifact id 42dfdfc8b1522192 |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
<!-- no wins recorded yet -->

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

## F-1 — WIN-26 zombie-open: lite stack fully shipped but tracker said "Phases 1-3 designed"

**Observed:** 2026-07-02, perf+Windows brainstorm (recon pass before proposing approaches).

**When:** Summarizing open Windows work to scope the brainstorm; about to propose "finish WIN-26 Phases 1-3" as a design approach.

**Expected:** Tracker row WIN-26 (`docs/trackers/windows-platform-support.md`): status `open`, "Phase 0 shipped 825c0c52; Phases 1-3 designed in the plan."

**Got:** `docs/plans/2026-06-16-two-stack-retrieval-lite.md` marks Phases 0-4 ALL DONE (`0ff972f7`, `b96c8ae4`, `93ef0d43`, `9d40d36b`, `5c1ecfa8`); `git branch --contains 5c1ecfa8` → **master**; `src/retrieval/sqlite_code_store.rs` + `src/memory/sqlite_semantic_store.rs` exist. The lite stack is shipped and the lean build is the default.

**Probable cause:** Fix-then-forget: phases landed under `feat(...)` commits naming the plan, not the tracker row; no gate flips WIN-N rows (same root cause as CLAUDE.md's verify-open cadence note / W-7 of the bug-fix stream).

**Workaround:** Flipped the row open→fixed via `artifact(update, body_edits)` + History entry, same session.

**Severity:** med — already caused one wrong "what's open" report to the user this session; unrepaired, the brainstorm would have produced a spec re-implementing shipped code (a wasted design cycle at minimum).

**Status:** fixed-verified

**Fix idea / Pointer:** Row flipped 2026-07-02 (this session). Residual: the plan header still says `Status: draft` and its "Quality tradeoff" benchmark is unrun — left for the owner to decide whether the plan flips to done before a lite-quality benchmark.

---

## F-2 — windows-platform-support tracker: augmentation missing, body cites nonexistent artifact id

**Observed:** 2026-07-02, same recon pass, while attempting the documented WIN-26 row flip.

**When:** Following the tracker's "How to append" protocol (artifact_augment merge + table re-sync).

**Expected:** Tracker is an augmented artifact; `artifact_augment(id="42dfdfc8b1522192", merge=true, params={issues:[...]})` maintains the WIN-N rows; `entry_filter` queries work.

**Got:** Catalog artifact `52451519052d207c` has `augmentation: null`; `artifact(get, id="42dfdfc8b1522192")` → null (id not in catalog). The documented maintenance protocol and the advertised `entry_filter` queries are impossible — the rendered markdown table is the only real surface.

**Probable cause:** Tracker file recreated or re-cataloged after the original artifact (`42dfdfc8b1522192`) was created; the replacement never got re-augmented and the body comments kept the dead id.

**Workaround:** Maintained the row via `artifact(update, patch={body_edits:[...]})` directly.

**Severity:** med — silent: an agent following the in-file instructions either errors or creates a divergent fresh augmentation; `entry_filter` consumers get empty results that read as "no open issues".

**Status:** open

**Fix idea / Pointer:** Re-augment `52451519052d207c` with `issues` params rebuilt from the 26-row table (params_path route — payload >9KB), set `entry_collection="issues"`, fix both in-body id references. Candidate task for the perf-windows plan.

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
