# Session Log — Bug-Fix Work Stream

> **Scope:** Multi-session work to close the OPEN bugs in `docs/issues/`.
> Started 2026-05-17 when the controller began scouting the 4 open buffer/markdown
> bugs filed 2026-05-09. F-N entries capture drift between bug-file Resume hints
> and current code reality; W-N entries capture practices that prevented wasted
> fix work.


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
| F-1 | 2026-05-17 | low | plan-prose | fixed-verified | Bug-file Resume paths cite non-existent layout |
| F-2 | 2026-05-17 | med | self-friction | fixed-verified | 2 of 3 buffer bugs likely stale — code reads correct |
| F-3 | 2026-05-18 | med | plan-prose | fixed-verified | Plan test assertions cited non-existent `RecoverableError.hint` field |
## Wins Index

| ID | Date | Impact | Pattern | Counterfactual | Status |
|----|------|-------:|---------|----------------|--------|
| W-1 | 2026-05-17 | med | Scout helper-fn bodies before fixing reported bugs | Would have written instrumentation / "fix" for `extract_lines` and `extract_json_path` despite both being correct + having passing tests | promoted-to-permanent-docs |
| W-2 | 2026-05-18 | med | Pre-dispatch recon scouts type accessors named in plan assertions | Task 2's first subagent would have failed `cargo check` on `err.hint.as_deref()` (no such field); 1+ wasted round-trip per test, controller drift mid-dispatch | validated |
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

## F-1 — Bug-file Resume paths cite non-existent layout

**Observed:** 2026-05-17, scouting `docs/issues/2026-05-09-*.md` to start fix work.

**When:** First `symbols(path)` calls after reading the 4 OPEN bug files.

**Expected:** Bug-file Resume sections cited `src/tools/buffer/handlers.rs` and `src/tools/search/grep.rs` as the starting points.

**Got:** `path not found: src/tools/buffer/` and `path not found: src/tools/search/grep.rs`. Real layout: `src/tools/output_buffer.rs`, `src/tools/read_file.rs` (with `read_from_buffer` helper inside), `src/tools/grep.rs` (top-level — no `search/` subdir).

**Probable cause:** Module restructure between 2026-05-09 (when bugs filed) and 2026-05-17 (current session). Bug files are append-only and never refresh path hints when the layout shifts.

**Workaround:** Ran `tree(src/tools)` to discover the real layout, then proceeded from `read_from_buffer`, `Grep::call`, `extract_lines`, `extract_json_path`.

**Severity:** low

**Status:** fixed-verified

**Fix idea / Pointer:** Resume sections for the 3 closed bug files (`grep-buffer-false-negatives`, `read-file-buffer-midpoint-empty`, `read-file-json-path-array-elements`) were rewritten in the same session to cite real symbols (`Grep::call`, `extract_lines` at `src/util/text.rs:23-33`, `extract_json_path` at `src/tools/file_summary/file_summary.rs:419`). Bug #1 (`edit-markdown-insert-after-h1`) still cites `src/tools/markdown/edit_markdown.rs` which is a real path — not affected.

---

## F-2 — 2 of 3 buffer bugs likely stale; code reads correct

**Observed:** 2026-05-17, reading `extract_lines` + `extract_json_path` bodies before writing fixes.

**When:** Investigating root causes for `2026-05-09-read-file-buffer-midpoint-empty.md` and `2026-05-09-read-file-json-path-array-elements.md`.

**Expected:** Per bug-file hypotheses, expected to find chunk-walker not advancing (`#3`) and array-index json_path mishandled (`#4`).

**Got:**
- `extract_lines` (12 lines, `src/util/text.rs:21-33`) — trivially correct line-range filter with existing passing test `extract_lines_out_of_bounds_returns_empty`. No chunk-walker exists.
- `extract_json_path` (`src/tools/file_summary/file_summary.rs:419`) — handles array indexing + has passing test `extract_json_path_array_index` using `$.users[0]`. Comment in code explicitly cites `$.symbols[0].body` as a working case.

**Probable cause:** Either (a) intervening commit closed the bugs without crediting back the bug files, or (b) original reporter observed empty STRING value at the json path (correct return) and misread it as "broken." `Bug #2` (`grep-buffer-false-negatives`) IS real — `Grep::call` has no `@*` ref handling at all.

**Workaround:** Plan to write probe tests mirroring the exact bug-file reproductions. If they pass: close as `wontfix-false-alarm` + commit the test as a regression guard. If they fail: real defect surfaces.

**Severity:** med

**Status:** fixed-verified

**Fix idea / Pointer:** Probe tests confirmed prediction:
- Bug #2 `grep-buffer-false-negatives` FAILED probe → confirmed real → fixed via `grep_in_buffer` + `@`-prefix branch in `Grep::call`.
- Bug #3 `read-file-buffer-midpoint-empty` PASSED probe → closed `wontfix`, test kept as regression pin.
- Bug #4 `read-file-json-path-array-elements` PASSED probe → closed `wontfix`, test kept as regression pin.

---

## W-1 — Scout helper-fn bodies before "fixing" reported bugs

**Observed:** 2026-05-17, bug-fix work stream kickoff.

**Pattern:** Read the `extract_lines` and `extract_json_path` bodies before writing any failing test or any fix. Each was small (~12 / ~50 lines) and surfaced its own correctness immediately.

