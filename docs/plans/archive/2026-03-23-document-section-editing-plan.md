# Document Section Editing — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add heading-addressed editing, read-coverage tracking, and enhanced read/edit operations for markdown files in codescout.

**Architecture:** Six features built on a shared `parse_all_headings()` foundation in `file_summary.rs`. A new `section_edit.rs` module houses the `EditSection` tool. `SectionCoverage` is session-scoped state on `ToolContext`. Existing `ReadFile` and `EditFile` tools get new optional params (`headings`, `heading`, `edits`).

**Tech Stack:** Rust, serde_json, regex (for inline format stripping), tokio (async), anyhow/RecoverableError

**Spec:** `docs/plans/2026-03-23-document-section-editing-design.md`

---

## Chunk 1: Foundation — Heading Parsing & Format Stripping

### Task 1: Extract `parse_all_headings` from `summarize_markdown`

**Files:**
- Modify: `src/tools/file_summary.rs:79-133` (refactor `summarize_markdown` + `heading_level`)

This task extracts the heading-parsing logic from `summarize_markdown` into a reusable `parse_all_headings()` function. `summarize_markdown` currently parses headings inline and truncates to 30. We split this into: (1) `parse_all_headings` that returns ALL headings, and (2) `summarize_markdown` calls it and truncates for display.

- [ ] **Step 1: Write failing tests for `parse_all_headings`**

```rust
// In src/tools/file_summary.rs, inside the existing #[cfg(test)] mod tests block

#[test]
fn parse_all_headings_basic() {
    let content = "# Title\ntext\n## Setup\ndo this\n## Usage\nuse it";
    let headings = parse_all_headings(content);
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].text, "# Title");
    assert_eq!(headings[0].level, 1);
    assert_eq!(headings[0].line, 1);
    assert_eq!(headings[0].end_line, 6);
    assert_eq!(headings[1].text, "## Setup");
    assert_eq!(headings[1].line, 3);
    assert_eq!(headings[1].end_line, 4);
    assert_eq!(headings[2].text, "## Usage");
    assert_eq!(headings[2].line, 5);
    assert_eq!(headings[2].end_line, 6);
}

#[test]
fn parse_all_headings_skips_code_blocks() {
    let content = "# Title\n```\n## Not a heading\n```\n## Real heading\ntext";
    let headings = parse_all_headings(content);
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0].text, "# Title");
    assert_eq!(headings[1].text, "## Real heading");
}

#[test]
fn parse_all_headings_no_truncation() {
    // 35 headings — must all be returned (old summarize_markdown truncated at 30)
    let mut content = String::from("# Title\n");
    for i in 1..=35 {
        content.push_str(&format!("## Section {i}\ntext\n"));
    }
    let headings = parse_all_headings(&content);
    assert_eq!(headings.len(), 36); // 1 title + 35 sections
}

#[test]
fn parse_all_headings_empty_doc() {
    let headings = parse_all_headings("no headings here\njust text");
    assert!(headings.is_empty());
}
```

Also add the `HeadingInfo` struct (above the test block, public):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct HeadingInfo {
    pub text: String,    // e.g. "## Setup"
    pub level: usize,    // 1-6
    pub line: usize,     // 1-indexed
    pub end_line: usize, // 1-indexed, inclusive
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test parse_all_headings -- --nocapture 2>&1 | head -40`
Expected: FAIL — `parse_all_headings` function not found

- [ ] **Step 3: Implement `parse_all_headings`**

```rust
/// Parse all markdown headings with their line ranges. No truncation.
/// Skips headings inside fenced code blocks.
pub fn parse_all_headings(content: &str) -> Vec<HeadingInfo> {
    let line_count = content.lines().count();
    let mut in_code_block = false;

    // First pass: collect heading positions
    let mut raw: Vec<(String, usize, usize)> = Vec::new(); // (text, level, line_1indexed)
    for (idx, line) in content.lines().enumerate() {
        if line.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if let Some(level) = heading_level(line) {
            raw.push((line.to_string(), level, idx + 1));
        }
    }

    // Second pass: compute end_line for each heading
    raw.iter()
        .enumerate()
        .map(|(i, (text, level, line))| {
            let end_line = raw[i + 1..]
                .iter()
                .find(|(_, l, _)| *l <= *level)
                .map(|(_, _, next_line)| next_line - 1)
                .unwrap_or(line_count);
            HeadingInfo {
                text: text.clone(),
                level: *level,
                line: *line,
                end_line,
            }
        })
        .collect()
}
```

- [ ] **Step 4: Refactor `summarize_markdown` to use `parse_all_headings`**

Replace the body of `summarize_markdown` (lines 79-121) with:

```rust
pub fn summarize_markdown(content: &str) -> Value {
    let line_count = content.lines().count();
    let all_headings = parse_all_headings(content);

    let mut headings: Vec<Value> = all_headings
        .iter()
        .map(|h| {
            serde_json::json!({
                "heading": h.text,
                "level": h.level,
                "line": h.line,
                "end_line": h.end_line,
            })
        })
        .collect();

    headings.truncate(30); // Display cap only

    serde_json::json!({
        "type": "markdown",
        "line_count": line_count,
        "headings": headings,
    })
}
```

- [ ] **Step 5: Run all tests to verify nothing broke**

Run: `cargo test -p codescout 2>&1 | tail -20`
Expected: All existing tests pass + new `parse_all_headings_*` tests pass

- [ ] **Step 6: Commit**

```bash
git add src/tools/file_summary.rs
git commit -m "refactor: extract parse_all_headings from summarize_markdown

No truncation in the parser — summarize_markdown truncates at display layer only.
Foundation for edit_section and resolve_section_range."
```

### Task 2: Add `strip_inline_formatting` and format-aware heading matching

**Files:**
- Modify: `src/tools/file_summary.rs`

- [ ] **Step 1: Write failing tests for `strip_inline_formatting`**

```rust
#[test]
fn strip_inline_formatting_backticks() {
    assert_eq!(strip_inline_formatting("## The `auth` Module"), "## The auth Module");
}

#[test]
fn strip_inline_formatting_bold() {
    assert_eq!(strip_inline_formatting("## **Important** Notes"), "## Important Notes");
}

#[test]
fn strip_inline_formatting_italic() {
    assert_eq!(strip_inline_formatting("## _Setup_ Guide"), "## Setup Guide");
}

#[test]
fn strip_inline_formatting_mixed() {
    assert_eq!(
        strip_inline_formatting("## The `auth` **middleware** _layer_"),
        "## The auth middleware layer"
    );
}

#[test]
fn strip_inline_formatting_no_formatting() {
    assert_eq!(strip_inline_formatting("## Plain heading"), "## Plain heading");
}

#[test]
fn strip_inline_formatting_collapses_spaces() {
    assert_eq!(strip_inline_formatting("##  Extra   spaces "), "## Extra spaces");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test strip_inline_formatting -- --nocapture 2>&1 | head -20`
Expected: FAIL — function not found

- [ ] **Step 3: Implement `strip_inline_formatting`**

```rust
/// Strip inline markdown formatting from a heading string.
/// Removes backtick spans, bold/italic markers, collapses spaces, trims.
pub fn strip_inline_formatting(s: &str) -> String {
    let mut result = s.to_string();
    // Remove backtick spans: `code` → code
    loop {
        let Some(start) = result.find('`') else { break };
        let Some(end) = result[start + 1..].find('`') else { break };
        let inner = result[start + 1..start + 1 + end].to_string();
        result = format!("{}{}{}", &result[..start], inner, &result[start + 1 + end + 1..]);
    }
    // Remove bold/italic: **text** → text, __text__ → text, *text* → text, _text_ → text
    // Order matters: ** before *, __ before _
    for marker in &["**", "__", "*", "_"] {
        while let Some(start) = result.find(marker) {
            if let Some(end) = result[start + marker.len()..].find(marker) {
                let inner = &result[start + marker.len()..start + marker.len() + end];
                result = format!(
                    "{}{}{}",
                    &result[..start],
                    inner,
                    &result[start + marker.len() + end + marker.len()..]
                );
            } else {
                break;
            }
        }
    }
    // Collapse multiple spaces to single, trim
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test strip_inline_formatting -- --nocapture 2>&1 | head -20`
Expected: All 6 tests pass

- [ ] **Step 5: Commit**

```bash
git add src/tools/file_summary.rs
git commit -m "feat: add strip_inline_formatting for heading matching

Strips backticks, bold, italic markers and collapses whitespace.
Used by resolve_section_range for format-aware heading matching."
```

### Task 3: Implement `resolve_section_range`

**Files:**
- Modify: `src/tools/file_summary.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn resolve_section_range_exact_match() {
    let content = "# Title\ntext\n## Setup\ndo this\n## Usage\nuse it";
    let range = resolve_section_range(content, "## Setup").unwrap();
    assert_eq!(range.heading_line, 3);
    assert_eq!(range.body_start_line, 4);
    assert_eq!(range.end_line, 4);
    assert_eq!(range.heading_text, "## Setup");
    assert_eq!(range.level, 2);
}

#[test]
fn resolve_section_range_stripped_match() {
    let content = "# Title\n## The `auth` Module\ndetails";
    let range = resolve_section_range(content, "## The auth Module").unwrap();
    assert_eq!(range.heading_text, "## The `auth` Module");
    assert_eq!(range.heading_line, 2);
}

