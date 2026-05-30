---
status: fixed
opened: 2026-05-29
closed: 2026-05-30
severity: medium
owner: marius
related:
  - docs/issues/archive/2026-05-18-edit-code-replace-misses-outer-attrs.md
  - docs/issues/archive/2026-05-15-edit-code-replace-strips-doc.md
  - docs/issues/archive/2026-03-20-replace-symbol-mod-tests-eats-adjacent.md
  - docs/issues/archive/2026-04-24-find-symbol-kotlin-multi-session.md
tags:
  - edit_code
  - lsp
  - kotlin
  - tree-sitter
kind: bug
---

# BUG: edit_code unusable on a class of Kotlin symbols — stale/overshoot LSP ranges and AST-parse failures block legit edits (guard restores file, but edit can't complete)

## Summary
On Kotlin source (observed in `backend-kotlin` and its `weekly-pattern` worktree),
`edit_code` repeatedly fails to complete a structural edit because the symbol
range it works from is wrong: the LSP returns a stale or selection-only range
that overshoots into adjacent code, or tree-sitter cannot determine the symbol's
end for `insert-after`. The sibling-drop guard (added by the 2026-05 edit_code
fixes) correctly detects the overshoot and restores the file — so there is **no
corruption** — but the edit never lands, forcing an `edit_file` fallback. This
makes `edit_code` effectively unusable for backtick-named Kotlin test functions
and for large symbols whose LSP range is reported short.

