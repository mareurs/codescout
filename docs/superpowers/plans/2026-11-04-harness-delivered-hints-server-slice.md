---
id: '1af444a66fc5c47c'
kind: plan
status: draft
title: Harness-delivered hints, server slice — implementation plan
owners:
- marius
---

# Harness-delivered hints, server slice — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `edit_code` `replace` reports a verified callers advice in a new `advice` result field only when the edit changed the symbol's declaration and a call to it by name exists in another file; the `rename` hint names the new symbol; the unverified-edit warning is no longer overwritten.

**Architecture:** Two pure functions (a declaration-header comparison, a tree-sitter call counter) feed one async check with a 500 ms budget that uses the existing `Grep` tool to find candidate files. `do_replace` runs the check before it writes and attaches the outcome to its response as `advice`. No hook, plugin or channel change.

**Tech Stack:** Rust (codescout, `src/symbol/edit.rs`, `src/tools/symbol/`), tree-sitter, tokio, serde_json. Tests are `cargo test --lib` unit and integration tests in `src/tools/symbol/tests.rs`.

**Spec:** `docs/superpowers/specs/2026-11-04-harness-delivered-hints-design.md` (id `95abaa31c77daa39`), parts 1 and 2 and rollout slice 1. Bugs this plan closes: `053ca236198f1b9b`, `f5e732b0efeebd9e`.

## Global Constraints

- `advice` is an object `{"text": "...", "verified": true|false}`. It always stays in the result. `hint` stays one string for protocol pointers and safety warnings and is never overwritten by the callers text on `replace`.
- At most one `advice` per result.
- Outcomes: definite yes → `advice` with `verified: true`; definite no, or declaration unchanged → no `advice`; cannot tell → `advice` with `verified: false` whose text is exactly today's `verify_callers_hint`.
- Cannot tell covers: budget exhausted, more than `MAX_CANDIDATE_FILES` candidate files with no call found, a language without call classification, no readable declaration header, a `walk_cut` or `completeness_warning` key in the `Grep` result.
- Budget for the whole check: 500 ms. Basis (usage.db, 2026-11-04): `grep` success p50 13 ms, p95 465 ms; `edit_code` success p50 103 ms, p95 721 ms; `references` success p50 2,181 ms, p90 16,607 ms.
- `rename` is not relocated: its text stays in `hint`, unverified, and names the new symbol.
- The advice text says the match is syntactic and not resolved by the language server.
- No change in `claude-plugins`, in hooks, or in any prompt surface (`grep` for `verify callers` finds no prompt or manual text that mentions it).
- Every task ends green on its own tests; the last task runs `./scripts/gate.sh`. Commit with the shared-checkout sequence (`docs/conventions/shared-checkout-commit-sequence.md`): `git add` by file name, `git diff --cached --name-only`, `git commit -m ... -- <paths>`, trailer `Session-Id: <your session id>`.

## Review Focus

Most likely first. Each line has its test in the task named.

1. A declaration header that spans several lines, or has attributes, doc comments or decorators above it, must not look changed (Task 1).
2. A name that appears only as an argument, in a string or in a comment in another file is not a call (Task 2).
3. A call in the edited file itself (recursion) is not an outside call (Task 3).
4. A language with no call classification must give "cannot tell", never "no outside calls" (Tasks 2 and 3).
5. A `replace` result that already carries a safety warning keeps it when advice is attached (Task 4).

## File structure

- `src/symbol/edit.rs` — gains `declaration_changed` (pure).
- `src/tools/symbol/call_edges/resolver.rs` — gains `count_calls_named` (pure, next to the private helpers it needs); `call_edges/mod.rs` makes `resolver` reachable as `pub(crate) mod resolver`.
- `src/tools/symbol/callers_check.rs` (new) — `CallersCheck`, `check_callers`, `advice_for`, one responsibility: decide and describe the callers claim.
- `src/tools/symbol/edit_code.rs` — calls the check in `do_replace`, attaches `advice`, stops writing `hint` on `replace`, fixes the `rename` hint, renders `advice` in `format_compact`.
- `src/tools/symbol/mod.rs` — `mod callers_check;`.
- `src/tools/symbol/tests.rs` — integration tests (existing file; follow `edit_code_replace_reports_a_symbol_the_body_added_outside_the_target` for the tempdir, `Agent`, `MockLspProvider` and `ToolContext` setup).

---

### Task 1: `declaration_changed`

