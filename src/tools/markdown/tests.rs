use crate::tools::Tool;

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
    assert_eq!(out["lines"].as_u64(), Some(0));
    assert!(out.get("hint").is_none());
    assert!(out.get("file_id").is_none());
}

#[tokio::test]
async fn empty_file_returns_slim_shape() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.md");
    std::fs::write(&path, "").unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(serde_json::json!({"path": path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert_eq!(result.get("content").and_then(|v| v.as_str()), Some(""));
    assert_eq!(result.get("lines").and_then(|v| v.as_u64()), Some(0));
    assert!(
        result.get("format").is_none(),
        "expected no `format` field, got: {result}"
    );
    assert!(
        result.get("heading_count").is_none(),
        "expected no `heading_count` field, got: {result}"
    );
}

#[tokio::test]
async fn read_markdown_large_no_headings_hint_pivots_to_line_ranges() {
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
    assert_eq!(out["headings"].as_array().map(|a| a.len()), Some(0));
    let hint = out["hint"].as_str().unwrap();
    assert!(
        hint.contains("start_line"),
        "hint must mention start_line; got: {hint}"
    );
    assert!(
        !hint.contains("heading=\""),
        "hint must not suggest heading nav when there are no headings; got: {hint}"
    );
}

use super::edit_markdown::{find_consumed_subsections, perform_scoped_edit, perform_section_edit};

// ── perform_section_edit tests (moved from section_edit.rs) ──────────

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

#[tokio::test]
async fn read_markdown_small_returns_full_content_no_hint() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("small.md");
    // synth_md(30, 0) → only a "# Title" heading (1 heading total) → below the ≥2
    // threshold, so no nav hint is emitted.  The 2-section variant is covered by
    // `small_file_with_multiple_sections_gets_nav_hint`.
    std::fs::write(&file, synth_md(30, 0)).unwrap();

    let out = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    assert!(
        out.get("content").is_some(),
        "small tier must include content"
    );
    assert!(
        out.get("hint").is_none(),
        "small tier must not include hint when fewer than 2 sections"
    );
    assert!(out.get("file_id").is_none(), "small tier must not buffer");
    assert!(
        out.get("heading_map").is_none(),
        "small tier has no heading_map"
    );
    assert!(
        out.get("heading_count").is_none(),
        "small tier must not report heading_count (dropped in B4)"
    );
}

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
    assert!(out.get("lines").is_some(), "medium tier reports line count");
    assert!(out.get("file_id").is_none(), "medium tier does not buffer");
    let hint = out["hint"].as_str().unwrap();
    assert!(
        hint.contains("heading="),
        "hint must reference heading-nav recipe"
    );
}

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

    assert!(
        out.get("content").is_none(),
        "large tier must NOT include content"
    );
    assert!(
        out.get("file_id").is_some(),
        "large tier buffers with file_id"
    );
    assert!(
        out.get("headings").is_some(),
        "large tier includes headings"
    );
    assert!(out.get("hint").is_some(), "large tier includes hint");
    assert!(out.get("lines").is_some());
    let hint = out["hint"].as_str().unwrap();
    assert!(hint.contains("heading="), "hint mentions heading nav");
    assert!(
        hint.contains("@file_"),
        "hint references the file_id to steer reuse; got: {hint}"
    );
}

#[tokio::test]
async fn read_markdown_large_includes_hint_referencing_file_id() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("big.md");
    std::fs::write(&file, synth_md(10_000, 20)).unwrap();

    let out = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();

    let hint = out["hint"]
        .as_str()
        .expect("large-tier response must include hint");
    assert!(
        hint.contains("@file_"),
        "hint must reference the file_id to steer reuse, got: {hint}"
    );
}

#[tokio::test]
async fn heading_on_large_section_returns_ok_false_with_hint_and_section_map() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("big.md");
    // One H1 containing many H2 subsections — H1 match will be oversized.
    let mut body = String::from("# Root\n\n");
    for i in 0..200 {
        body.push_str(&format!("## Sub {i}\n\n"));
        body.push_str(&"word ".repeat(500));
        body.push_str("\n\n");
    }
    std::fs::write(&file, &body).unwrap();

    let err = super::ReadMarkdown
        .call(
            json!({ "path": file.to_str().unwrap(), "heading": "# Root" }),
            &ctx,
        )
        .await
        .unwrap_err();

    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("oversized heading must be RecoverableError (isError:false)");
    assert!(
        rec.message.contains("too large") || rec.message.contains("exceeds"),
        "error message should explain oversize; got: {}",
        rec.message
    );
    let hint = rec.hint().expect("expected Hint guidance");
    assert!(
        hint.contains("@file_") || hint.contains("section_map") || hint.contains("start_line"),
        "hint must steer to file_id/section_map/line-range; got: {hint}"
    );
    assert!(
        rec.extra.get("file_id").is_some(),
        "extra must include file_id for subsequent buffer-ref reads"
    );
    let sm = rec
        .extra
        .get("section_map")
        .expect("extra must include nested section_map");
    let arr = sm.as_array().expect("section_map is an array");
    assert!(
        !arr.is_empty(),
        "section_map must list nested sub-headings (H2s under H1)"
    );
    let first = &arr[0];
    assert!(
        first.get("h").is_some() && first.get("l").is_some(),
        "section_map entries must use {{h, l}} shape; got: {first}"
    );
    assert!(
        rec.extra.get("next_actions").is_some(),
        "extra must include concrete next_actions"
    );
}