## Symptom (Effect)
Field-observed via `.codescout/usage.db` `tool_calls` (window 2026-05-27→29).
Three distinct failure strings, all on Kotlin (one Python variant of #1):

**(1) Stale/overshoot range — replace refused (sibling-drop guard fires):**
```
edit_code replace('no penalty when subject requires facilities') would have dropped sibling symbols:
HomeRoomConstraintTest/createSolverFactory, HomeRoomConstraintTest/`no penalty when weight is zero default config`,
... [15 HomeRoomConstraintTest siblings] ...
The edit range overshot into adjacent code (likely a stale LSP range). File restored.
— hint: Try symbols(path) to refresh, then retry; or narrow the edit via edit_file with unique anchors.
```
Also the single-sibling and Python forms:
```
edit_code replace('HomeRoomConstraintTest/`no penalty when subject requires facilities`') would have dropped sibling symbols: HomeRoomConstraintTest/`no penalty when subject requires facilities`. The edit range overshot into adjacent code (likely a stale LSP range). File restored.
edit_code replace('TestVerdict') would have dropped sibling symbols: TestVerdict/test_below_threshold_is_no_signal. The edit range overshot into adjacent code (likely a stale LSP range). File restored.
```

**(2) LSP returned a selection range, not the full symbol range:**
```
LSP returned suspicious range for 'FeasibilityValidationService' (lines 64-77, but AST shows it spans to line 958)
— hint: The LSP server may have returned a selection range instead of the full symbol range. Try edit_file for this symbol, or check symbols(path) to verify the range.
```

**(3) tree-sitter can't find the symbol end for insert-after (backtick-named Kotlin tests):**
```
cannot determine end of 'cancelOperation - cancels active solve and returns true' for insert-after — AST parse failed
cannot determine end of 'convert in ROOM_PER_SG mode value-range must contain facility-matching rooms when subject requires a facility absent from preferred room' for insert-after — AST parse failed
— hint: The file likely has syntax errors that broke tree-sitter's parse, or the symbol has duplicate-name siblings without a clear name_path. Fix the syntax errors first, or use edit_file with explicit context.
```

**Out of scope (working-as-intended — NOT this bug):** `edit_code replace('_exact_hit') dropped the symbol definition — body must be the complete declaration` and the `FeasibilityValidationService` *restore* path are the guard doing its job on a model-side body-only edit. Counted here only to keep the distinction explicit.

## Reproduction
Not yet reproduced in codescout's own test suite. Best lead: the `HomeRoomConstraintTest`
file in `backend-kotlin` (Kotlin test class with many backtick-named sibling methods like
`` `no penalty when subject requires facilities` ``). Steps to attempt:
1. Open a Kotlin file with a class containing ≥10 backtick-named test methods.
2. `edit_code(symbol="ClassName/\`backtick method\`", action="replace", body=...)`.
3. Observe either the overshoot/sibling-drop refusal (#1) or, for `insert`, the AST-parse failure (#3).
For (#2): pick a large Kotlin class/service (~900 lines) and `edit_code(symbol=..., action="replace")`; observe the suspicious-range rejection if the LSP returns the name selection range instead of the full body range.

A minimal Kotlin fixture under `tests/fixtures/` with backtick-named methods is the
likely path to a deterministic repro.

## Environment
- OS: Linux (Arch, kernel 7.0.9)
- Language: Kotlin (`kotlin-lsp`); one Python variant of failure #1
- Projects: `mirela/backend-kotlin` and `…/backend-kotlin/.worktrees/weekly-pattern`
- codescout: live release binary; window 2026-05-27→29
- Related instability in-window: `Failed to start LSP server: kotlin-lsp` ×3 and `LSP server is not running` ×1 — kotlin-lsp cold-start/crash may be feeding stale ranges into the edit path (see archived `2026-04-24-find-symbol-kotlin-multi-session.md`).

## Root cause

**Confirmed 2026-05-29 by code trace (debugging-yeti session).** What looked like
three separate defects is **one root cause surfacing in two exact-equality name
matchers, plus two unrelated kotlin-lsp range bugs that the guards handle correctly.**

### The real codescout defect — Kotlin backtick name mismatch (signatures 3 and 1b)
Kotlin backtick identifiers are stored **with** backticks by the AST extractor
(`` `no penalty …` ``) — this is asserted as "AST truth" by the **passing** test
`kotlin_backtick_function_names` (`src/ast/parser.rs:2242`; names come from
`child_name` in `extract_kotlin_symbols`, `src/ast/parser.rs:935`). kotlin-lsp
reports the same symbols **without** backticks (`no penalty …`) — confirmed by the
field error strings, which print `sym.name` (LSP-sourced) with no backticks.

Two matchers compare LSP name to AST name with plain `==`:
- `collect_ast_candidates` (`src/symbol/query.rs:401`): `sym.name == name && start_line.abs_diff(lsp_start) <= 1`
- `find_ast_name_path` (`src/symbol/edit.rs:248`): `s.name == lsp_name && …`

`` `no penalty …` `` ≠ `no penalty …`, so both return no match → `None`:
- **Signature (3)** — `do_insert` position="after" (`src/tools/symbol/edit_code.rs:694`)
  calls `editing_end_line_strict` → `ast_confirmed_end_line` (`src/symbol/edit.rs:159`)
  → `find_ast_end_line_in` → `collect_ast_candidates`. `None` ⇒ universal refuse
  with the message **"cannot determine end of '…' — AST parse failed"**, whose hint
  blames *syntax errors*. **The hint is misleading** — the Kotlin files compile and
  tree-sitter parses them (the test proves the extractor handles backticks). This is
  a name-normalization mismatch, **not** a broken parse, and **not** a grammar gap.
- **Signature (1b)** — `do_replace` (`src/tools/symbol/edit_code.rs:646-672`) resolves
  `target_ast_name_path` via `find_ast_name_path`. For a backtick target this is `None`,
  so the dropped-set filter (`target_ast_name_path.as_deref() != Some(np)`) can never
  exclude the target → the guard misreports, in the field case naming the **target
  itself** as a "dropped sibling".

### Not codescout defects — kotlin-lsp bad ranges caught by deliberate guards (signatures 2 and 1a)
- **Signature (2)** — `validate_symbol_range` (`src/symbol/query.rs:170`, called pre-edit)
  bails when the AST end exceeds the LSP `end_line` ("suspicious range 64-77 vs 958").
  It **skips when the file has syntax errors** (`query.rs:181`), so the file parsed
  clean — kotlin-lsp simply returned a **selection range** (name only) for a ~900-line
  symbol. This guard is the deliberate output of the `augment_body_range_from_ast →
  validate_symbol_range` redesign (`docs/superpowers/plans/2026-03-02-symbol-range-redesign-*.md`):
  the team consciously chose **"trust LSP, validate, fail loudly"** over auto-repairing
  from AST. **The original "Fix idea: prefer AST span over LSP" would re-revert that
  decision — do not pursue it.**
- **Signature (1a)** — `do_replace` post-write AST set-diff (`edit_code.rs:660`) catches a
  replace range that overshot into *other* siblings (kotlin-lsp `end_line` too large;
  the parent-clamp only bounds to the enclosing class, not the next sibling). Guard
  restores the file — no corruption. Upstream cause is the kotlin-lsp range, plausibly
  worsened by kotlin-lsp cold-start/crash instability (3× "Failed to start LSP server:
  kotlin-lsp" in the same usage window, and it won't start at all in codescout's own dev env).
## Evidence
### usage.db (backend-kotlin), 2026-05-27→29
Error-breakdown query (last 2 days) surfaced failures (1) and (3) plus the kotlin-lsp
start failures. Source: `/home/marius/work/mirela/backend-kotlin/.codescout/usage.db`,
`tool_calls WHERE outcome='error' AND called_at >= date('now','-2 days')`.

### usage.db (weekly-pattern worktree), 2026-05-27→29
Failure (2) — the `FeasibilityValidationService` suspicious-range (64-77 vs 958) — and 3×
`Failed to start LSP server: kotlin-lsp`. Source:
`/home/marius/work/mirela/backend-kotlin/.worktrees/weekly-pattern/.codescout/usage.db`.

Full cross-project breakdown: `docs/usage-reports/2026-05-29-usage-analysis.md` (friction #5).

### Code trace (debugging-yeti, 2026-05-29) — the two boundary values
- **AST side:** `kotlin_backtick_function_names` (`src/ast/parser.rs:2242`) is a **passing** test asserting backtick functions are indexed as `` `is carried as a constructor value` `` and name_path `` MyTest/`is carried as a constructor value` `` — comment: *"Backtick names must be indexed with backticks preserved (AST truth)"*.
- **LSP side:** field error `cannot determine end of 'cancelOperation - cancels active solve and returns true' for insert-after` prints `sym.name` (from `fetch_validated_symbol`, LSP `documentSymbol`) with **no** backticks.
- **Comparator:** `collect_ast_candidates` (`src/symbol/query.rs:401`) and `find_ast_name_path` (`src/symbol/edit.rs:248`) both use `==`. Mismatch ⇒ `None` ⇒ refuse/misreport.
- **Env note:** `symbols` (LSP path) on a Kotlin fixture fails here with `Failed to start LSP server: kotlin-lsp` — codescout's dev env has no kotlin-lsp, so these only surface in `backend-kotlin`/`mirela`. The AST path (`extract_symbols_from_source`) needs no LSP and is unit-testable in CI.

## Hypotheses tried
1. **Hypothesis:** Same as the archived edit_code corruption bugs (strips-doc / misses-outer-attrs / mod-tests-eats-adjacent). **Test:** Compared error strings. **Verdict:** rejected — those were *corruption* bugs; these say "File restored", i.e. the guard those fixes installed now works. **Evidence:** archived files in `related:`.
2. **Hypothesis:** The "dropped the symbol definition" messages are part of this bug. **Test:** Read the message semantics. **Verdict:** rejected — that path is the guard catching a model-supplied body-only edit; working as intended.
3. **Hypothesis (signature 3):** "AST parse failed" means the Kotlin file has syntax errors (as the error hint and archived `2026-05-02:86` closure assert). **Test:** Traced `do_insert` → `editing_end_line_strict` → `ast_confirmed_end_line` → `find_ast_end_line_in` → `collect_ast_candidates`; read the matcher; checked the AST extractor's backtick handling. **Verdict:** **rejected** — the files compile and tree-sitter extracts backtick functions fine (`kotlin_backtick_function_names` passes). The `None` comes from a name mismatch, not a parse failure. **Evidence:** `src/ast/parser.rs:2242` (AST keeps backticks) vs field `sym.name` (LSP strips them) vs `==` in `src/symbol/query.rs:401`.
4. **Hypothesis (signature 3):** tree-sitter-kotlin grammar doesn't extract backtick-named functions at all (grammar gap). **Test:** Read `extract_kotlin_symbols` (`parser.rs:874`) and the passing `kotlin_backtick_function_names` test. **Verdict:** **rejected** — they ARE extracted, with backticks preserved. The defect is the *comparison* (`==`), not the extraction. **Confirmed root cause: backtick name-normalization mismatch (see Root cause).**
5. **Hypothesis (signatures 2/1a):** edit_code should prefer the AST span over the LSP range. **Test:** Read `validate_symbol_range` doc comment + the `2026-03-02-symbol-range-redesign` plans. **Verdict:** **rejected as a fix direction** — the team deliberately moved from auto-augmenting-from-AST to validating-and-failing-loudly. The bad range is kotlin-lsp's; edit_code's refusal is correct.
## Fix
**Signatures (3) + (1b) — FIXED on `experiments` (experiments-side `342a7be1`, not yet shipped to master).**
Root cause was a *partial-fix* regression: `symbol_name_matches` (`src/symbol/query.rs:522`)
already normalized kotlin-lsp's backtick-stripping, but the AST-end / sibling-guard matchers
never got the same treatment. Fix mirrors that normalization:
- Added `names_match_ignoring_backticks(a, b)` (`src/symbol/query.rs`, `pub(crate)`) — strips
  backticks and retries, only allocating when a backtick is present.
- Applied at the three lagging matchers:
  - `collect_ast_candidates` (`src/symbol/query.rs`)
  - `find_ast_end_line_in` name_path branch (`src/symbol/query.rs`)
  - `find_ast_name_path` (`src/symbol/edit.rs`)
- Deliberately **left `find_matching_symbol` (`query.rs:505`) untouched** — its caller
  `resolve_range_via_document_symbols` compares LSP-against-LSP (both backtick-stripped), so
  `==` is correct there; normalizing would be needless.

The misleading "AST parse failed → syntax errors" hint on `do_insert` (`edit_code.rs:701`) is
**not yet reworded** — left as a follow-up; the structural cause is now fixed so the message
fires far less often.

**Signatures (2) + (1a) — kotlin-lsp range bugs, not codescout.** No change (guards are
correct and deliberate). Workaround stands: `edit_file` with unique anchors. kotlin-lsp
cold-start/crash instability tracked under
`docs/issues/archive/2026-04-24-find-symbol-kotlin-multi-session.md`.

When this ships to master, cite the **master-side** SHA here (after cherry-pick).
## Tests added
`find_ast_end_line_in_resolves_kotlin_lsp_name_without_backticks` — in the new
`backtick_match_tests` module at the end of `src/symbol/query.rs`. Drives
`find_ast_end_line_in` with an LSP-style name (no backticks) against AST symbols
extracted by `extract_symbols_from_source` (which preserves backticks). Asserts it
resolves to `Some(end_line)`. **Verified red before the fix** (`got None, right: Some(3)`)
**and green after** — a true regression test, not a happy-path confirmation. Needs no live
kotlin-lsp (drives the matcher directly), so it runs in CI.

Pairs with the pre-existing `kotlin_backtick_function_names` (`src/ast/parser.rs:2242`),
which pins the AST-keeps-backticks invariant this fix depends on.
## Workarounds
- For backtick-named Kotlin methods and large Kotlin classes, fall back to `edit_file` with
  unique anchor strings (the error hint already says this) — no corruption risk, since the
  guard restores on overshoot.
- Run `symbols(path)` first to refresh the range, then retry `edit_code` (sometimes clears a
  stale range).
- If kotlin-lsp is crash-looping (`Failed to start LSP server: kotlin-lsp`), the stale ranges
  worsen — restart the session / `/mcp` reconnect before editing.

## Resume
Root cause for signatures (3)+(1b) is confirmed (backtick name mismatch). Next concrete action:
1. Add a shared `names_match(lsp, ast)` helper that strips one pair of surrounding backticks before comparing; use it in `collect_ast_candidates` (`src/symbol/query.rs:401`) and `find_ast_name_path` (`src/symbol/edit.rs:248`).
2. Regression test: extend the AST/edit tests with a Kotlin backtick symbol where the LSP-style name (no backticks) must resolve to the AST symbol (backticks) — assert `find_ast_end_line_in` returns `Some` and `do_insert` "after" succeeds. No live kotlin-lsp needed if the test drives the matcher directly with a synthesized `SymbolInfo` (LSP name without backticks).
3. Fix the `do_insert` "AST parse failed" hint (`src/tools/symbol/edit_code.rs:701`) to stop asserting "syntax errors".
4. Leave signatures (2)/(1a) alone — kotlin-lsp range bugs; guards correct.
## References
- `docs/usage-reports/2026-05-29-usage-analysis.md` — friction #5 (Kotlin LSP range cluster)
- `docs/issues/archive/2026-04-24-find-symbol-kotlin-multi-session.md` — kotlin-lsp concurrent-instance instability (fixed; CLAUDE.md cites it under a stale `2026-03-24-…` name)
- `docs/issues/archive/2026-05-18-edit-code-replace-misses-outer-attrs.md`, `…/2026-05-15-edit-code-replace-strips-doc.md`, `…/2026-03-20-replace-symbol-mod-tests-eats-adjacent.md` — prior edit_code range bugs whose fixes added the now-working restore guard