**Files:**
- Modify: `src/symbol/edit.rs` (add the function and its tests in the file's existing tests module; find it with `symbols(path="src/symbol/edit.rs", kind="module")`)

**Interfaces:**
- Produces: `pub fn declaration_changed(old: &str, new: &str) -> Option<bool>` — `None` when no declaration header can be read from either text.
- Header rule: skip leading blank lines and lines whose trimmed text starts with `//`, `/*`, `*`, `#` or `@`; take lines from there up to and including the first line that contains `{`, ends with `:`, or ends with `;`; join with a space and collapse every run of whitespace to one space. Equal headers mean unchanged.

- [ ] **Step 1: Write the failing tests** (names and assertions):
  - `declaration_changed_is_false_for_a_body_only_edit`: `"fn a() -> u8 {\n    1\n}"` vs `"fn a() -> u8 {\n    3\n}"` → `Some(false)`.
  - `declaration_changed_is_true_when_a_parameter_is_added`: new header `fn a(x: u8) -> u8 {` → `Some(true)`.
  - `declaration_changed_is_true_when_visibility_changes`: `fn a() {}` vs `pub fn a() {}` → `Some(true)`.
  - `declaration_changed_ignores_line_breaks_and_runs_of_spaces_in_the_header`: `"fn a(x: u8) -> u8 {"` vs `"fn  a(x: u8)\n    -> u8 {"` → `Some(false)`.
  - `declaration_changed_skips_attributes_doc_comments_and_decorators`: `"/// doc\n#[inline]\nfn a() {}"` vs `"fn a() {}"` → `Some(false)`; Python `"@d\ndef f(a):\n    pass"` vs `"def f(a):\n    return 1"` → `Some(false)`; `"def f(a, b):\n    pass"` as the new text → `Some(true)`.
  - `declaration_changed_is_none_without_a_header`: `""` or `"// only a comment"` on either side → `None`.
- [ ] **Step 2: Run to verify they fail** — `scripts/with-slot.sh cargo test --lib declaration_changed` — Expected: compile error, function not found.
- [ ] **Step 3: Implement `declaration_changed`** in `src/symbol/edit.rs` per the header rule above.
- [ ] **Step 4: Run to verify they pass** — same command — Expected: `test result: ok. 6 passed`.
- [ ] **Step 5: Commit** — `git add src/symbol/edit.rs`; message `feat(edit_code): declaration_changed compares declaration headers`.

### Task 2: `count_calls_named`

**Files:**
- Modify: `src/tools/symbol/call_edges/resolver.rs` (function plus tests in its existing `tests` module)
- Modify: `src/tools/symbol/call_edges/mod.rs` (`mod resolver;` → `pub(crate) mod resolver;` if it is private)

**Interfaces:**
- Consumes: the private `parse_ts_tree(src: &str, language_id: &str) -> Option<tree_sitter::Tree>`, `call_kinds_for(language_id: &str) -> &'static [&'static str]`, `callee_identifier(node: tree_sitter::Node<'_>, src: &str, language_id: &str) -> Option<String>`.
- Produces: `pub(crate) fn count_calls_named(src: &str, language_id: &str, name: &str) -> Option<usize>` — `None` when the language has no call kinds or the parse fails; otherwise the number of call nodes whose callee identifier equals `name`.

- [ ] **Step 1: Write the failing tests:**
  - `count_calls_named_counts_function_and_method_calls_in_rust`: `"fn main() { a(1); x.a(2); b(a); let s = \"a(3)\"; /* a(4) */ }"`, name `a` → `Some(2)`.
  - `count_calls_named_counts_calls_in_python`: `"a(1)\nobj.a(2)\nprint(a)\n"`, name `a` → `Some(2)`.
  - `count_calls_named_counts_calls_in_typescript`: `"a(1); obj.a(2); foo(a);"`, language `typescript`, name `a` → `Some(2)`.
  - `count_calls_named_is_none_for_a_language_without_call_kinds`: language `go` → `None`.
- [ ] **Step 2: Run to verify they fail** — `scripts/with-slot.sh cargo test --lib count_calls_named` — Expected: compile error.
- [ ] **Step 3: Implement `count_calls_named`** next to `resolve_callees_via_ts`; walk every node of the tree (a `TreeCursor` loop) and test call nodes only.
- [ ] **Step 4: Run to verify they pass** — Expected: `4 passed`.
- [ ] **Step 5: Commit** — `git add src/tools/symbol/call_edges/resolver.rs src/tools/symbol/call_edges/mod.rs`; message `feat(call_edges): count_calls_named counts calls by callee name`.

### Task 3: `check_callers` and `advice_for`

**Files:**
- Create: `src/tools/symbol/callers_check.rs`
- Modify: `src/tools/symbol/mod.rs` (add `mod callers_check;` next to `mod edit_code;`)
- Test: pure tests inside `callers_check.rs`; project-level tests in `src/tools/symbol/tests.rs`

**Interfaces:**
- Consumes: `crate::symbol::edit::declaration_changed` (Task 1); `crate::tools::symbol::call_edges::resolver::count_calls_named` (Task 2); `crate::tools::grep::Grep` (a `Tool`; call it with `Grep.call(json!({...}), ctx)`); `super::edit_code::verify_callers_hint(name_path: &str, rel_path: &str) -> String`.
- Produces:
  - `pub(super) const CALLERS_CHECK_BUDGET: Duration = Duration::from_millis(500);` and `pub(super) const MAX_CANDIDATE_FILES: usize = 25;`
  - `#[derive(Debug, Clone, PartialEq, Eq)] pub(super) enum CallersCheck { Unchanged, NoOutsideCalls, OutsideCalls { file: String }, CannotTell }` (`file` project-relative with forward slashes)
  - `pub(super) async fn check_callers(ctx: &ToolContext, full_path: &Path, language_id: &str, symbol_name: &str, old_text: &str, new_text: &str) -> CallersCheck`
  - `pub(super) async fn within_budget<F: Future<Output = CallersCheck>>(budget: Duration, fut: F) -> CallersCheck` — maps a timeout to `CannotTell`.
  - `pub(super) fn advice_for(check: &CallersCheck, symbol_name: &str, name_path: &str, rel_path: &str) -> Option<serde_json::Value>`
- `check_callers` order: `declaration_changed` (`Some(false)` → `Unchanged`; `None` → `CannotTell`); then, inside `within_budget`, `Grep.call` with `{"pattern": "\\b<escaped name>\\b", "mode": "files", "glob": "*.<extension of full_path>"}`; any `walk_cut` or `completeness_warning` key in its result → `CannotTell` (`Grep` sets `walk_cut` when its time budget cut the walk, and `completeness_warning` when a zero total may hide skipped files); the result's `files` array holds `{"file", "count"}` entries ranked by count, with `file` possibly absolute or relative to the project root; drop the entry for the edited file (compare canonical paths; a returned path may be relative to the project root); inspect at most `MAX_CANDIDATE_FILES` files in ranked order, reading each and calling `count_calls_named` (`None` → `CannotTell` at once; `Some(n)` with `n > 0` → `OutsideCalls`); when none has a call: `NoOutsideCalls` if every candidate was inspected, else `CannotTell`.
- `advice_for`: `OutsideCalls { file }` → `{"text": "<verify_callers_hint(name_path, rel_path)> | `<symbol_name>` is called by name in <file> (a syntactic match, not resolved by the language server)", "verified": true}`; `CannotTell` → `{"text": <verify_callers_hint(name_path, rel_path)>, "verified": false}`; `Unchanged` and `NoOutsideCalls` → `None`.

- [ ] **Step 1: Write the failing tests:**
  - in `callers_check.rs`: `advice_for_a_found_call_is_verified_and_names_the_file`, `advice_for_cannot_tell_is_unverified_with_todays_text` (text equals `verify_callers_hint(..)`), `advice_for_unchanged_and_no_outside_calls_is_none`, `within_budget_maps_a_timeout_to_cannot_tell` (`std::future::pending()` with 10 ms).
  - in `tests.rs` (tempdir project with `src/lib.rs` holding `fn a() -> u8 { 1 }`, ctx as in the model test): `check_callers_finds_a_call_in_another_file` (`src/other.rs` calls `crate::a()` → `OutsideCalls { file: "src/other.rs" }`); `check_callers_ignores_a_mention_that_is_not_a_call` (`let f = a;` and `"a(1)"` in a string → `NoOutsideCalls`); `check_callers_ignores_a_call_in_the_edited_file` (recursion in `lib.rs` only → `NoOutsideCalls`); `check_callers_is_unchanged_for_a_body_only_edit`; `check_callers_cannot_tell_for_a_language_without_call_classification` (`.go` files, language `go`); `check_callers_cannot_tell_when_candidates_exceed_the_cap` (26 files that mention `a` without calling it); `check_callers_cannot_tell_when_no_header_can_be_read`.
- [ ] **Step 2: Run to verify they fail** — `scripts/with-slot.sh cargo test --lib check_callers` and `... advice_for` and `... within_budget` — Expected: compile errors.
- [ ] **Step 3: Implement the items above** in `callers_check.rs`; register the module.
- [ ] **Step 4: Run to verify they pass** — same commands — Expected: all pass.
- [ ] **Step 5: Commit** — `git add src/tools/symbol/callers_check.rs src/tools/symbol/mod.rs src/tools/symbol/tests.rs`; message `feat(edit_code): check_callers decides the verified-callers claim within a budget`.

### Task 4: wire into `edit_code`

**Files:**
- Modify: `src/tools/symbol/edit_code.rs` — `do_replace`, `call` (`rename` and `replace` arms), `format_compact`
- Test: `src/tools/symbol/tests.rs`

**Interfaces:**
- Consumes: `check_callers`, `advice_for` (Task 3).
- Produces:
  - `fn attach_advice(response: &mut Value, advice: Option<Value>)` — sets `response["advice"]` when `Some`; never touches `hint`.
  - `fn renamed_name_path(name_path: &str, new_name: &str) -> String` — replaces the last `/` segment (`"Agent/old"`, `"new"` → `"Agent/new"`; `"a"`, `"b"` → `"b"`).

- [ ] **Step 1: Write the failing tests:**
  - integration, model setup as above, with `src/other.rs` where needed: `edit_code_replace_with_a_changed_signature_and_an_outside_call_carries_verified_advice` (body `fn a(x: u8) -> u8 {\n    x\n}`; `out["advice"]["verified"] == true`; text contains `verify callers: references(symbol="a"` and `src/other.rs`; `out.get("hint")` is `None`); `edit_code_replace_of_a_body_only_edit_carries_no_advice_and_no_hint`; `edit_code_replace_with_a_changed_signature_and_no_outside_call_carries_no_advice`; `edit_code_replace_that_cannot_tell_carries_unverified_advice` (26 mentioning files).
  - unit: `attach_advice_keeps_an_existing_safety_warning` (response `{"hint": "The edit was written but NOT verified against dropped symbols. ..."}` plus advice → `hint` unchanged, `advice` set); `renamed_name_path_replaces_the_last_segment`; `rename_hint_reads_back_to_the_new_symbol_and_the_path` (build the hint as the `rename` arm does and parse it with the existing `json_string_after` helper of `route_path_tests.rs`); `format_compact_shows_the_advice_text_after_the_hint` (result with `hint` and `advice` → compact text contains both).
- [ ] **Step 2: Run to verify they fail** — `scripts/with-slot.sh cargo test --lib edit_code_replace` and `... attach_advice` and `... renamed_name_path` — Expected: failures or compile errors.
- [ ] **Step 3: Implement.** In `do_replace`, immediately before the second `let pre_ast = crate::ast::extract_symbols(&full_path).ok();` (the one after `checked_rebase(`), compute `old_text = lines[start..end].join("\n")` and `let callers = check_callers(ctx, &full_path, &lang, &sym.name, &old_text, &effective_body).await;`. Before the final `Ok(response)`, `attach_advice(&mut response, advice_for(&callers, &sym.name, name_path, rel_path))`. In `call()`, delete `result["hint"] = json!(verify_callers_hint(name_path, rel_path));` from the `replace` arm, and in the `rename` arm pass `&renamed_name_path(name_path, new_name)`. In `format_compact`, after the `hint` line, append `advice.text` when it is a string.
- [ ] **Step 4: Run to verify they pass** — same commands — Expected: all pass; also `scripts/with-slot.sh cargo test --lib route_path` still passes.
- [ ] **Step 5: Commit** — `git add src/tools/symbol/edit_code.rs src/tools/symbol/tests.rs`; message `feat(edit_code): replace reports verified callers advice; rename hint names the new symbol; the unverified-edit warning is kept`.

### Task 5: gate and close the bugs

**Files:**
- Modify (through `doc`): bugs `053ca236198f1b9b` and `f5e732b0efeebd9e`; the design `95abaa31c77daa39`

- [ ] **Step 1: Run the gate** — `./scripts/gate.sh` — Expected: exit 0 with all stages green. A red stage not caused by this change is recorded in the bug, not hidden.
- [ ] **Step 2: Close both bugs.** For each: `status` to `fixed`, `closed` to today's date, fill `Fix` and `Tests added` with the test names above, record the fix SHA labelled `experiments` and its patch id (`git show <sha> | git patch-id --stable`), clear the `unverified` caveat with `CLEARED <date>`, then `doc(action="move", ...)` into `docs/issues/archive/`.
- [ ] **Step 3: Update the design.** Add to `## Rollout` that slice 1 is done, with the SHA; note what the integration tests measured about the check's latency on this repository (run `check_callers` on three real symbols and record the milliseconds).
- [ ] **Step 4: Commit** — `git add` the three files by name; message `docs: slice 1 of harness-delivered hints done; bugs 053ca236198f1b9b and f5e732b0efeebd9e fixed on experiments`.
