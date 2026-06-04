# edit_file Whitespace-Normalized Fallback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When `edit_file`'s `old_string` fails to match only on whitespace/line-endings and the intended location is unambiguous, apply the edit (re-indented to the file's real formatting) instead of returning an error that forces a grep→read→retry loop.

**Architecture:** All new logic lives in the `match_count == 0` branch of `perform_edit` (`src/tools/edit_file/mod.rs:479-484`). Three pure helpers (line-window matcher, re-indenter, nearest-text hint) do the work and are unit-tested in isolation; the branch wires them together, gated by a before/after AST syntax check on the relaxed path only. Exact-match and multi-match paths are untouched → zero regression surface for currently-passing edits.

**Tech Stack:** Rust, tree-sitter (`crate::ast`), serde_json, the existing `EditFile` Tool + `project_ctx()` tempdir test harness.

**Design source:** `docs/superpowers/specs/2026-06-04-edit-file-whitespace-normalized-fallback-design.md`

---

## File Structure

- `src/tools/edit_file/mod.rs` — add three free helpers (`split_old_lines`, `find_normalized_windows`, `leading_ws`, `reindent_block`, `nearest_window_hint`) + the `NormWindow` struct near the other free fns (top of file, alongside `def_keywords_for_lang`). Modify the `match_count == 0` branch of `perform_edit`.
- `src/tools/edit_file/tests.rs` — add unit tests for the helpers and integration tests via `project_ctx()` + `EditFile.call`.

No new files. No new modules.

---

### Task 1: Normalized line-window matcher

**Files:**
- Modify: `src/tools/edit_file/mod.rs` (add `NormWindow`, `split_old_lines`, `find_normalized_windows` near the top free fns)
- Test: `src/tools/edit_file/tests.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn normalized_window_unique_on_indentation_diff() {
    let content = "fn a() {\n        let x = 1;\n        let y = 2;\n}\n";
    // old_string under-indented (4 spaces vs file's 8)
    let old = "    let x = 1;\n    let y = 2;";
    let w = find_normalized_windows(content, old);
    assert_eq!(w.len(), 1);
    assert_eq!((w[0].start_line, w[0].end_line), (2, 3));
}

#[test]
fn normalized_window_zero_on_content_diff() {
    let content = "    let x = 1;\n";
    let old = "    let x = 2;"; // real content differs, not whitespace
    assert_eq!(find_normalized_windows(content, old).len(), 0);
}

#[test]
fn normalized_window_ambiguous_returns_all() {
    let content = "    log(x)\nmid\n        log(x)\n";
    let old = "log(x)";
    assert_eq!(find_normalized_windows(content, old).len(), 2);
}

#[test]
fn normalized_window_exact_aligned_multiline() {
    let content = "a\nb\nc\nd\n";
    let old = "b\nc";
    let w = find_normalized_windows(content, old);
    assert_eq!(w.len(), 1);
    assert_eq!((w[0].start_line, w[0].end_line), (2, 3));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib edit_file::tests::normalized_window`
Expected: FAIL — `find_normalized_windows` / `NormWindow` not defined.

- [ ] **Step 3: Implement the matcher**

