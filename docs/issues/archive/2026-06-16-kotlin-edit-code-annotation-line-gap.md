---
status: fixed
opened: 2026-06-16
closed: 2026-06-16
severity: high
owner: marius
related:
  - docs/issues/archive/2026-05-29-edit-code-kotlin-stale-lsp-range.md
  - docs/issues/2026-06-04-kotlin-ast-drops-nested-classes.md
tags:
  - edit_code
  - kotlin
  - ast
  - lsp
kind: bug
---

# BUG: edit_code insert/replace fails on Kotlin methods with 2+ annotation lines — AST/LSP start-line gap exceeds ±1 tolerance

## Summary
`edit_code(action="insert", position="after")` (and the same AST-end-line resolution
path used by replace) refuses to operate on Kotlin test methods that carry two or more
annotation lines (e.g. `@Test` + `@DisplayName`), reporting the misleading
*"cannot determine end of '…' for insert-after — AST parse failed"*. The file parses
fine; the real cause is a start-line disagreement between the AST and kotlin-lsp that
is larger than the matcher's ±1 line tolerance.

## Symptom (Effect)
On `backend-kotlin/.../RoomConstraintsTest.kt`, inserting a sibling test after
`RoomConstraintsTest/room conflict fires when two lessons share same room date and timeslot`:

```
{
  "ok": false,
  "error": "cannot determine end of 'room conflict fires when two lessons share same room date and timeslot' for insert-after — AST parse failed",
  "hint": "The file likely has syntax errors that broke tree-sitter's parse, or the symbol has duplicate-name siblings without a clear name_path. Fix the syntax errors first, or use edit_file with explicit context."
}
```

`edit_file` is no fallback — its structural guard refuses any edit containing `fun `.
Both write paths are blocked; the user worked around it by creating a whole separate
test file (`RoomConflictSpanOverlapTest.kt`).

The hint is wrong on both counts: the file has no syntax errors (tree-sitter parses it
cleanly — `symbols()` lists every method), and there are no duplicate-name siblings.

## Reproduction
- Commit: `eca9902ec3f37fd06578bb3ed998c1ea9af16173` (experiments, pre-fix).
- File: `ktor-server/src/test/kotlin/edu/planner/solver/constraints/stage2/hard/RoomConstraintsTest.kt`,
  clean (committed) state.
- Call: `edit_code(path=…, symbol="RoomConstraintsTest/room conflict fires when two lessons share same room date and timeslot", action="insert", position="after", body=…)`.
- Result: the error above, deterministically.

Reduced (no LSP needed), exercising the matcher directly:
```
let source = "class MyTest {\n    @Test\n    @DisplayName(\"x\")\n    fun `room conflict fires`() {\n        val a = 1\n    }\n}\n";
let ast = extract_symbols_from_source(source, Some("kotlin"), …);
// AST start_line = 1 (the @Test line); kotlin-lsp reports start = 3 (the `fun` line).
find_ast_end_line_in(&ast, "room conflict fires", 3, Some("MyTest/room conflict fires")); // pre-fix: None
```

## Environment
- codescout 0.15.0, branch `experiments`.
- Target project: backend-kotlin, kotlin-lsp running (it supplied backtick-stripped names + `fun`-line starts).
- MCP transport: stdio (Claude Code).

## Root cause
`do_insert` "after" calls `editing_end_line_strict` → `ast_confirmed_end_line`
(`src/symbol/edit.rs`) → `find_ast_end_line_in` (`src/symbol/query.rs`). The old
`find_ast_end_line_in` collected candidate AST symbols through `collect_ast_candidates`,
which gated on **both** name match **and** `sym.start_line.abs_diff(lsp_start) <= 1`
*before* the `name_path` tiebreaker ran.

The two coordinate systems disagree on a symbol's **start** line:
- The tree-sitter Kotlin function node **spans from its first annotation** — for this
  method the `@Test` line. AST `start_line = 214` (0-based).
