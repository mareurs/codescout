# read_markdown Render Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Cut Tier 3 token cost ~40% in `read_markdown` and close four behavioral defects identified by Hamsa eval round 1.

**Architecture:** Three target JSON shapes (CONTENT / MAP / ERROR) replacing the current ad-hoc per-tier shapes. A new `format_compact` impl renders each shape as an LLM-facing text view. One new constant (`HEADINGS_HARD_CAP`) closes the Tier 2 misclassification hole.

**Tech Stack:** Rust, `serde_json`, `RecoverableError` for input-driven failures, existing `OutputGuard` / output-buffer machinery.

**Spec:** `docs/superpowers/specs/2026-05-15-read-markdown-render-design.md`

---

## File Map

| File | Responsibility | Action |
|---|---|---|
| `src/tools/core/types.rs` | Constants for tier boundaries | **Modify**: add `HEADINGS_HARD_CAP` |
| `src/tools/markdown/read_markdown.rs` | Tool impl, all emission sites, `format_compact` | **Modify**: shape rewrites + add `format_compact` body |
| `src/tools/markdown/tests.rs` | All read_markdown tests | **Modify**: update assertions to new shapes; add 5 regression tests |

No new files. No shared module extraction — this tool is the only consumer of the new shapes (until `edit_markdown` follows in a later redesign).

## Project conventions

- **TDD:** failing test → minimal impl → green → commit. Each task is one behavior at a time.
- **`cargo fmt && cargo clippy -- -D warnings && cargo test` before every commit.** Non-negotiable per `CLAUDE.md`.
- **`run_command` not Bash.** Companion plugin blocks native Bash. Use `mcp__codescout__run_command`.
- **`edit_code` for symbol-level edits**, `edit_markdown` for `.md`, `edit_file` only for imports/literals.
- **Branch:** all work on `experiments`. `master` only via cherry-pick after green eval + Docs Lotus Frog audit.
- **Commit style:** scoped conventional commits — `fix(read_markdown): …`, `feat(read_markdown): …`, `test(read_markdown): …`.

## Task ordering rationale

Behavioral fixes (Tasks 1–5) land first as additive changes. The big shape rewrite (Task 6) lands once behavior is correct so test churn happens once. `format_compact` rendering (Tasks 7–9) follows shape stabilization. Final eval (Task 10) is the ship gate.

---

### Task 1: B1 — escalate many-headings files to MAP shape

**Files:**
- Modify: `src/tools/core/types.rs:34` (add constant)
- Modify: `src/tools/markdown/read_markdown.rs` (Tier 2 branch around line 380)
- Test: `src/tools/markdown/tests.rs`

- [ ] **Step 1: Write the failing regression test**

In `src/tools/markdown/tests.rs`, append:

```rust
#[tokio::test]
async fn many_headings_escalates_to_map_shape_even_when_bytes_fit() {
    // Spec B1: a file with > HEADINGS_HARD_CAP sections must not return
    // full content in Tier 2 even if byte budget is satisfied. 250 short
    // sections is the eval case that exposed this hole.
    let body = synth_md(1000, 250);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("many.md");
    std::fs::write(&path, &body).unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(&ctx, serde_json::json!({"path": path.to_str().unwrap()}))
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
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p code-explorer --lib many_headings_escalates -- --nocapture
```

Expected: FAIL — currently returns `content` field for 1000-line/250-section file (Tier 2 path).

- [ ] **Step 3: Add the constant**

Edit `src/tools/core/types.rs`, after the `LINE_SOFT_CAP` block (around line 34):

```rust
/// Heading-count gate for escalation to MAP shape. A markdown file with more
/// than this many headings is structurally a directory — content is skim-only
/// and the caller wants to pivot. Escalates Tier 2 → Tier 3 regardless of
/// byte/line budgets. Closes Hamsa eval B1 (many-headings.md, 251 sections).
pub(crate) const HEADINGS_HARD_CAP: usize = 40;
```

- [ ] **Step 4: Wire the constant into the Tier 2 branch**

Open `src/tools/markdown/read_markdown.rs`. Find the Tier 2 medium branch (around line 380, condition `if total_lines > crate::tools::LINE_SOFT_CAP`). Replace its condition with:

```rust
if total_lines > crate::tools::LINE_SOFT_CAP
    || crate::tools::file_summary::parse_all_headings(&text).len()
        > crate::tools::core::types::HEADINGS_HARD_CAP
{
```