```rust
/// One line-aligned window in `content` matching `old_string` under whitespace
/// normalization. Line numbers 1-based; byte offsets bound the matched text
/// (start of first matched line .. end of last matched line, excluding that
/// line's trailing newline).
#[derive(Debug, Clone, PartialEq)]
struct NormWindow {
    start_line: usize,
    end_line: usize,
    start_byte: usize,
    end_byte: usize,
}

/// Split `old_string` into logical lines, dropping a single trailing empty
/// element produced by a trailing newline.
fn split_old_lines(old_string: &str) -> Vec<&str> {
    let mut v: Vec<&str> = old_string.split('\n').collect();
    if v.len() > 1 && v.last() == Some(&"") {
        v.pop();
    }
    v
}

/// Every line-aligned window in `content` whose lines equal `old_string`'s lines
/// after trimming leading+trailing whitespace on both sides. Internal content
/// must match exactly. Returns all matches; the caller enforces uniqueness.
fn find_normalized_windows(content: &str, old_string: &str) -> Vec<NormWindow> {
    let old_lines = split_old_lines(old_string);
    let k = old_lines.len();
    if k == 0 {
        return Vec::new();
    }
    // (line_without_newline, start_byte, end_byte_excl_newline)
    let mut spans: Vec<(&str, usize, usize)> = Vec::new();
    let mut offset = 0usize;
    for raw in content.split_inclusive('\n') {
        let text = raw.strip_suffix('\n').unwrap_or(raw);
        spans.push((text, offset, offset + text.len()));
        offset += raw.len();
    }
    let mut out = Vec::new();
    if spans.len() < k {
        return out;
    }
    for i in 0..=(spans.len() - k) {
        if (0..k).all(|j| spans[i + j].0.trim() == old_lines[j].trim()) {
            out.push(NormWindow {
                start_line: i + 1,
                end_line: i + k,
                start_byte: spans[i].1,
                end_byte: spans[i + k - 1].2,
            });
        }
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib edit_file::tests::normalized_window`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/tools/edit_file/mod.rs src/tools/edit_file/tests.rs
git commit -m "feat(edit_file): line-aligned whitespace-normalized window matcher"
```

---

### Task 2: Re-indentation helper

**Files:**
- Modify: `src/tools/edit_file/mod.rs` (add `leading_ws`, `reindent_block`)
- Test: `src/tools/edit_file/tests.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn reindent_shifts_base_indent() {
    // agent authored at 4 spaces; file base is 8 spaces
    let new = "    let x = 1;\n    let y = 2;";
    let out = reindent_block(new, "    ", "        ");
    assert_eq!(out, "        let x = 1;\n        let y = 2;");
}

#[test]
fn reindent_preserves_relative_deeper_indent() {
    let new = "    if c {\n        body()\n    }";
    let out = reindent_block(new, "    ", "        ");
    assert_eq!(out, "        if c {\n            body()\n        }");
}

