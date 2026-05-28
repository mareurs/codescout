# Session Log — Template

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
| F-1 | 2026-05-28 | med | architectural | open | Filter engine is compile-to-SQL only; entry-grain filtering needs a new in-memory evaluator |
| F-2 | 2026-05-28 | med | architectural | open | tracker_design archetypes already structure entries, but heterogeneously — no common collection contract for a generic filter |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | YYYY-MM-DD | low/med/high | <pattern> | <what-would-have-happened> | open |

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

## F-1 — Filter engine is compile-to-SQL only; entry-grain filtering needs a new in-memory evaluator

**Observed:** 2026-05-28, pre-spec reconnaissance while brainstorming a metadata-filtering feature for trackers.

**When:** About to write a design doc whose approaches claimed they "reuse the filter engine" to filter entries *within* a tracker (not just across artifacts).

**Expected:** `src/librarian/filter.rs` exposes a reusable evaluator that can match the `{field:{op:value}}` AST against arbitrary key/value maps, so per-entry tracker rows could be filtered with the same syntax used for artifact frontmatter.

**Got:** The AST *type* is reusable (`FilterNode` enum lines 10-15; `LeafOp` + `FromStr` lines 18-48), but the ONLY engine is `compile(&FilterNode) -> SqlFragment{sql, params}` (lines 91-243) which emits a SQL `WHERE` clause. Two blockers on reusing it at entry grain: (1) `ALLOWED_FIELDS` (lines 75-89) is a hardcoded allowlist of artifact *columns* (`kind, status, topic, ..., id`) and `compile_leaf` rejects any field outside it (anti-injection) — so arbitrary entry fields like `category` / `severity` are rejected outright; (2) op semantics live in SQL (`LIKE %v%`, `EXISTS json_each(...)` array membership, `LIKE 'v%' ESCAPE '\'`) and must be re-implemented in Rust for an in-memory path. All callers (`find`, `count_matching`, `catalog_summary` in `catalog/find.rs`) go AST -> SQL.

**Probable cause:** The filter subsystem was designed for catalog-grain filtering (artifact frontmatter columns) only; there was never a need to evaluate the AST in memory against non-column data.

**Workaround:** Any entry-filter design must budget a NEW `eval_leaf(&FilterNode, &serde_json::Map) -> bool` evaluator (~mirrors `compile_leaf`'s 116 lines, minus the `ALLOWED_FIELDS` gate, since an in-memory match has no injection surface) PLUS a dual-engine consistency test asserting SQL-compile and in-memory-eval agree on a shared fixture. Do not claim "free reuse of the filter engine" in the spec.

**Severity:** med — without this catch, the design doc would have under-scoped the entry-filter component; a subagent implementing from it would have hit the `ALLOWED_FIELDS` rejection wall (or routed entries through SQL) and forced a mid-build plan revision.

**Status:** open

**Fix idea / Pointer:** Feeds the metadata-filtering design (this work stream). Candidate shape: an `eval` fn beside `compile` in `filter.rs` sharing `LeafOp` + value-coercion; in-memory path drops the allowlist; add a consistency test. Re-scope severity once the design picks params-source vs body-parse vs SQL-table.

---
## F-2 — tracker_design archetypes already structure entries, but heterogeneously — no common collection contract

**Observed:** 2026-05-28, scouting `tracker_design` (piece-2 seam) during the metadata-filtering brainstorm.

**When:** About to claim the spec would introduce a NEW "filterable entries" convention and a NEW tracker_design archetype to teach it.

**Expected:** Structured per-entry data in params is a new convention this feature must introduce; `tracker_design` needs a new archetype.

**Got:** `tracker_design` (`src/librarian/tools/tracker_design.rs`) ALREADY ships archetypes whose params hold structured, schema-validated entry collections — `archetype_failure_table` (line 79) keeps `params.failures: [{id,status,owner,last_seen,notes}]` with an enum-constrained `status` and a `render_template` that already does filtered counts (`selectattr("status","equalto","fail")`). `task_list`/`goal` keep `children`. `child_status_pure(archetype, params)` (`goal_aggregation.rs:43`) already maps archetype+params -> status. The SYSTEM_PROMPT already teaches the params / params_schema / render_template split. BUT each archetype names its collection differently (`failures` vs `children`) — there is NO common key or pointer a generic entry-filter engine could target. Separately, `archetype_reflective` (line 257) is the Path-B shape: params deliberately minimal, "the body IS the tracker" — this is what the prose W/F session logs are, and what retrofitting converts FROM.

**Probable cause:** Archetypes were each designed for refresh/render in isolation, never for a cross-archetype query engine; no uniform entry contract was ever needed.

**Workaround / design pivot:** (a) Do NOT introduce a redundant convention — the entry-in-params pattern exists (`failure_table` is the exemplar). (b) DO add a common **`entry_collection` pointer** on the augmentation naming which params array is the filterable collection (e.g. `"failures"`), or a reserved `entries` key. (c) The F-1 engine filters whatever the pointer names. (d) Retrofitting = `reflective`-shape -> `failure_table`-shape transform. This also means the scout PREVENTED a redundant parallel convention + an engine built against a single invented shape that would silently miss the 2+ existing collection shapes.

**Severity:** med — without this, the engine would target one invented shape and silently fail on existing archetype collections, or a forked convention would split the tracker model.

**Status:** open

**Fix idea / Pointer:** Spec must define the `entry_collection` pointer + its home (augmentation field vs reserved params key). Scout the augmentation params/record shape before the spec pins that. Feeds F-1's engine target.

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
