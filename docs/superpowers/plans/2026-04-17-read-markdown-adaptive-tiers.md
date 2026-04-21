# Adaptive `read_markdown` Tiers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `read_markdown` the sole markdown entry point with size-adaptive output (small/medium/large tiers), kill `read_file(mode="complete")`, and align the companion hook message with actual tool routing.

**Architecture:** Replace the binary fork in `read_markdown`'s default branch (content-or-summary) with three tiers decided by byte budget + line soft-cap. Remove `mode=complete` from `read_file` and its `/plans/` exemption. Simplify the `.md` redirect hint. Update the `codescout-companion` `PreToolUse` hook to reference `read_markdown` instead of `read_file(heading=...)`. Bump `ONBOARDING_VERSION`.

**Tech Stack:** Rust (tokio, serde_json, async-trait), bash (companion hook).

**Spec:** `docs/superpowers/specs/2026-04-17-read-markdown-adaptive-tiers-design.md`

---

## File Map

Touched in this plan:

- **Create:** none.
- **Modify:**
  - `src/tools/markdown.rs` — rewrite default-branch tier logic; add `LINE_SOFT_CAP`; add buffer-ref support to `ReadMarkdown::call`.
  - `src/tools/file.rs` — strip `mode` param + `read_complete_mode` + related tests; simplify `.md` gate hint.
  - `src/tools/workflow.rs` — bump `ONBOARDING_VERSION`; update `build_system_prompt_draft()` markdown guidance.
  - `src/prompts/server_instructions.md` — update markdown-reading guidance.
  - `src/prompts/onboarding_prompt.md` — update markdown-reading guidance.
  - `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh` — rewrite `.md` branch (lines ~184-196).

---

## Task 1: Pre-flight — verify clean tree and green baseline

**Files:** none modified.

- [ ] **Step 1: Verify clean working tree**

Run: `git status`
Expected: clean, on `experiments` branch.

- [ ] **Step 2: Verify baseline green**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: all pass.

If any fail, stop and resolve before starting.

---

## Task 2: Add `LINE_SOFT_CAP` constant

**Files:**
- Modify: `src/tools/mod.rs` (near `INLINE_BYTE_BUDGET` at line 48)

- [ ] **Step 1: Add constant**

Locate the existing `INLINE_BYTE_BUDGET` constant in `src/tools/mod.rs:48`. Immediately after it, add:

```rust
/// Soft line-count nudge for markdown default reads.
///
/// Files whose line count exceeds this threshold — but whose byte size still
/// fits `INLINE_BYTE_BUDGET` — get full content plus a focused-read hint.
/// Files larger than `INLINE_BYTE_BUDGET` are buffered regardless of line count.
pub(crate) const LINE_SOFT_CAP: usize = 150;
```

- [ ] **Step 2: Verify compile**