// ── BUG-043: subsection-consumption detection ──────────────────────────

/// `find_consumed_subsections` returns empty when the section has no nested
/// sub-headings — safe to `replace` without losing structure.
#[test]
fn find_consumed_subsections_empty_for_leaf_section() {
    let content = "# Title\n## Setup\nsome content\n## Usage\nuse it\n";
    let result = find_consumed_subsections(content, "## Setup").unwrap();
    assert!(
        result.is_empty(),
        "leaf section has no subsections to consume: {result:?}"
    );
}

/// Core BUG-043 repro: `## File Map` is the only level-2 heading and is
/// followed only by level-3 task headings. Its section extends to EOF,
/// so `replace` would wipe every `###` task. `find_consumed_subsections`
/// must return those `###` headings so the tool can refuse the edit.
#[test]
fn find_consumed_subsections_lists_level3_children_under_level2() {
    let content = "\
# Plan
intro

## File Map
map body

### Task A
work
### Task B
more work
### Task C
even more
";
    let result = find_consumed_subsections(content, "## File Map").unwrap();
    assert_eq!(
        result,
        vec![
            "### Task A".to_string(),
            "### Task B".to_string(),
            "### Task C".to_string(),
        ],
        "must list every heading that would be wiped by replace"
    );
}

/// Sibling `##` heading is NOT a child — doesn't count as consumed.
#[test]
fn find_consumed_subsections_stops_at_sibling_heading() {
    let content = "# Title\n## Setup\n### Step 1\ndo it\n## Usage\nuse it\n";
    let result = find_consumed_subsections(content, "## Setup").unwrap();
    assert_eq!(
        result,
        vec!["### Step 1".to_string()],
        "only the ### under ## Setup is consumed; ## Usage is a sibling"
    );
}

#[test]
fn replace_body_only() {
    let content = "# Title\n## Setup\nold content\nmore old\n## Usage\nuse it\n";
    let result =
        perform_section_edit(content, "## Setup", "replace", Some("new content\n")).unwrap();
    assert_eq!(
        result,
        "# Title\n## Setup\n\nnew content\n## Usage\nuse it\n"
    );
}

#[test]
fn replace_with_heading() {
    let content = "# Title\n## Setup\nold content\n## Usage\nuse it\n";
    let result = perform_section_edit(
        content,
        "## Setup",
        "replace",
        Some("## Installation\nnew steps\n"),
    )
    .unwrap();
    assert_eq!(
        result,
        "# Title\n## Installation\nnew steps\n## Usage\nuse it\n"
    );
}

#[test]
fn replace_empty_section() {
    let content = "# Title\n## Empty\n## Next\nstuff\n";
    let result =
        perform_section_edit(content, "## Empty", "replace", Some("now has content\n")).unwrap();
    assert_eq!(
        result,
        "# Title\n## Empty\n\nnow has content\n## Next\nstuff\n"
    );
}

#[test]
fn insert_before() {
    let content = "# Title\n## Setup\ncontent\n";
    let result = perform_section_edit(
        content,
        "## Setup",
        "insert_before",
        Some("## Prerequisites\ninstall stuff\n"),
    )
    .unwrap();
    assert_eq!(
        result,
        "# Title\n## Prerequisites\ninstall stuff\n## Setup\ncontent\n"
    );
}

#[test]
fn insert_after() {
    let content = "# Title\n## Setup\ncontent\n## Usage\nuse it\n";
    let result = perform_section_edit(
        content,
        "## Setup",
        "insert_after",
        Some("\n## Testing\ntest it\n"),
    )
    .unwrap();
    assert_eq!(
        result,
        "# Title\n## Setup\ncontent\n\n## Testing\ntest it\n## Usage\nuse it\n"
    );
}

#[test]
fn remove_section() {
    let content = "# Title\n## Setup\ncontent\n\n## Usage\nuse it\n";
    let result = perform_section_edit(content, "## Setup", "remove", None).unwrap();
    assert_eq!(result, "# Title\n## Usage\nuse it\n");
}

#[test]
fn remove_last_section() {
    let content = "# Title\n## Setup\ncontent\n";
    let result = perform_section_edit(content, "## Setup", "remove", None).unwrap();
    assert_eq!(result, "# Title\n");
}

#[test]
fn nested_section_replace() {
    let content = "# Title\n## Parent\nparent text\n### Child\nchild text\n## Sibling\nsibling\n";
    let result =
        perform_section_edit(content, "## Parent", "replace", Some("replaced all\n")).unwrap();
    assert_eq!(
        result,
        "# Title\n## Parent\n\nreplaced all\n## Sibling\nsibling\n"
    );
}

