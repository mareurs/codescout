---
status: archived
title: Session Log — edit_markdown Batch Ordering
---
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
| F-1 | 2026-07-17 | med | architectural | fixed-verified | Batch-ordering fix's hard part is write-span conflict granularity, not a resolve/apply split (which already exists) |
| F-2 | 2026-07-17 | med | plan-prose | fixed-verified | `join_lines` invariant leaks at `i == lines.len()`; insert_after-last-section needs a `"\n"`-prefix rule the plan omitted |
| F-3 | 2026-07-17 | med | plan-prose | fixed-verified | Byte-splice refactor drops legacy newline-preservation guards (2nd instance): scoped-edit brief omitted `ensure_trailing_newline`; existing test caught it |
| F-4 | 2026-07-17 | high | architectural | fixed-verified | `apply_planned_edits` mis-orders coincident zero-width insert + non-zero span (same start byte) → order-dependent silent corruption; caught only by final Opus review |

## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-07-17 | med | Scout the apply functions before speccing their refactor | Spec would have inverted the cost model AND baked a section-granularity overlap rule that rejects the target use case | validated |
| W-2 | 2026-07-17 | high | Opus final whole-branch review on shared-infra core | A Critical order-dependent corruption bug (C-1) at a task seam would have shipped — invisible to every per-task review | validated |

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

## F-1 — Batch-ordering fix's hard part is write-span conflict granularity, not a resolve/apply split

