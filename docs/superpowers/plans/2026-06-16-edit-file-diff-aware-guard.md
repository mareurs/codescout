# Diff-Aware Structural Guard for `edit_file` — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop `edit_file`'s structural guard from rejecting edits where a definition keyword appears only on an *unchanged context line* (e.g. inserting a blank line before an existing `fn`/`fun`).

**Architecture:** Make `guard_structural_rewrite` *diff-aware*: a definition keyword trips the guard only when it sits on a line the edit adds or removes (a line present in one of `old_string`/`new_string` but not byte-identically in the other), instead of being present anywhere in either multi-line string. Relaxation-only — never blocks more than today.

**Tech Stack:** Rust; the codescout MCP server; `cargo` (fmt/clippy/test) + `cargo rb` (alias = `build --release --features server-stack`) for the live MCP binary.

**Spec:** `docs/superpowers/specs/2026-06-16-edit-file-structural-guard-diff-aware-design.md`

**Scout corrections already made (do not re-derive):**
- Tests live in the **external file `src/tools/edit_file/tests.rs`** (`mod tests;`), *not* an inline module at line 779. `tests.rs` has `use super::*;` and `use super::super::RecoverableError;`, so the guard/helpers are callable unprefixed and `err.message` is in scope.
- **No existing test covers the structural guard** — this work is purely additive; there is no test encoding the old blocking behavior to update.
- `guard_structural_rewrite` is called from **3 sites** (the flag-gated `is_structural_edit` gate at `mod.rs:347`, the batch pre-pass at `:414`, and the unconditional single-path call at `:568`). All three inherit the fix from the one function — no per-site changes.
- `find_def_keyword` (`mod.rs:38`) has **no callers outside the guard** (only `:216` and `:220`), so it can be replaced outright without leaving dead code (which would fail `clippy -D warnings`).

**Commit discipline note:** This repo's `CLAUDE.md` overrides the superpowers "commit per task" default — *batch related changes into one well-tested commit*. So the plan commits **once**, at the end, after the full gate is green (Task 4). Earlier tasks leave the tree dirty intentionally.

---

## File Structure

- Modify: `src/tools/edit_file/mod.rs`
  - Replace `find_def_keyword` (`:38-49`) with `find_def_keyword_in_lines` (takes a line iterator).
  - Add `lines_only_in` helper (set-difference of lines).
  - Rewrite the `old_kw` / `new_kw` computation inside `guard_structural_rewrite` (`:214-221`); the rest of the function is unchanged.
- Modify (tests): `src/tools/edit_file/tests.rs` — add 6 unit tests calling `guard_structural_rewrite` directly.
- Modify (prompt surface): `src/prompts/guides/iron-laws-detail.md` (~`:52`) — one-line note that keyword-bearing *context* lines do not trip the guard.

No new files. No change to `def_keywords_for_lang`, `is_structural_edit`, `perform_edit`, or `edit_repair.rs` (the escaped-quote recovery is a separate session).

---

## Task 1: Add diff-aware guard tests (red)

**Files:**
- Test: `src/tools/edit_file/tests.rs` (append at end of file, inside the existing test module scope)

- [ ] **Step 1: Write the tests**

Append these to `src/tools/edit_file/tests.rs`. They call the private guard directly (in scope via `use super::*`). Paths use `.rs` so `detect_lsp_language` resolves without depending on Kotlin LSP config; one `.kt` test mirrors the original repro.

```rust
// ---- diff-aware structural guard (spec 2026-06-16) ----

#[test]
fn guard_allows_blank_line_before_unchanged_fn() {
    // Repro class: inserting a blank line before an existing fn (ktlint).
    // The `fn` line is byte-identical in old and new — unchanged context.
    let old = "    let flat = HashMap::new();\n    fn covered() {}";
    let new = "    let flat = HashMap::new();\n\n    fn covered() {}";
    assert!(guard_structural_rewrite("x.rs", old, new).is_ok());
}

#[test]
fn guard_allows_kotlin_blank_line_before_unchanged_fun() {
    // Exact backend-kotlin 2026-06-16 shape.
    let old = "    val m = HashMap<String, Int>()\n    fun covered(): Int = 0";
    let new = "    val m = HashMap<String, Int>()\n\n    fun covered(): Int = 0";
    assert!(guard_structural_rewrite("x.kt", old, new).is_ok());
}

#[test]
fn guard_allows_comment_added_before_unchanged_fn() {
    let old = "let x = 1;\nfn foo() {}";
    let new = "let x = 1;\n// helper\nfn foo() {}";
    assert!(guard_structural_rewrite("x.rs", old, new).is_ok());
}

#[test]
fn guard_blocks_fn_rename() {
    let old = "fn foo() {\n    body();\n}";
    let new = "fn bar() {\n    body();\n}";
    let err = guard_structural_rewrite("x.rs", old, new).unwrap_err();
    assert!(err.message.contains("fn "), "got: {}", err.message);
}

#[test]
fn guard_blocks_new_fn_introduced_in_new_string() {
    // BUG-050: a new symbol spliced into new_string. The `fn helper` line is
    // an *added* line absent from old_string -> still blocked.
    let old = "let a = 1;\nlet b = 2;";
    let new = "let a = 1;\nfn helper() {}\nlet b = 2;";
    assert!(guard_structural_rewrite("x.rs", old, new).is_err());
}

#[test]
fn guard_blocks_changed_keyword_line_despite_unchanged_keyword_context() {
    // Unchanged `fn keep` context must not mask the changed `fn foo` -> `fn bar`.
    let old = "fn keep() {}\nfn foo() {}";
    let new = "fn keep() {}\nfn bar() {}";
    let err = guard_structural_rewrite("x.rs", old, new).unwrap_err();
    assert!(err.message.contains("fn "), "got: {}", err.message);
}
```