#[test]
fn trailing_newline_normalization() {
    let content = "# Title\n## Setup\ncontent";
    let result = perform_section_edit(content, "## Setup", "replace", Some("new")).unwrap();
    assert!(
        result.ends_with('\n'),
        "result should end with newline: {:?}",
        result
    );
}

#[test]
fn replace_body_preserves_blank_line_after_heading() {
    let content = "# Title\n\n## Goals\n\n- item 1\n- item 2\n\n## Next\n\nmore\n";
    let result = perform_section_edit(content, "Goals", "replace", Some("- new item\n")).unwrap();
    assert!(
        result.contains("## Goals\n\n- new item\n"),
        "should have blank line between heading and body: {:?}",
        result
    );
}

#[test]
fn replace_body_no_double_blank_when_content_starts_with_newline() {
    let content = "# Title\n\n## Goals\n\n- item 1\n";
    let result = perform_section_edit(content, "Goals", "replace", Some("\n- new item\n")).unwrap();
    assert!(
        result.contains("## Goals\n\n- new item\n"),
        "should not produce double blank line: {:?}",
        result
    );
    assert!(
        !result.contains("## Goals\n\n\n"),
        "must not have triple newline: {:?}",
        result
    );
}

#[test]
fn remove_only_section() {
    let content = "## Only\ncontent\n";
    let result = perform_section_edit(content, "## Only", "remove", None).unwrap();
    assert!(result.trim().is_empty() || result == "\n");
}

#[test]
fn consecutive_edits() {
    let content = "# Title\n## A\noriginal a\n## B\noriginal b\n";
    let after_first =
        perform_section_edit(content, "## A", "replace", Some("updated a\n")).unwrap();
    assert!(after_first.contains("updated a"));
    let after_second =
        perform_section_edit(&after_first, "## B", "replace", Some("updated b\n")).unwrap();
    assert!(after_second.contains("updated a"));
    assert!(after_second.contains("updated b"));
}

#[test]
fn smart_replace_detection_non_heading() {
    let content = "# Title\n## Setup\nold content\n";
    let result =
        perform_section_edit(content, "## Setup", "replace", Some("#hashtag comment\n")).unwrap();
    assert!(result.contains("## Setup"));
    assert!(result.contains("#hashtag comment"));
}

#[test]
fn heading_inside_code_block_edit() {
    // A heading inside a fenced code block is part of the section body,
    // so replacing the section should consume it.
    let content = "# Title\n## Real\ncontent\n```\n## Fake\n```\n";
    let result =
        perform_section_edit(content, "## Real", "replace", Some("new content\n")).unwrap();
    assert!(result.contains("## Real"));
    assert!(result.contains("new content"));
    // ## Fake is inside a code block — it's part of ## Real's body and gets replaced
    assert!(
        !result.contains("## Fake"),
        "code block content should be replaced as part of the section body"
    );
}

/// Regression: a level-1 heading inside a fenced code block must NOT split a
/// level-2 section boundary. Without code-block tracking in `compute_section_end`,
/// the `# comment` line would be treated as a section boundary, leaving a stray
/// tail and corrupting the document.
#[test]
fn code_block_heading_different_level_does_not_split_section() {
    let content =
        "# Title\n## Section\ntext\n```bash\n# not a heading\nmore code\n```\n## Next\nstuff\n";
    let result =
        perform_section_edit(content, "## Section", "replace", Some("replaced\n")).unwrap();
    assert!(result.contains("## Section"));
    assert!(result.contains("replaced"));
    assert!(result.contains("## Next"));
    assert!(result.contains("stuff"));
    // The code block content must be consumed as part of ## Section's body
    assert!(
        !result.contains("# not a heading"),
        "code block content should have been replaced along with the section body"
    );
}

/// Regression: `insert_after` on a section whose body contains a fenced code
/// block with a higher-level heading must insert AFTER the code block, not
/// in the middle of it.
#[test]
fn insert_after_section_with_code_block_heading() {
    let content = "## Reading\n```bash\n# shell comment\nls -la\n```\n## Next\ntext\n";
    let result = perform_section_edit(
        content,
        "## Reading",
        "insert_after",
        Some("## Inserted\nnew section\n"),
    )
    .unwrap();
    // The inserted section should appear between ## Reading and ## Next
    let reading_pos = result.find("## Reading").unwrap();
    let inserted_pos = result.find("## Inserted").unwrap();
    let next_pos = result.find("## Next").unwrap();
    assert!(
            reading_pos < inserted_pos && inserted_pos < next_pos,
            "## Inserted should be between ## Reading and ## Next, got positions: reading={reading_pos}, inserted={inserted_pos}, next={next_pos}"
        );
    // The code block should remain intact inside ## Reading
    assert!(result.contains("# shell comment"));
}