#[test]
fn reindent_blank_lines_stay_blank() {
    let new = "    a\n\n    b";
    let out = reindent_block(new, "    ", "  ");
    assert_eq!(out, "  a\n\n  b");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib edit_file::tests::reindent`
Expected: FAIL — `reindent_block` not defined.

- [ ] **Step 3: Implement**

```rust
/// Leading-whitespace prefix of a line (may be empty).
fn leading_ws(line: &str) -> &str {
    &line[..line.len() - line.trim_start().len()]
}

/// Re-indent `new_string` so its base indentation matches the file. `agent_base`
/// is the leading whitespace of the agent's first non-blank old line; `file_base`
/// the leading whitespace of the file's first matched line. Lines starting with
/// `agent_base` get that prefix swapped for `file_base` (deeper relative indent
/// preserved). Blank lines stay blank. Lines shallower than `agent_base` get
/// their own leading whitespace replaced by `file_base` (best-effort; see the
/// tab/space risk note in the design doc).
fn reindent_block(new_string: &str, agent_base: &str, file_base: &str) -> String {
    let mut out = String::with_capacity(new_string.len());
    for (idx, line) in new_string.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if line.trim().is_empty() {
            continue; // blank line → emit empty
        }
        if let Some(rest) = line.strip_prefix(agent_base) {
            out.push_str(file_base);
            out.push_str(rest);
        } else {
            out.push_str(file_base);
            out.push_str(line.trim_start());
        }
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib edit_file::tests::reindent`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/tools/edit_file/mod.rs src/tools/edit_file/tests.rs
git commit -m "feat(edit_file): re-indent replacement to file's base indentation"
```

---

### Task 3: Nearest-text hint for the no-match error

**Files:**
- Modify: `src/tools/edit_file/mod.rs` (add `nearest_window_hint`)
- Test: `src/tools/edit_file/tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn nearest_window_returns_best_ratio_region() {
    let content = "alpha\nlet x = 1;\nlet y = 9;\nomega\n";
    let old = "let x = 1;\nlet y = 2;"; // line 1 matches, line 2 differs
    let (s, e, text) = nearest_window_hint(content, old).unwrap();
    assert_eq!((s, e), (2, 3));
    assert_eq!(text, "let x = 1;\nlet y = 9;");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib edit_file::tests::nearest_window`
Expected: FAIL — `nearest_window_hint` not defined.

- [ ] **Step 3: Implement**

```rust
/// Best-effort nearest window for an error hint when no unique normalized match
/// exists. Returns (start_line, end_line, actual_text) of the content window with
/// the highest count of normalized-matching lines against `old_string`.
fn nearest_window_hint(content: &str, old_string: &str) -> Option<(usize, usize, String)> {
    let old_lines = split_old_lines(old_string);
    let k = old_lines.len();
    if k == 0 {
        return None;
    }
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < k {
        return None;
    }
    let mut best: Option<(usize, usize)> = None; // (score, start_index)
    for i in 0..=(lines.len() - k) {
        let score = (0..k)
            .filter(|&j| lines[i + j].trim() == old_lines[j].trim())
            .count();
        if best.map_or(true, |(b, _)| score > b) {
            best = Some((score, i));
        }
    }
    best.filter(|&(score, _)| score > 0).map(|(_, i)| {
        (i + 1, i + k, lines[i..i + k].join("\n"))
    })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib edit_file::tests::nearest_window`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/edit_file/mod.rs src/tools/edit_file/tests.rs
git commit -m "feat(edit_file): nearest-text hint for old_string misses"
```

---

### Task 4: Wire the normalized fallback into perform_edit

**Files:**
- Modify: `src/tools/edit_file/mod.rs` — the `match_count == 0` branch in `perform_edit` (`:479-484`)
- Test: `src/tools/edit_file/tests.rs`

**Context:** `perform_edit` already reads `content`, computes `match_count`, and on `> 1` returns an ambiguity error. We replace only the `== 0` branch. To avoid drift between the exact and relaxed apply paths, extract the post-write notifications into one helper and call it from both.

- [ ] **Step 1: Write the failing integration tests** (use the `project_ctx()` harness; invoke `EditFile.call(json!({...}), &ctx)`)

```rust
#[tokio::test]
async fn normalized_apply_lands_and_reindents_with_note() {
    let (dir, ctx) = project_ctx().await;
    let f = dir.path().join("a.rs");
    std::fs::write(&f, "fn a() {\n        let x = 1;\n}\n").unwrap();
    let result = EditFile
        .call(json!({
            "path": f.to_str().unwrap(),
            "old_string": "    let x = 1;",     // 4 spaces, file has 8
            "new_string": "    let x = 42;"
        }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["applied_via"], "whitespace-normalized match");
    // re-indented to the file's 8-space base, not the agent's 4
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "fn a() {\n        let x = 42;\n}\n");
}

#[tokio::test]
async fn exact_match_preferred_no_note() {
    let (dir, ctx) = project_ctx().await;
    let f = dir.path().join("a.txt");
    std::fs::write(&f, "hello world\n").unwrap();
    let result = EditFile
        .call(json!({ "path": f.to_str().unwrap(), "old_string": "world", "new_string": "there" }), &ctx)
        .await
        .unwrap();
    assert!(result.get("applied_via").is_none(), "exact path must not carry the relaxed note");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "hello there\n");
}

#[tokio::test]
async fn normalized_ambiguous_errors_without_writing() {
    let (dir, ctx) = project_ctx().await;
    let f = dir.path().join("a.txt");
    let original = "    log()\nmid\n        log()\n";
    std::fs::write(&f, original).unwrap();
    let result = EditFile
        .call(json!({ "path": f.to_str().unwrap(), "old_string": "log()", "new_string": "trace()" }), &ctx)
        .await;
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&f).unwrap(), original, "file must be untouched");
}