- kotlin-lsp reports the **declaration keyword** line — the `fun` line. LSP
  `start_line = 216`.

The gap equals the number of annotation lines (here 2: `@Test`, `@DisplayName`), so
`abs_diff(214, 216) = 2 > 1`. The candidate set comes back **empty**, so
`find_ast_end_line_in` returns `None` before the (already backtick-tolerant)
`name_path` matcher is consulted → strict end-line is `None` → universal refuse with
the "AST parse failed" message.

This is why the failure is *selective*: a method with a single annotation line has a
1-line gap (within ±1, edit succeeds); 2+ annotation lines push it out of tolerance.
The 2026-05-29 backtick-name-normalization fix was necessary but not sufficient — the
line gate is the residual defect.

Ground truth (throwaway test dumping the real file's AST, 2026-06-16):
```
name=`room conflict fires when two lessons share same room date and timeslot` start=214 end=266
find_ast_end_line_in(lsp_start=215) -> Some(266)   # within ±1 of AST 214
find_ast_end_line_in(lsp_start=216) -> None         # the real LSP value, gap 2
find_ast_end_line_in(lsp_start=217) -> None
```

## Evidence
### AST-vs-LSP start lines (throwaway unit test on the real file)
`symbols()` (LSP-backed) listed the method at display line 217 (`fun` line, backticks
stripped). The AST extractor placed it at 0-based 214 (the `@Test` line, backticks
kept). Replaying `find_ast_end_line_in` with the LSP value (216) returned `None`; with
215 (within ±1 of the AST 214) it returned `Some(266)`. This isolates the ±1 line gate
as the sole failure point — name matching and AST parsing both succeed.

## Hypotheses tried
1. **Backtick name mismatch (the 2026-05-29 cause) regressed.**
   Test: read `names_match_ignoring_backticks` + `collect_ast_candidates`.
   Verdict: rejected — name matching is already backtick-tolerant.
2. **File has syntax errors / AST parse genuinely fails.**
   Test: `symbols()` lists all methods; throwaway extractor call succeeds.
   Verdict: rejected — AST parses cleanly, symbol present with correct end line.
3. **Start-line gap from leading annotations exceeds the ±1 tolerance.**
   Test: dumped AST start (214) vs LSP start (216); replayed matcher at 215/216/217.
   Verdict: **confirmed** — `Some` at 215, `None` at 216/217.

## Fix
Reworked `find_ast_end_line_in` (`src/symbol/query.rs`) to key on `name_path` **first,
without any line gate**: collect all same-name symbols anywhere in the tree
(`collect_by_name`, replacing the line-gated `collect_ast_candidates`), and when the
caller supplies a `name_path` (it always does in production — `src/symbol/edit.rs`
passes `Some(&sym.name_path)`), a unique backtick-/suffix-tolerant `name_path` match
wins regardless of line distance. Line proximity is retained only as the *fallback*
disambiguator for same-name siblings when `name_path` is absent or ambiguous.

`name_path` encodes the full parent chain, so it is unique and is the correct
authoritative key; line proximity was a pre-`name_path` heuristic that mis-fired
whenever the two coordinate systems disagreed on start line by more than a line.

Change lives in `src/symbol/query.rs` (`find_ast_end_line_in`, `collect_by_name`).
Master-side SHA: *pending cherry-pick* (experiments-side commit to be recorded after
ship per CLAUDE.md § "After cherry-pick").

## Tests added
- `src/symbol/query.rs` `backtick_match_tests::find_ast_end_line_in_bridges_annotation_line_gap`
  — inline Kotlin fixture with `@Test` + `@DisplayName` (2-line gap), asserts the AST
  start is the annotation line and that `find_ast_end_line_in` resolves the unique
  `name_path` to the right end line despite the gap.
- Existing regressions still green:
  `find_ast_end_line_in_resolves_kotlin_lsp_name_without_backticks`,
  `find_ast_end_line_in_resolves_nested_kotlin_symbols`,
  `find_ast_end_line_in_resolves_ts_namespace_nested_symbol`.
- Full `cargo test --lib`: 2790 passed, 6 ignored. `cargo clippy --all-targets -D warnings`: clean.

## Workarounds
- Insert into a *separate* test file (what the user did), or
- Use `edit_file` with explicit context **only for non-structural** edits — it refuses
  any body containing `fun `, so it cannot add a new test method.
- Pre-fix only: target a sibling whose annotation count is ≤1 and adjust manually.

## Resume
N/A — fixed and **live-verified 2026-06-19**: after `cargo rb` + `/mcp` reconnect, the
exact failing call (`edit_code` insert-after on `RoomConstraintsTest/room conflict fires
when two lessons share same room date and timeslot`) returned `ok:true` (inserted at
line 268); probe reverted, file clean. The end-to-end LSP+AST chain is confirmed.
Remaining: ship to master via the Standard Ship Sequence and record the master-side SHA
in the Fix section.
### 2026-06-23 follow-up — convention now obsolete; regression pinned

A backend-kotlin session (2026-06-23) again created a *separate* `RoomAvailableSpanOverlapTest.kt`
rather than splice a method into `RoomAvailableConstraintTest`, citing that "the in-place edit
tooling could not splice a method into that class's backtick-named test block" as the codebase's
"established convention for SI-5-class span regressions (also `SchedulingPolicyConstraintsSpanTest`)".

Investigated: this is a **stale belief**, not a live limitation. Evidence (all from
`mirela/backend-kotlin/.codescout/usage.db`):

- The running MCP binary upgraded from the **pre-fix** `eca9902e` (all the 06-15…06-21
  "AST parse failed" / "symbol not found" rows) to `46f48231` (06-22 onward).
- Row **26319** (2026-06-22, sha `46f48231`): `edit_code` `action=insert` `position=after` with
  `symbol = BoxedBlockAnalyzerTest/\`analyze unpacks the solution and sources availability…\``
  spliced **two** new `@Test fun \`…\`()` methods into a backtick-named test block → **ok**. That
  is exactly the operation the convention claims is impossible.
- **No** `edit_code` errors are logged after 2026-06-21; the 06-23 session never actually retried a
  splice — it followed the convention pre-emptively.

`RoomAvailableConstraintTest` is structurally identical to `BoxedBlockAnalyzerTest` (top-level class,
single `@Test` per method, backtick names). Its shape — resolve + insert-after the **last** backtick
method — is now pinned deterministically by
`src/symbol/query.rs::backtick_match_tests::backtick_test_class_resolves_for_insert_after_last_method`,
which exercises **both** insert gates: `find_unique_symbol_by_name_path` (resolution, backtick and
no-backtick query forms — the "symbol not found" gate, previously untested at this layer) and
`find_ast_end_line_in` (the "AST parse failed" end-line gate).

Action item is **downstream**, not in codescout: retire the separate-`*SpanTest.kt` convention from
the backend-kotlin project docs so sessions stop creating one-off files for a bug that no longer
bites. (The still-open contributor noted in the 2026-06-04 file — softening `do_insert`'s misleading
"AST parse failed / syntax errors" hint — is what seeded this self-perpetuating convention in the
first place.)
## References
- `src/symbol/query.rs` — `find_ast_end_line_in`, `collect_by_name`, `names_match_ignoring_backticks`
- `src/symbol/edit.rs` — `editing_end_line_strict`, `ast_confirmed_end_line`
- `src/tools/symbol/edit_code.rs` — `do_insert` (the "after" refuse path)
- docs/issues/archive/2026-05-29-edit-code-kotlin-stale-lsp-range.md (backtick normalization, prior layer)
- docs/issues/2026-06-04-kotlin-ast-drops-nested-classes.md (sibling Kotlin AST issue)