**Counterfactual:** Without reading the helper-fn bodies first, would have written either (a) instrumentation patches to "debug" non-existent chunk-walker bugs in `extract_lines`, or (b) a "fix" adding array indexing to `extract_json_path` that already exists and is tested. Both = wasted commits + churn on green code. Concrete evidence: bug-file hypothesis "Buffer is chunked internally and the chunk-walker may not advance past the first chunk" was confidently wrong — there is no chunk walker.

**Confirming data points:**
1. `extract_lines` body reveals no chunk walker exists — bug-file hypothesis #3 invalidated in one read.
2. `extract_json_path_array_index` test (passing, `$.users[0]`) invalidates bug-file hypothesis #4 in one read.
3. By contrast, scouting `Grep::call` confirmed bug #2 is real — same scouting move found one true defect among three claimed defects.
4. Probe tests for #3 and #4 passed immediately, proving the prediction; probe test for #2 failed with an explicit panic naming the failure mode (`relative path '@tool_*' requires an active project`), letting the fix shape itself.

**Impact:** med

**Promote-when:** This is a specific instance of the existing reconnaissance discipline. Already covered by `codescout-companion:reconnaissance` skill. No promotion needed — count this as a confirming data point for the skill, not a new rule.

**Status:** validated

---
## F-3 — Plan test assertions cited non-existent `RecoverableError.hint` field

**When:** Pre-dispatch reconnaissance for the jsonpath negative-slice
implementation plan (`docs/superpowers/plans/2026-05-18-jsonpath-negative-slice.md`).
About to dispatch Task 1 subagent.

**Expected (plan):** `RecoverableError` has accessible `.hint: Option<String>`
field; plan tests used `err.hint.as_deref().unwrap_or("")`.

**Got (scouted reality):** `RecoverableError` at `src/tools/core/types.rs:169`
exposes `pub message: String` and `pub guidance: Option<Guidance>` — there is
NO `.hint` field. There IS a method `.hint() -> Option<&str>` that returns the
text only for the `Guidance::Hint` variant. The `Display` impl's own comment
explicitly recommends `to_string().contains(...)` for test assertions because
it renders `"{message} — Hint: {text}"` and is the documented stable contract:

> "Display renders only `message`. The structured `hint` and `recovery_steps`
> are intentionally omitted here so existing `to_string().contains(...)` test
> assertions stay stable."

Wait — re-reading: Display renders `message` PLUS `" — {field_name}: {text}"`
when guidance is present. The contract is: `to_string().contains(hint_text)`
holds. So tests can use `err.to_string().contains("...")` regardless of which
guidance variant is attached.

**Probable cause:** Plan was written from the design spec; spec didn't pin
the assertion-side accessor shape; writing-plans phase didn't scout
`RecoverableError`. Standard "scout helper-fn bodies" rule (W-1 in this same
session log) applies to type shapes too.

**Workaround:** Edit the plan's Task 2 + Task 3 test code to use
`err.to_string().contains("...")` everywhere a hint-text or message-text
assertion is made. Drops the `.hint` field reference. Less brittle than
`err.message.contains(...)` because it also covers cases where the failing
substring lives in the guidance text rather than the message text.

**Severity:** med — would have caused first subagent's tests to fail to
compile; controller would then absorb the failed-task drift mid-dispatch.

**Status:** open → fixed-verified after plan edit lands this turn.
## W-2 — Pre-dispatch recon caught test-shape error before any subagent ran

**When:** About to dispatch Task 1 of the jsonpath negative-slice plan to a
fresh subagent (subagent-driven-development mode).

**Pattern:** Before the first subagent dispatch on a plan that names *types*
in test assertions (not just *fns*), invoke the reconnaissance skill and
scout each referenced type's actual field/method shape. Specifically:
`symbols(name=<TypeName>, include_body=true)` for any type whose accessors
the plan tests mention.

**Counterfactual:** Without this scout, Task 2's first subagent would have
written `err.hint.as_deref().unwrap_or("")` and failed `cargo check` on the
first parse test. The subagent would have flailed (probable retries with
`err.guidance`, or `err.hint().unwrap_or("")`, or `err.to_string()`) without
the Display-impl contract context. Best case: 1 extra round-trip per failing
test (~11 round-trips for the 11 parser tests in Task 2). Worst case: the
subagent gives up, controller re-scopes plan mid-dispatch, F-N entry written
*after* drift instead of *before*.

**Confirming data points:**
1. F-3 (this session) — `RecoverableError.hint` field cited by plan does not
   exist; scout caught it pre-dispatch.
2. Pending: any future plan that names types in assertions.

**Impact:** med — saves ≥1 failed subagent task and prevents controller
context absorption of cascading test failures. The saving compounds
across the 4 implementation tasks in the plan.

**Promote-when:** A second pre-dispatch recon catches a similarly hidden
type-shape mismatch (any plan, any type). At 2 datapoints, promote to
CLAUDE.md's "Before dispatching the first subagent of an implementation
plan, scout every type whose accessors the plan asserts on" rule.

**Status:** validated — single datapoint, drift caught + fixed in the same
turn before any subagent dispatch. Awaiting promotion criterion.
## Template for new entries

<!-- Insert new F-N / W-N entries above this line via:
     edit_markdown(action="insert_before",
                   heading="## Template for new entries",
                   content="## F-N — title\n...")
     Also update the matching Index / Wins Index table row at the top. -->