#[tokio::test]
async fn content_diff_falls_through_to_nearest_text_error() {
    let (dir, ctx) = project_ctx().await;
    let f = dir.path().join("a.txt");
    let original = "    let x = 1;\n";
    std::fs::write(&f, original).unwrap();
    let result = EditFile
        .call(json!({ "path": f.to_str().unwrap(), "old_string": "    let x = 2;", "new_string": "let x = 3;" }), &ctx)
        .await;
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&f).unwrap(), original);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib edit_file::tests::normalized_apply edit_file::tests::exact_match_preferred edit_file::tests::normalized_ambiguous edit_file::tests::content_diff`
Expected: FAIL — branch still returns the static not-found error.

- [ ] **Step 3: Extract the write+notify tail into a helper**

Refactor the exact path's post-`match_count` write sequence (the `atomic_write` + `reload_config_if_project_toml_for` + `notify_file_changed` + `invalidate_call_edges_for` + `mark_file_dirty_for` + markdown `update_mtime`) into:

```rust
async fn commit_edit(ctx: &ToolContext, resolved: &std::path::Path, path: &str, new_content: &str) -> anyhow::Result<()> {
    crate::util::fs::atomic_write(resolved, new_content)?;
    ctx.agent.reload_config_if_project_toml_for(ctx.workspace_override.as_deref(), resolved).await;
    ctx.lsp.notify_file_changed(resolved).await;
    ctx.agent.invalidate_call_edges_for(ctx.workspace_override.as_deref(), resolved).await;
    ctx.agent.mark_file_dirty_for(ctx.workspace_override.as_deref(), resolved.to_path_buf()).await;
    if path.ends_with(".md") || path.ends_with(".markdown") {
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.update_mtime(resolved);
        }
    }
    Ok(())
}
```

Keep the exact path's behavior identical (it still does its own `atomic_write` + warn-only syntax check, OR call `commit_edit` then run the warn check — preserve current observable behavior; a regression test in the existing suite must stay green).

- [ ] **Step 4: Replace the `match_count == 0` branch**

```rust
if match_count == 0 {
    let windows = find_normalized_windows(&content, old_string);
    match windows.len() {
        1 => {
            let w = &windows[0];
            let matched = &content[w.start_byte..w.end_byte];
            let first_file_line = matched.lines().next().unwrap_or("");
            let file_base = leading_ws(first_file_line).to_string();
            let agent_base = split_old_lines(old_string)
                .into_iter()
                .find(|l| !l.trim().is_empty())
                .map(|l| leading_ws(l).to_string())
                .unwrap_or_default();
            let reindented = reindent_block(new_string, &agent_base, &file_base);
            let mut new_content = String::with_capacity(content.len());
            new_content.push_str(&content[..w.start_byte]);
            new_content.push_str(&reindented);
            new_content.push_str(&content[w.end_byte..]);

            // (AST backstop inserted by Task 5 — here, before commit_edit.)

            commit_edit(ctx, &resolved, path, &new_content).await?;
            return Ok(json!({
                "status": "ok",
                "applied_via": "whitespace-normalized match",
                "lines": format!("{}-{}", w.start_line, w.end_line),
                "note": "old_string matched after normalizing indentation/line-endings; verify the result"
            }));
        }
        0 => {
            let msg = match nearest_window_hint(&content, old_string) {
                Some((s, e, text)) =>
                    format!("old_string not found in {path}. Nearest content at lines {s}-{e}:\n{text}"),
                None => format!("old_string not found in {path}"),
            };
            return Err(super::RecoverableError::with_hint(
                msg,
                "No exact or whitespace-normalized match. Copy the actual bytes shown (or from read_file) and retry.",
            ).into());
        }
        _ => {
            let ranges = windows.iter()
                .map(|w| format!("{}-{}", w.start_line, w.end_line))
                .collect::<Vec<_>>().join(", ");
            return Err(super::RecoverableError::with_hint(
                format!("old_string matches {} regions after whitespace normalization (lines {ranges})", windows.len()),
                "Ambiguous — add surrounding context so exactly one region matches, or fix whitespace to match one exactly.",
            ).into());
        }
    }
}
```

Note: mid-line (non-line-aligned) `old_string` misses naturally fall to the `0`-window branch (their lines won't normalize-match a whole-line window) → nearest-text error. No special-casing needed; add the assertion test below to lock it.

- [ ] **Step 5: Add the mid-line guard test, then run all Task 4 tests**

```rust
#[tokio::test]
async fn mid_line_miss_stays_exact_only() {
    let (dir, ctx) = project_ctx().await;
    let f = dir.path().join("a.txt");
    let original = "let total = a + b + c;\n";
    std::fs::write(&f, original).unwrap();
    let result = EditFile
        .call(json!({ "path": f.to_str().unwrap(), "old_string": "a + X + c", "new_string": "a + b + d" }), &ctx)
        .await;
    assert!(result.is_err());
    assert_eq!(std::fs::read_to_string(&f).unwrap(), original);
}
```

Run: `cargo test --lib edit_file`
Expected: PASS (all new + all pre-existing edit_file tests).

- [ ] **Step 6: Commit**

```bash
git add src/tools/edit_file/
git commit -m "feat(edit_file): apply whitespace-normalized edit on unique match, nearest-text hint otherwise"
```

---

### Task 5: AST backstop on the relaxed path

**Files:**
- Modify: `src/tools/edit_file/mod.rs` — inside the `windows.len() == 1` arm, before `commit_edit`
- Test: `src/tools/edit_file/tests.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[tokio::test]
async fn normalized_apply_aborts_on_introduced_syntax_error() {
    let (dir, ctx) = project_ctx().await;
    let f = dir.path().join("a.rs");
    let original = "fn a() {\n        let x = 1;\n}\n";
    std::fs::write(&f, original).unwrap();
    // relaxed match on the body line, but new_string drops the semicolon AND a brace → parse breaks
    let result = EditFile
        .call(json!({
            "path": f.to_str().unwrap(),
            "old_string": "    let x = 1;",
            "new_string": "    let x = 1; {{{"
        }), &ctx)
        .await;
    assert!(result.is_err(), "relaxed edit that breaks parse must be rejected");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), original, "file must be unchanged");
}