#[test]
fn resolve_section_range_prefix_match() {
    let content = "# Title\n## Authentication Guide\ndetails";
    let range = resolve_section_range(content, "## Auth").unwrap();
    assert_eq!(range.heading_text, "## Authentication Guide");
}

#[test]
fn resolve_section_range_empty_section() {
    let content = "# Title\n## Empty\n## Next\nstuff";
    let range = resolve_section_range(content, "## Empty").unwrap();
    assert_eq!(range.heading_line, 2);
    assert_eq!(range.body_start_line, 3);
    assert_eq!(range.end_line, 2); // body_start > end_line means empty
}

#[test]
fn resolve_section_range_last_section() {
    let content = "# Title\n## Last\nfinal content\nmore";
    let range = resolve_section_range(content, "## Last").unwrap();
    assert_eq!(range.end_line, 4); // extends to EOF
}

#[test]
fn resolve_section_range_not_found() {
    let content = "# Title\n## Setup\ntext";
    let err = resolve_section_range(content, "## Nonexistent").unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn resolve_section_range_duplicate_heading_error() {
    let content = "# Title\n## Example\nfirst\n## Other\n## Example\nsecond";
    let err = resolve_section_range(content, "## Example").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("2") || msg.contains("multiple"), "should mention duplicate count: {msg}");
}

#[test]
fn resolve_section_range_nested_sections() {
    let content = "# Title\n## Parent\nparent text\n### Child\nchild text\n## Sibling\nsibling";
    let range = resolve_section_range(content, "## Parent").unwrap();
    assert_eq!(range.heading_line, 2);
    assert_eq!(range.end_line, 5); // includes ### Child
}

