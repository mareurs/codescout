# IL1 Friction Diagnosis — Session Log

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
| F-1 | 2026-07-09 | high | codescout-tool | pinned-as-eval-baseline | IL1 recovery-cost distribution: mean gap 21-174 calls, 32-42% never recover same-file within session |
| F-2 | 2026-07-09 | high | codescout-tool | pinned-as-eval-baseline | IL1 ambiguity rate: only 33% of sampled errors map to exactly one overlapping symbol |
| F-3 | 2026-07-09 | high | codescout-tool | pinned-as-eval-baseline | IL1 repeat-offender pattern: 74% of affected sessions hit it on >1 distinct file |
| F-4 | 2026-07-09 | low | codescout-tool | pinned-as-eval-baseline | IL1 dispatch correlation: only 5-11% of sessions hit first IL1 within their first 5-10 calls |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|

_No wins logged for this work stream — analysis-only evidence-gathering pass (Task 3 of the IL1 friction-diagnosis plan)._
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

## F-1 — IL1 recovery-cost distribution (Measurement 1)

**Observed:** 2026-07-09, IL1 friction-diagnosis plan Task 3 (evidence-gathering, no code changes)

**When:** Measuring how expensive an `il1_read_overlaps_symbol` error is once it fires — how many subsequent tool calls elapse before the same session successfully calls `symbols()` on the same file, and what fraction of errors ever get that same-file follow-up at all.

**Expected:** No specific expectation was pinned before running the query; this is a first-time measurement (the task brief calls it "recovery-cost distribution").

**Got:** Ran the brief's SQL (id-gap CTE join: same `cc_session_id`, same `path`, `s.id > e.id`, `s.tool_name='symbols'`, `s.outcome='success'`) against all three repos' `.codescout/usage.db`:

| Repo | total_errors | recovered_count | recovered % | mean_gap (tool-call ids) | max_gap |
|---|---:|---:|---:|---:|---:|
| codescout | 205 | 140 | 68.3% | 31.46 | 694 |
| backend-kotlin | 705 | 413 | 58.6% | 174.13 | 4932 |
| claude-plugins | 22 | 15 | 68.2% | 21.33 | 204 |
| **combined** | **932** | **568** | **61.0%** | — | — |

`total_errors` matches the controller's pre-verified sanity-check counts exactly (205/705/22) in all three repos, confirming the query and DB state are aligned.

32-42% of `il1_read_overlaps_symbol` errors in each repo never get a same-file `symbols()` success later in the same session (`recovered_count/total_errors` is 58-68%, not 100%) — a sizeable minority of these errors are followed by the session moving on without a same-file `symbols()` success (different tool, different file, or the line of work was dropped). Among the errors that *do* recover, the mean gap is 21-174 tool calls later — up to nearly 5,000 calls later in the worst backend-kotlin case — so recovery is typically not "the very next call."