**Observed:** 2026-07-17, brainstorming a fix for the `edit_markdown` batch-ordering contention (a batch that both renames `## The 7 cases` → `## The 8 cases` AND adds a row under that section fails: edit[1] resolves its heading against the buffer already mutated by edit[0]'s rename, so the heading is gone — atomic rollback).

**When:** About to harden a proposed "patch-model" ADR (resolve all anchors against the original, apply bottom-to-top) into a plan, before touching code.

**Expected (my ADR claim):** The fix requires *splitting* `perform_scoped_edit` (edit_markdown.rs:401) and `perform_section_edit_ext` (edit_markdown.rs:78) into a resolve(→span) phase and an apply(span) phase — framed as the "real refactor" cost — plus "define overlap detection" (hand-waved).

**Got (scouted reality):**
- The resolve/apply split **already exists.** Both functions delegate anchor resolution to a shared `resolve_section_range` (`src/tools/file_summary/file_summary.rs:227`), which returns `SectionRange { heading_line, body_start_line, end_line, heading_text, level }` (1-indexed, inclusive) — exactly the precise span bottom-to-top application needs. The apply half is `compute_section_end` + splice, already isolated after the `range` binding.
- The genuinely hard part is the opposite of what I hand-waved: **conflict granularity.** The target failure case (rename heading + edit a row) targets the *same* `SectionRange`. A naive "reject overlapping section ranges" rule would reject precisely the batch we want to support. The two edits touch disjoint *lines* within one section (heading line vs a body row). `action="edit"` rewrites only the `old_string` span; `action="replace"` rewrites the whole body. Order-independence + conflict-rejection must operate on each edit's **actual write-span**, not on the coarse section range.

**Probable cause:** I asserted the shape of `edit_markdown`'s internals (and the location of the difficulty) from the batch-loop symptom without reading `perform_scoped_edit` / `perform_section_edit_ext` / `resolve_section_range` this session. The plumbing looked hard and the semantics looked easy; the scout inverted both.

**Workaround:** Re-scope the design before writing the plan: (1) plumbing is cheap — reuse `resolve_section_range` against the original `file_content`; (2) the load-bearing design decision is computing per-edit write-spans (sub-section for `edit`, whole-body for `replace`), detecting overlap on *those*, and applying bottom-to-top by descending write-span start. The cheap fallback (A3: pre-scan + precise "reorder or split" error) still covers the irreducible true-conflict case.

**Severity:** med — no failed tool call, but the un-scouted ADR would have (a) inflated the cost estimate onto the wrong component and (b) baked in a section-granularity overlap rule that rejects the exact use case the fix exists to enable. That's a correctness trap in the design, caught before any plan or code.

**Status:** fixed-verified — corrected design (snapshot-resolution + write-span model) shipped across Tasks 3-5; live batch verify passed.

**Fix idea / Pointer:** Design in progress this session; no bug file yet (this is a feature/refactor, tracked here). Candidate ADR: resolve-against-original + write-span conflict model for `edit_markdown` batch mode.

---
## W-1 — Scouting the apply functions before speccing their refactor inverted the design

**Observed:** 2026-07-17, brainstorming the `edit_markdown` batch-ordering fix, immediately before writing the design spec.

**Pattern:** Before writing a spec that proposes refactoring functions X and Y ("split them into resolve+apply"), read X and Y's actual bodies this session — `symbols(name=..., include_body=true)` — and follow their delegations. Don't assert the *shape* or the *location of the difficulty* from the calling-site symptom alone. (Instance of R-19 / the "assert a checkable fact ⇒ read it first" rule.)

**Counterfactual:** Without the scout, the spec would have (1) named the resolve/apply split as the "real refactor" cost — but that split *already exists* via the shared `resolve_section_range` (`file_summary.rs:227`), so the plumbing is cheap; and (2) defined overlap detection at **section-range** granularity — which would have *rejected the exact batch the fix exists to enable* (rename heading + edit a body row target the same section but disjoint lines). Two inverted load-bearing decisions, both baked into a spec, both feeding a `writing-plans` plan and subagent dispatch downstream. Estimated blast radius: a full spec + plan revision cycle after the first implementer discovered the overlap rule rejected the motivating test case.

**Confirming data points:**
1. F-1 (this session) — the resolve/apply split already existed; conflict granularity (not the split) was the hard part.
2. The scout further resolved the spec's one open question (write-span representation) to byte-offsets by reading all five action branches — a design decision that would otherwise have shipped as "medium confidence, TBD in plan."

**Impact:** med — saved a spec+plan revision cycle and a likely mid-implementation reversal; upgraded the design's confidence from medium to high before any code.

**Promote-when:** A second instance where pre-spec scouting of the functions-to-be-refactored inverts a stated cost or correctness assumption. At 2 datapoints, promote to CLAUDE.md as "Before speccing a refactor of functions X/Y, read their bodies and follow delegations this session — the cost and the hard part are routinely not where the calling-site symptom suggests."

**Status:** validated

---
## F-2 — `join_lines` boundary invariant leaks at `i == lines.len()`; insert_after-last-section needs a `"\n"`-prefix rule

**Observed:** 2026-07-17, Task 1 review during SDD execution of the batch-ordering plan.

**When:** Verifying the Task 1 implementer's flagged concern about the third test's loop bound before accepting the commit.

**Expected (plan):** The design invariant `content[..off.line_start(i)] == join_lines(&lines[..i])` holds for all `i`, so each action arm's byte-splice is a pure `content[..line_start(A)] + MID + content[line_start(B)..]` with no special cases.

**Got (verified reality):** The invariant holds only for `i < lines.len()`. At `i == lines.len()`, `join_lines(&lines[..i])` appends a spurious trailing `"\n"` (it equals `content + "\n"`, for content with or without a trailing newline). Only `insert_after` with `at="end-of-section"` on the **last** section reaches this (`insert_idx == compute_section_end fallback == lines.len()`). Legacy therefore emits a blank line before the appended text (`"## A\nbody\n"` + insert → `"## A\nbody\n\nX\n"`); the plan's naive zero-width splice at `content.len()` would have produced `"## A\nbody\nX\n"` — a silent behavior change.

**Probable cause:** The plan derived the per-arm spans from the common `(before, mid, after)` shape without checking the `join_lines` trailing-`"\n"` edge at the last line. The Task 1 implementer's test comment reinforced the wrong mental model (claimed all call sites use index `< lines.len()`).

**Workaround:** Patched plan Task 3 before dispatch: (1) span-table `insert_after` row now prefixes the replacement with `"\n"` iff `insert_idx == lines.len()`; (2) added note ‡ with the verified `join_lines(&lines[..lines.len()]) == content + "\n"` fact; (3) added `plan_section_edit_insert_after_last_section_matches_legacy` regression test (the pre-existing Task 3 test used a non-last section and would have missed it).

**Severity:** med — would have shipped a silent formatting regression on append-to-last-section, caught only if a test happened to cover that exact shape (none did). Fixed in-plan before the Task 3 implementer ran.

**Status:** fixed-verified — Task 3 shipped the EOF `"\n"` prefix + regression test (green); live verify passed.

**Fix idea / Pointer:** Plan Task 3 note ‡ + `plan_section_edit_insert_after_last_section_matches_legacy`. Task 1 test-comment cleanup tracked as a Minor in the SDD ledger.

---
## F-3 — Byte-splice refactor silently drops legacy newline-preservation guards (recurring)

**Observed:** 2026-07-17, Task 4 review (extract `plan_scoped_edit`) during SDD execution.

**When:** Reviewing the implementer's flagged deviation — it added a "Class-A fusion guard" not present in my Task 4 brief, to keep `scoped_edit_consuming_trailing_newline_preserves_following_heading` green.

**Expected (brief):** The scoped-edit byte-splice is a pure "find `old_string` in the section slice, emit a `PlannedEdit` for the matched byte range" — no newline fix-ups.

**Got (verified reality):** Legacy `perform_scoped_edit` did NOT splice the matched range — it rebuilt the whole section and ran `ensure_trailing_newline(&new_section)`. So when `old_string` consumed the section's trailing `\n` (the separator before the next heading) and `new_string` omitted it, legacy re-added the `\n`; the naive byte-splice would delete it and fuse the next heading onto the body. The implementer's guard (`if !new_string.ends_with('\n') && edits.last().span.end == sec_end { push('\n') }`) faithfully reproduces legacy — verified across all cases.

**Probable cause:** The refactor briefs derived spans from the `(before, mid, after)` shape but did not audit the newline-normalization each legacy arm applied on top (`join_lines`'s synthetic trailing `\n`, `ensure_trailing_newline`). This is the SAME root cause as F-2 (insert_after EOF `"\n"`-prefix). Two instances now → a pattern, not a one-off.

**Workaround:** Guard added in-code by the Task 4 implementer; accepted after byte-level verification against legacy. Suite 3146 green including the regression test.

**Severity:** med — would have shipped a heading-fusion regression on scoped edits that consume a section's trailing newline; caught by an existing regression test (not by the brief).

**Status:** fixed-verified — guard landed in commit cdb45612, verified faithful to legacy.

**Fix idea / Pointer:** Pattern lesson (promote-when a 3rd instance appears, or at branch close): **when refactoring a string-rebuild function to a byte-splice/PlannedEdit model, enumerate every newline-normalization the legacy applied (`ensure_trailing_newline`, `normalize_trailing_newline`, `join_lines`'s unconditional trailing `\n`) and reproduce each as an explicit span/replacement rule — the naive matched-range splice inherits none of them.** F-2 + F-3 are the two datapoints.

---
## F-4 — `apply_planned_edits` mis-orders coincident zero-width insert + non-zero span (silent corruption)

**Observed:** 2026-07-17, final whole-branch Opus review (85e3d72c..5c666bf1) of the batch-ordering feature.

**When:** Post-implementation review of the shared `apply_planned_edits` engine (Task 2 code), reading across all task seams.

**Expected:** Non-overlapping planned edits apply cleanly end-to-start regardless of array order (the feature's whole promise).

**Got:** `detect_overlaps` treats a zero-width insert at byte X and a non-zero span `[X, Y)` as non-conflicting (boundary-touching). But `apply_planned_edits` breaks equal-start ties by `order` (array position) alone. End-to-start correctness requires the non-zero span to `replace_range` BEFORE the coincident insert; if the insert applies first it shifts X, and the span then corrupts the buffer. Output flips with array order — reachable via `insert_before H + remove/replace H` and `insert_after A(end-of-section) + remove/replace B`. Verified real by controller trace: batch `[remove B, insert_before B]` deletes the freshly-inserted text; `[insert_before B, remove B]` is correct — different outputs. Untested.

**Probable cause:** The overlap model (Task 2) reasoned about spans intersecting but not about the *application-order* dependency between a boundary-touching insert and its neighbor span. A gap between `detect_overlaps` (what it permits) and `apply_planned_edits` (what it can safely apply in `order`-tie order).

**Workaround/Fix:** Sort refinement in `apply_planned_edits`: at equal start, non-zero spans before zero-width inserts (`.then(a_zero.cmp(&b_zero))`), then descending `order` among coincident inserts (preserves the existing coincident-insert test). Plus a both-orders regression test that fails pre-fix. Dispatched as a fix pass.

**Severity:** high — silent file corruption whose result depends on input order, i.e. the exact class the feature exists to eliminate; would have shipped (all per-task reviews + the full suite were green).

**Status:** fixed-verified — fix landed (commit dc6d24cb); regression test failed pre-fix / passed post-fix; full `cargo test` 3275 pass / 0 fail.

**Fix idea / Pointer:** `docs/superpowers/plans/...` C-1; spec §3.3 should be amended to state the equal-start application rule.

---

## W-2 — Opus final whole-branch review caught a Critical the per-task reviews structurally could not

**Observed:** 2026-07-17, after 5 clean per-task reviews (each spec-compliant, full suite green at every step).

**Pattern:** Budget a final whole-branch review on the most capable model (Opus) for shared, guard-bearing infrastructure — even when every per-task gate passed. Point it explicitly at cross-task seams and at the invariants the individual tasks each satisfied in isolation.

**Counterfactual:** C-1 (F-4) lives in the *interaction* between `detect_overlaps` (Task 2), the zero-width insert spans (Task 3), and `plan_batch` collection (Task 5). No single-task review sees that seam: Task 2's own tests never combine a real insert with a real coincident span, and each later task's diff looks locally correct. The full 3274-test suite was green. Without the whole-branch pass, an order-dependent silent-corruption bug — precisely the failure mode the feature was built to remove — ships to master. The controller's own byte-level per-task reviews (thorough as they were) also missed it, because they were task-scoped.

**Confirming data points:**
1. C-1 / F-4 (this session) — Critical found only at the whole-branch pass, verified real by controller trace.
2. Prior project precedent: the CLAUDE.md rule "budget an Opus pass on shared/parity-bearing infrastructure" (2026-07-07 EDU-Planner datapoint) — same lesson, different codebase.

**Impact:** high — prevented shipping order-dependent corruption in the exact feature meant to eliminate order-dependence.

**Promote-when:** Already codified in CLAUDE.md (Subagent Dispatch — Model Floor + Review Escalation). This is a second in-repo datapoint reinforcing it; no new promotion needed, but cite it if the rule is ever questioned.

**Status:** validated

---
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