#[test]
fn resolve_section_range_heading_in_code_block() {
    let content = "# Title\n```\n## Not a heading\n```\n## Real\ntext";
    let range = resolve_section_range(content, "## Real").unwrap();
    assert_eq!(range.heading_line, 5);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test resolve_section_range -- --nocapture 2>&1 | head -20`
Expected: FAIL — function not found

- [ ] **Step 3: Implement `SectionRange` and `resolve_section_range`**

Add the struct near `HeadingInfo`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct SectionRange {
    pub heading_line: usize,    // 1-indexed
    pub body_start_line: usize, // heading_line + 1
    pub end_line: usize,        // inclusive, last line of section
    pub heading_text: String,   // raw heading text (with formatting)
    pub level: usize,           // 1-6
}
```

Implement the function:

```rust
/// Resolve a heading query to a precise line range in a markdown document.
/// Uses a 4-tier matching cascade: exact raw → exact stripped → prefix stripped → substring stripped.
/// Errors on duplicate exact matches (ambiguity) or no match.
pub fn resolve_section_range(
    content: &str,
    heading_query: &str,
) -> Result<SectionRange, RecoverableError> {
    let headings = parse_all_headings(content);

    if headings.is_empty() {
        return Err(RecoverableError::with_hint(
            "no headings found in file",
            "The file contains no Markdown headings to navigate",
        ));
    }

    let query_stripped = strip_inline_formatting(heading_query);
    let query_stripped_lower = query_stripped.to_lowercase();

    // Tier 1: Exact match (raw)
    let exact_raw: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| h.text == heading_query)
        .map(|(i, _)| i)
        .collect();

    if exact_raw.len() == 1 {
        let h = &headings[exact_raw[0]];
        return Ok(SectionRange {
            heading_line: h.line,
            body_start_line: h.line + 1,
            end_line: h.end_line,
            heading_text: h.text.clone(),
            level: h.level,
        });
    }
    if exact_raw.len() > 1 {
        let lines: Vec<String> = exact_raw.iter().map(|&i| headings[i].line.to_string()).collect();
        return Err(RecoverableError::with_hint(
            format!(
                "heading '{}' found {} times (lines {})",
                heading_query,
                exact_raw.len(),
                lines.join(", ")
            ),
            "Provide a more specific heading or use edit_file with start_line/end_line to target a specific occurrence.",
        ));
    }

    // Tier 2: Exact match (stripped)
    let exact_stripped: Vec<usize> = headings
        .iter()
        .enumerate()
        .filter(|(_, h)| strip_inline_formatting(&h.text) == query_stripped)
        .map(|(i, _)| i)
        .collect();

    if exact_stripped.len() == 1 {
        let h = &headings[exact_stripped[0]];
        return Ok(SectionRange {
            heading_line: h.line,
            body_start_line: h.line + 1,
            end_line: h.end_line,
            heading_text: h.text.clone(),
            level: h.level,
        });
    }
    if exact_stripped.len() > 1 {
        let lines: Vec<String> = exact_stripped.iter().map(|&i| headings[i].line.to_string()).collect();
        return Err(RecoverableError::with_hint(
            format!(
                "heading '{}' found {} times (lines {})",
                heading_query,
                exact_stripped.len(),
                lines.join(", ")
            ),
            "Provide a more specific heading or use edit_file with start_line/end_line to target a specific occurrence.",
        ));
    }

    // Tier 3: Prefix match (stripped, case-insensitive)
    if let Some(idx) = headings.iter().position(|h| {
        strip_inline_formatting(&h.text)
            .to_lowercase()
            .starts_with(&query_stripped_lower)
    }) {
        let h = &headings[idx];
        return Ok(SectionRange {
            heading_line: h.line,
            body_start_line: h.line + 1,
            end_line: h.end_line,
            heading_text: h.text.clone(),
            level: h.level,
        });
    }

    // Tier 4: Substring match (stripped, case-insensitive)
    if let Some(idx) = headings.iter().position(|h| {
        strip_inline_formatting(&h.text)
            .to_lowercase()
            .contains(&query_stripped_lower)
    }) {
        let h = &headings[idx];
        return Ok(SectionRange {
            heading_line: h.line,
            body_start_line: h.line + 1,
            end_line: h.end_line,
            heading_text: h.text.clone(),
            level: h.level,
        });
    }

    // No match
    let available: Vec<&str> = headings.iter().map(|h| h.text.as_str()).take(15).collect();
    Err(RecoverableError::with_hint(
        format!("heading '{}' not found", heading_query),
        format!("Available headings: {}", available.join(", ")),
    ))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test resolve_section_range -- --nocapture 2>&1 | head -40`
Expected: All 9 tests pass

- [ ] **Step 5: Run full test suite + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -10`
Expected: All clean

- [ ] **Step 6: Commit**

```bash
git add src/tools/file_summary.rs
git commit -m "feat: add resolve_section_range with format-aware heading matching

4-tier cascade: exact raw → exact stripped → prefix → substring.
Errors on duplicate headings with line numbers for disambiguation.
Strips inline markdown formatting (backticks, bold, italic) before matching."
```

### Task 3b: Refactor `extract_markdown_section` to use `resolve_section_range`

**Files:**
- Modify: `src/tools/file_summary.rs:135-227` (refactor `extract_markdown_section`)

The existing `extract_markdown_section` calls `summarize_markdown` which truncates to 30 headings. It also has its own 3-tier matching cascade that lacks the stripped-match tier. Refactor it to delegate to `resolve_section_range`, getting both no-truncation and format-aware matching for free.

- [ ] **Step 1: Write test verifying heading > 30 works**

```rust
#[test]
fn extract_markdown_section_beyond_30_headings() {
    let mut content = String::from("# Title\n");
    for i in 1..=35 {
        content.push_str(&format!("## Section {i}\ncontent {i}\n"));
    }
    // Section 35 is beyond the old 30-heading truncation
    let result = extract_markdown_section(&content, "## Section 35").unwrap();
    assert!(result.content.contains("content 35"));
}

#[test]
fn extract_markdown_section_stripped_match() {
    let content = "# Title\n## The `auth` Module\ndetails here\n";
    let result = extract_markdown_section(&content, "## The auth Module").unwrap();
    assert!(result.content.contains("details here"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test extract_markdown_section_beyond -- --nocapture && cargo test extract_markdown_section_stripped -- --nocapture`
Expected: FAIL — first one can't find heading, second one can't match without stripping

- [ ] **Step 3: Refactor `extract_markdown_section` to delegate to `resolve_section_range`**

```rust
pub fn extract_markdown_section(
    content: &str,
    heading_query: &str,
) -> Result<SectionResult, RecoverableError> {
    let range = resolve_section_range(content, heading_query)?;
    let all_headings = parse_all_headings(content);

    // Extract content
    let lines: Vec<&str> = content.lines().collect();
    let start = (range.heading_line - 1).min(lines.len());
    let end = range.end_line.min(lines.len());
    let section_content = lines[start..end].join("\n");

    // Build breadcrumb: walk backwards collecting parents (lower level numbers)
    let mut breadcrumb = Vec::new();
    let mut current_level = range.level;
    for h in all_headings.iter().rev() {
        if h.line > range.heading_line {
            continue;
        }
        if h.level < current_level || h.line == range.heading_line {
            breadcrumb.push(h.text.clone());
            current_level = h.level;
        }
    }
    breadcrumb.reverse();

    // Find siblings: same level headings (excluding the matched one)
    let siblings: Vec<String> = all_headings
        .iter()
        .filter(|h| h.level == range.level && h.text != range.heading_text)
        .map(|h| h.text.clone())
        .collect();

    Ok(SectionResult {
        content: section_content,
        line_range: (range.heading_line, range.end_line),
        breadcrumb,
        siblings,
        format: "markdown".to_string(),
    })
}
```

- [ ] **Step 4: Run tests — existing + new**

Run: `cargo test extract_markdown_section -- --nocapture 2>&1 | head -30`
Expected: All pass (existing exact/prefix/not_found tests + new beyond-30 and stripped tests)

- [ ] **Step 5: Commit**

```bash
git add src/tools/file_summary.rs
git commit -m "refactor: extract_markdown_section delegates to resolve_section_range

Removes 30-heading truncation from read_file heading= path.
Gets format-aware matching (stripped backticks/bold) for free."
```

---

## Chunk 2: `SectionCoverage` + `ToolContext` Wiring

### Task 4: Add `SectionCoverage` struct and wire into `ToolContext`

**Files:**
- Create: `src/tools/section_coverage.rs`
- Modify: `src/tools/mod.rs:64-73` (add field to `ToolContext`)
- Modify: `src/server.rs:37-48` (add field to `CodeScoutServer`) and `src/server.rs:97-105` (initialize in `from_parts`) and `src/server.rs:145-151` (pass in `call_tool`)

- [ ] **Step 1: Create `section_coverage.rs` with struct and methods**

```rust
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

/// Session-scoped tracking of which markdown sections have been read.
/// Used to hint when an LLM acts on a file it hasn't fully read.
#[derive(Debug, Default)]
pub struct SectionCoverage {
    /// canonical_path → set of heading texts that have been "seen"
    seen: HashMap<PathBuf, HashSet<String>>,
    /// mtime at time of recording — external changes invalidate coverage
    mtimes: HashMap<PathBuf, SystemTime>,
}

/// Coverage status for a single file.
pub struct CoverageStatus {
    pub read_count: usize,
    pub total_count: usize,
    pub unread: Vec<String>,
}

impl SectionCoverage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record headings as "seen" for a file. Updates mtime.
    pub fn mark_seen(&mut self, path: &PathBuf, headings: &[String]) {
        let entry = self.seen.entry(path.clone()).or_default();
        for h in headings {
            entry.insert(h.clone());
        }
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(mtime) = meta.modified() {
                self.mtimes.insert(path.clone(), mtime);
            }
        }
    }

    /// Update stored mtime after an in-session write (prevents spurious invalidation).
    pub fn update_mtime(&mut self, path: &PathBuf) {
        if let Ok(meta) = std::fs::metadata(path) {
            if let Ok(mtime) = meta.modified() {
                self.mtimes.insert(path.clone(), mtime);
            }
        }
    }

    /// Check if coverage data exists and is still valid for this file.
    /// Returns None if no coverage recorded or mtime changed externally.
    fn validate(&mut self, path: &PathBuf) -> bool {
        if !self.seen.contains_key(path) {
            return false;
        }
        // Check if file was externally modified
        if let Some(stored_mtime) = self.mtimes.get(path) {
            if let Ok(meta) = std::fs::metadata(path) {
                if let Ok(current_mtime) = meta.modified() {
                    if current_mtime != *stored_mtime {
                        // Externally modified — invalidate
                        self.seen.remove(path);
                        self.mtimes.remove(path);
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Get coverage status for a file given its full heading list.
    /// Returns None if no coverage is tracked for this file.
    pub fn status(&mut self, path: &PathBuf, all_headings: &[String]) -> Option<CoverageStatus> {
        if !self.validate(path) {
            return None;
        }
        let seen = self.seen.get(path)?;
        let unread: Vec<String> = all_headings
            .iter()
            .filter(|h| !seen.contains(*h))
            .cloned()
            .collect();
        Some(CoverageStatus {
            read_count: all_headings.len() - unread.len(),
            total_count: all_headings.len(),
            unread,
        })
    }

    /// Check if there are unread sections (for write-time hints).
    /// Returns None if no coverage tracked for the file.
    pub fn unread_hint(&mut self, path: &PathBuf, all_headings: &[String]) -> Option<String> {
        let status = self.status(path, all_headings)?;
        if status.unread.is_empty() {
            return None;
        }
        let preview: Vec<&str> = status.unread.iter().map(|s| s.as_str()).take(5).collect();
        let suffix = if status.unread.len() > 5 {
            format!(", ... ({} more)", status.unread.len() - 5)
        } else {
            String::new()
        };
        Some(format!(
            "{} unread sections in this file: {}{}",
            status.unread.len(),
            preview.join(", "),
            suffix,
        ))
    }
}
```

- [ ] **Step 2: Write tests for `SectionCoverage`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_temp_file(content: &str) -> (NamedTempFile, PathBuf) {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f.flush().unwrap();
        let path = f.path().to_path_buf();
        (f, path)
    }

    #[test]
    fn mark_seen_and_status() {
        let (_f, path) = make_temp_file("# Title\n## A\n## B\n## C\n");
        let mut cov = SectionCoverage::new();
        let all = vec!["# Title".into(), "## A".into(), "## B".into(), "## C".into()];

        // No coverage yet
        assert!(cov.status(&path, &all).is_none());

        // Mark some as seen
        cov.mark_seen(&path, &["# Title".into(), "## A".into()]);
        let s = cov.status(&path, &all).unwrap();
        assert_eq!(s.read_count, 2);
        assert_eq!(s.total_count, 4);
        assert_eq!(s.unread, vec!["## B", "## C"]);
    }

    #[test]
    fn mark_all_seen_no_unread() {
        let (_f, path) = make_temp_file("# Title\n## A\n");
        let mut cov = SectionCoverage::new();
        let all = vec!["# Title".into(), "## A".into()];
        cov.mark_seen(&path, &["# Title".into(), "## A".into()]);
        let s = cov.status(&path, &all).unwrap();
        assert!(s.unread.is_empty());
    }

    #[test]
    fn mtime_invalidation() {
        let (f, path) = make_temp_file("# Title\n## A\n");
        let mut cov = SectionCoverage::new();
        cov.mark_seen(&path, &["# Title".into(), "## A".into()]);

        // Externally modify the file
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&path, "# Title\n## A\n## B\n").unwrap();

        let all = vec!["# Title".into(), "## A".into(), "## B".into()];
        // Coverage should be invalidated
        assert!(cov.status(&path, &all).is_none());
        drop(f); // keep tempfile alive until here
    }

    #[test]
    fn update_mtime_prevents_invalidation() {
        let (_f, path) = make_temp_file("# Title\n## A\n");
        let mut cov = SectionCoverage::new();
        cov.mark_seen(&path, &["# Title".into(), "## A".into()]);

        // Simulate in-session write
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&path, "# Title\n## A modified\n").unwrap();
        cov.update_mtime(&path); // <-- this is what edit_section/edit_file would call

        let all = vec!["# Title".into(), "## A".into()];
        // Coverage should still be valid
        assert!(cov.status(&path, &all).is_some());
    }

    #[test]
    fn unread_hint_format() {
        let (_f, path) = make_temp_file("x");
        let mut cov = SectionCoverage::new();
        cov.mark_seen(&path, &["# Title".into()]);
        let all: Vec<String> = (0..7).map(|i| format!("## Section {i}")).collect();
        let all_with_title = std::iter::once("# Title".to_string()).chain(all.clone()).collect::<Vec<_>>();
        let hint = cov.unread_hint(&path, &all_with_title).unwrap();
        assert!(hint.contains("7 unread"));
        assert!(hint.contains("2 more")); // 7 total, showing 5
    }

    #[test]
    fn no_hint_when_no_coverage() {
        let (_f, path) = make_temp_file("x");
        let mut cov = SectionCoverage::new();
        let all = vec!["## A".into()];
        assert!(cov.unread_hint(&path, &all).is_none());
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test section_coverage -- --nocapture 2>&1 | head -30`
Expected: All 6 tests pass

- [ ] **Step 4: Add `pub mod section_coverage;` to `src/tools/mod.rs`**

Add after the last existing `pub mod` line in the tools module.

- [ ] **Step 5: Wire `SectionCoverage` into `ToolContext`**

In `src/tools/mod.rs`, add to the `ToolContext` struct (after the `peer` field):

```rust
    /// Session-scoped markdown section read-coverage tracker.
    pub section_coverage: std::sync::Arc<std::sync::Mutex<section_coverage::SectionCoverage>>,
```

In `src/server.rs`, add to `CodeScoutServer` struct:

```rust
    section_coverage: Arc<std::sync::Mutex<crate::tools::section_coverage::SectionCoverage>>,
```

In `from_parts`, after `let output_buffer = ...`:

```rust
        let section_coverage = Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        ));
```

Add `section_coverage` to the `Self { ... }` return in `from_parts`.

In `call_tool_inner` where `ToolContext` is constructed (around line 145-151), add:

```rust
            section_coverage: self.section_coverage.clone(),
```

- [ ] **Step 6: Update `test_ctx()` in `src/tools/file.rs`**

Every test file that creates a `ToolContext` needs the new field. In `src/tools/file.rs` `test_ctx()` (line 1646), add:

```rust
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
```

Search for other `test_ctx` or `ToolContext {` constructions across the codebase and add the field there too:

Run: `cargo grep "ToolContext {" src/` to find all construction sites. Each one needs the new field.

- [ ] **Step 7: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -20`
Expected: All clean — no behavior change yet, just wiring

- [ ] **Step 8: Commit**

```bash
git add src/tools/section_coverage.rs src/tools/mod.rs src/server.rs src/tools/file.rs
git commit -m "feat: add SectionCoverage and wire into ToolContext

Session-scoped tracking of which markdown sections have been read.
Supports mtime-based invalidation with in-session write protection.
No behavioral changes yet — just the infrastructure."
```

---

## Chunk 3: `EditSection` Tool

### Task 5: Implement `EditSection` tool

**Files:**
- Create: `src/tools/section_edit.rs`
- Modify: `src/tools/mod.rs` (add `pub mod section_edit;`)
- Modify: `src/server.rs:60-105` (register tool in `from_parts`)
- Modify: `src/util/path_security.rs:358-360` (add to `check_tool_access`)

- [ ] **Step 1: Write failing tests for section edit operations**

Create `src/tools/section_edit.rs` with tests first:

```rust
use anyhow::Result;
use serde_json::{json, Value};

use super::file_summary::{resolve_section_range, strip_inline_formatting};
use super::{guard_worktree_write, require_str_param, RecoverableError, Tool, ToolContext};
use crate::util::path_security;

/// Perform a section edit operation on a markdown file.
fn perform_section_edit(
    content: &str,
    heading_query: &str,
    action: &str,
    new_content: Option<&str>,
) -> Result<String, RecoverableError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_body_only() {
        let content = "# Title\n## Setup\nold content\nmore old\n## Usage\nuse it\n";
        let result = perform_section_edit(content, "## Setup", "replace", Some("new content\n")).unwrap();
        assert_eq!(result, "# Title\n## Setup\nnew content\n## Usage\nuse it\n");
    }

    #[test]
    fn replace_with_heading() {
        let content = "# Title\n## Setup\nold content\n## Usage\nuse it\n";
        let result = perform_section_edit(content, "## Setup", "replace", Some("## Installation\nnew steps\n")).unwrap();
        assert_eq!(result, "# Title\n## Installation\nnew steps\n## Usage\nuse it\n");
    }

    #[test]
    fn replace_empty_section() {
        let content = "# Title\n## Empty\n## Next\nstuff\n";
        let result = perform_section_edit(content, "## Empty", "replace", Some("now has content\n")).unwrap();
        assert_eq!(result, "# Title\n## Empty\nnow has content\n## Next\nstuff\n");
    }

    #[test]
    fn insert_before() {
        let content = "# Title\n## Setup\ncontent\n";
        let result = perform_section_edit(content, "## Setup", "insert_before", Some("## Prerequisites\ninstall stuff\n")).unwrap();
        assert_eq!(result, "# Title\n## Prerequisites\ninstall stuff\n## Setup\ncontent\n");
    }

    #[test]
    fn insert_after() {
        let content = "# Title\n## Setup\ncontent\n## Usage\nuse it\n";
        let result = perform_section_edit(content, "## Setup", "insert_after", Some("\n## Testing\ntest it\n")).unwrap();
        assert_eq!(result, "# Title\n## Setup\ncontent\n\n## Testing\ntest it\n## Usage\nuse it\n");
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
        let result = perform_section_edit(content, "## Parent", "replace", Some("replaced all\n")).unwrap();
        assert_eq!(result, "# Title\n## Parent\nreplaced all\n## Sibling\nsibling\n");
    }

    #[test]
    fn trailing_newline_normalization() {
        let content = "# Title\n## Setup\ncontent";  // no trailing newline
        let result = perform_section_edit(content, "## Setup", "replace", Some("new")).unwrap();
        assert!(result.ends_with('\n'), "result should end with newline: {:?}", result);
    }

    #[test]
    fn remove_only_section() {
        let content = "## Only\ncontent\n";
        let result = perform_section_edit(content, "## Only", "remove", None).unwrap();
        // Should produce empty or minimal valid content
        assert!(result.trim().is_empty() || result == "\n");
    }

    #[test]
    fn consecutive_edits() {
        // First edit changes content, second must work on the new content
        let content = "# Title\n## A\noriginal a\n## B\noriginal b\n";
        let after_first = perform_section_edit(content, "## A", "replace", Some("updated a\n")).unwrap();
        assert!(after_first.contains("updated a"));
        let after_second = perform_section_edit(&after_first, "## B", "replace", Some("updated b\n")).unwrap();
        assert!(after_second.contains("updated a"));
        assert!(after_second.contains("updated b"));
    }

    #[test]
    fn smart_replace_detection_non_heading() {
        // Content starting with # but not a valid heading (no space after #, or 7+ hashes)
        let content = "# Title\n## Setup\nold content\n";
        let result = perform_section_edit(content, "## Setup", "replace", Some("#hashtag comment\n")).unwrap();
        // Should be body-only replace since "#hashtag" is not a valid heading
        assert!(result.contains("## Setup"));
        assert!(result.contains("#hashtag comment"));
    }

    #[test]
    fn heading_inside_code_block_edit() {
        let content = "# Title\n## Real\ncontent\n```\n## Fake\n```\n";
        let result = perform_section_edit(content, "## Real", "replace", Some("new content\n")).unwrap();
        assert!(result.contains("## Real"));
        assert!(result.contains("new content"));
        assert!(result.contains("## Fake")); // code block preserved
    }

    #[test]
    fn duplicate_heading_edit_error() {
        let content = "# Title\n## Example\nfirst\n## Other\n## Example\nsecond\n";
        let err = perform_section_edit(content, "## Example", "replace", Some("x")).unwrap_err();
        assert!(err.to_string().contains("found") && err.to_string().contains("times"));
    }

    #[test]
    fn heading_not_found() {
        let content = "# Title\n## Setup\ntext";
        let err = perform_section_edit(content, "## Nonexistent", "replace", Some("x")).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn missing_content_for_replace() {
        let content = "# Title\n## Setup\ntext";
        let err = perform_section_edit(content, "## Setup", "replace", None).unwrap_err();
        assert!(err.to_string().contains("content"));
    }

    #[test]
    fn invalid_action() {
        let content = "# Title\n## Setup\ntext";
        let err = perform_section_edit(content, "## Setup", "invalid", Some("x")).unwrap_err();
        assert!(err.to_string().contains("invalid"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test section_edit -- --nocapture 2>&1 | head -20`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement `perform_section_edit`**

```rust
/// Perform a section edit operation on markdown content.
/// Returns the modified content string.
fn perform_section_edit(
    content: &str,
    heading_query: &str,
    action: &str,
    new_content: Option<&str>,
) -> Result<String, RecoverableError> {
    // Validate action
    if !["replace", "insert_before", "insert_after", "remove"].contains(&action) {
        return Err(RecoverableError::with_hint(
            format!("invalid action: {action:?}"),
            "action must be one of: replace, insert_before, insert_after, remove",
        ));
    }

    // Require content for non-remove actions
    if action != "remove" && new_content.is_none() {
        return Err(RecoverableError::with_hint(
            format!("content is required for action '{action}'"),
            "Provide the content parameter with the new text.",
        ));
    }

    let range = resolve_section_range(content, heading_query)?;
    let lines: Vec<&str> = content.lines().collect();
    let mut result_lines: Vec<&str> = Vec::with_capacity(lines.len());

    match action {
        "replace" => {
            let new = new_content.unwrap();
            let starts_with_heading = new.lines().next().map_or(false, |first| {
                let trimmed = first.trim_start();
                let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
                (1..=6).contains(&hashes) && trimmed.as_bytes().get(hashes) == Some(&b' ')
            });

            if starts_with_heading {
                // Replace heading + body
                result_lines.extend_from_slice(&lines[..range.heading_line - 1]);
                // Don't use push for multi-line — we'll join at the end
            } else {
                // Replace body only, preserve heading
                result_lines.extend_from_slice(&lines[..range.heading_line]); // includes heading line
            }

            let mut result = result_lines.join("\n");
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(new);

            // Append remaining lines after the section
            if range.end_line < lines.len() {
                if !result.ends_with('\n') {
                    result.push('\n');
                }
                let remaining = lines[range.end_line..].join("\n");
                result.push_str(&remaining);
            }

            // Normalize trailing newline
            if !result.ends_with('\n') {
                result.push('\n');
            }
            Ok(result)
        }
        "insert_before" => {
            let new = new_content.unwrap();
            result_lines.extend_from_slice(&lines[..range.heading_line - 1]);
            let mut result = result_lines.join("\n");
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(new);
            if !result.ends_with('\n') {
                result.push('\n');
            }
            let remaining = lines[range.heading_line - 1..].join("\n");
            result.push_str(&remaining);
            if !result.ends_with('\n') {
                result.push('\n');
            }
            Ok(result)
        }
        "insert_after" => {
            let new = new_content.unwrap();
            result_lines.extend_from_slice(&lines[..range.end_line]);
            let mut result = result_lines.join("\n");
            result.push('\n');
            result.push_str(new);
            if range.end_line < lines.len() {
                if !result.ends_with('\n') {
                    result.push('\n');
                }
                let remaining = lines[range.end_line..].join("\n");
                result.push_str(&remaining);
            }
            if !result.ends_with('\n') {
                result.push('\n');
            }
            Ok(result)
        }
        "remove" => {
            result_lines.extend_from_slice(&lines[..range.heading_line - 1]);
            // Consume one trailing blank line if present
            let skip_to = if range.end_line < lines.len()
                && lines.get(range.end_line).map_or(false, |l| l.trim().is_empty())
            {
                range.end_line + 1
            } else {
                range.end_line
            };
            if skip_to < lines.len() {
                result_lines.extend_from_slice(&lines[skip_to..]);
            }
            let mut result = result_lines.join("\n");
            if !result.ends_with('\n') && !result.is_empty() {
                result.push('\n');
            }
            if result.is_empty() {
                result.push('\n');
            }
            Ok(result)
        }
        _ => unreachable!(),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test section_edit -- --nocapture 2>&1 | head -40`
Expected: All 14 tests pass

- [ ] **Step 5: Implement `EditSection` tool struct**

Add above the tests in `section_edit.rs`:

```rust
pub struct EditSection;

#[async_trait::async_trait]
impl Tool for EditSection {
    fn name(&self) -> &str {
        "edit_section"
    }

    fn description(&self) -> &str {
        "Edit a document section by heading. Actions: replace, insert_before, insert_after, remove. Supports Markdown."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path" },
                "heading": { "type": "string", "description": "Section heading to target, e.g. '## Auth'" },
                "action": {
                    "type": "string",
                    "enum": ["replace", "insert_before", "insert_after", "remove"],
                    "description": "Operation to perform on the section"
                },
                "content": { "type": "string", "description": "New content. Required for replace/insert_before/insert_after." }
            },
            "required": ["path", "heading", "action"]
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        guard_worktree_write(ctx).await?;
        let path = require_str_param(&input, "path")?;
        let heading = require_str_param(&input, "heading")?;
        let action = require_str_param(&input, "action")?;
        let content = input["content"].as_str();

        let root = ctx.agent.require_project_root().await?;
        let security = ctx.agent.security_config().await;
        let resolved = path_security::validate_write_path(path, &root, &security)?;

        // Only markdown files supported for now
        if !path.ends_with(".md") && !path.ends_with(".markdown") {
            return Err(RecoverableError::with_hint(
                "edit_section currently supports Markdown files only",
                "For TOML use edit_file with toml_key, for JSON use edit_file with json_path.",
            )
            .into());
        }

        let file_content = std::fs::read_to_string(&resolved)?;
        let new_content = perform_section_edit(&file_content, heading, action, content)?;
        std::fs::write(&resolved, &new_content)?;

        // Update coverage mtime so we don't invalidate on next read
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.update_mtime(&resolved);
        }

        ctx.agent.reload_config_if_project_toml(&resolved).await;
        ctx.lsp.notify_file_changed(&resolved).await;
        ctx.agent.mark_file_dirty(resolved).await;

        Ok(json!("ok"))
    }
}
```

- [ ] **Step 6: Register `EditSection` in `from_parts` and `check_tool_access`**

In `src/server.rs` `from_parts`, add after the `EditFile` line:

```rust
            Arc::new(crate::tools::section_edit::EditSection),
```

In `src/util/path_security.rs` `check_tool_access`, update the write-tool match arm to include `"edit_section"`:

```rust
        "create_file" | "edit_file" | "replace_symbol" | "insert_code" | "rename_symbol"
        | "remove_symbol" | "register_library" | "edit_section" => {
```

In `src/tools/mod.rs`, add:

```rust
pub mod section_edit;
```

- [ ] **Step 7: Run full test suite + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -20`
Expected: All clean

- [ ] **Step 8: Commit**

```bash
git add src/tools/section_edit.rs src/tools/mod.rs src/server.rs src/util/path_security.rs
git commit -m "feat: add edit_section tool for heading-addressed markdown editing

Actions: replace (smart heading detection), insert_before, insert_after, remove.
Registered as tool #30, gated by check_tool_access write-tool arm.
Updates SectionCoverage mtime after writes."
```

---

## Chunk 4: Coverage Integration into `ReadFile` and `EditFile`

### Task 6: Wire coverage tracking into `ReadFile`

**Files:**
- Modify: `src/tools/file.rs` (`ReadFile::call`, around lines 41-527)

This task adds two behaviors to `ReadFile`:
1. After reading a markdown file, record which headings were "seen" in `SectionCoverage`
2. Include a `coverage` field in the response when sections are unread

- [ ] **Step 1: Write integration test**

Add to the test module in `src/tools/file.rs`:

```rust
#[tokio::test]
async fn read_file_coverage_full_read_marks_all() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Title\n## A\ntext\n## B\nmore\n").unwrap();

    // Activate project at the temp dir
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = ReadFile.call(json!({"path": "test.md"}), &ctx).await.unwrap();

    // Check that coverage was recorded
    let resolved = dir.path().join("test.md").canonicalize().unwrap();
    let all = vec!["# Title".to_string(), "## A".to_string(), "## B".to_string()];
    let status = ctx.section_coverage.lock().unwrap().status(&resolved, &all);
    assert!(status.is_some());
    let s = status.unwrap();
    assert_eq!(s.read_count, 3);
    assert!(s.unread.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test read_file_coverage_full_read -- --nocapture 2>&1 | head -20`
Expected: FAIL — coverage not recorded yet

- [ ] **Step 3: Implement coverage recording in `ReadFile::call`**

At the end of `ReadFile::call`, after the response is built but before returning, add logic to detect markdown files and record coverage. The key insertion point is just before the final `Ok(result)`:

```rust
// Record section coverage for markdown files
// Note: reuse `text` (the already-loaded file content from earlier in call) — do NOT re-read.
if path.ends_with(".md") || path.ends_with(".markdown") {
    if let Ok(resolved) = crate::util::path_security::resolve_path(path, &root) {
        // `text` is already in scope from the file read above — use it directly
        let content = &text;
        let all_headings = crate::tools::file_summary::parse_all_headings(&content);

        if !all_headings.is_empty() {
            let heading_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();

            // Determine which headings were "seen" based on what was read
            let seen: Vec<String> = if heading.is_some() {
                // Single heading read — only that one
                if let Ok(range) = crate::tools::file_summary::resolve_section_range(&content, heading.unwrap()) {
                    vec![range.heading_text]
                } else {
                    vec![]
                }
            } else if start_line.is_some() || end_line.is_some() {
                // Line range — headings within range
                let start = start_line.unwrap_or(1);
                let end = end_line.unwrap_or(usize::MAX);
                all_headings.iter()
                    .filter(|h| h.line >= start && h.line <= end)
                    .map(|h| h.text.clone())
                    .collect()
            } else {
                // Full file read — all headings
                heading_texts.clone()
            };

            if !seen.is_empty() {
                if let Ok(mut cov) = ctx.section_coverage.lock() {
                    cov.mark_seen(&resolved, &seen);
                }
            }

            // Add coverage hint to response
            if let Ok(mut cov) = ctx.section_coverage.lock() {
                if let Some(status) = cov.status(&resolved, &heading_texts) {
                    if !status.unread.is_empty() {
                        if let Some(obj) = result.as_object_mut() {
                            obj.insert("coverage".to_string(), json!({
                                "read": status.read_count,
                                "total": status.total_count,
                                "unread": status.unread,
                            }));
                        }
                    }
                }
            }
        }
    }
}
```

Note: The exact insertion point depends on the structure of `ReadFile::call`. The coverage logic should run after the response `Value` is constructed but before the final return. Variables `heading`, `start_line`, `end_line` should already be in scope from the input parsing at the top of `call`.

- [ ] **Step 4: Run tests**

Run: `cargo test read_file_coverage -- --nocapture 2>&1 | head -20`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: wire section coverage tracking into ReadFile

Records which markdown headings were seen on each read.
Adds coverage field to response when unread sections exist."
```

### Task 7: Wire coverage hints into `EditFile`

**Files:**
- Modify: `src/tools/file.rs` (`EditFile::call`, around lines 1527-1585)

- [ ] **Step 1: Write test**

```rust
#[tokio::test]
async fn edit_file_coverage_hint_on_unread() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Title\n## A\ntext a\n## B\ntext b\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    // Read only section A
    let _ = ReadFile.call(json!({"path": "test.md", "heading": "## A"}), &ctx).await.unwrap();

    // Edit something — should get hint about unread B
    let result = EditFile.call(json!({
        "path": "test.md",
        "old_string": "text a",
        "new_string": "updated a"
    }), &ctx).await.unwrap();

    // The result should mention unread sections (if coverage exists and has unread)
    // Exact format TBD during implementation — verify the hint is present
    // For json!("ok"), the hint needs to be wrapped: json!({"status": "ok", "hint": "..."})
}
```

Note: Currently write tools return `json!("ok")`. For the hint to be visible, we conditionally return `json!({"status": "ok", "hint": "..."})` when there are unread sections. This is a narrow response format change — only for markdown files with coverage data and unread sections. The hint is genuinely new information (not echoing back what the LLM sent), so it does not violate the "No Echo in Write Responses" principle. The `format_compact` method for `EditFile` should handle both `json!("ok")` and the `{"status": "ok", "hint": ...}` shape. Existing consumers that check `result == "ok"` should be updated to also accept `result["status"] == "ok"` — verify the companion plugin handles this.

- [ ] **Step 2: Implement coverage hint in `EditFile::call`**

After `perform_edit` succeeds and before returning, add:

```rust
// Coverage hint for markdown files
if (path.ends_with(".md") || path.ends_with(".markdown")) {
    // Update mtime to prevent spurious invalidation
    if let Ok(mut cov) = ctx.section_coverage.lock() {
        cov.update_mtime(&resolved);
    }

    // Check for unread sections
    let content = std::fs::read_to_string(&resolved).unwrap_or_default();
    let all_headings = crate::tools::file_summary::parse_all_headings(&content);
    if !all_headings.is_empty() {
        let heading_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            if let Some(hint) = cov.unread_hint(&resolved, &heading_texts) {
                return Ok(json!({"status": "ok", "hint": hint}));
            }
        }
    }
}

Ok(json!("ok"))
```

- [ ] **Step 3: Run tests**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -20`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: add coverage hint to EditFile for markdown files

When editing a markdown file with unread sections, response includes
a hint listing unread headings. Updates mtime to prevent spurious invalidation."
```

---

## Chunk 5: Enhanced `ReadFile` and `EditFile` Features

### Task 8: Multi-heading read (`headings` param on `ReadFile`)

**Files:**
- Modify: `src/tools/file.rs` (`ReadFile::input_schema` and `ReadFile::call`)

- [ ] **Step 1: Write tests**

```rust
#[tokio::test]
async fn read_file_multiple_headings() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Title\n## A\ntext a\n## B\ntext b\n## C\ntext c\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = ReadFile.call(json!({
        "path": "test.md",
        "headings": ["## A", "## C"]
    }), &ctx).await.unwrap();

    let content = result["content"].as_str().unwrap();
    assert!(content.contains("text a"));
    assert!(content.contains("text c"));
    assert!(!content.contains("text b")); // B was not requested
}

#[tokio::test]
async fn read_file_headings_and_heading_mutual_exclusion() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Title\n## A\ntext\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = ReadFile.call(json!({
        "path": "test.md",
        "heading": "## A",
        "headings": ["## A"]
    }), &ctx).await;

    // Should error
    assert!(result.is_err() || result.unwrap()["error"].is_string());
}

#[tokio::test]
async fn read_headings_marks_all_seen() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Title\n## A\ntext a\n## B\ntext b\n## C\ntext c\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    // Read two of three sections
    let _ = ReadFile.call(json!({
        "path": "test.md",
        "headings": ["## A", "## C"]
    }), &ctx).await.unwrap();

    let resolved = dir.path().join("test.md").canonicalize().unwrap();
    let all = vec!["# Title".into(), "## A".into(), "## B".into(), "## C".into()];
    let status = ctx.section_coverage.lock().unwrap().status(&resolved, &all).unwrap();
    assert_eq!(status.read_count, 2); // A and C seen
    assert_eq!(status.unread, vec!["# Title", "## B"]); // Title and B unread
}
```

- [ ] **Step 2: Implement multi-heading read**

In `ReadFile::input_schema`, add:

```json
"headings": {
    "type": "array",
    "items": { "type": "string" },
    "description": "List of headings to read (returns multiple sections). Mutually exclusive with heading."
}
```

In `ReadFile::call`, after parsing `heading`, add:

```rust
let headings_param = input["headings"].as_array();

// Mutual exclusivity check
if heading.is_some() && headings_param.is_some() {
    return Err(RecoverableError::with_hint(
        "heading and headings are mutually exclusive",
        "Use heading for a single section, or headings for multiple sections.",
    ).into());
}
```

Then add a branch that handles the multi-heading case (similar to the single heading branch but iterating):

```rust
if let Some(headings_arr) = headings_param {
    // ... resolve each heading via extract_markdown_section, concatenate results
    // Mark all matched headings as seen in coverage
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test read_file_multiple_headings -- --nocapture && cargo test read_file_headings_and_heading -- --nocapture`
Expected: Both pass

- [ ] **Step 4: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: add multi-heading read to ReadFile (headings param)

Returns multiple sections in one call, reducing tool round-trips.
All matched headings recorded in SectionCoverage."
```

### Task 9: Section-scoped `edit_file` (`heading` param on `EditFile`)

**Files:**
- Modify: `src/tools/file.rs` (`EditFile::input_schema` and `EditFile::call`)

- [ ] **Step 1: Write tests**

```rust
#[tokio::test]
async fn edit_file_heading_scoped_match() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    // "Returns a list" appears in both sections
    std::fs::write(&md, "# API\n## Users\nReturns a list of users\n## Posts\nReturns a list of posts\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    // Without heading scope, this would fail (2 matches)
    let result = EditFile.call(json!({
        "path": "test.md",
        "heading": "## Users",
        "old_string": "Returns a list",
        "new_string": "Returns a paginated list"
    }), &ctx).await.unwrap();

    let content = std::fs::read_to_string(dir.path().join("test.md")).unwrap();
    assert!(content.contains("Returns a paginated list of users"));
    assert!(content.contains("Returns a list of posts")); // Unchanged
}

#[tokio::test]
async fn edit_file_heading_scoped_not_found() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Title\n## Setup\ncontent\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = EditFile.call(json!({
        "path": "test.md",
        "heading": "## Setup",
        "old_string": "nonexistent",
        "new_string": "x"
    }), &ctx).await;

    // Error should mention the section scope
    assert!(result.is_err());
}

#[tokio::test]
async fn edit_file_heading_on_non_markdown() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("config.toml"), "[section]\nkey = 1\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = EditFile.call(json!({
        "path": "config.toml",
        "heading": "## Setup",
        "old_string": "key = 1",
        "new_string": "key = 2"
    }), &ctx).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn edit_file_heading_with_replace_all() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    // "item" appears in both sections
    std::fs::write(&md, "# API\n## Users\nitem one\nitem two\n## Posts\nitem three\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let _ = EditFile.call(json!({
        "path": "test.md",
        "heading": "## Users",
        "old_string": "item",
        "new_string": "entry",
        "replace_all": true
    }), &ctx).await.unwrap();

    let content = std::fs::read_to_string(dir.path().join("test.md")).unwrap();
    assert!(content.contains("entry one"));
    assert!(content.contains("entry two"));
    assert!(content.contains("item three"), "Posts section should be unchanged");
}
```

- [ ] **Step 2: Implement section-scoped matching**

In `EditFile::input_schema`, add:

```json
"heading": {
    "type": "string",
    "description": "Scope string matching to a markdown section. Only valid for .md files."
}
```

In `EditFile::call`, after `old_string` is extracted and before calling `perform_edit`, add:

```rust
let heading_scope = input["heading"].as_str();

if let Some(heading) = heading_scope {
    // Validate markdown-only
    if !path.ends_with(".md") && !path.ends_with(".markdown") {
        return Err(RecoverableError::with_hint(
            "heading param is only supported for Markdown files",
            "For TOML files use toml_key, for JSON use json_path.",
        ).into());
    }

    let root = ctx.agent.require_project_root().await?;
    let security = ctx.agent.security_config().await;
    let resolved = path_security::validate_write_path(path, &root, &security)?;
    let content = std::fs::read_to_string(&resolved)?;

    let range = crate::tools::file_summary::resolve_section_range(&content, heading)?;
    let lines: Vec<&str> = content.lines().collect();
    let section_text: String = lines[range.heading_line - 1..range.end_line].join("\n");

    // Match within section only
    let match_count = section_text.matches(old_string).count();
    if match_count == 0 {
        return Err(RecoverableError::with_hint(
            format!("old_string not found in section '{}' (lines {}-{})", range.heading_text, range.heading_line, range.end_line),
            "Check whitespace and indentation. Use search_pattern to verify exact text.",
        ).into());
    }
    if match_count > 1 && !replace_all {
        return Err(RecoverableError::with_hint(
            format!("old_string found {match_count} times in section '{}'", range.heading_text),
            "Include more context or set replace_all: true.",
        ).into());
    }

    // Replace within section, rebuild full content
    let new_section = if replace_all {
        section_text.replace(old_string, new_string)
    } else {
        section_text.replacen(old_string, new_string, 1)
    };

    let mut result = String::new();
    if range.heading_line > 1 {
        result.push_str(&lines[..range.heading_line - 1].join("\n"));
        result.push('\n');
    }
    result.push_str(&new_section);
    if range.end_line < lines.len() {
        result.push('\n');
        result.push_str(&lines[range.end_line..].join("\n"));
    }
    if !result.ends_with('\n') {
        result.push('\n');
    }

    std::fs::write(&resolved, &result)?;
    ctx.lsp.notify_file_changed(&resolved).await;
    ctx.agent.mark_file_dirty(resolved).await;
    return Ok(json!("ok"));
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test edit_file_heading -- --nocapture 2>&1 | head -30`
Expected: All 3 pass

- [ ] **Step 4: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: add heading param to EditFile for section-scoped matching

old_string matching restricted to within a heading's section range.
Markdown-only; errors with format-appropriate suggestion on non-markdown."
```

### Task 10: Batch `edit_file` (`edits` param on `EditFile`)

**Files:**
- Modify: `src/tools/file.rs` (`EditFile::input_schema` and `EditFile::call`)

- [ ] **Step 1: Write tests**

```rust
#[tokio::test]
async fn batch_edit_applies_all() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Title\nfoo\nbar\nbaz\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = EditFile.call(json!({
        "path": "test.md",
        "edits": [
            {"old_string": "foo", "new_string": "FOO"},
            {"old_string": "bar", "new_string": "BAR"},
            {"old_string": "baz", "new_string": "BAZ"}
        ]
    }), &ctx).await.unwrap();

    let content = std::fs::read_to_string(dir.path().join("test.md")).unwrap();
    assert!(content.contains("FOO"));
    assert!(content.contains("BAR"));
    assert!(content.contains("BAZ"));
}

#[tokio::test]
async fn batch_edit_atomic_rollback() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Title\nfoo\nbar\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    // Second edit should fail — "nonexistent" not in file
    let result = EditFile.call(json!({
        "path": "test.md",
        "edits": [
            {"old_string": "foo", "new_string": "FOO"},
            {"old_string": "nonexistent", "new_string": "X"}
        ]
    }), &ctx).await;

    assert!(result.is_err());
    // File should be unchanged (atomic — no partial writes)
    let content = std::fs::read_to_string(dir.path().join("test.md")).unwrap();
    assert!(content.contains("foo"), "first edit should have been rolled back");
}