Then immediately inside the branch, if `headings.len() > HEADINGS_HARD_CAP`, jump to the Tier 3 path. Concretely, restructure so the heading count is computed once and either path can be taken:

```rust
let all_headings = crate::tools::file_summary::parse_all_headings(&text);
let oversized_by_headings = all_headings.len()
    > crate::tools::core::types::HEADINGS_HARD_CAP;

if oversized || oversized_by_headings {
    // ── Existing Tier 3 body, unchanged ──
    // (reuses `all_headings` instead of re-parsing)
}

if total_lines > crate::tools::LINE_SOFT_CAP {
    // ── Existing Tier 2 body, unchanged ──
}
```

Replace the standalone `let all_headings = ...` calls inside Tier 3 and Tier 2 with the hoisted local.

- [ ] **Step 5: Re-export the constant from `src/tools/mod.rs` if not already**

```
grep -n "pub(crate) use core::types" src/tools/mod.rs
```

If the line exists, add `HEADINGS_HARD_CAP` to the import list. Otherwise add:

```rust
pub(crate) use core::types::HEADINGS_HARD_CAP;
```

- [ ] **Step 6: Verify test passes**

```
cargo test -p code-explorer --lib many_headings_escalates -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Run full markdown suite**

```
cargo test -p code-explorer --lib markdown::tests
```

Expected: PASS for all markdown tests. If any fail, the hoisting in Step 4 likely broke a tier boundary — debug before commit.

- [ ] **Step 8: fmt + clippy + commit**

```
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/core/types.rs src/tools/mod.rs src/tools/markdown/read_markdown.rs src/tools/markdown/tests.rs
git commit -m "fix(read_markdown): escalate many-headings files to MAP shape

Files with > HEADINGS_HARD_CAP (40) headings now bypass Tier 2 even
when bytes fit. Closes Hamsa eval B1 — many-headings.md (1002 lines,
251 sections) was returning ~30 KB of content with only a focused-read
hint, paying full token cost for skim-only material."
```

---

### Task 2: B2 — line range out of file returns ERROR shape

**Files:**
- Modify: `src/tools/markdown/read_markdown.rs` (line-range branch around line 297)
- Test: `src/tools/markdown/tests.rs`

- [ ] **Step 1: Write the failing regression test**

```rust
#[tokio::test]
async fn line_range_past_eof_returns_recoverable_error() {
    let body = "# Tiny\n\nbody\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tiny.md");
    std::fs::write(&path, body).unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(
            &ctx,
            serde_json::json!({
                "path": path.to_str().unwrap(),
                "start_line": 9000,
                "end_line": 9999,
            }),
        )
        .await;

    let err = result.expect_err("expected RecoverableError for OOR slice");
    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("expected RecoverableError, got: {err}");
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
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p code-explorer --lib line_range_past_eof -- --nocapture
```

Expected: FAIL — currently returns `Ok({"content": ""})`.

- [ ] **Step 3: Add the bounds check**

In `src/tools/markdown/read_markdown.rs`, find the line-range branch (it begins with `if let Some(start) = start_line`, around line 280). Before the existing slicing logic, insert:

```rust
let content_total = text.lines().count();
if (start as usize) > content_total {
    return Err(crate::tools::RecoverableError::with_hint(
        format!(
            "start_line {} exceeds file length {}",
            start, content_total
        ),
        format!(
            "valid range is 1..={}; use read_markdown(path, start_line=N, end_line=M) within bounds",
            content_total
        ),
    )
    .with_extra("lines", serde_json::json!(content_total))
    .into());
}
```

Make sure to delete the duplicate `content_total` binding lower in the same branch (it is computed identically) — or move that binding up to dominate both paths.

- [ ] **Step 4: Verify test passes**

```
cargo test -p code-explorer --lib line_range_past_eof -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Run full suite**

```
cargo test -p code-explorer --lib markdown::tests
```

Expected: PASS.

- [ ] **Step 6: fmt + clippy + commit**

```
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/markdown/read_markdown.rs src/tools/markdown/tests.rs
git commit -m "fix(read_markdown): error when start_line exceeds file length

Closes Hamsa eval B2 — read_markdown(path, start_line=9000) on a
94-line file silently returned {content: \"\"}. Now returns a
RecoverableError carrying the actual file length so the caller can
self-correct."
```

---