- [ ] **Step 2: Run the tests to confirm the expected red/green split**

Run: `cargo test --lib edit_file::tests::guard_ -- --nocapture`
Expected: 3 PASS, 3 FAIL. The three that **FAIL** demonstrate the bug:
- `guard_allows_blank_line_before_unchanged_fn` — FAIL (currently blocked)
- `guard_allows_kotlin_blank_line_before_unchanged_fun` — FAIL (currently blocked)
- `guard_allows_comment_added_before_unchanged_fn` — FAIL (currently blocked)

The three that **PASS** under current code (`guard_blocks_fn_rename`, `guard_blocks_new_fn_introduced_in_new_string`, `guard_blocks_changed_keyword_line_despite_unchanged_keyword_context`) are the invariants the fix must preserve.

If `err.message` does not compile (field visibility tightened since the scout), switch those two asserts to `err.to_string().contains("fn ")` — the Display impl renders the message and is the documented stable test contract.

---

## Task 2: Implement the diff-aware guard (green)

**Files:**
- Modify: `src/tools/edit_file/mod.rs:14-49` (helpers) and `:201-235` (`guard_structural_rewrite`)

> Apply these with **`edit_code`** (LSP-aware) — they are structural changes to functions. Do **not** use `edit_file` on itself for these.

- [ ] **Step 1: Replace `find_def_keyword` with `find_def_keyword_in_lines` and add `lines_only_in`**

Replace the whole `find_def_keyword` function (`:38-49`) with these two functions:

```rust
/// Scans `lines` for the first definition keyword for `lang`, skipping comment
/// lines (// /* * #) so a keyword inside a comment does not falsely trip the guard.
fn find_def_keyword_in_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
    lang: &str,
) -> Option<&'static str> {
    let keywords = def_keywords_for_lang(lang);
    lines
        .filter(|line| {
            let t = line.trim_start();
            !t.starts_with("//")
                && !t.starts_with("/*")
                && !t.starts_with('*')
                && !t.starts_with('#')
        })
        .find_map(|line| keywords.iter().find(|kw| line.contains(**kw)).copied())
}

/// Lines present in `from` but not (byte-identical) in `to`. Restricts the
/// structural-keyword check to lines the edit actually adds or removes — a keyword
/// on an unchanged context line is an anchor, not a rewrite.
fn lines_only_in<'a>(from: &'a str, to: &str) -> Vec<&'a str> {
    let to_lines: std::collections::HashSet<&str> = to.lines().collect();
    from.lines().filter(|l| !to_lines.contains(l)).collect()
}
```

- [ ] **Step 2: Rewrite the `old_kw` / `new_kw` block in `guard_structural_rewrite`**

Inside `guard_structural_rewrite`, replace the current `old_kw`/`new_kw` computation (`:214-221`) with:

```rust
    // Diff-aware: a definition keyword signals a structural change only when it
    // sits on a line the edit actually introduces or removes. A keyword on a line
    // byte-identical in old and new is unchanged context (an anchor), not a
    // rewrite — ignore it. Relaxation-only; a newly-introduced symbol line
    // (BUG-050) is by construction absent from old_string, so it stays caught.
    let old_kw = old_string
        .contains('\n')
        .then(|| {
            find_def_keyword_in_lines(lines_only_in(old_string, new_string).into_iter(), lang)
        })
        .flatten();
    let new_kw = new_string
        .contains('\n')
        .then(|| {
            find_def_keyword_in_lines(lines_only_in(new_string, old_string).into_iter(), lang)
        })
        .flatten();
```

Leave the rest of the function (the `is_source_path` / `detect_lsp_language` early returns, the `let Some(keyword) = old_kw.or(new_kw)` arm, the `infer_edit_hint` + `RecoverableError` error) unchanged.

- [ ] **Step 3: Run the guard tests — all green**

Run: `cargo test --lib edit_file::tests::guard_`
Expected: 6 passed, 0 failed.