#[tokio::test]
async fn batch_edit_and_old_string_mutual_exclusion() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("test.md"), "# Title\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = EditFile.call(json!({
        "path": "test.md",
        "old_string": "foo",
        "new_string": "bar",
        "edits": [{"old_string": "x", "new_string": "y"}]
    }), &ctx).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn batch_edit_with_heading_scope() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    // "value" appears in both sections
    std::fs::write(&md, "# Config\n## Dev\nvalue = 1\n## Prod\nvalue = 2\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let _ = EditFile.call(json!({
        "path": "test.md",
        "edits": [
            {"old_string": "value = 1", "new_string": "value = 10", "heading": "## Dev"},
            {"old_string": "value = 2", "new_string": "value = 20", "heading": "## Prod"}
        ]
    }), &ctx).await.unwrap();

    let content = std::fs::read_to_string(dir.path().join("test.md")).unwrap();
    assert!(content.contains("value = 10"));
    assert!(content.contains("value = 20"));
}

#[tokio::test]
async fn batch_edit_line_shift() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "# Title\nline one\nline two\nline three\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    // First edit adds lines, shifting content for second edit
    let _ = EditFile.call(json!({
        "path": "test.md",
        "edits": [
            {"old_string": "line one", "new_string": "line one\nextra line a\nextra line b"},
            {"old_string": "line three", "new_string": "line three updated"}
        ]
    }), &ctx).await.unwrap();

    let content = std::fs::read_to_string(dir.path().join("test.md")).unwrap();
    assert!(content.contains("extra line a"));
    assert!(content.contains("extra line b"));
    assert!(content.contains("line three updated"));
}
```

- [ ] **Step 2: Implement batch edit mode**

In `EditFile::input_schema`, add:

```json
"edits": {
    "type": "array",
    "items": {
        "type": "object",
        "properties": {
            "old_string": { "type": "string" },
            "new_string": { "type": "string" },
            "heading": { "type": "string" },
            "replace_all": { "type": "boolean" }
        },
        "required": ["old_string", "new_string"]
    },
    "description": "Batch mode: array of edit operations applied atomically."
}
```

In `EditFile::call`, before the existing `old_string` extraction, add:

```rust
let edits = input["edits"].as_array();
let has_old_string = input["old_string"].as_str().is_some();

