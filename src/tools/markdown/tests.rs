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

use super::edit_markdown::{
    find_consumed_subsections, perform_scoped_edit, perform_section_edit, perform_section_edit_ext,
};

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
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
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

/// Class-A fusion (sibling of the insert_after bug): when the scoped-edit
/// old_string consumes the section's trailing newline and new_string does
/// not restore it, the edited section fuses onto the following heading,
/// silently demoting it. The tool must guarantee the section keeps its
/// trailing newline.
#[test]
fn scoped_edit_consuming_trailing_newline_preserves_following_heading() {
    let content = "## A\nkeep\nold line\n## B\nbody\n";
    // old_string includes the line's trailing newline; new_string omits it.
    let result = perform_scoped_edit(content, "## A", "old line\n", "new line", false).unwrap();
    assert!(
        !result.contains("new line## B"),
        "scoped edit fused onto the following heading: {result:?}"
    );
    let headings = crate::tools::file_summary::parse_all_headings(&result);
    let heading_texts: Vec<&str> = headings.iter().map(|h| h.text.as_str()).collect();
    assert!(
        heading_texts.contains(&"## B"),
        "following heading demoted to body text; headings found: {heading_texts:?}"
    );
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

/// Unclosed code fence — historically (CommonMark-conformant) everything from
/// ``` to EOF was treated as inside the code block, so `# looks like heading`
/// was masked and `## Broken`'s section extended to EOF. As of the
/// last-heading-unaddressable fix (docs/issues/2026-05-21-edit-markdown-last-heading-unaddressable.md),
/// unbalanced ``` is treated as plain text — otherwise an unmatched fence
/// dropped into a batch edit silently hides every subsequent heading. With
/// the new rule, `# looks like heading` is parsed as a real H1, so it
/// terminates `## Broken`'s section and survives the replace.
#[test]
fn unclosed_code_fence() {
    let content = "# Title\n## Broken\ntext\n```\n# looks like heading\ncode\n";
    let result = perform_section_edit(content, "## Broken", "replace", Some("fixed\n")).unwrap();
    assert!(result.contains("fixed"));
    // Unbalanced fence is treated as plain text, so the H1 below it survives.
    assert!(result.contains("# looks like heading"));
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

#[test]
fn insert_after_h1_default_appends_at_end_of_section() {
    // Regression pin for 2026-05-09-edit-markdown-insert-after-h1.
    // Default `at="end-of-section"` semantic: for a sole H1 wrapping the
    // whole document, "section end" == EOF. Content lands at the bottom.
    let content = "# Title\n\nintro paragraph\n\nmore body\n";
    let result =
        perform_section_edit(content, "# Title", "insert_after", Some("appended\n")).unwrap();
    assert_eq!(
        result, "# Title\n\nintro paragraph\n\nmore body\n\nappended\n",
        "default insert_after on sole H1 should land at end-of-section (EOF here)"
    );
}

#[test]
fn insert_after_h1_with_at_after_heading_line_inserts_right_after_heading() {
    // Fix for 2026-05-09-edit-markdown-insert-after-h1.
    // Opt-in `at="after-heading-line"` puts content immediately after the
    // heading line itself, which is the intuitive behavior for whole-doc-wrap H1.
    let content = "# Title\n\nintro paragraph\n\nmore body\n";
    let result = perform_section_edit_ext(
        content,
        "# Title",
        "insert_after",
        Some("inserted right after\n"),
        Some("after-heading-line"),
        false,
    )
    .unwrap();
    assert_eq!(
        result, "# Title\ninserted right after\n\nintro paragraph\n\nmore body\n",
        "at=after-heading-line should insert directly after the heading line"
    );
}

#[test]
fn insert_after_with_explicit_end_of_section_matches_default() {
    // `at="end-of-section"` is the explicit form of the default.
    let content = "# Title\n## Setup\ncontent\n## Usage\nuse it\n";
    let default_result = perform_section_edit(
        content,
        "## Setup",
        "insert_after",
        Some("\n## Testing\ntest\n"),
    )
    .unwrap();
    let explicit_result = perform_section_edit_ext(
        content,
        "## Setup",
        "insert_after",
        Some("\n## Testing\ntest\n"),
        Some("end-of-section"),
        false,
    )
    .unwrap();
    assert_eq!(default_result, explicit_result);
}

#[test]
fn insert_after_invalid_at_value_errors() {
    let content = "# Title\nbody\n";
    let err = perform_section_edit_ext(
        content,
        "# Title",
        "insert_after",
        Some("x\n"),
        Some("nonsense"),
        false,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("invalid at=") && msg.contains("nonsense"),
        "error should name the invalid value, got: {msg}"
    );
}

#[test]
fn replace_refuses_when_surface_markers_would_be_dropped() {
    // F-7: replace whose new content omits `<!-- @surface NAME -->` or
    // `<!-- @end -->` lines present in the OLD body must refuse with a
    // RecoverableError listing the lost markers. The bug surfaced in real
    // use when editing src/prompts/source.md — the `## Deeper guidance`
    // section's body contained the @end marker for `server_instructions`
    // surface AND the @surface opener for `onboarding_prompt`; replace
    // wiped both, breaking the build's slice extraction.
    let content = "## Deeper guidance\n\
                   \n\
                   list:\n\
                   - one\n\
                   - two\n\
                   <!-- @end -->\n\
                   \n\
                   <!-- @surface next_surface -->\n\
                   intro\n\
                   ## Next Heading\nbody\n";
    let err = perform_section_edit_ext(
        content,
        "## Deeper guidance",
        "replace",
        Some("list:\n- new\n- entries\n"),
        None,
        false,
    )
    .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("surface marker"),
        "error should mention 'surface marker', got: {msg}"
    );
    assert!(
        msg.contains("<!-- @end -->"),
        "error should list the @end marker, got: {msg}"
    );
    assert!(
        msg.contains("<!-- @surface next_surface -->"),
        "error should list the @surface marker, got: {msg}"
    );
}

#[test]
fn replace_with_force_drops_markers_silently() {
    // F-7: force=true bypasses the marker-preservation gate. Used when the
    // structural change is intentional (e.g. removing a deprecated surface).
    let content = "## Section\n\
                   body\n\
                   <!-- @end -->\n";
    let result = perform_section_edit_ext(
        content,
        "## Section",
        "replace",
        Some("new body\n"),
        None,
        true,
    )
    .unwrap();
    assert!(
        !result.contains("<!-- @end -->"),
        "force=true must allow marker removal, got:\n{result}"
    );
    assert!(
        result.contains("new body"),
        "result should contain new body, got:\n{result}"
    );
}

#[test]
fn replace_preserves_markers_when_new_content_includes_them() {
    // F-7: happy path — when the new content explicitly re-includes the
    // markers, the gate passes silently and the replace proceeds.
    let content = "## Section\n\
                   list:\n\
                   - one\n\
                   <!-- @end -->\n\
                   \n\
                   <!-- @surface other -->\n";
    let result = perform_section_edit_ext(
        content,
        "## Section",
        "replace",
        Some("list:\n- new\n<!-- @end -->\n\n<!-- @surface other -->\n"),
        None,
        false,
    )
    .unwrap();
    assert!(
        result.contains("<!-- @end -->"),
        "@end marker should be preserved, got:\n{result}"
    );
    assert!(
        result.contains("<!-- @surface other -->"),
        "@surface marker should be preserved, got:\n{result}"
    );
    assert!(
        result.contains("- new"),
        "new content should be in result, got:\n{result}"
    );
}

#[test]
fn extract_surface_markers_ignores_marker_shaped_text_in_prose() {
    // F-7 false-positive guard: marker matching must be strict (anchored
    // to whole-line, exact prefix/suffix), so prose that *quotes* the
    // marker shape doesn't trip the gate. Pairs with F-5's parser-side
    // issue in extract_surface — both should be line-anchored.
    let content = "## Section\n\
                   Prose with inline reference to <!-- @surface foo --> not on its own line.\n\
                   Another line says the marker is `<!-- @end -->` (code-quoted).\n";
    // Should be allowed — no markers are alone on a line.
    let result = perform_section_edit_ext(
        content,
        "## Section",
        "replace",
        Some("new body\n"),
        None,
        false,
    );
    assert!(
        result.is_ok(),
        "marker-shaped text in prose should not trigger the gate, got: {:?}",
        result.err()
    );
}

/// Regression: insert_after with content lacking a trailing newline must NOT
/// fuse the inserted block's last line onto the following heading. Previously
/// `format!("{}{}{}", before, new, after)` concatenated `new` directly with
/// the next heading line, silently demoting it (and every sibling after it)
/// to body text. The tool must guarantee a trailing newline regardless of
/// whether the caller supplied one.
#[test]
fn insert_after_without_trailing_newline_preserves_following_heading() {
    let content = "## Section A\ncontent here\n\n## Constraint Stream Patterns\nmore content\n";
    // Caller forgets the trailing newline — the historical foot-gun.
    let result =
        perform_section_edit(content, "## Section A", "insert_after", Some("new entry")).unwrap();

    // The following heading must survive as a recognized heading, not fuse
    // into "new entry## Constraint Stream Patterns".
    assert!(
        !result.contains("new entry## Constraint Stream Patterns"),
        "inserted content fused onto the following heading: {result:?}"
    );
    let headings = crate::tools::file_summary::parse_all_headings(&result);
    let heading_texts: Vec<&str> = headings.iter().map(|h| h.text.as_str()).collect();
    assert!(
        heading_texts.contains(&"## Constraint Stream Patterns"),
        "following heading was demoted to body text; headings found: {heading_texts:?}"
    );
    assert!(
        result.contains("new entry"),
        "inserted content missing: {result:?}"
    );
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
async fn read_markdown_call_content_returns_text_map_not_json() {
    // Regression: small heading-map (ToC) results used to serialize as pretty
    // JSON via the default Tool::call_content path because ReadMarkdown did not
    // declare OutputForm::Text. The MAP renderer existed in format_compact but
    // only fired on the buffered (large-byte) axis. Now both axes reach it, so
    // a sub-threshold ToC comes through as the indented `# Heading  Ln` form.
    let body = synth_md(205, 41); // > HEADINGS_HARD_CAP, ~1.8 KB → small path, MAP shape
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("toc.md");
    std::fs::write(&path, &body).unwrap();

    let ctx = test_ctx().await;
    let content = super::ReadMarkdown
        .call_content(serde_json::json!({"path": path.to_str().unwrap()}), &ctx)
        .await
        .unwrap();

    assert_eq!(content.len(), 1, "expected exactly 1 content block");
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        text.contains("lines") && text.contains('L'),
        "expected text MAP form with line markers, got: {text}"
    );
    assert!(
        !text.trim_start().starts_with('{'),
        "MAP output must be text, not JSON, got: {text}"
    );
    assert!(
        !text.contains("\"headings\""),
        "MAP output must not carry the raw JSON headings key, got: {text}"
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

// ── format_compact live-render verification ───────────────────────────────────

#[tokio::test]
async fn format_compact_live_renders_claude_md_as_map_shape() {
    // Live verification: read the real CLAUDE.md from the repo root, then
    // invoke format_compact on the response. This exercises the same rendering
    // path call_content uses when the response is buffered. Round 1 round 2
    // scored JSON only — this test closes the format_compact gap.
    use crate::tools::Tool;

    let claude_md = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("CLAUDE.md");
    if !claude_md.exists() {
        // Project's own CLAUDE.md is the fixture; skip if missing (e.g. CI checkout).
        eprintln!("SKIP: CLAUDE.md not present at {}", claude_md.display());
        return;
    }

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(
            serde_json::json!({"path": claude_md.to_str().unwrap()}),
            &ctx,
        )
        .await
        .unwrap();

    // The response shape must be MAP (CLAUDE.md exceeds line cap).
    assert!(
        result.get("headings").is_some(),
        "expected MAP shape, got: {result}"
    );
    assert!(result.get("file_id").is_some(), "MAP requires file_id");

    let rendered = tool
        .format_compact(&result)
        .expect("format_compact must return Some for MAP shape");

    // Structural invariants on the rendered text.
    assert!(
        rendered.contains("lines  @file_"),
        "MAP header must show `<n> lines  <file_id>`, got first 200 chars: {}",
        &rendered.chars().take(200).collect::<String>()
    );
    assert!(
        rendered.contains("# codescout  L1"),
        "MAP must render top heading with line number, got: {}",
        &rendered.chars().take(500).collect::<String>()
    );
    assert!(
        rendered.contains("  ### "),
        "MAP must indent level-3 headings by 4 spaces (level-1*2)"
    );
    assert!(rendered.contains("next: "), "MAP must end with next-cue");
    // The hint must carry the file_id verbatim so the agent can copy-paste it.
    let file_id = result["file_id"].as_str().unwrap();
    assert!(
        rendered.contains(file_id),
        "rendered text must include file_id `{}` verbatim",
        file_id
    );
}

#[tokio::test]
async fn format_compact_live_renders_heading_not_found_as_error_with_headings() {
    // Live verification of the ERROR branch: read a real file with a bogus
    // heading, then render the error response. Validates F1 closure end-to-end.
    use crate::tools::Tool;

    let body = "# A\n\n## B\n\n## C\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.md");
    std::fs::write(&path, body).unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let err = tool
        .call(
            serde_json::json!({"path": path.to_str().unwrap(), "heading": "## Nonexistent"}),
            &ctx,
        )
        .await
        .expect_err("expected RecoverableError");

    // Reconstruct the JSON envelope the MCP server would emit for this error.
    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("expected RecoverableError");
    let mut envelope = serde_json::json!({
        "ok": false,
        "error": rec.message.clone(),
    });
    if let Some(hint) = rec.hint() {
        envelope["hint"] = serde_json::json!(hint);
    }
    for (k, v) in rec.extra.iter() {
        envelope[k] = v.clone();
    }

    let rendered = tool
        .format_compact(&envelope)
        .expect("format_compact must return Some for ERROR shape");

    assert!(
        rendered.starts_with("error: "),
        "ERROR must start with `error: `, got: {rendered}"
    );
    assert!(
        rendered.contains("Nonexistent"),
        "error msg must reference the missing heading"
    );
    assert!(
        rendered.contains("available headings:"),
        "ERROR with headings must show the list"
    );
    assert!(
        rendered.contains("# A  L1"),
        "ERROR must indent headings same as MAP, got: {rendered}"
    );
    assert!(rendered.contains("## B  L3"), "ERROR list missing entry");
    assert!(rendered.contains("next: "), "ERROR must end with next-cue");
}

// ── empty file + heading arg regression ──────────────────────────────────────

#[tokio::test]
async fn empty_file_with_heading_arg_returns_recoverable_error() {
    // Spec F-R2-04: when caller asks for a heading on an empty file, return
    // ERROR shape (not silent success with empty content).
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.md");
    std::fs::write(&path, "").unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(
            serde_json::json!({"path": path.to_str().unwrap(), "heading": "## Anything"}),
            &ctx,
        )
        .await;

    let err = result.expect_err("expected RecoverableError for heading on empty file");
    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("expected RecoverableError");
    // The error must indicate the heading wasn't found OR that the file has no headings.
    assert!(
        rec.message.to_lowercase().contains("not found")
            || rec.message.to_lowercase().contains("empty")
            || rec.message.to_lowercase().contains("no headings"),
        "error should mention not-found / empty / no-headings, got: {}",
        rec.message
    );
}

// ── apply_frontmatter_mutation wrapper tests ────────────────────────────────

#[test]
fn frontmatter_set_flips_status_in_place() {
    use super::edit_markdown::apply_frontmatter_mutation;
    let src = "---\nstatus: open\nopened: 2026-04-24\nclosed:\n---\n\n# BUG: ...\nbody\n";
    let param = json!({"set": {"status": "fixed", "closed": "2026-05-17"}});
    let out = apply_frontmatter_mutation(src, &param).unwrap();
    assert_eq!(
        out,
        "---\nstatus: fixed\nopened: 2026-04-24\nclosed: 2026-05-17\n---\n\n# BUG: ...\nbody\n"
    );
}

#[test]
fn frontmatter_delete_removes_key_preserves_body() {
    use super::edit_markdown::apply_frontmatter_mutation;
    let src = "---\nstatus: open\nlegacy_field: yes\n---\nbody\n";
    let param = json!({"delete": ["legacy_field"]});
    let out = apply_frontmatter_mutation(src, &param).unwrap();
    assert_eq!(out, "---\nstatus: open\n---\nbody\n");
}

#[test]
fn frontmatter_set_and_delete_combined_atomic() {
    use super::edit_markdown::apply_frontmatter_mutation;
    let src = "---\nstatus: open\nlegacy: x\n---\nbody\n";
    let param = json!({"set": {"status": "fixed"}, "delete": ["legacy"]});
    let out = apply_frontmatter_mutation(src, &param).unwrap();
    assert_eq!(out, "---\nstatus: fixed\n---\nbody\n");
}

#[test]
fn frontmatter_set_bootstraps_block_on_file_without_frontmatter() {
    use super::edit_markdown::apply_frontmatter_mutation;
    let src = "# Title\n\nbody\n";
    let param = json!({"set": {"status": "fixed", "kind": "bug"}});
    let out = apply_frontmatter_mutation(src, &param).unwrap();
    let a = "---\nstatus: fixed\nkind: bug\n---\n\n# Title\n\nbody\n";
    let b = "---\nkind: bug\nstatus: fixed\n---\n\n# Title\n\nbody\n";
    assert!(out == a || out == b, "got: {out:?}");
}

#[test]
fn frontmatter_bootstrap_does_not_double_blank_when_body_already_blank_first() {
    use super::edit_markdown::apply_frontmatter_mutation;
    let src = "\nbody\n";
    let param = json!({"set": {"status": "fixed"}});
    let out = apply_frontmatter_mutation(src, &param).unwrap();
    assert_eq!(out, "---\nstatus: fixed\n---\n\nbody\n");
}

#[test]
fn frontmatter_bootstrap_on_empty_file_produces_block_only() {
    use super::edit_markdown::apply_frontmatter_mutation;
    let src = "";
    let param = json!({"set": {"status": "fixed"}});
    let out = apply_frontmatter_mutation(src, &param).unwrap();
    assert_eq!(out, "---\nstatus: fixed\n---\n");
}

#[test]
fn frontmatter_delete_only_on_file_without_frontmatter_is_noop() {
    use super::edit_markdown::apply_frontmatter_mutation;
    let src = "# Title\n\nbody\n";
    let param = json!({"delete": ["legacy_field"]});
    let out = apply_frontmatter_mutation(src, &param).unwrap();
    assert_eq!(out, "# Title\n\nbody\n");
}

#[test]
fn frontmatter_empty_set_and_delete_rejected() {
    use super::edit_markdown::apply_frontmatter_mutation;
    let src = "---\nstatus: open\n---\nbody\n";
    let param = json!({});
    let err = apply_frontmatter_mutation(src, &param)
        .unwrap_err()
        .to_string();
    assert!(err.contains("at least one"), "got: {err}");
}

#[test]
fn frontmatter_param_not_object_rejected() {
    use super::edit_markdown::apply_frontmatter_mutation;
    let src = "---\nstatus: open\n---\nbody\n";
    let param = json!("not an object");
    let err = apply_frontmatter_mutation(src, &param)
        .unwrap_err()
        .to_string();
    assert!(err.contains("must be an object"), "got: {err}");
}

#[test]
fn replace_preserves_trailing_horizontal_rule_separator() {
    // F-3: a `---` separator between two same-level sections must survive a
    // wholesale-body `replace` on the section above it. The HR belongs to
    // neither section; it is a between-sections separator.
    let content = "## Scan state\n\nold body line\n\n---\n\n## How to use\n\ndetails\n";
    let result = perform_section_edit(
        content,
        "Scan state",
        "replace",
        Some("new table content\n"),
    )
    .unwrap();

    assert!(
        result.contains("new table content"),
        "new body must be present: {result:?}"
    );
    assert!(
        result.contains("---"),
        "trailing horizontal-rule separator must survive replace: {result:?}"
    );
    // The HR must still sit between the two headings (not be re-injected
    // somewhere else by accident).
    let hr_pos = result.find("---").unwrap();
    let next_heading_pos = result.find("## How to use").unwrap();
    assert!(
        hr_pos < next_heading_pos,
        "HR must precede the next heading: {result:?}"
    );
    assert!(
        result.find("new table content").unwrap() < hr_pos,
        "new content must precede the HR: {result:?}"
    );
}

#[test]
fn replace_does_not_preserve_hr_when_body_is_only_hr() {
    // Edge case: section body is *only* an HR (no real content before it).
    // In that case the HR is the body — replace should consume it like any
    // other body content. Otherwise we would preserve content the caller
    // explicitly asked to overwrite.
    let content = "## Divider\n\n---\n\n## Next\n\ndetails\n";
    let result =
        perform_section_edit(content, "Divider", "replace", Some("real content\n")).unwrap();

    assert!(result.contains("real content"));
    // The HR that used to BE the body is gone. (The trailing-`---` heuristic
    // only fires when there's body content BEFORE the HR.)
    let divider_pos = result.find("## Divider").unwrap();
    let next_pos = result.find("## Next").unwrap();
    let between = &result[divider_pos..next_pos];
    assert!(
        !between.contains("---"),
        "HR-only body must be replaced, not preserved: {between:?}"
    );
}

#[test]
fn replace_preserves_hr_separator_with_trailing_blank_lines() {
    // Variant: `---` followed by multiple blank lines before the next heading.
    // Must still be detected and preserved.
    let content = "## A\n\nbody A\n\n---\n\n\n## B\n\nbody B\n";
    let result = perform_section_edit(content, "A", "replace", Some("new A\n")).unwrap();

    assert!(result.contains("new A"));
    assert!(
        result.contains("---"),
        "HR with multiple trailing blank lines must survive: {result:?}"
    );
}

#[test]
fn replace_does_not_misdetect_emphasis_as_hr() {
    // Sanity: `***` is also a CommonMark HR marker, but a single `*` is not.
    // Ensure the detector doesn't trigger on lines containing emphasis markers
    // that aren't true HRs.
    let content = "## A\n\nbody with *emphasis* end\n\n## B\n\nbody B\n";
    let result = perform_section_edit(content, "A", "replace", Some("new A\n")).unwrap();
    assert!(result.contains("new A"));
    assert!(
        !result.contains("*emphasis*"),
        "original body must be fully replaced when no HR is present: {result:?}"
    );
}

#[test]
fn replace_preserves_asterisk_hr_separator() {
    // CommonMark supports `***` and `___` as HR markers too. Confirm.
    let content = "## A\n\nbody A\n\n***\n\n## B\n\nbody B\n";
    let result = perform_section_edit(content, "A", "replace", Some("new A\n")).unwrap();
    assert!(result.contains("new A"));
    assert!(
        result.contains("***"),
        "`***` HR must survive replace: {result:?}"
    );
}