#[tokio::test]
async fn normalized_apply_allowed_when_file_already_broken() {
    let (dir, ctx) = project_ctx().await;
    let f = dir.path().join("a.rs");
    // already broken (missing closing brace); relaxed edit adds no NEW errors
    std::fs::write(&f, "fn a() {\n        let x = 1;\n").unwrap();
    let result = EditFile
        .call(json!({
            "path": f.to_str().unwrap(),
            "old_string": "    let x = 1;",
            "new_string": "    let x = 2;"
        }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["applied_via"], "whitespace-normalized match");
    assert!(std::fs::read_to_string(&f).unwrap().contains("let x = 2;"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib edit_file::tests::normalized_apply_aborts edit_file::tests::normalized_apply_allowed`
Expected: FAIL — the abort test writes the broken file (no gate yet).

- [ ] **Step 3: Insert the before/after gate**

Inside the `windows.len() == 1` arm, after building `new_content`, before `commit_edit`:

```rust
if let Some(lang) = crate::ast::detect_language(std::path::Path::new(path)) {
    let before = crate::ast::has_syntax_errors(&content, lang);
    let after = crate::ast::has_syntax_errors(&new_content, lang);
    if after && !before {
        return Err(super::RecoverableError::with_hint(
            format!(
                "whitespace-normalized match at lines {}-{} would introduce syntax errors — not written",
                w.start_line, w.end_line
            ),
            "Verify the target with read_file and retry edit_file with the exact text.",
        ).into());
    }
}
```

(Confirm `detect_language` / `has_syntax_errors` signatures match their existing use in `perform_edit`'s post-write warn check — same module, same call shape.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib edit_file`
Expected: PASS (both new tests + full suite).

- [ ] **Step 5: Commit**

```bash
git add src/tools/edit_file/
git commit -m "feat(edit_file): gate relaxed apply on before/after syntax check"
```

---

### Task 6: Prompt-surface review + tool description

**Files:**
- Modify: `src/tools/edit_file/mod.rs` — `fn description` of `impl Tool for EditFile`
- Possibly: `src/prompts/source.md` (the `error-handling` slice) — only if the slice should mention the recovery mode
- Test: `cargo test --lib prompt`

- [ ] **Step 1: Update the tool description**

Add one sentence to `EditFile::description` so the LLM knows the recovery mode exists:

> "If `old_string` differs from the file only in whitespace/indentation and matches exactly one region, the edit is applied (re-indented to the file) and the response notes `applied_via: whitespace-normalized match` — verify it. If it matches zero or multiple regions, the error shows the nearest actual text."

- [ ] **Step 2: Decide whether the `error-handling` guide slice needs the same note**

Read `src/prompts/source.md` (`error-handling` surface) and the `edit_file` references. If the slice describes `edit_file` failure recovery, add the relaxed-match line. Keep the `server_instructions` slice under its 2200-byte cap (`prompts::redesign_invariants::source_md_under_cap`).

- [ ] **Step 3: Confirm no ONBOARDING_VERSION bump is needed**

This change touches the tool description + (optionally) the `server_instructions`/`error-handling` surface, both delivered fresh at session start. It does NOT touch the `onboarding_prompt` surface or `build_system_prompt_draft()`. Per CLAUDE.md "Which surface needs a bump?", **no bump**. Verify by confirming no edits landed in the `onboarding_prompt` slice or `builders.rs`.

- [ ] **Step 4: Run the prompt-surface gates**

Run: `cargo test --lib prompt`
Expected: PASS — including `prompt_surfaces_reference_only_real_tools` and `source_md_under_cap`. If `source_md_under_cap` fails, move content to a `get_guide` topic and leave a pointer (do NOT raise the cap).

- [ ] **Step 5: Full gate + commit**

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
git add src/tools/edit_file/mod.rs src/prompts/source.md
git commit -m "docs(edit_file): document whitespace-normalized recovery in tool description"
```

---

## Final verification (after all tasks)

- [ ] `cargo fmt && cargo clippy -- -D warnings && cargo test` all clean.
- [ ] Manual MCP check: `cargo build --release`, restart via `/mcp`, run an `edit_file` with a deliberately under-indented multi-line `old_string` against a real Kotlin file in a sibling project; confirm it lands with the `applied_via` note and correct indentation.
- [ ] Update `docs/issues/2026-06-04-edit-file-old-string-miss-no-closest-match.md`: Fix section → cite the master-side SHA(s) after cherry-pick; flip status to `fixed` once shipped + verified; archive per the Standard Ship Sequence.

## Notes / deferred

- **Batch path** (`edit[]`, `edit_file/mod.rs:335`) is out of scope for this plan — separate follow-up.
- **tab/space-width re-indentation** edge: v1 treats indentation as opaque prefix strings (the `strip_prefix(agent_base)` path). If a real mixed-tab/space datapoint surfaces, add a width-aware variant + test then.
- The sibling `read_markdown` JSON-vs-text asymmetry (`docs/issues/2026-06-04-read-markdown-heading-miss-emits-json.md`) is the same bug class; consider folding its fix into the same shipping batch.