if edits.is_some() && has_old_string {
    return Err(RecoverableError::with_hint(
        "edits and old_string are mutually exclusive",
        "Use edits for batch mode, or old_string/new_string for single edit.",
    ).into());
}

if let Some(edits_arr) = edits {
    let root = ctx.agent.require_project_root().await?;
    let security = ctx.agent.security_config().await;
    let resolved = path_security::validate_write_path(path, &root, &security)?;
    let mut content = std::fs::read_to_string(&resolved)?;

    for (i, edit) in edits_arr.iter().enumerate() {
        let old_s = edit["old_string"].as_str().ok_or_else(|| {
            RecoverableError::new(format!("edit[{i}]: old_string is required"))
        })?;
        let new_s = edit["new_string"].as_str().unwrap_or("");
        let replace_all_edit = super::parse_bool_param(&edit["replace_all"]);
        let heading_scope = edit["heading"].as_str();

        if old_s.is_empty() {
            return Err(RecoverableError::with_hint(
                format!("edit[{i}]: old_string must not be empty"),
                "Each edit in the batch must have a non-empty old_string.",
            ).into());
        }

        // Optionally scope to a heading
        let search_content = if let Some(heading) = heading_scope {
            let range = crate::tools::file_summary::resolve_section_range(&content, heading)
                .map_err(|e| RecoverableError::new(format!("edit[{i}]: {e}")))?;
            let lines: Vec<&str> = content.lines().collect();
            lines[range.heading_line - 1..range.end_line].join("\n")
        } else {
            content.clone()
        };

        let match_count = search_content.matches(old_s).count();
        if match_count == 0 {
            return Err(RecoverableError::with_hint(
                format!("edit[{i}]: old_string not found"),
                "Check whitespace and indentation. Batch aborted — no changes written.",
            ).into());
        }
        if match_count > 1 && !replace_all_edit {
            return Err(RecoverableError::with_hint(
                format!("edit[{i}]: old_string found {match_count} times"),
                "Add more context or set replace_all: true. Batch aborted.",
            ).into());
        }

        // Apply edit — if heading-scoped, splice within the section to avoid
        // accidentally replacing a match outside the section boundary.
        if let Some(heading) = heading_scope {
            let range = crate::tools::file_summary::resolve_section_range(&content, heading)
                .map_err(|e| RecoverableError::new(format!("edit[{i}]: {e}")))?;
            let lines: Vec<&str> = content.lines().collect();
            let section_text = lines[range.heading_line - 1..range.end_line].join("\n");
            let new_section = if replace_all_edit {
                section_text.replace(old_s, new_s)
            } else {
                section_text.replacen(old_s, new_s, 1)
            };
            let mut result = String::new();
            if range.heading_line > 1 {
                result.push_str(&lines[..range.heading_line - 1].join("\n"));
                result.push('\n');
            }
            result.push_str(&new_section);
            if range.end_line < lines.len() {
                result.push('\n');
                result.push_str(&lines[range.end_line..].join("\n"));
            }
            if !result.ends_with('\n') {
                result.push('\n');
            }
            content = result;
        } else {
            if replace_all_edit {
                content = content.replace(old_s, new_s);
            } else {
                content = content.replacen(old_s, new_s, 1);
            }
        }
    }

    std::fs::write(&resolved, &content)?;
    ctx.lsp.notify_file_changed(&resolved).await;
    ctx.agent.mark_file_dirty(resolved).await;
    return Ok(json!("ok"));
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test batch_edit -- --nocapture 2>&1 | head -30`
Expected: All 3 pass

- [ ] **Step 4: Run full suite + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -20`
Expected: All clean

- [ ] **Step 5: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: add batch edit_file mode (edits array param)

Multiple edits applied atomically — if any fails, no changes written.
Each edit can optionally scope to a heading. Mutually exclusive with old_string."
```

---

## Chunk 6: Complete Read Mode for Plan Files

### Task 10b: Implement `mode="complete"` on `ReadFile`

**Files:**
- Modify: `src/tools/file.rs` (`ReadFile::input_schema` and `ReadFile::call`)

This adds a `mode="complete"` param to `read_file` that bypasses buffer/pagination and returns the entire file inline with a delivery receipt. Scoped to files in `plans/` directories only.

- [ ] **Step 1: Write failing tests**

```rust
#[tokio::test]
async fn complete_mode_returns_full_content() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans")).unwrap();
    let plan = dir.path().join("plans/test-plan.md");
    let mut content = String::from("# Plan\n");
    for i in 1..=10 {
        content.push_str(&format!("## Task {i}\n- [ ] Step 1\n- [x] Step 2\ncontent {i}\n\n"));
    }
    std::fs::write(&plan, &content).unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = ReadFile.call(json!({
        "path": "plans/test-plan.md",
        "mode": "complete"
    }), &ctx).await.unwrap();

    let text = result["content"].as_str().unwrap();
    // Should contain all tasks inline, not a buffer ref
    assert!(text.contains("## Task 1"));
    assert!(text.contains("## Task 10"));
    assert!(!text.contains("@file_")); // no buffer ref
}