#[test]
fn duplicate_heading_edit_error() {
    let content = "# Title\n## Example\nfirst\n## Other\n## Example\nsecond\n";
    let err = perform_section_edit(content, "## Example", "replace", Some("x")).unwrap_err();
    assert!(
        err.to_string().contains("found") && err.to_string().contains("times"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn heading_not_found() {
    let content = "# Title\n## Setup\ntext";
    let err = perform_section_edit(content, "## Nonexistent", "replace", Some("x")).unwrap_err();
    assert!(
        err.to_string().contains("not found"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn missing_content_for_replace() {
    let content = "# Title\n## Setup\ntext";
    let err = perform_section_edit(content, "## Setup", "replace", None).unwrap_err();
    assert!(
        err.to_string().contains("content"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn invalid_action() {
    let content = "# Title\n## Setup\ntext";
    let err = perform_section_edit(content, "## Setup", "invalid", Some("x")).unwrap_err();
    assert!(
        err.to_string().contains("invalid"),
        "unexpected error: {}",
        err
    );
}

// ── perform_scoped_edit tests (action="edit") ────────────────────────

#[test]
fn scoped_edit_first_occurrence() {
    let content = "# Title\n## Setup\nfoo bar foo\nmore foo\n## Next\nfoo\n";
    let result = perform_scoped_edit(content, "## Setup", "foo", "baz", false).unwrap();
    assert_eq!(
        result,
        "# Title\n## Setup\nbaz bar foo\nmore foo\n## Next\nfoo\n"
    );
}

#[test]
fn scoped_edit_replace_all() {
    let content = "# Title\n## Setup\nfoo bar foo\nmore foo\n## Next\nfoo\n";
    let result = perform_scoped_edit(content, "## Setup", "foo", "baz", true).unwrap();
    assert_eq!(
        result,
        "# Title\n## Setup\nbaz bar baz\nmore baz\n## Next\nfoo\n"
    );
}

#[test]
fn scoped_edit_not_found() {
    let content = "# Title\n## Setup\ncontent\n";
    let err = perform_scoped_edit(content, "## Setup", "nonexistent", "x", false).unwrap_err();
    assert!(
        err.to_string().contains("not found"),
        "unexpected error: {}",
        err
    );
}

#[test]
fn scoped_edit_does_not_affect_other_sections() {
    let content = "# Title\n## A\nhello world\n## B\nhello world\n";
    let result = perform_scoped_edit(content, "## A", "hello", "goodbye", false).unwrap();
    assert!(result.contains("## A\ngoodbye world"));
    assert!(result.contains("## B\nhello world"));
}

#[test]
fn scoped_edit_empty_replacement() {
    let content = "# Title\n## Setup\nremove this word\n";
    let result = perform_scoped_edit(content, "## Setup", " this", "", false).unwrap();
    assert_eq!(result, "# Title\n## Setup\nremove word\n");
}

// ── batch mode tests ────────────────────────────────────────────────

#[test]
fn batch_replace_two_sections() {
    let content = "# Title\n## A\nold a\n## B\nold b\n";
    let after_a = perform_section_edit(content, "## A", "replace", Some("new a\n")).unwrap();
    let after_b = perform_section_edit(&after_a, "## B", "replace", Some("new b\n")).unwrap();
    assert!(after_b.contains("new a"));
    assert!(after_b.contains("new b"));
}

#[test]
fn batch_mixed_actions() {
    let content = "# Title\n## A\ncontent a\n## B\ncontent b\n## C\ncontent c\n";
    let step1 = perform_section_edit(content, "## A", "replace", Some("updated a\n")).unwrap();
    let step2 = perform_section_edit(&step1, "## B", "remove", None).unwrap();
    let step3 = perform_section_edit(
        &step2,
        "## C",
        "insert_after",
        Some("\n## D\nnew section\n"),
    )
    .unwrap();
    assert!(step3.contains("updated a"));
    assert!(!step3.contains("## B"));
    assert!(step3.contains("## D\nnew section"));
}

#[test]
fn batch_edit_action() {
    let content = "# Title\n## A\nhello world\n## B\nhello world\n";
    let result = perform_scoped_edit(content, "## A", "hello", "goodbye", false).unwrap();
    let result = perform_scoped_edit(&result, "## B", "hello", "hi", false).unwrap();
    assert!(result.contains("goodbye world"));
    assert!(result.contains("hi world"));
}

// ── fenced code block edge cases ────────────────────────────────────

/// Multiple code blocks in a single section — all must be part of the section body.
#[test]
fn multiple_code_blocks_in_section() {
    let content = concat!(
        "# Title\n",
        "## Setup\n",
        "First block:\n",
        "```bash\n",
        "# install deps\n",
        "apt install foo\n",
        "```\n",
        "Second block:\n",
        "```python\n",
        "# run script\n",
        "import sys\n",
        "```\n",
        "## Next\n",
        "other\n",
    );
    let result =
        perform_section_edit(content, "## Setup", "replace", Some("simplified\n")).unwrap();
    assert!(result.contains("## Setup"));
    assert!(result.contains("simplified"));
    assert!(result.contains("## Next"));
    assert!(!result.contains("# install deps"));
    assert!(!result.contains("# run script"));
}

/// Code block with language tag — the ``` fence detection must work with ```bash, ```python, etc.
#[test]
fn code_block_with_language_tag() {
    let content = "## Sec\n```rust\n// # Not a heading\nfn main() {}\n```\n## Next\ntext\n";
    let result = perform_section_edit(content, "## Sec", "replace", Some("new\n")).unwrap();
    assert!(result.contains("## Sec"));
    assert!(result.contains("## Next"));
    assert!(!result.contains("fn main"));
}

/// Section whose entire body is a code block.
#[test]
fn section_body_is_entirely_code_block() {
    let content = "## Code\n```\n# heading-like\nsome code\n```\n## After\ntext\n";
    let result = perform_section_edit(content, "## Code", "replace", Some("replaced\n")).unwrap();
    assert_eq!(result, "## Code\n\nreplaced\n## After\ntext\n");
}

/// Code block at the very end of the file (last section, code block is last content).
#[test]
fn code_block_at_end_of_file() {
    let content = "# Title\n## Last\ntext\n```\n# inside fence\ncode\n```\n";
    let result = perform_section_edit(content, "## Last", "replace", Some("new last\n")).unwrap();
    assert!(result.contains("new last"));
    assert!(!result.contains("# inside fence"));
    assert!(result.ends_with('\n'));
}

/// Unclosed code fence — everything after ``` to EOF is "inside" the code block.
/// The section boundary should extend to EOF since no real heading follows.
#[test]
fn unclosed_code_fence() {
    let content = "# Title\n## Broken\ntext\n```\n# looks like heading\ncode\n";
    let result = perform_section_edit(content, "## Broken", "replace", Some("fixed\n")).unwrap();
    assert!(result.contains("fixed"));
    // The unclosed fence content is part of the section — gets replaced
    assert!(!result.contains("# looks like heading"));
}

/// Multiple `#` levels inside a single code block — none should act as boundaries.
#[test]
fn multiple_heading_levels_inside_code_block() {
    let content = concat!(
        "## Section\n",
        "```markdown\n",
        "# H1 inside\n",
        "## H2 inside\n",
        "### H3 inside\n",
        "```\n",
        "## Real Next\n",
        "content\n",
    );
    let result = perform_section_edit(content, "## Section", "replace", Some("clean\n")).unwrap();
    assert!(result.contains("clean"));
    assert!(result.contains("## Real Next"));
    assert!(!result.contains("# H1 inside"));
    assert!(!result.contains("## H2 inside"));
    assert!(!result.contains("### H3 inside"));
}

/// Consecutive code fences with no content between them.
#[test]
fn consecutive_code_fences() {
    let content = "## Sec\n```\n# a\n```\n```\n# b\n```\n## Next\ntext\n";
    let result = perform_section_edit(content, "## Sec", "replace", Some("new\n")).unwrap();
    assert!(result.contains("## Next"));
    assert!(!result.contains("# a"));
    assert!(!result.contains("# b"));
}

/// Insert_before a section that is preceded by a code block ending.
#[test]
fn insert_before_section_after_code_block() {
    let content = "## First\ntext\n```\n# comment\n```\n## Second\nmore\n";
    let result = perform_section_edit(
        content,
        "## Second",
        "insert_before",
        Some("## Middle\ninserted\n"),
    )
    .unwrap();
    let first_pos = result.find("## First").unwrap();
    let middle_pos = result.find("## Middle").unwrap();
    let second_pos = result.find("## Second").unwrap();
    assert!(first_pos < middle_pos && middle_pos < second_pos);
}

/// Remove a section whose body contains code blocks.
#[test]
fn remove_section_with_code_blocks() {
    let content =
        "# Title\n## Keep\nkept\n## Remove\ntext\n```\n# fake\ncode\n```\n## Also Keep\nstuff\n";
    let result = perform_section_edit(content, "## Remove", "remove", None).unwrap();
    assert!(result.contains("## Keep"));
    assert!(result.contains("kept"));
    assert!(result.contains("## Also Keep"));
    assert!(result.contains("stuff"));
    assert!(!result.contains("## Remove"));
    assert!(!result.contains("# fake"));
}

/// Scoped edit (action="edit") within a section that has code blocks —
/// the old_string/new_string should work on the full section body including code blocks.
#[test]
fn scoped_edit_in_section_with_code_block() {
    let content =
            "## Config\nSet `foo=bar` in config.\n```toml\n# main config\nfoo = \"bar\"\n```\n## Next\ntext\n";
    let result = perform_scoped_edit(content, "## Config", "foo", "baz", true).unwrap();
    assert!(result.contains("Set `baz=bar`"));
    assert!(result.contains("baz = \"bar\""));
    // Should not touch ## Next
    assert!(result.contains("## Next\ntext"));
}

// ── heading matching edge cases ─────────────────────────────────────

/// Heading with inline code backticks — the tool should match via stripped formatting.
#[test]
fn heading_with_backtick_code() {
    let content = "# Title\n## The `auth` Module\ncontent\n## Other\ntext\n";
    // Query without backticks should match via strip_inline_formatting
    let result =
        perform_section_edit(content, "## The auth Module", "replace", Some("new\n")).unwrap();
    assert!(result.contains("new"));
    assert!(result.contains("## Other"));
}

/// Heading with bold formatting — matched via stripping.
#[test]
fn heading_with_bold_formatting() {
    let content = "# Title\n## **Important** Notes\ncontent\n";
    let result =
        perform_section_edit(content, "## Important Notes", "replace", Some("updated\n")).unwrap();
    assert!(result.contains("updated"));
}

/// Prefix match — partial heading should match.
#[test]
fn heading_prefix_match() {
    let content = "# Title\n## Installation and Setup Guide\ncontent\n";
    let result =
        perform_section_edit(content, "## Installation", "replace", Some("simplified\n")).unwrap();
    assert!(result.contains("simplified"));
}

// ── boundary conditions ─────────────────────────────────────────────

/// Section with only whitespace lines as body.
#[test]
fn section_with_whitespace_only_body() {
    let content = "# Title\n## Empty-ish\n\n\n\n## Next\ncontent\n";
    let result =
        perform_section_edit(content, "## Empty-ish", "replace", Some("now has stuff\n")).unwrap();
    assert!(result.contains("now has stuff"));
    assert!(result.contains("## Next"));
}

/// Replace the top-level `#` heading — its section spans to EOF (or next `#`),
/// so all child sections (##, ###, etc.) are part of its body and get replaced.
#[test]
fn replace_top_level_heading_consumes_children() {
    let content = "# Title\nintro text\n## Child\nchild text\n";
    let result = perform_section_edit(content, "# Title", "replace", Some("new intro\n")).unwrap();
    assert!(result.contains("new intro"));
    // ## Child is a subsection of # Title — it gets replaced too
    assert!(
        !result.contains("## Child"),
        "child section should be consumed by parent replace"
    );
}

/// Insert after the last section in the document.
#[test]
fn insert_after_last_section() {
    let content = "# Title\n## Only\ncontent\n";
    let result = perform_section_edit(
        content,
        "## Only",
        "insert_after",
        Some("\n## Appended\nnew stuff\n"),
    )
    .unwrap();
    assert!(result.contains("## Only\ncontent"));
    assert!(result.contains("## Appended\nnew stuff"));
}

/// Deeply nested section (###) inside a ## section — replace ## consumes ### children.
#[test]
fn replace_consumes_nested_children() {
    let content = "# Title\n## Parent\ntext\n### Child1\nc1\n### Child2\nc2\n## Sibling\nother\n";
    let result = perform_section_edit(content, "## Parent", "replace", Some("flat now\n")).unwrap();
    assert!(result.contains("flat now"));
    assert!(result.contains("## Sibling"));
    assert!(!result.contains("### Child1"));
    assert!(!result.contains("### Child2"));
}

/// Code block inside a nested ### section — replace of parent ## should consume everything.
#[test]
fn code_block_inside_nested_child_consumed_by_parent_replace() {
    let content = concat!(
        "## Parent\n",
        "intro\n",
        "### Child\n",
        "```bash\n",
        "# shell comment\n",
        "echo hello\n",
        "```\n",
        "## Next\n",
        "other\n",
    );
    let result = perform_section_edit(content, "## Parent", "replace", Some("replaced\n")).unwrap();
    assert!(result.contains("replaced"));
    assert!(result.contains("## Next"));
    assert!(!result.contains("### Child"));
    assert!(!result.contains("# shell comment"));
}

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

    // Second call: use the buffer ref for a line slice.
    let slice = super::ReadMarkdown
        .call(
            json!({ "path": file_id, "start_line": 1, "end_line": 5 }),
            &ctx,
        )
        .await
        .unwrap();

    let content = slice["content"].as_str().unwrap();
    assert!(content.lines().count() <= 5);
}

#[tokio::test]
async fn buffer_ref_accepts_single_heading_nav() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("big.md");
    std::fs::write(&file, synth_md(10_000, 20)).unwrap();

    let first = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let fid = first["file_id"].as_str().unwrap().to_string();

    let second = super::ReadMarkdown
        .call(json!({ "path": fid, "heading": "## Section 5" }), &ctx)
        .await
        .unwrap();
    assert!(
        second.get("content").is_some() || second.get("file_id").is_some(),
        "heading nav on @file_* must return content or a nested buffer, got: {second}"
    );
}

#[tokio::test]
async fn buffer_ref_accepts_multi_heading_nav() {
    let ctx = test_ctx().await;
    let dir = tempdir().unwrap();
    let file = dir.path().join("big.md");
    // 500 sections keeps each section small (~20 lines) so combining two
    // doesn't overflow the inline limit, while the file total (~100KB) still
    // triggers Tier-3 and returns a file_id.
    std::fs::write(&file, synth_md(10_000, 500)).unwrap();

    let first = super::ReadMarkdown
        .call(json!({ "path": file.to_str().unwrap() }), &ctx)
        .await
        .unwrap();
    let fid = first["file_id"].as_str().unwrap().to_string();

    let second = super::ReadMarkdown
        .call(
            json!({
                "path": fid,
                "headings": ["## Section 3", "## Section 5"],
            }),
            &ctx,
        )
        .await
        .unwrap();
    let content = second["content"].as_str().expect("content present");
    assert!(content.contains("## Section 3") && content.contains("## Section 5"));
}

#[tokio::test]
async fn many_headings_escalates_to_map_shape_even_when_bytes_fit() {
    // Spec B1: a file with > HEADINGS_HARD_CAP (40) sections must not return
    // full content even if byte budget is satisfied. We use 41 sections with
    // minimal bodies so the file stays well under INLINE_BYTE_BUDGET (~9 KB)
    // but exceeds the heading-count gate. Without the fix, this hits Tier 2
    // and returns full content; with the fix it escalates to Tier 3 MAP shape.
    let body = synth_md(205, 41); // ~41 sections, ~205 lines, ~1.8 KB
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.md");
    std::fs::write(&path, &body).unwrap();

    let ctx = test_ctx().await;
    let result = super::ReadMarkdown
        .call(serde_json::json!({"path": path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(
        result.get("content").is_none(),
        "expected MAP shape (no content), got: {result}"
    );
    assert!(
        result.get("headings").is_some() || result.get("heading_map").is_some(),
        "expected headings array, got: {result}"
    );
    assert!(
        result.get("file_id").is_some(),
        "expected file_id for MAP shape, got: {result}"
    );
}

#[tokio::test]
async fn line_range_past_eof_returns_recoverable_error() {
    let body = "# Tiny\n\nbody\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tiny.md");
    std::fs::write(&path, body).unwrap();

    let ctx = test_ctx().await;
    let result = super::ReadMarkdown
        .call(
            serde_json::json!({
                "path": path.to_str().unwrap(),
                "start_line": 9000,
                "end_line": 9999,
            }),
            &ctx,
        )
        .await;

    let err = result.expect_err("expected RecoverableError for OOR slice");
    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("expected RecoverableError");
    assert!(
        rec.message.contains("start_line"),
        "expected start_line in message, got: {}",
        rec.message
    );
    assert_eq!(
        rec.extra.get("lines").and_then(|v| v.as_u64()),
        Some(3),
        "expected lines=3 in extra, got: {:?}",
        rec.extra
    );
}

#[tokio::test]
async fn bogus_heading_error_carries_headings_array() {
    let body = "# A\n\n## B\n\n## C\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.md");
    std::fs::write(&path, body).unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(
            serde_json::json!({
                "path": path.to_str().unwrap(),
                "heading": "## Nonexistent",
            }),
            &ctx,
        )
        .await;

    let err = result.expect_err("expected RecoverableError");
    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("expected RecoverableError");
    let headings = rec
        .extra
        .get("headings")
        .and_then(|v| v.as_array())
        .expect("expected `headings` array in extra");
    assert_eq!(headings.len(), 3, "expected 3 headings, got: {headings:?}");
    let first = &headings[0];
    assert_eq!(first.get("h").and_then(|v| v.as_str()), Some("# A"));
    assert_eq!(first.get("l").and_then(|v| v.as_u64()), Some(1));
}

#[tokio::test]
async fn small_file_with_multiple_sections_gets_nav_hint() {
    let body = "# A\n\nbody\n\n## B\n\nmore\n\n## C\n\nend\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.md");
    std::fs::write(&path, body).unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(serde_json::json!({"path": path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    let hint = result
        .get("hint")
        .and_then(|v| v.as_str())
        .expect("expected nav hint for small file with ≥2 sections");
    assert!(
        hint.contains("heading"),
        "hint should mention heading argument, got: {hint}"
    );
    assert!(
        hint.contains("3 sections"),
        "hint should mention section count, got: {hint}"
    );
}

#[tokio::test]
async fn small_file_with_no_sections_has_no_nav_hint() {
    let body = "plain text\nno headings\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flat.md");
    std::fs::write(&path, body).unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(serde_json::json!({"path": path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert!(
        result.get("hint").is_none(),
        "expected no hint when no headings exist, got: {result}"
    );
}

#[tokio::test]
async fn tier3_map_shape_fields_are_canonical() {
    // Spec MAP shape: {lines, headings:[{h,l}], file_id, hint}
    // Forbidden: format, total_lines, total_bytes, heading_count,
    // heading_map, must_follow, sections_returned.
    let body = synth_md(5000, 50); // forces Tier 3 via byte budget
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.md");
    std::fs::write(&path, &body).unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(serde_json::json!({"path": path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    for forbidden in [
        "format",
        "total_lines",
        "total_bytes",
        "heading_count",
        "heading_map",
        "must_follow",
        "sections_returned",
    ] {
        assert!(
            result.get(forbidden).is_none(),
            "forbidden field `{forbidden}` present in MAP response: {result}"
        );
    }
    assert!(result.get("lines").is_some(), "expected `lines`");
    assert!(result.get("headings").is_some(), "expected `headings`");
    assert!(result.get("file_id").is_some(), "expected `file_id`");
    assert!(result.get("hint").is_some(), "expected `hint`");

    let first = &result["headings"][0];
    assert!(
        first.get("h").is_some() && first.get("l").is_some(),
        "expected heading entry shape {{h, l}}, got: {first}"
    );
    assert!(
        first.get("level").is_none() && first.get("text").is_none() && first.get("line").is_none(),
        "expected old fields absent, got: {first}"
    );
}

// ── format_compact CONTENT shape ─────────────────────────────────────────────

#[test]
fn format_compact_content_passthrough_with_hint_footer() {
    use crate::tools::Tool;
    let response = serde_json::json!({
        "content": "# Hi\n\nbody\n",
        "lines": 3,
        "hint": "3 lines, 2 sections — read_markdown(path, heading=\"## Section\") to focus",
    });
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let out = tool.format_compact(&response).unwrap_or_default();
    assert!(out.contains("# Hi"), "missing body, got: {out}");
    assert!(out.contains("body"), "missing body, got: {out}");
    assert!(out.contains("2 sections"), "missing hint, got: {out}");
}

#[test]
fn format_compact_content_no_hint_when_absent() {
    use crate::tools::Tool;
    let response = serde_json::json!({"content": "# Hi\n", "lines": 1});
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let out = tool.format_compact(&response).unwrap_or_default();
    assert_eq!(out.trim(), "# Hi");
}

#[test]
fn format_compact_content_with_breadcrumb_renders_section_header() {
    use crate::tools::Tool;
    let response = serde_json::json!({
        "content": "## Mid\n\nbody\n",
        "lines": 3,
        "breadcrumb": ["# Top", "## Mid"],
        "line_range": [10, 20],
    });
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let out = tool.format_compact(&response).unwrap_or_default();
    assert!(
        out.contains("§ ## Mid"),
        "missing section header, got: {out}"
    );
}

// ── format_compact MAP shape ───────────────────────────────────────────────────

#[test]
fn format_compact_map_shape_renders_indented_headings() {
    use crate::tools::Tool;
    let response = serde_json::json!({
        "lines": 329,
        "headings": [
            {"h": "# codescout", "l": 1},
            {"h": "## Development Commands", "l": 7},
            {"h": "### Skill Frictions", "l": 32},
        ],
        "file_id": "@file_xyz",
        "hint": "use \"@file_xyz\" — heading=\"## Section\" or start_line/end_line",
    });
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let out = tool.format_compact(&response).unwrap_or_default();

    assert!(out.contains("329 lines"), "missing line count, got: {out}");
    assert!(out.contains("@file_xyz"), "missing file_id, got: {out}");
    assert!(
        out.contains("# codescout  L1"),
        "missing level-1 heading, got: {out}"
    );
    assert!(
        out.contains("## Development Commands  L7"),
        "missing level-2 heading, got: {out}"
    );
    assert!(
        out.contains("  ### Skill Frictions  L32"),
        "level-3 heading should be indented by 4 spaces (level-1*2), got: {out}"
    );
    assert!(
        out.starts_with("329"),
        "header line should come first, got: {out}"
    );
    assert!(out.contains("next: "), "missing next cue, got: {out}");
}

#[test]
fn format_compact_section_map_renders_same_as_headings() {
    // Heading-targeted oversized uses `section_map` instead of `headings`.
    // Same rendering rules apply.
    use crate::tools::Tool;
    let response = serde_json::json!({
        "lines": 200,
        "section_map": [
            {"h": "### Sub A", "l": 100},
            {"h": "### Sub B", "l": 150},
        ],
        "file_id": "@file_abc",
        "hint": "use \"@file_abc\" — pick a sub-heading from `section_map` or start_line/end_line",
    });
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let out = tool.format_compact(&response).unwrap_or_default();
    assert!(
        out.contains("  ### Sub A  L100"),
        "section_map should render with indent, got: {out}"
    );
    assert!(
        out.contains("  ### Sub B  L150"),
        "section_map should render with indent, got: {out}"
    );
    assert!(out.contains("@file_abc"));
}

#[test]
fn format_compact_error_shape_renders_headings_with_error_prefix() {
    use crate::tools::Tool;
    let response = serde_json::json!({
        "ok": false,
        "error": "heading '## Foo' not found",
        "headings": [
            {"h": "# A", "l": 1},
            {"h": "## B", "l": 5},
        ],
        "hint": "pick a heading from `headings` array or use start_line/end_line",
    });
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let out = tool.format_compact(&response).unwrap_or_default();

    assert!(
        out.starts_with("error:"),
        "expected error prefix, got: {out}"
    );
    assert!(
        out.contains("## Foo' not found"),
        "missing error message, got: {out}"
    );
    assert!(
        out.contains("# A  L1") && out.contains("## B  L5"),
        "missing available headings, got: {out}"
    );
    assert!(out.contains("next: "), "missing next cue, got: {out}");
}

#[test]
fn format_compact_error_without_headings_still_renders_error_prefix() {
    use crate::tools::Tool;
    let response = serde_json::json!({
        "ok": false,
        "error": "start_line 9000 exceeds file length 3",
        "lines": 3,
        "hint": "valid range is 1..=3; ...",
    });
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let out = tool.format_compact(&response).unwrap_or_default();

    assert!(out.starts_with("error:"));
    assert!(out.contains("exceeds file length"));
    assert!(out.contains("next: valid range"));
}