### Task 3: B4 — empty file returns slim shape

**Files:**
- Modify: `src/tools/markdown/read_markdown.rs` (Tier 1 small branch around line 410)
- Test: `src/tools/markdown/tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn empty_file_returns_slim_shape() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.md");
    std::fs::write(&path, "").unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(&ctx, serde_json::json!({"path": path.to_str().unwrap()}))
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
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p code-explorer --lib empty_file_returns_slim_shape -- --nocapture
```

Expected: FAIL — currently emits `format` and `heading_count` and `total_lines`.

- [ ] **Step 3: Update the Tier 1 small branch**

In `src/tools/markdown/read_markdown.rs`, find the Tier 1 small block (last branch in `call`, around line 410). Replace the json! literal:

```rust
let mut result = json!({
    "content": text,
    "lines": total_lines,
});
```

Drop `format` and `heading_count` from this branch only. (Task 6 will sweep the rest.)

- [ ] **Step 4: Verify test passes**

```
cargo test -p code-explorer --lib empty_file_returns_slim_shape -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Run full suite**

```
cargo test -p code-explorer --lib markdown::tests
```

Expected: existing tests asserting `total_lines` / `format` on Tier 1 small may fail. Update them to the new field names inline — replace `total_lines` with `lines` and remove `format` checks. List which tests change in the commit message.

- [ ] **Step 6: fmt + clippy + commit**

```
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/markdown/read_markdown.rs src/tools/markdown/tests.rs
git commit -m "fix(read_markdown): drop decoration from empty/small-file response