- [ ] **Step 4: Run the full edit_file test module (no regressions)**

Run: `cargo test --lib edit_file::`
Expected: all pass (the pre-existing read_file / create_file / edit_file tests are unaffected).

---

## Task 3: Update the prompt surface

**Files:**
- Modify: `src/prompts/guides/iron-laws-detail.md` (~`:52`, the `edit_file is blocked for structural edits` description)

- [ ] **Step 1: Read the current text**

Run: `read_markdown` on `src/prompts/guides/iron-laws-detail.md` for the section that contains "edit content contains a symbol-definition keyword".

- [ ] **Step 2: Add a one-line clarification**

After the line describing that the guard fires when "edit content contains a symbol-definition keyword … or overlaps a known symbol range", add (via `edit_markdown`):

```
> A definition keyword on a line that is identical in old_string and new_string
> (pure context/anchor) does NOT trip the guard — only added/removed/changed
> definition lines do.
```

- [ ] **Step 3: Confirm no `ONBOARDING_VERSION` bump is needed**

`iron-laws-detail.md` is a `get_guide` topic (loaded fresh per session), not the `onboarding_prompt` surface. Per `CLAUDE.md`'s surface table, **do not** bump `ONBOARDING_VERSION`. No change to `src/tools/onboarding.rs`.

- [ ] **Step 4: Run the prompt-surface guard test**

Run: `cargo test --lib prompt`
Expected: all pass (incl. `prompt_surfaces_reference_only_real_tools` and `source_md_under_cap` — we touched a guide, not the capped `server_instructions` slice, but run it to be safe).

---

## Task 4: Gate, live-verify, and commit (single commit)

**Files:** none new — verification + commit only.

- [ ] **Step 1: Format**

Run: `cargo fmt`
Expected: no diff noise beyond the edited functions.

- [ ] **Step 2: Clippy (deny warnings)**

Run: `cargo clippy -- -D warnings`
Expected: clean. (Watch specifically for an "unused function" warning — it would mean a stray reference to the removed `find_def_keyword` remains.)

- [ ] **Step 3: Full test suite**

Run: `cargo test`
Expected: all pass.

- [ ] **Step 4: Build the live MCP binary and reconnect**

Run: `cargo rb`  (alias for `cargo build --release --features server-stack`)
Then in the Claude Code session: `/mcp` to reconnect (the symlink `~/.cargo/bin/codescout → target/release/codescout` updates automatically).

- [ ] **Step 5: Manual live verification**

Against a real `.rs`/`.kt` file, run an `edit_file` that inserts a blank line before an existing `fn`/`fun`, anchoring on the line *including* that `fn`/`fun` as context. Expected: the edit applies (no "edit contains a symbol definition" error). Then confirm a genuine rename via `edit_file` is still rejected and routed to `edit_code`.

- [ ] **Step 6: Commit (one commit, all changes)**

```bash
git add src/tools/edit_file/mod.rs src/tools/edit_file/tests.rs src/prompts/guides/iron-laws-detail.md
git commit -m "fix(edit_file): make structural guard diff-aware

guard_structural_rewrite blocked edits when a definition keyword appeared
on an unchanged context line (e.g. inserting a blank line before an existing
fn). Now a keyword only trips the guard when it sits on a line the edit adds
or removes. Relaxation-only: renames and new-symbol splices (BUG-050) stay
blocked. See docs/superpowers/specs/2026-06-16-edit-file-structural-guard-diff-aware-design.md

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

This stays on `experiments`. Cherry-pick to `master` via the Standard Ship Sequence only after the live verification in Step 5 passes. Coordinate the rebase with the concurrent escaped-quote-recovery session (same file, disjoint regions) — re-read `git reflog` before any rebase/reset.

## Execution notes / deviations (2026-06-16)

The plan was executed with three deviations, all discovered during a pre-edit re-scout after a
concurrent session committed the escaped-quote recovery to the same file:

1. **Kept `find_def_keyword`; used a join.** Rather than refactoring it to a line iterator and
   removing it (which would have broken the existing `find_def_keyword_ignores_class_in_comment`
   test and risked dead-code), the guard passes it the joined changed lines:
   `find_def_keyword(&lines_only_in(old, new).join("\n"), lang)`. One new helper, `lines_only_in`.
2. **"No existing test" was wrong.** There were several guard tests (the grep that concluded
   "none" capped at 150 matches in a 3000-line file). Four integration tests asserted *blocking*
   on function-body edits — the deliberate "route bodies to edit_code" policy.
3. **User chose to embrace body edits** (vs. narrowing to whitespace/comments only). The four
   integration tests were repurposed to **rename** fixtures (still structural → still block), and a
   new `edit_file_allows_body_edit_on_lsp_language` covers the now-allowed body edit. All 225
   `edit_file` tests pass.