**Probable cause:** Not investigated in this task (analysis-only; the brief doesn't ask for causal attribution here) — the gap size itself is the deliverable.

**Workaround:** N/A — no code changes in this task.

**Severity:** high

**Status:** pinned-as-eval-baseline

**Fix idea / Pointer:** Feeds a follow-up brainstorm on the fail-loud-vs-auto-redirect fork for IL1 (does not decide it). Baseline for measuring any future change to IL1's error message / redirect behavior.

---

## F-2 — IL1 ambiguity rate (Measurement 2)

**Observed:** 2026-07-09, IL1 friction-diagnosis plan Task 3.

**When:** Measuring, for a reproducible 30-error sample (10 most-recent `il1_read_overlaps_symbol` errors per repo, `ORDER BY id DESC LIMIT 10`), how often the requested `[start_line, end_line]` range overlaps exactly one named symbol vs. more than one — bears on whether an automatic redirect to a single symbol would even be well-defined.

**Expected:** No specific expectation was pinned before running the query.

**Got:** For each of the 30 sampled errors, ran a live `symbols(path=<file>)` overview call (pinned to the correct repo via `workspace=`) and counted overlapping named symbols against `[start_line, end_line]`. Two judgment calls were needed (full rationale in the task-3 report): (1) function-local variable bindings (`kind="Variable"` nested one level under a `Function`) were excluded from the tally — they aren't independently addressable via `symbols(include_body=true)` the way a function/class/struct/field/property is; (2) a container symbol (`Module`/`Struct`/`Class`/`Object`/`impl` block) was counted only when *none* of its children overlapped the range — if a child did overlap, only the child(ren) counted, to avoid trivially double-counting "class + the one method inside it" as 2.

| Outcome | Count | % of 30 |
|---|---:|---:|
| Unambiguous (exactly 1 overlapping symbol) | 10 | 33.3% |
| Ambiguous (>1 overlapping symbols) | 18 | 60.0% |
| Zero overlap (symbol layout drifted since error was logged) | 1 | 3.3% |
| File missing/renamed since error was logged | 1 | 3.3% |
| **Total** | **30** | **100%** |

Separately: 0/30 sampled errors had a NULL `start_line`/`end_line` (0% were whole-file reads under the brief's definition) — every sampled error carried an explicit numeric range.

Two of the ambiguous cases are structurally distinct from "range straddles unrelated symbols" and are called out separately in the full report: one row's range (`VariableDurationFullAssertTest.kt`, `1-244`) covers almost exactly one Kotlin test class's full extent (the class spans 50-244) and mechanically overlaps all 14 of that class's members; two "dense declaration file" cases (`codescout/src/lib.rs`'s one-line `mod X;` declarations, `backend-kotlin`'s `Stage2ConstraintConfiguration.kt` one-line-per-constraint properties) produce high overlap counts (12 and 6 respectively) that reflect file structure rather than genuine navigation confusion.

**Probable cause:** Not investigated (analysis-only).

**Workaround:** N/A.

**Severity:** high

**Status:** pinned-as-eval-baseline

**Fix idea / Pointer:** Feeds the fail-loud-vs-auto-redirect brainstorm — the 33%/60% split is the number that gates whether an auto-redirect could pick a single target deterministically. Does not decide the question.

---

## F-3 — IL1 repeat-offender pattern (Measurement 3)

**Observed:** 2026-07-09, IL1 friction-diagnosis plan Task 3.

**When:** Measuring, per session that ever hit an `il1_read_overlaps_symbol` error, whether the session hit it on exactly one distinct file (once-and-corrected) or on more than one distinct file (standing habit).

**Expected:** No specific expectation was pinned before running the query.

**Got:**

| Repo | sessions w/ IL1 | distinct_files>1 | % | distinct_files=1 | % | anomalous (path=NULL) |
|---|---:|---:|---:|---:|---:|---:|
| codescout | 37 | 26 | 70.3% | 10 | 27.0% | 1 (2.7%) |
| backend-kotlin | 49 | 41 | 83.7% | 8 | 16.3% | 0 |
| claude-plugins | 9 | 3 | 33.3% | 6 | 66.7% | 0 |
| **combined** | **95** | **70** | **73.7%** | **24** | **25.3%** | **1 (1.1%)** |

`sessions w/ IL1` matches the controller's pre-verified distinct-session sanity check (37/49/9) exactly. The one codescout anomaly is a single session whose one IL1 error row has `json_extract(input_json,'$.path')` = NULL, so `COUNT(DISTINCT path)` = 0 — it falls outside both the `=1` and `>1` buckets rather than being silently absorbed into either.

Top single-session offender counts (distinct files touched in one session): codescout max 14 (session `ef2bd921…`), backend-kotlin max 77 (session `61fbed7d…`), claude-plugins max 7 (session `43e04b0c…`).

The large majority of sessions that ever hit `il1_read_overlaps_symbol` (74% combined; 70-84% in the two larger repos) hit it on more than one distinct file, not just once.

**Probable cause:** Not investigated (analysis-only).

**Workaround:** N/A.

**Severity:** high

**Status:** pinned-as-eval-baseline

**Fix idea / Pointer:** Feeds the fail-loud-vs-auto-redirect brainstorm. Does not decide it.

---

## F-4 — IL1 subagent-dispatch correlation (Measurement 4)

**Observed:** 2026-07-09, IL1 friction-diagnosis plan Task 3.

**When:** Measuring, per session that ever hit an `il1_read_overlaps_symbol` error, how many tool calls preceded the *first* such error in that session — testing whether IL1 is concentrated near session/subagent-dispatch start (a fresh agent immediately reaching for a raw read) or spread later into sessions.

**Expected:** No specific expectation was pinned before running the query.

**Got:**

| Repo | sessions w/ IL1 | within first 5 calls | % | within first 10 calls | % | mean calls before first IL1 |
|---|---:|---:|---:|---:|---:|---:|
| codescout | 37 | 2 | 5.4% | 4 | 10.8% | 57.8 |
| backend-kotlin | 49 | 3 | 6.1% | 5 | 10.2% | 54.8 |
| claude-plugins | 9 | 0 | 0.0% | 1 | 11.1% | 63.2 |
| **combined** | **95** | **5** | **5.3%** | **10** | **10.5%** | — |

("within first N calls" = fewer than N tool calls in that session preceded the first IL1 error, i.e. the error itself was the session's (N or fewer)-th call.)

Only 5.3% of sessions hit their first IL1 error within the first 5 tool calls, and 10.5% within the first 10; the mean number of calls preceding the first IL1 error is 55-63 across the three repos. IL1 errors are not concentrated near session start — most sessions that hit one only do so well into the session (min-before values across repos ranged 2-7 calls, so it is *possible* early, just not typical).

**Probable cause:** Not investigated (analysis-only).

**Workaround:** N/A.

**Severity:** low

**Status:** pinned-as-eval-baseline

**Fix idea / Pointer:** Feeds the fail-loud-vs-auto-redirect brainstorm (for this data, rules out "fresh dispatch reaches for raw read immediately" as the dominant driver). Does not decide the fork.

---

## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