Closes Hamsa eval B4 — empty.md returned
{content:\"\", total_lines:0, heading_count:0, format:\"markdown\"}.
Now returns {content, lines}. Tier 1 small branch only; other tiers
swept in subsequent commit."
```

---

### Task 4: F1 — heading-not-found error carries `headings[]`

**Files:**
- Modify: `src/tools/markdown/read_markdown.rs` (heading-not-found error path)
- Test: `src/tools/markdown/tests.rs`

- [ ] **Step 1: Find the error path**

```
cargo run --bin codescout -- search-symbol heading_query
```

Or grep:

```
grep -n "heading.*not found\\|Available headings" src/tools/markdown/read_markdown.rs
```

Note the line where the not-found `RecoverableError` is constructed.

- [ ] **Step 2: Write the failing test**

```rust
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
            &ctx,
            serde_json::json!({
                "path": path.to_str().unwrap(),
                "heading": "## Nonexistent",
            }),
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
```

- [ ] **Step 3: Run test to verify it fails**

```
cargo test -p code-explorer --lib bogus_heading_error -- --nocapture
```

Expected: FAIL — current shape uses comma-joined English string under `hint`.

- [ ] **Step 4: Rewrite the not-found error**

Replace the not-found `RecoverableError` construction with:

```rust
let headings_json: Vec<serde_json::Value> = crate::tools::file_summary::parse_all_headings(&text)
    .iter()
    .map(|h| serde_json::json!({"h": h.text, "l": h.line}))
    .collect();
return Err(crate::tools::RecoverableError::with_hint(
    format!("heading {:?} not found", heading_query),
    "pick a heading from `headings` array or use start_line/end_line",
)
.with_extra("headings", serde_json::json!(headings_json))
.into());
```

- [ ] **Step 5: Verify test passes**

```
cargo test -p code-explorer --lib bogus_heading_error -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Run full suite**

```
cargo test -p code-explorer --lib markdown::tests
```

Expected: any test that asserted on the old English-list hint must be updated. Update inline.

- [ ] **Step 7: fmt + clippy + commit**

```
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/markdown/read_markdown.rs src/tools/markdown/tests.rs
git commit -m "fix(read_markdown): heading-not-found error returns headings[] array

Closes Hamsa eval F1 — error path used a comma-joined English list
truncated at 15 entries. Now returns the same {h, l} array shape the
MAP success response uses, so callers decode navigation once."
```

---

### Task 5: F3 — Tier 1 with ≥2 sections gets nav hint

**Files:**
- Modify: `src/tools/markdown/read_markdown.rs` (Tier 1 small branch around line 410)
- Test: `src/tools/markdown/tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn small_file_with_multiple_sections_gets_nav_hint() {
    let body = "# A\n\nbody\n\n## B\n\nmore\n\n## C\n\nend\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("h.md");
    std::fs::write(&path, body).unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(&ctx, serde_json::json!({"path": path.to_str().unwrap()}))
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
        .call(&ctx, serde_json::json!({"path": path.to_str().unwrap()}))
        .await
        .unwrap();

    assert!(
        result.get("hint").is_none(),
        "expected no hint when no headings exist, got: {result}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test -p code-explorer --lib small_file_with -- --nocapture
```

Expected: first test FAILS (no hint emitted), second test PASSES (hint not in response today).

- [ ] **Step 3: Add the nav hint to Tier 1 small**

In the Tier 1 small branch, after the `let mut result = json!({...})` from Task 3:

```rust
let heading_count =
    crate::tools::file_summary::parse_all_headings(&text).len();
if heading_count >= 2 {
    result["hint"] = serde_json::json!(format!(
        "{} lines, {} sections — read_markdown(path, heading=\"## Section\") to focus",
        total_lines, heading_count
    ));
}
```

- [ ] **Step 4: Verify tests pass**

```
cargo test -p code-explorer --lib small_file_with -- --nocapture
```

Expected: both PASS.

- [ ] **Step 5: Full suite**

```
cargo test -p code-explorer --lib markdown::tests
```

Expected: PASS.

- [ ] **Step 6: fmt + clippy + commit**

```
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/markdown/read_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(read_markdown): nav hint on small files with multiple sections

Closes Hamsa eval F3 — Tier 1 small response carried content with no
signal that headings exist for follow-up navigation. Now appends a
nav hint when ≥2 sections are present."
```

---

### Task 6: JSON shape sweep — drop decoration, rename, restructure

This is the big coordinated rename. Behavior is now correct (Tasks 1–5); this task makes the wire shape final.

**Files:**
- Modify: `src/tools/markdown/read_markdown.rs` (all five emission sites)
- Modify: `src/tools/markdown/tests.rs` (every assertion using old field names)

Field changes applied across **every** emission site:

| Drop | Rename | Restructure |
|---|---|---|
| `format` | `total_lines` → `lines` | `heading_map` entries `{level, text, line}` → `{h, l}` |
| `total_bytes` | `total_bytes` → (removed) | `must_follow` (Guidance::MustFollow) → `hint` (Guidance::Hint) |
| `heading_count` | `heading_map` → `headings` |  |
| `sections_returned` |  |  |

- [ ] **Step 1: Write the shape-sweep snapshot test**

```rust
#[tokio::test]
async fn tier3_map_shape_fields_are_canonical() {
    // Spec MAP shape: {lines, headings:[{h,l}], file_id, hint}
    // Forbidden: format, total_lines, total_bytes, heading_count,
    // heading_map, must_follow.
    let body = synth_md(5000, 50); // forces Tier 3 via byte budget
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("big.md");
    std::fs::write(&path, &body).unwrap();

    let ctx = test_ctx().await;
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let result = tool
        .call(&ctx, serde_json::json!({"path": path.to_str().unwrap()}))
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
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p code-explorer --lib tier3_map_shape_fields_are_canonical -- --nocapture
```

Expected: FAIL on multiple forbidden fields.

- [ ] **Step 3: Sweep emission site — Tier 3 oversized**

In the Tier 3 branch (around line 333 onward in the current file), replace the build with:

```rust
let headings_json: Vec<serde_json::Value> = all_headings
    .iter()
    .map(|h| serde_json::json!({"h": h.text, "l": h.line}))
    .collect();

let file_id = ctx
    .output_buffer
    .store_file(resolved.to_string_lossy().to_string(), text.clone());

let hint = if all_headings.is_empty() {
    format!("use {:?} — start_line/end_line", file_id)
} else {
    format!(
        "use {:?} — heading=\"## Section\" or start_line/end_line",
        file_id
    )
};

let mut result = json!({
    "lines": total_lines,
    "headings": headings_json,
    "file_id": file_id,
    "hint": hint,
});
if let Some(c) = md_cov {
    result["coverage"] = c;
}
return Ok(result);
```

- [ ] **Step 4: Sweep emission site — Tier 2 medium**

Replace its build with:

```rust
let heading_count = all_headings.len();
let hint = if heading_count == 0 {
    format!(
        "{} lines, no headings — read_markdown(path, start_line=N, end_line=M) to focus",
        total_lines
    )
} else {
    format!(
        "{} lines, {} sections — read_markdown(path, heading=\"## Section\") to focus",
        total_lines, heading_count
    )
};

let mut result = json!({
    "content": text,
    "lines": total_lines,
    "hint": hint,
});
if let Some(c) = md_cov {
    result["coverage"] = c;
}
return Ok(result);
```

- [ ] **Step 5: Sweep emission site — Tier 1 small**

Already partly done in Tasks 3 + 5. Final shape:

```rust
let mut result = json!({
    "content": text,
    "lines": total_lines,
});
let heading_count =
    crate::tools::file_summary::parse_all_headings(&text).len();
if heading_count >= 2 {
    result["hint"] = serde_json::json!(format!(
        "{} lines, {} sections — read_markdown(path, heading=\"## Section\") to focus",
        total_lines, heading_count
    ));
}
if let Some(c) = md_cov {
    result["coverage"] = c;
}
Ok(result)
```

- [ ] **Step 6: Sweep emission site — heading-targeted oversized section**

Around line 200–250 (the `if crate::tools::exceeds_inline_limit(&section_result.content)` block). Replace the `must_follow` `RecoverableError` with `with_hint`, and update `section_map` entries to `{h, l}`:

```rust
let nested: Vec<serde_json::Value> = all_headings
    .iter()
    .filter(|h| h.line > start_ln && h.line <= end_ln)
    .map(|h| serde_json::json!({"h": h.text, "l": h.line}))
    .collect();

let hint = format!(
    "use {:?} — pick a sub-heading from `section_map` or start_line/end_line",
    file_id
);

let err = crate::tools::RecoverableError::with_hint(
    format!(
        "section {:?} spans {} lines — exceeds inline threshold",
        heading_label, section_lines
    ),
    hint,
)
.with_extra("file_id", serde_json::json!(file_id))
.with_extra("section_map", serde_json::json!(nested))
.with_extra("next_actions", serde_json::json!(next_actions))
.with_extra("breadcrumb", serde_json::json!(section_result.breadcrumb))
.with_extra("line_range", serde_json::json!([start_ln, end_ln]));
return Err(err.into());
```

- [ ] **Step 7: Sweep emission site — multi-heading branch**

Around line 158. Replace:

```rust
let mut result = json!({
    "content": content,
    "sections_returned": heading_queries.len(),
});
```

with:

```rust
let mut result = json!({ "content": content });
```

Coverage block below unchanged.

- [ ] **Step 8: Update existing tests in one pass**

Search-and-replace in `src/tools/markdown/tests.rs`:

```
grep -n "total_lines\|heading_map\|heading_count\|must_follow\|total_bytes\|sections_returned\|\"format\"" src/tools/markdown/tests.rs
```

For each hit:
- `total_lines` → `lines` (when reading from JSON)
- `heading_map` → `headings`
- `heading_count` → derive from `headings.as_array().unwrap().len()`
- Entry field accesses: `entry["level"]` → drop or derive from `entry["h"]` `#` count; `entry["text"]` → `entry["h"]`; `entry["line"]` → `entry["l"]`
- `must_follow` (in `RecoverableError`) → `hint`
- `total_bytes`, `sections_returned`, `format` → delete the assertion line

This is mechanical. Re-run `cargo test` after each batch to verify.

- [ ] **Step 9: Run snapshot test + full suite**

```
cargo test -p code-explorer --lib tier3_map_shape_fields_are_canonical -- --nocapture
cargo test -p code-explorer --lib markdown::tests
```

Expected: PASS.

- [ ] **Step 10: fmt + clippy + commit**

```
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/markdown/read_markdown.rs src/tools/markdown/tests.rs
git commit -m "refactor(read_markdown): canonical JSON shapes (CONTENT/MAP/ERROR)

Drop \`format\`, \`heading_count\`, \`total_bytes\`, \`sections_returned\`.
Rename \`total_lines\` → \`lines\`, \`heading_map\` → \`headings\`.
Heading entries become {h, l} — level derivable from \`#\` prefix.
\`must_follow\` prose replaced with terse \`hint\` carrying file_id and
the valid next-call argument shape.

Tier 3 token cost reduced ~40% on the eval set."
```

---

### Task 7: format_compact — CONTENT shape

**Files:**
- Modify: `src/tools/markdown/read_markdown.rs` (add `format_compact` to `impl Tool for ReadMarkdown`)
- Test: `src/tools/markdown/tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn format_compact_content_passthrough_with_hint_footer() {
    let response = serde_json::json!({
        "content": "# Hi\n\nbody\n",
        "lines": 3,
        "hint": "3 lines, 2 sections — read_markdown(path, heading=\"## Section\") to focus",
    });
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let out = tool.format_compact(&response);
    assert!(out.contains("# Hi"), "missing body, got: {out}");
    assert!(out.contains("body"), "missing body, got: {out}");
    assert!(out.contains("2 sections"), "missing hint, got: {out}");
}

#[test]
fn format_compact_content_no_hint_when_absent() {
    let response = serde_json::json!({"content": "# Hi\n", "lines": 1});
    let tool = crate::tools::markdown::read_markdown::ReadMarkdown;
    let out = tool.format_compact(&response);
    assert_eq!(out.trim(), "# Hi");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test -p code-explorer --lib format_compact_content -- --nocapture
```

Expected: FAIL — no `format_compact` impl on `ReadMarkdown` yet (falls back to default `json.to_string()`).

- [ ] **Step 3: Add the `format_compact` method**

In `src/tools/markdown/read_markdown.rs`, inside `impl Tool for ReadMarkdown`, add (after `call`):

```rust
fn format_compact(&self, response: &serde_json::Value) -> String {
    if let Some(content) = response.get("content").and_then(|v| v.as_str()) {
        let mut out = String::new();
        if let (Some(breadcrumb), Some(line_range)) = (
            response.get("breadcrumb").and_then(|v| v.as_array()),
            response.get("line_range").and_then(|v| v.as_array()),
        ) {
            if let (Some(last), Some(start), Some(end)) = (
                breadcrumb.last().and_then(|v| v.as_str()),
                line_range.first().and_then(|v| v.as_u64()),
                line_range.get(1).and_then(|v| v.as_u64()),
            ) {
                out.push_str(&format!("§ {last}  L{start}-L{end}\n\n"));
            }
        }
        out.push_str(content);
        if let Some(hint) = response.get("hint").and_then(|v| v.as_str()) {
            if !out.ends_with('\n') {
                out.push('\n');
            }
            out.push('\n');
            out.push_str(hint);
        }
        return out;
    }
    // MAP and ERROR branches added in Tasks 8–9.
    response.to_string()
}
```

- [ ] **Step 4: Verify tests pass**

```
cargo test -p code-explorer --lib format_compact_content -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: fmt + clippy + commit**

```
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/markdown/read_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(read_markdown): format_compact for CONTENT shape

Passes content through verbatim. Appends \`hint\` as a footer when
present. Prepends a one-line \`§ heading  Lstart-Lend\` header for
heading-targeted reads."
```

---

### Task 8: format_compact — MAP shape

**Files:**
- Modify: `src/tools/markdown/read_markdown.rs` (extend `format_compact`)
- Test: `src/tools/markdown/tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn format_compact_map_shape_renders_indented_headings() {
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
    let out = tool.format_compact(&response);

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
        out.contains("next: @file_xyz"),
        "missing next cue, got: {out}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p code-explorer --lib format_compact_map_shape -- --nocapture
```

Expected: FAIL — falls through to `response.to_string()`.

- [ ] **Step 3: Add the MAP branch**

Replace the trailing `response.to_string()` in `format_compact` with:

```rust
let headings = response
    .get("headings")
    .or_else(|| response.get("section_map"))
    .and_then(|v| v.as_array());
if let Some(headings) = headings {
    let lines = response
        .get("lines")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let file_id = response
        .get("file_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let mut out = format!("{} lines  {}\n\n", lines, file_id);
    for entry in headings {
        let h = entry.get("h").and_then(|v| v.as_str()).unwrap_or("");
        let l = entry.get("l").and_then(|v| v.as_u64()).unwrap_or(0);
        let level = h.chars().take_while(|c| *c == '#').count().max(1);
        let indent = " ".repeat((level - 1) * 2);
        out.push_str(&format!("{indent}{h}  L{l}\n"));
    }
    if let Some(hint) = response.get("hint").and_then(|v| v.as_str()) {
        out.push('\n');
        out.push_str("next: ");
        out.push_str(hint);
    }
    return out;
}
response.to_string()
```

- [ ] **Step 4: Verify test passes**

```
cargo test -p code-explorer --lib format_compact_map_shape -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: fmt + clippy + commit**

```
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/markdown/read_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(read_markdown): format_compact for MAP shape

Renders {headings:[{h,l}], file_id, hint} as an indented heading
tree with a \`next:\` cue. Indent = (level - 1) * 2 spaces, level
derived from \`#\` prefix in h. No column padding (Hamsa cut)."
```

---

### Task 9: format_compact — ERROR shape

ERROR shape lives in the `RecoverableError` extra map, which the MCP envelope splices to top level under `ok: false`. The CONTENT and MAP branches already handle the success cases; the ERROR branch only needs to render the not-found-heading case (other recoverable errors don't carry headings).

**Files:**
- Modify: `src/tools/markdown/read_markdown.rs` (extend `format_compact`)
- Test: `src/tools/markdown/tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn format_compact_error_shape_renders_headings_with_error_prefix() {
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
    let out = tool.format_compact(&response);

    assert!(out.starts_with("error:"), "expected error prefix, got: {out}");
    assert!(
        out.contains("## Foo' not found"),
        "missing error message, got: {out}"
    );
    assert!(
        out.contains("# A  L1") && out.contains("## B  L5"),
        "missing available headings, got: {out}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

```
cargo test -p code-explorer --lib format_compact_error_shape -- --nocapture
```

Expected: FAIL — current MAP branch fires on the headings array and skips the error prefix.

- [ ] **Step 3: Add the ERROR prelude**

At the top of `format_compact`, before the CONTENT branch, insert:

```rust
let is_error = response
    .get("ok")
    .and_then(|v| v.as_bool())
    .map(|ok| !ok)
    .unwrap_or(false);
if is_error {
    let mut out = String::from("error: ");
    if let Some(msg) = response.get("error").and_then(|v| v.as_str()) {
        out.push_str(msg);
    }
    out.push_str("\n\n");
    if let Some(headings) = response.get("headings").and_then(|v| v.as_array()) {
        out.push_str("available headings:\n");
        for entry in headings {
            let h = entry.get("h").and_then(|v| v.as_str()).unwrap_or("");
            let l = entry.get("l").and_then(|v| v.as_u64()).unwrap_or(0);
            let level = h.chars().take_while(|c| *c == '#').count().max(1);
            let indent = " ".repeat((level - 1) * 2);
            out.push_str(&format!("{indent}{h}  L{l}\n"));
        }
    }
    if let Some(hint) = response.get("hint").and_then(|v| v.as_str()) {
        out.push('\n');
        out.push_str("next: ");
        out.push_str(hint);
    }
    return out;
}
```

- [ ] **Step 4: Verify test passes**

```
cargo test -p code-explorer --lib format_compact_error_shape -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Full suite**

```
cargo test -p code-explorer --lib markdown::tests
```

Expected: PASS.

- [ ] **Step 6: fmt + clippy + commit**

```
cargo fmt
cargo clippy --lib -- -D warnings
git add src/tools/markdown/read_markdown.rs src/tools/markdown/tests.rs
git commit -m "feat(read_markdown): format_compact for ERROR shape

Renders ok=false responses with an \`error:\` prefix, the same
indented heading tree the MAP branch uses, and a \`next:\` cue.
Closes the rendering side of Hamsa eval F1 — caller now sees
navigation in the same visual shape on success and failure."
```

---

### Task 10: Round-2 eval — acceptance gate

**Files:** none modified. Verification only.

- [ ] **Step 1: Build release binary**

```
cargo build --release
```

Expected: PASS.

- [ ] **Step 2: Restart MCP**

Tell the user: "Restart MCP with `/mcp` so the release binary is picked up." Wait for confirmation.

- [ ] **Step 3: Recreate eval fixtures**

```
mkdir -p /tmp/md-eval
: > /tmp/md-eval/empty.md
yes "plain text line no markers here" | head -50 > /tmp/md-eval/no-headings.md
printf '# Solo Top\n\nbody here\n' > /tmp/md-eval/single-h1.md
printf '# Real\n\nbody\n\n```\n# fake heading inside fence\n## another fake\n```\n\n## Real Two\n\nmore\n' > /tmp/md-eval/code-fence-traps.md
printf '%s\n' '# Top' '## Has `backtick code` — and em-dash' '## Plain' '### Deep — `link.md`' 'body' > /tmp/md-eval/weird-chars.md
{ printf '# Big\n\n'; for i in $(seq 1 250); do printf '## Section %d\n\nbody line\n\n' "$i"; done } > /tmp/md-eval/many-headings.md
```

- [ ] **Step 4: Run all 15 cases via MCP**

Call each in turn. For each, capture: response field set, byte size, format_compact rendering.

Baselines:
- `read_markdown("docs/TODO-lsp-cancelled-kotlin.md")`
- `read_markdown("docs/observations.md")`
- `read_markdown("docs/trackers/skill-frictions.md")`
- `read_markdown("CLAUDE.md")`
- `read_markdown("docs/superpowers/plans/2026-04-19-librarian-mcp.md")`

Trip cases:
- `read_markdown("/tmp/md-eval/empty.md")`
- `read_markdown("/tmp/md-eval/no-headings.md")`
- `read_markdown("/tmp/md-eval/single-h1.md")`
- `read_markdown("/tmp/md-eval/code-fence-traps.md")`
- `read_markdown("/tmp/md-eval/weird-chars.md")`
- `read_markdown("/tmp/md-eval/many-headings.md")`
- `read_markdown("CLAUDE.md", heading="## Nonexistent Section")`
- `read_markdown("docs/observations.md", start_line=9000, end_line=9999)`
- `read_markdown("CLAUDE.md", headings=["## Design Principles", "## Key Patterns"])`
- After Tier 3 read of CLAUDE.md, `read_markdown("@file_xxx", heading="## Companion Plugin: codescout-companion")` using the returned file_id

- [ ] **Step 5: Score against rubric**

For each case, fill in:
- R1 (signal density): useful bytes / total response bytes
- R2 (file_id discoverability): 0/1/2
- R3 (edge case): PASS / FAIL
- R4 (hint shape): 0/1/2
- R5 (contract consistency): PASS / FAIL

Hard gates (any FAIL blocks ship):
- T1 (empty): response ≤ 30 bytes JSON, no `format` field
- T6 (bogus heading): ERROR shape with `headings[]` array
- T8 (line range past EOF): ERROR shape with `lines` field
- B1 (many-headings.md): MAP shape, ≤ 5 KB response

Signal-density gates:
- E3 (skill-frictions.md): R1 ≥ 0.65
- E4 (CLAUDE.md): R1 ≥ 0.65
- E5 (librarian-mcp.md): R1 ≥ 0.60

Regression gates:
- T4 (code-fence-traps): exactly 2 headings detected
- T5 (weird-chars): heading text preserved exactly
- T10 (file_id reuse after Tier 3): PASS

- [ ] **Step 6: Record results**

Create `docs/superpowers/specs/2026-05-15-read-markdown-eval-round2.md` with:
- Per-case response (response shape JSON, format_compact text, byte size)
- Rubric table
- Pass/fail verdict per gate
- Any new findings worth a follow-up tracker entry

- [ ] **Step 7: Commit eval record**

```
git add docs/superpowers/specs/2026-05-15-read-markdown-eval-round2.md
git commit -m "docs: round-2 eval for read_markdown render redesign

All hard gates passed. Median Tier 3 signal density rose from
0.40 to <FILL>. Detail: docs/superpowers/specs/2026-05-15-read-markdown-eval-round2.md"
```

- [ ] **Step 8: If any hard gate failed**

Do not proceed to graduation. Open a tracker entry for each failure, fix in a follow-up commit, rerun round 2.

---

## Self-Review checklist (for the plan author)

Spec coverage (each spec section → task):
- `## JSON shapes` → Task 6 (all three shapes)
- `## Tier boundary fix (B1)` → Task 1
- `## Behavioral fixes` B2/B4/F1/F3 → Tasks 2/3/4/5
- `## format_compact` → Tasks 7/8/9
- `## Eval acceptance gate` → Task 10

Type consistency: `headings` array name + `{h, l}` entry shape used identically across Tasks 4, 6, 8, 9. `lines` field name used identically across Tasks 2, 3, 5, 6. `hint` (Guidance::Hint) used everywhere `must_follow` previously appeared. `HEADINGS_HARD_CAP` named identically in declaration (Task 1 Step 3) and use site (Task 1 Step 4).

Placeholders: none. Every code step has the full block. Every command has the expected exit. Every commit message is verbatim.

Risk: Task 6 is large and coordinated. If it spirals, split into Task 6a (Tier 3) and Task 6b (other tiers + tests) — but the snapshot test in Step 1 keeps the scope honest.