Run: `cargo build --lib`
Expected: success, no warnings about the new constant (unused imports OK; it's consumed in Task 3).

- [ ] **Step 3: Commit**

```bash
git add src/tools/mod.rs
git commit -m "refactor(tools): add LINE_SOFT_CAP constant for markdown tiers"
```

---

## Task 3: Write failing tests for the three tiers

**Files:**
- Modify: `src/tools/markdown.rs` (add tests in existing `#[cfg(test)] mod tests` block; locate it by searching for `mod tests` in the file)

- [ ] **Step 1: Locate test module**

Run: `grep -n "mod tests" src/tools/markdown.rs`
Note the line number. New tests go inside that module.

- [ ] **Step 2: Add test-module imports, `test_ctx()` helper, and `synth_md` helper**

The existing `mod tests` block in `markdown.rs` contains only sync `#[test]` functions for `perform_section_edit`. It has no async/tempdir imports and no `test_ctx` helper. Add these at the top of the `mod tests` block, just after `use super::*;`:

```rust
    use crate::agent::Agent;
    use crate::lsp::LspManager;
    use serde_json::json;
    use tempfile::tempdir;

    async fn test_ctx() -> crate::tools::ToolContext {
        crate::tools::ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        }
    }

    /// Synthesize markdown content with `lines` total lines and `sections` H2 sections.
    fn synth_md(lines: usize, sections: usize) -> String {
        let mut out = String::from("# Title\n\n");
        let per_section = (lines.saturating_sub(2) / sections.max(1)).max(1);
        for i in 0..sections {
            out.push_str(&format!("## Section {}\n\n", i + 1));
            for _ in 0..per_section {
                out.push_str("body line\n");
            }
        }
        out
    }
```

The `test_ctx()` helper mirrors the one in `src/tools/file.rs:1971`. Field list must match `ToolContext`'s current struct definition — if compilation fails with missing/unknown fields, diff against the `file.rs` helper and align.

- [ ] **Step 3: Add failing test — small tier returns full content, no hint**

```rust
#[tokio::test]
async fn read_markdown_small_returns_full_content_no_hint() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("small.md");
    std::fs::write(&file, synth_md(30, 2)).unwrap();

    let out = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    assert!(out.get("content").is_some(), "small tier must include content");
    assert!(out.get("hint").is_none(), "small tier must not include hint");
    assert!(out.get("file_id").is_none(), "small tier must not buffer");
    assert!(out.get("heading_map").is_none(), "small tier has no heading_map");
}
```

- [ ] **Step 4: Add failing test — medium tier returns full content + hint**

```rust
#[tokio::test]
async fn read_markdown_medium_returns_content_with_hint() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("medium.md");
    // 300 lines: > LINE_SOFT_CAP (150) but well under INLINE_BYTE_BUDGET.
    std::fs::write(&file, synth_md(300, 6)).unwrap();

    let out = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    assert!(out.get("content").is_some(), "medium tier includes content");
    assert!(out.get("hint").is_some(), "medium tier includes hint");
    assert!(out.get("heading_count").is_some(), "medium tier reports heading_count");
    assert!(out.get("file_id").is_none(), "medium tier does not buffer");
    let hint = out["hint"].as_str().unwrap();
    assert!(hint.contains("heading="), "hint must reference heading-nav recipe");
}
```

- [ ] **Step 5: Add failing test — large tier returns summary, no content body**

```rust
#[tokio::test]
async fn read_markdown_large_returns_summary_no_content() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("large.md");
    // Force byte size above INLINE_BYTE_BUDGET. Each "body line\n" = 10 bytes;
    // 10_000 lines ≈ 100KB, comfortably above typical INLINE_BYTE_BUDGET.
    std::fs::write(&file, synth_md(10_000, 20)).unwrap();

    let out = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    assert!(out.get("content").is_none(), "large tier must NOT include content");
    assert!(out.get("file_id").is_some(), "large tier buffers with file_id");
    assert!(out.get("heading_map").is_some(), "large tier includes heading_map");
    assert!(out.get("recipe").is_some(), "large tier includes recipe string");
    assert!(out.get("total_lines").is_some());
    assert!(out.get("total_bytes").is_some());
    assert!(out.get("heading_count").is_some());
    let recipe = out["recipe"].as_str().unwrap();
    assert!(recipe.contains("heading="), "recipe mentions heading nav");
}
```

- [ ] **Step 6: Run tests, verify all three fail**

Run: `cargo test --lib tools::markdown -- read_markdown_small_returns_full_content_no_hint read_markdown_medium_returns_content_with_hint read_markdown_large_returns_summary_no_content`
Expected: all three FAIL. Small may pass today for content-presence but the no-truncation assertion or exploring cap will trip it; medium and large will fail on missing `hint`/`recipe`/`file_id` shape differences.

If any passes already, investigate and adjust the test so it meaningfully exercises the new shape.

- [ ] **Step 7: Commit the failing tests**

```bash
git add src/tools/markdown.rs
git commit -m "test(markdown): failing tests for adaptive read_markdown tiers"
```

---

## Task 4: Implement three-tier logic in `read_markdown`

**Files:**
- Modify: `src/tools/markdown.rs` — default branch at the end of `ReadMarkdown::call` (grep for `// ── Default: heading map ─────` in the file to locate it).

- [ ] **Step 1: Replace the default branch**

Locate the comment `// ── Default: heading map ─────` inside `ReadMarkdown::call`. Replace everything from that comment to the end of the function body (but *before* the closing `}` of `call`) with:

```rust
        // ── Default branch: adaptive tiers ────────────────────────────────
        let total_bytes = text.len();
        let total_lines = text.lines().count();
        let oversized = crate::tools::exceeds_inline_limit(&text);

        let md_cov = super::file::markdown_coverage(&text, &resolved, ctx, None, None, None);

        // ── Tier 3: large — heading map + recipe, no body ─────────────────
        if oversized {
            let all_headings = crate::tools::file_summary::parse_all_headings(&text);
            let heading_count = all_headings.len();
            let heading_map: Vec<Value> = all_headings
                .iter()
                .map(|h| {
                    json!({
                        "level": h.level,
                        "text": h.text,
                        "line": h.line,
                    })
                })
                .collect();

            let file_id = ctx
                .output_buffer
                .store_file(resolved.to_string_lossy().to_string(), text.clone());

            let recipe = if heading_count == 0 {
                format!(
                    "{}-line markdown, no headings detected. \
                     Use read_markdown({:?}, start_line=N, end_line=M) for line slices.",
                    total_lines, file_id
                )
            } else {
                format!(
                    "{}-line markdown with {} sections. \
                     Use read_markdown({:?}, heading=\"## Section\") for one section, \
                     or read_markdown({:?}, start_line=N, end_line=M) for line slices.",
                    total_lines, heading_count, file_id, file_id
                )
            };

            let mut result = json!({
                "format": "markdown",
                "total_lines": total_lines,
                "total_bytes": total_bytes,
                "heading_count": heading_count,
                "heading_map": heading_map,
                "file_id": file_id,
                "recipe": recipe,
            });
            if let Some(c) = md_cov {
                result["coverage"] = c;
            }
            return Ok(result);
        }

        // ── Tier 2: medium — full content + soft hint ─────────────────────
        if total_lines > crate::tools::LINE_SOFT_CAP {
            let all_headings = crate::tools::file_summary::parse_all_headings(&text);
            let heading_count = all_headings.len();
            let hint = if heading_count == 0 {
                format!(
                    "{} lines, no headings. For focused reads: read_markdown(path, start_line=N, end_line=M).",
                    total_lines
                )
            } else {
                format!(
                    "{} lines, {} sections. For focused reads: read_markdown(path, heading=\"## Section\").",
                    total_lines, heading_count
                )
            };

            let mut result = json!({
                "format": "markdown",
                "content": text,
                "total_lines": total_lines,
                "heading_count": heading_count,
                "hint": hint,
            });
            if let Some(c) = md_cov {
                result["coverage"] = c;
            }
            return Ok(result);
        }

        // ── Tier 1: small — full content only ─────────────────────────────
        let mut result = json!({
            "format": "markdown",
            "content": text,
            "total_lines": total_lines,
        });
        if let Some(c) = md_cov {
            result["coverage"] = c;
        }
        Ok(result)
    }
```

Note: this replacement **removes** the old exploring-mode cap (`OutputGuard` / `OverflowInfo` use in the default branch) and the old `summarize_markdown` path. The `OutputGuard`/`OverflowInfo` imports at the top of `call` may become unused — Task 5 addresses that.

- [ ] **Step 2: Remove now-unused imports from `ReadMarkdown::call`**

At the top of `ReadMarkdown::call`, the line `use super::output::{OutputGuard, OutputMode, OverflowInfo};` becomes unused after Task 4 Step 1. Remove it.

- [ ] **Step 3: Run the three tier tests, verify pass**

Run: `cargo test --lib tools::markdown -- read_markdown_small_returns_full_content_no_hint read_markdown_medium_returns_content_with_hint read_markdown_large_returns_summary_no_content`
Expected: all three PASS.

- [ ] **Step 4: Run the full markdown test module, verify no regressions**

Run: `cargo test --lib tools::markdown`
Expected: all pass. Existing tests for heading nav, multi-heading, line-range behavior are unchanged by Task 4.

If any pre-existing test fails, the likely cause is an expectation on the old default-branch shape (e.g., `overflow` field). Read the test, decide whether it was pinning a behavior the spec explicitly changes; if yes, update the test assertion to match the new tier shape and note the change in the commit message.

- [ ] **Step 5: Commit**

```bash
git add src/tools/markdown.rs
git commit -m "feat(read_markdown): three-tier adaptive output (small/medium/large)

Small (<=LINE_SOFT_CAP lines, fits byte budget): full content only.
Medium (>LINE_SOFT_CAP lines, fits budget): full content + focused-read hint.
Large (exceeds byte budget): heading_map + stats + recipe + file_id, no body.

Removes the exploring-mode line cap from the default branch; byte budget is
now the sole hard ceiling for markdown reads."
```

---

## Task 5: Edge-case tests — empty file + no-headings large file

**Files:**
- Modify: `src/tools/markdown.rs` — add tests inside `mod tests`.

- [ ] **Step 1: Add failing test — empty markdown file**

```rust
#[tokio::test]
async fn read_markdown_empty_file_returns_small_tier() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("empty.md");
    std::fs::write(&file, "").unwrap();

    let out = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    assert_eq!(out["content"].as_str(), Some(""));
    assert_eq!(out["total_lines"].as_u64(), Some(0));
    assert!(out.get("hint").is_none());
    assert!(out.get("file_id").is_none());
}
```

- [ ] **Step 2: Add failing test — large file with no headings**

```rust
#[tokio::test]
async fn read_markdown_large_no_headings_recipe_pivots_to_line_ranges() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("flat.md");
    // ~100KB of plain lines, no headings.
    let content: String = (0..10_000).map(|i| format!("line {}\n", i)).collect();
    std::fs::write(&file, &content).unwrap();

    let out = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    assert!(out.get("file_id").is_some(), "still large tier");
    assert_eq!(out["heading_count"].as_u64(), Some(0));
    assert_eq!(out["heading_map"].as_array().map(|a| a.len()), Some(0));
    let recipe = out["recipe"].as_str().unwrap();
    assert!(
        recipe.contains("start_line") && recipe.contains("end_line"),
        "recipe must pivot to line ranges, got: {}",
        recipe
    );
    assert!(
        !recipe.contains("heading="),
        "recipe must not suggest heading nav when none exist, got: {}",
        recipe
    );
}
```

- [ ] **Step 3: Run, verify pass**

Run: `cargo test --lib tools::markdown -- read_markdown_empty_file_returns_small_tier read_markdown_large_no_headings_recipe_pivots_to_line_ranges`
Expected: PASS (logic from Task 4 already handles these branches).

- [ ] **Step 4: Commit**

```bash
git add src/tools/markdown.rs
git commit -m "test(markdown): edge-case coverage for empty and heading-less files"
```

---

## Task 6: Add `@file_*` buffer-ref support to `read_markdown`

**Files:**
- Modify: `src/tools/markdown.rs` — near the top of `ReadMarkdown::call`, before the `.md` extension gate.

- [ ] **Step 1: Write failing test — buffer ref + line range**

Add inside `mod tests`:

```rust
#[tokio::test]
async fn read_markdown_accepts_file_id_buffer_ref_for_line_range() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("large.md");
    std::fs::write(&file, synth_md(10_000, 20)).unwrap();

    // First call: populate the buffer via the large tier.
    let first = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let file_id = first["file_id"].as_str().unwrap().to_string();

    // Second call: read a line slice via the @file_* ref.
    let slice = super::ReadMarkdown
        .call(
            json!({ "path": file_id, "start_line": 1, "end_line": 5 }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(slice.get("content").is_some());
    let content = slice["content"].as_str().unwrap();
    assert!(content.lines().count() <= 5);
}

#[tokio::test]
async fn read_markdown_rejects_heading_nav_on_buffer_ref() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("large.md");
    std::fs::write(&file, synth_md(10_000, 20)).unwrap();

    let first = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let file_id = first["file_id"].as_str().unwrap().to_string();

    // heading= on a buffer ref should be rejected with a clear error.
    let err = super::ReadMarkdown
        .call(
            json!({ "path": file_id, "heading": "## Section 1" }),
            &ctx,
        )
        .await
        .err()
        .expect("heading on buffer ref must error");
    let msg = err.to_string();
    assert!(
        msg.contains("buffer") || msg.contains("@file_"),
        "error should mention buffer ref, got: {}",
        msg
    );
}
```

- [ ] **Step 2: Run tests, verify fail**

Run: `cargo test --lib tools::markdown -- read_markdown_accepts_file_id_buffer_ref_for_line_range read_markdown_rejects_heading_nav_on_buffer_ref`
Expected: both FAIL (current `read_markdown` rejects paths without `.md` extension).

- [ ] **Step 3: Implement buffer-ref branch**

At the top of `ReadMarkdown::call`, after `let path = super::require_str_param(&input, "path")?;` and **before** the `.md` extension gate (`if !path.ends_with(".md") ...`), insert:

```rust
        // Buffer-ref branch — serve line slices over @file_* refs.
        // Heading navigation is rejected: buffers are plain text, no structure.
        if path.starts_with("@file_") {
            let raw = ctx
                .output_buffer
                .get(path)
                .ok_or_else(|| {
                    RecoverableError::with_hint(
                        format!("buffer reference not found: '{}'", path),
                        "Buffer refs expire when the session resets. Re-run read_markdown on the file to get a fresh ref.",
                    )
                })?
                .stdout
                .clone();

            if input.get("heading").is_some() || input.get("headings").is_some() {
                return Err(RecoverableError::with_hint(
                    "heading navigation is not supported on @file_* buffer refs",
                    "Pass the original file path for heading navigation, or use start_line/end_line for line slices on the buffer ref.",
                )
                .into());
            }

            let start_line = optional_u64_param(&input, "start_line");
            let end_line = optional_u64_param(&input, "end_line");

            if start_line.is_some() != end_line.is_some() {
                return Err(RecoverableError::with_hint(
                    "both start_line and end_line are required",
                    "Provide both start_line and end_line for a line range, e.g. start_line=1, end_line=50",
                )
                .into());
            }

            let total_lines = raw.lines().count();
            let (content, total) = match (start_line, end_line) {
                (Some(s), Some(e)) => {
                    if s == 0 || e < s {
                        return Err(RecoverableError::with_hint(
                            format!(
                                "invalid line range: start_line={} end_line={} \
                                 (start_line must be >= 1 and end_line >= start_line)",
                                s, e
                            ),
                            "Lines are 1-indexed. Example: start_line=1, end_line=50",
                        )
                        .into());
                    }
                    (extract_lines(&raw, s as usize, e as usize), total_lines)
                }
                _ => (raw.clone(), total_lines),
            };

            return Ok(json!({
                "content": content,
                "total_lines": total,
            }));
        }
```

- [ ] **Step 4: Run buffer tests, verify pass**

Run: `cargo test --lib tools::markdown -- read_markdown_accepts_file_id_buffer_ref_for_line_range read_markdown_rejects_heading_nav_on_buffer_ref`
Expected: both PASS.

- [ ] **Step 5: Run full markdown suite**

Run: `cargo test --lib tools::markdown`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/markdown.rs
git commit -m "feat(read_markdown): accept @file_* buffer refs for line-range reads

Enables paginating over large-tier buffers via the same tool, matching
read_file symmetry. Heading nav is rejected on buffer refs with a
clear error hint."
```

---

## Task 7: Strip `mode=complete` from `read_file`

**Files:**
- Modify: `src/tools/file.rs` — remove `mode` schema, `is_complete_mode` branch, `read_complete_mode` helper, and related tests.

- [ ] **Step 1: Remove `mode` from the schema**

In `src/tools/file.rs`, locate the `input_schema` method of `ReadFile` (around line 25). Remove the `"mode"` property entry (lines ~36-40). The resulting `properties` block no longer mentions `mode`.

- [ ] **Step 2: Remove `is_complete_mode` logic in `call`**

Locate in `ReadFile::call` (around line 70):

```rust
        // Gate: redirect .md files to read_markdown (except mode=complete plan files)
        let is_complete_mode = input["mode"].as_str() == Some("complete");
        if !is_complete_mode && resolved.extension().is_some_and(|e| e == "md") {
            return Err(RecoverableError::with_hint(
                "Use read_markdown for markdown files",
                "read_markdown provides heading-based navigation for .md files.",
            )
            .into());
        }
```

Replace with:

```rust
        // Gate: redirect .md files to read_markdown
        if resolved.extension().is_some_and(|e| e == "md") {
            return Err(RecoverableError::with_hint(
                "Use read_markdown for markdown files",
                "read_markdown provides heading-based navigation, size-adaptive output, and buffer-ref slicing for .md files.",
            )
            .into());
        }
```

- [ ] **Step 3: Remove the `is_complete_mode` block further down**

Still in `ReadFile::call`, locate (around line 90-115):

```rust
        if is_complete_mode {
            if start_line.is_some()
                || end_line.is_some()
                || input["json_path"].is_string()
                || input["toml_key"].is_string()
            {
                return Err(RecoverableError::with_hint(
                    "mode=complete is mutually exclusive with start_line, end_line, json_path, toml_key",
                    "Use mode=complete alone to read the entire plan file.",
                )
                .into());
            }
            if !path.contains("/plans/") && !path.starts_with("plans/") {
                return Err(RecoverableError::with_hint(
                    "mode=complete is restricted to plan files (paths containing /plans/)",
                    "Use read_markdown for markdown files, or json_path/toml_key for structured data files.",
                )
                .into());
            }
            return read_complete_mode(path, text, &resolved, ctx);
        }
```

Delete this entire block.

- [ ] **Step 4: Remove the `read_complete_mode` helper**

Search for `fn read_complete_mode(` (around line 350). Delete the function and its doc comment.

If any other code in `file.rs` references `read_complete_mode`, the compiler will flag it. Expected: no other references.

- [ ] **Step 5: Remove `mode=complete` tests**

Locate the test section header `// ── ReadFile — mode=complete ───` (around line 4238). Delete from that header through the closing `}` of the final test in that section (around line 4430). Keep everything after that section intact.

- [ ] **Step 6: Build and test**

Run: `cargo build --lib`
Expected: success. Any compile errors indicate a missed reference; fix them.

Run: `cargo test --lib tools::file`
Expected: all remaining `tools::file` tests pass. The deleted `mode=complete` tests are gone, not failing.

- [ ] **Step 7: Commit**

```bash
git add src/tools/file.rs
git commit -m "refactor(read_file): remove mode=complete escape hatch

read_markdown's adaptive tiers cover the use case (full plan content
for small/medium plans; stats + recipe + file_id for large plans).
Simplifies the .md redirect hint to reflect the unified entry point."
```

---

## Task 8: Update prompt surfaces

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Modify: `src/tools/workflow.rs` — function `build_system_prompt_draft` (grep for it).

- [ ] **Step 1: Grep all three surfaces for stale references**

Run:

```bash
grep -n "mode=complete\|mode.*complete\|read_file.*heading\|read_file.*\.md" \
  src/prompts/server_instructions.md \
  src/prompts/onboarding_prompt.md \
  src/tools/workflow.rs
```

Expected: matches point to sections to update. Record the line numbers.

- [ ] **Step 2: Update `server_instructions.md`**

For every hit, rewrite the surrounding guidance to use `read_markdown` as the `.md` entry point. Typical pattern:

Before:

```
For markdown files, use read_file with heading= to navigate sections,
or mode=complete to read the whole plan.
```

After:

```
For markdown files, use read_markdown. It returns heading map + stats + recipe
for large files, full content + hint for medium files, and full content for
small files. Pass heading= or headings= to read specific sections, or
start_line/end_line for line slices (including over @file_* buffer refs).
```

Apply equivalent rewrites for every hit. Keep the section order and adjacent text intact (prompt caching).

- [ ] **Step 3: Update `onboarding_prompt.md`**

Same rewrite pattern as Step 2.

- [ ] **Step 4: Update `build_system_prompt_draft` in `workflow.rs`**

Open `src/tools/workflow.rs`. Find `fn build_system_prompt_draft` (grep for it). Apply the same rewrite pattern to any inline prompt strings that mention `mode=complete` or tell the agent to use `read_file` for markdown navigation.

- [ ] **Step 5: Build and test**

Run: `cargo build --lib && cargo test --lib workflow`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/prompts/server_instructions.md src/prompts/onboarding_prompt.md src/tools/workflow.rs
git commit -m "docs(prompts): align markdown guidance across all three surfaces

server_instructions, onboarding_prompt, and build_system_prompt_draft
now describe read_markdown's adaptive tiers and buffer-ref support,
and drop references to the removed read_file mode=complete escape hatch."
```

---

## Task 9: Bump `ONBOARDING_VERSION`

**Files:**
- Modify: `src/tools/workflow.rs` (line 15 and the assertion test around line 5743).

- [ ] **Step 1: Bump the constant**

In `src/tools/workflow.rs:15`, change:

```rust
const ONBOARDING_VERSION: u32 = 5;
```

to:

```rust
const ONBOARDING_VERSION: u32 = 6;
```

- [ ] **Step 2: Update the pin assertion test**

Around `src/tools/workflow.rs:5743`, change:

```rust
        assert_eq!(ONBOARDING_VERSION, 5);
```

to:

```rust
        assert_eq!(ONBOARDING_VERSION, 6);
```

- [ ] **Step 3: Run workflow tests**

Run: `cargo test --lib tools::workflow`
Expected: pass.

- [ ] **Step 4: Commit**

```bash
git add src/tools/workflow.rs
git commit -m "chore: bump ONBOARDING_VERSION to 6

Tool parameter semantics changed for read_markdown (three-tier output,
buffer-ref support) and read_file (mode=complete removed). Existing
onboarded projects will be prompted for a fresh system prompt."
```

---

## Task 10: Fix companion hook (separate repo)

**Files:**
- Modify: `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh` (lines ~184-196).

Note: this is a **separate repo**. Commit lands there, not in `code-explorer`.

- [ ] **Step 1: Open the hook script**

Read the `.md` branch of the `Read` case. The current text (verbatim from earlier grep):

```
WRONG TOOL. You called Read on a markdown file but codescout has HEADING-LEVEL NAVIGATION.

STOP. Do NOT read the full file: ${FILE_PATH}

Reading a full markdown file dumps all content into context — WASTEFUL when you need one section.
codescout read_file returns a STRUCTURAL SUMMARY with heading tree first, then lets you navigate:

  read_file("${REL_PATH}")                         — heading tree summary (see full structure instantly)
  read_file("${REL_PATH}", heading="## Section")  — jump directly to a named section
  search_pattern("pattern", path="${REL_PATH}")   — find specific content within the file

WORKFLOW: read_file first to see the heading tree → then read_file with heading= to get the section.
Do not call Read on markdown files.
```

- [ ] **Step 2: Replace with the new message**

Replace the block between the `enforce "` opening (line ~184) and its matching closing `"` (line ~196) with:

```
WRONG TOOL. You called Read on a markdown file but codescout has HEADING-LEVEL NAVIGATION.

STOP. Do NOT read the full file: ${FILE_PATH}

Reading a full markdown file dumps all content into context — WASTEFUL when you need one section.
Use read_markdown — size-adaptive output (full content for small files, content+hint for medium, heading map+recipe for large):

  read_markdown("${REL_PATH}")                            — adaptive output (start here)
  read_markdown("${REL_PATH}", heading="## Section")      — one section
  read_markdown("${REL_PATH}", headings=["## A", "## B"]) — multiple sections
  search_pattern("pattern", path="${REL_PATH}")           — content search

WORKFLOW: read_markdown first → heading=/headings= for specific sections → line ranges only as last resort.
Do not call Read on markdown files.
```

- [ ] **Step 3: Verify the shell syntax still parses**

Run: `bash -n /home/marius/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh`
Expected: no syntax errors.

- [ ] **Step 4: Manual smoke test**

Simulate a hook call with a `.md` Read:

```bash
echo '{"tool_name":"Read","tool_input":{"file_path":"'"$(pwd)"'/README.md"},"cwd":"'"$(pwd)"'"}' | \
  /home/marius/work/claude/claude-plugins/codescout-companion/hooks/pre-tool-guard.sh
```

(Run from the `code-explorer` working directory so `is_in_workspace` matches.) Expected: the new message prints; exit code reflects deny (the existing `enforce` helper decides — don't change it, only the text).

- [ ] **Step 5: Commit in the companion repo**

```bash
cd /home/marius/work/claude/claude-plugins/codescout-companion
git add hooks/pre-tool-guard.sh
git commit -m "fix(hooks): point .md Read redirect to read_markdown

read_file rejects .md files and routes to read_markdown. The old
guidance sent agents through a redundant second gate. Update to
reference read_markdown directly and mention its adaptive tiers."
cd /home/marius/work/claude/code-explorer
```

---

## Task 11: Final verification

**Files:** none modified.

- [ ] **Step 1: Full test suite**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
Expected: all pass. If any fails, fix before proceeding.

- [ ] **Step 2: Release build for MCP verification**

Run: `cargo build --release`
Expected: success. Required because the MCP server runs the release binary.

- [ ] **Step 3: Manual MCP verification**

Restart MCP in the host session: `/mcp` then reconnect codescout.

Manually exercise each tier:

```
read_markdown("README.md")                                          # small or medium tier
read_markdown("docs/superpowers/plans/2026-04-17-read-markdown-adaptive-tiers.md")  # this plan — medium tier
read_markdown(<some large markdown>)                                # large tier
```

For the large-tier call, copy the returned `file_id` and verify:

```
read_markdown("@file_xxx", start_line=1, end_line=50)               # buffer line slice
```

Also verify the removed escape hatch errors cleanly:

```
read_file("README.md")                                              # should error: Use read_markdown
```

- [ ] **Step 4: Companion hook smoke test via live session**

In a session with `codescout-companion` active and this codescout build loaded, trigger native `Read` on a `.md`:

```
Read("docs/superpowers/specs/2026-04-17-read-markdown-adaptive-tiers-design.md")
```

Expected: block with the new `read_markdown`-referencing message.

- [ ] **Step 5: Self-review the commits**

Run: `git log --oneline experiments ^master`
Expected: roughly one commit per task (plus Task 10's commit in the companion repo). Each commit is focused and independently testable.

- [ ] **Step 6: Report completion**

Summarize in conversation:
- All tests pass (`cargo fmt/clippy/test`)
- Release build succeeded
- All four tiers exercised via live MCP
- Companion hook emits the new message

---

## Done

All tasks complete when:
- All tier tests pass (small, medium, large, empty, no-headings).
- Buffer-ref tests pass (accept, reject-heading).
- `read_file` no longer has `mode` param or `mode=complete` code path.
- Prompt surfaces reference `read_markdown` consistently; no stale `mode=complete` mentions.
- `ONBOARDING_VERSION = 6`.
- Companion hook message points to `read_markdown`.
- Manual MCP verification confirms each tier renders correctly.