#[tokio::test]
async fn complete_mode_delivery_receipt() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans")).unwrap();
    let plan = dir.path().join("plans/test-plan.md");
    std::fs::write(&plan, "# Plan\n## Task 1\n- [ ] Step A\n- [x] Step B\n## Task 2\n- [ ] Step C\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = ReadFile.call(json!({
        "path": "plans/test-plan.md",
        "mode": "complete"
    }), &ctx).await.unwrap();

    let text = result["content"].as_str().unwrap();
    assert!(text.contains("--- delivery receipt ---"));
    assert!(text.contains("Sections: 3")); // # Plan, ## Task 1, ## Task 2
    assert!(text.contains("Checkboxes:")); // should show done/pending counts
}

#[tokio::test]
async fn complete_mode_marks_all_seen() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans")).unwrap();
    let plan = dir.path().join("plans/test-plan.md");
    std::fs::write(&plan, "# Plan\n## A\ntext\n## B\ntext\n## C\ntext\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let _ = ReadFile.call(json!({
        "path": "plans/test-plan.md",
        "mode": "complete"
    }), &ctx).await.unwrap();

    let resolved = dir.path().join("plans/test-plan.md").canonicalize().unwrap();
    let all = vec!["# Plan".into(), "## A".into(), "## B".into(), "## C".into()];
    let status = ctx.section_coverage.lock().unwrap().status(&resolved, &all).unwrap();
    assert_eq!(status.read_count, 4);
    assert!(status.unread.is_empty());
}

#[tokio::test]
async fn complete_mode_rejects_non_plan_path() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    let md = dir.path().join("README.md");
    std::fs::write(&md, "# Big file\nLots of content\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = ReadFile.call(json!({
        "path": "README.md",
        "mode": "complete"
    }), &ctx).await;

    // Should error — not in a plans/ directory
    assert!(result.is_err() || result.unwrap()["error"].is_string());
}

#[tokio::test]
async fn complete_mode_mutual_exclusivity() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plans")).unwrap();
    std::fs::write(dir.path().join("plans/p.md"), "# Plan\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = ReadFile.call(json!({
        "path": "plans/p.md",
        "mode": "complete",
        "heading": "# Plan"
    }), &ctx).await;

    assert!(result.is_err() || result.unwrap()["error"].is_string());
}

#[tokio::test]
async fn complete_mode_nested_plans_dir() {
    let ctx = test_ctx().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("docs/superpowers/plans")).unwrap();
    let plan = dir.path().join("docs/superpowers/plans/feature.md");
    std::fs::write(&plan, "# Plan\n## Task 1\ncontent\n").unwrap();
    ctx.agent.activate(dir.path(), false).await.unwrap();

    let result = ReadFile.call(json!({
        "path": "docs/superpowers/plans/feature.md",
        "mode": "complete"
    }), &ctx).await.unwrap();

    let text = result["content"].as_str().unwrap();
    assert!(text.contains("## Task 1"));
    assert!(text.contains("--- delivery receipt ---"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test complete_mode -- --nocapture 2>&1 | head -20`
Expected: FAIL — `mode` param not handled yet

- [ ] **Step 3: Implement complete mode in `ReadFile::call`**

In `ReadFile::input_schema`, add to the properties:

```json
"mode": {
    "type": "string",
    "enum": ["complete"],
    "description": "Read mode. 'complete' returns entire file inline (plan files only, bypasses buffer)."
}
```

In `ReadFile::call`, early in the function after parsing params, add:

```rust
let mode = input["mode"].as_str();

if mode == Some("complete") {
    // Mutual exclusivity check
    if heading.is_some() || headings_param.is_some() || start_line.is_some()
        || end_line.is_some() || json_path.is_some() || toml_key.is_some()
    {
        return Err(RecoverableError::with_hint(
            "mode=complete is mutually exclusive with heading, headings, start_line, end_line, json_path, toml_key",
            "Use mode=complete alone to read the entire plan file.",
        ).into());
    }

    // Scope restriction: only plans/ directories
    if !path.contains("/plans/") && !path.starts_with("plans/") {
        return Err(RecoverableError::with_hint(
            "mode=complete is restricted to plan files (paths containing /plans/)",
            "Use heading= or headings= to read specific sections of non-plan files.",
        ).into());
    }

    let root = ctx.agent.require_project_root().await?;
    let security = ctx.agent.security_config().await;
    let resolved = crate::util::path_security::resolve_path(path, &root)?;
    let text = std::fs::read_to_string(&resolved)?;
    let line_count = text.lines().count();

    // Parse headings for receipt and coverage
    let all_headings = crate::tools::file_summary::parse_all_headings(&text);
    let heading_texts: Vec<String> = all_headings.iter().map(|h| h.text.clone()).collect();

    // Count checkboxes
    let done_count = text.lines().filter(|l| {
        let trimmed = l.trim_start();
        trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]")
    }).count();
    let pending_count = text.lines().filter(|l| {
        let trimmed = l.trim_start();
        trimmed.starts_with("- [ ]")
    }).count();
    let total_checkboxes = done_count + pending_count;

    // Build delivery receipt
    let section_list = heading_texts.join(", ");
    let receipt = format!(
        "\n\n--- delivery receipt ---\nFile: {path}\nLines: {line_count} | Sections: {} | Checkboxes: {total_checkboxes} ({done_count} done, {pending_count} pending)\nSections delivered: [{section_list}]\n",
        all_headings.len()
    );

    // Record all sections as seen in coverage
    if !heading_texts.is_empty() {
        if let Ok(mut cov) = ctx.section_coverage.lock() {
            cov.mark_seen(&resolved, &heading_texts);
        }
    }

    let mut content = text;
    content.push_str(&receipt);

    return Ok(json!({
        "content": content,
        "complete": true,
        "line_count": line_count,
    }));
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test complete_mode -- --nocapture 2>&1 | head -40`
Expected: All 6 tests pass

- [ ] **Step 5: Run full suite + clippy**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test 2>&1 | tail -20`
Expected: All clean

- [ ] **Step 6: Commit**

```bash
git add src/tools/file.rs
git commit -m "feat: add mode=complete to ReadFile for plan files

Bypasses buffer/pagination, returns entire file inline with a delivery
receipt (section list, line count, checkbox progress). Scoped to plans/
directories only to prevent misuse on arbitrary large files."
```

## Chunk 7: Prompt Surfaces & Final Verification

> Note: Task numbering continues from previous chunks. Task 11 and 12 unchanged.

### Task 11: Update prompt surfaces

**Files:**
- Modify: `src/prompts/server_instructions.md`
- Modify: `src/prompts/onboarding_prompt.md`
- Check: `src/tools/workflow.rs` (`build_system_prompt_draft`)

- [ ] **Step 1: Read current prompt surfaces**

Read `src/prompts/server_instructions.md` and `src/prompts/onboarding_prompt.md` to find the right insertion points. Check `build_system_prompt_draft` in `src/tools/workflow.rs` to see if it enumerates tools.

- [ ] **Step 2: Update `server_instructions.md`**

Add `edit_section` to the File I/O tool reference section:

```markdown
- `edit_section(path, heading, action, content?)` — edit a markdown section by heading.
  Actions: `replace` (smart: detects heading in content), `insert_before`, `insert_after`, `remove`.
  `heading` uses fuzzy matching (strips inline formatting, prefix/substring fallback).
```

Add to anti-patterns table:

```markdown
| `edit_file` / `create_file` to rewrite an entire markdown section | `edit_section(path, heading, action, content)` | Heading-addressed, no string matching needed |
```

Add note about `edit_file` enhancements:

```markdown
- `edit_file` now supports `heading` param for section-scoped matching (markdown only)
  and `edits` array for batch operations (atomic, one write).
```

Add note about complete read mode:

```markdown
- `read_file` with `mode="complete"` returns entire plan file inline with a delivery receipt.
  Only for files in `plans/` directories. Use when you need to read a full implementation plan.
```

- [ ] **Step 3: Update `onboarding_prompt.md`**

Add a brief mention in the file editing section:

```markdown
For markdown files, use `edit_section` to replace/insert/remove sections by heading.
Use `edit_file` with `heading` param for scoped string matching within a section.
```

- [ ] **Step 4: Check `build_system_prompt_draft`**

Read the function. If it lists tools by name, add `edit_section`. If it dynamically enumerates registered tools, no change needed.

- [ ] **Step 5: Commit**

```bash
git add src/prompts/server_instructions.md src/prompts/onboarding_prompt.md
git commit -m "docs: update prompt surfaces for edit_section and edit_file enhancements

Adds edit_section to tool reference, anti-patterns table.
Documents heading param and batch mode for edit_file."
```

### Task 12: Final verification

- [ ] **Step 1: Run full test suite**

Run: `cargo fmt && cargo clippy -- -D warnings && cargo test`
Expected: All clean, all tests pass

- [ ] **Step 2: Build release binary**

Run: `cargo build --release`
Expected: Clean build

- [ ] **Step 3: Manual smoke test via MCP**

Restart MCP server with `/mcp`, then:
1. `edit_section(path="README.md", heading="## Setup", action="replace", content="Updated setup instructions\n")` — should return `"ok"`
2. `read_file(path="README.md", headings=["## Setup", "## Usage"])` — should return both sections
3. `edit_file(path="README.md", heading="## Setup", old_string="Updated", new_string="Fresh")` — scoped match
4. Verify coverage hint appears on partial reads

- [ ] **Step 4: Commit any fixes from smoke testing**

```bash
git add -A
git commit -m "fix: address issues found during smoke testing"
```
