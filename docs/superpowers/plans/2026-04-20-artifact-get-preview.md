---
id: null
kind: null
status: shipped
title: null
owners: []
tags: []
topic: null
time_scope: null
---
# artifact_get Kind-Aware Previews Implementation Plan

> **Status:** ✅ SHIPPED 2026-04-21. All 13 tasks complete. Step checkboxes below were not individually flipped — see **Post-Ship Fixes** section at the bottom for what actually landed and what was fixed after ship. Treat the per-step checkboxes as original plan scaffolding, not live status.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the interim `include_body` flag on `artifact_get` with kind-aware structured previews (plan/spec/memory/default) plus progressive body access (`full` / `heading(s)` / `start_line`+`end_line`) under a soft-cap overflow pattern.

**Architecture:** New `src/preview/` module holds shared markdown parsers (`headings`, `summary`) plus one file per kind (`plan`, `spec`, `memory`, `default`). `mod.rs` dispatches on the catalog `kind` string. `tools/get.rs` wires the new params: computes preview on every call, routes body selectors with mutual-exclusion validation, applies the soft cap with an actionable overflow hint listing top-level headings. Zero schema changes — everything computed on the fly.

**Tech Stack:** Rust 2021, serde_json, existing librarian-mcp crate (`crates/librarian-mcp`). Tests use `tokio::test` + `tempfile` (dev-dep already present). Shared catalog API: `catalog::observations::list_for_artifact`.

**Spec:** `docs/superpowers/specs/2026-04-20-artifact-get-preview-design.md`

---

## File Structure

**New files:**
- `crates/librarian-mcp/src/preview/mod.rs` — module root + `extract(kind, row, body, ctx)` dispatch
- `crates/librarian-mcp/src/preview/headings.rs` — shared ATX heading parser
- `crates/librarian-mcp/src/preview/summary.rs` — shared first-paragraph extractor
- `crates/librarian-mcp/src/preview/default.rs` — fallback extractor
- `crates/librarian-mcp/src/preview/plan.rs` — plan extractor
- `crates/librarian-mcp/src/preview/spec.rs` — spec extractor
- `crates/librarian-mcp/src/preview/memory.rs` — memory extractor (reads observations)

**Modified files:**
- `crates/librarian-mcp/src/lib.rs` — `pub mod preview;`
- `crates/librarian-mcp/src/tools/get.rs` — new params, preview wiring, body routing, soft cap, overflow hint

---

## Task 1: Bootstrap `src/preview/` module

**Files:**
- Create: `crates/librarian-mcp/src/preview/mod.rs`
- Modify: `crates/librarian-mcp/src/lib.rs`

- [ ] **Step 1: Create empty preview module file**

Create `crates/librarian-mcp/src/preview/mod.rs`:

```rust
//! Kind-aware preview extractors for artifact_get.
//!
//! Each artifact kind (plan, spec, memory, ...) has its own preview shape.
//! Unknown kinds fall back to the `default` extractor. See
//! `docs/superpowers/specs/2026-04-20-artifact-get-preview-design.md`.

pub mod default;
pub mod headings;
pub mod memory;
pub mod plan;
pub mod spec;
pub mod summary;

use crate::catalog::artifact::ArtifactRow;
use crate::tools::ToolContext;
use serde_json::Value;

/// Compute a kind-specific preview for an artifact.
///
/// `body` is the markdown body with frontmatter already stripped.
/// Returns a tagged JSON object with at least a `"shape"` discriminator.
pub fn extract(kind: &str, row: &ArtifactRow, body: &str, ctx: &ToolContext) -> Value {
    match kind {
        "plan" => plan::extract(row, body),
        "spec" => spec::extract(row, body),
        "memory" => memory::extract(row, body, ctx),
        _ => default::extract(row, body),
    }
}
```

- [ ] **Step 2: Create stub files for every submodule**

Each of these is a placeholder so `cargo check` passes once `lib.rs` declares the module. We fill them in Tasks 2-7.

Create `crates/librarian-mcp/src/preview/headings.rs`:

```rust
//! Shared ATX heading parser (see spec "Heading Parser Rules").
```

Create `crates/librarian-mcp/src/preview/summary.rs`:

```rust
//! Shared first-paragraph extractor (see spec "Summary Extractor Rules").
```

Create `crates/librarian-mcp/src/preview/default.rs`:

```rust
//! Fallback preview for unknown artifact kinds.

use crate::catalog::artifact::ArtifactRow;
use serde_json::{json, Value};

pub fn extract(_row: &ArtifactRow, _body: &str) -> Value {
    json!({ "shape": "default" })
}
```

Create `crates/librarian-mcp/src/preview/plan.rs`:

```rust
//! `plan` artifact preview: heading map + checklist progress.

use crate::catalog::artifact::ArtifactRow;
use serde_json::{json, Value};

pub fn extract(_row: &ArtifactRow, _body: &str) -> Value {
    json!({ "shape": "plan" })
}
```

Create `crates/librarian-mcp/src/preview/spec.rs`:

```rust
//! `spec` artifact preview: heading map + summary.

use crate::catalog::artifact::ArtifactRow;
use serde_json::{json, Value};

pub fn extract(_row: &ArtifactRow, _body: &str) -> Value {
    json!({ "shape": "spec" })
}
```

Create `crates/librarian-mcp/src/preview/memory.rs`:

```rust
//! `memory` artifact preview: observation feed + summary.

use crate::catalog::artifact::ArtifactRow;
use crate::tools::ToolContext;
use serde_json::{json, Value};

pub fn extract(_row: &ArtifactRow, _body: &str, _ctx: &ToolContext) -> Value {
    json!({ "shape": "memory" })
}
```

- [ ] **Step 3: Register the module in `lib.rs`**

Find `crates/librarian-mcp/src/lib.rs` and locate the module list (`pub mod tools;` etc., in the first ~20 lines). Add `pub mod preview;` in alphabetical position (after `mod indexer;`).

Current surrounding lines:
```rust
pub mod indexer;
pub mod workspace;
```

Change to:
```rust
pub mod indexer;
pub mod preview;
pub mod workspace;
```

- [ ] **Step 4: Verify the workspace builds**

Run: `cargo build -p librarian-mcp 2>&1 | tail -5`
Expected: `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in X.XXs` (no errors).

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/preview/ crates/librarian-mcp/src/lib.rs
git commit -m "feat(librarian-mcp): scaffold preview module for kind-aware extractors"
```

---

## Task 2: Shared heading parser

**Files:**
- Modify: `crates/librarian-mcp/src/preview/headings.rs`

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/librarian-mcp/src/preview/headings.rs` with:

```rust
//! Shared ATX heading parser (see spec "Heading Parser Rules").

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub line: usize,
}

/// Parse all ATX headings (`# `, `## `, ...) from a markdown body.
/// Lines inside fenced code blocks (```` ``` ````) are skipped.
/// Returned `line` is 1-indexed.
pub fn parse(body: &str) -> Vec<Heading> {
    todo!("implement in next step")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_level_and_line_number() {
        let body = "# Title\n\n## Section A\n\ntext\n\n### Sub\n";
        let hs = parse(body);
        assert_eq!(
            hs,
            vec![
                Heading { level: 1, text: "Title".into(), line: 1 },
                Heading { level: 2, text: "Section A".into(), line: 3 },
                Heading { level: 3, text: "Sub".into(), line: 7 },
            ]
        );
    }

    #[test]
    fn ignores_hash_inside_fenced_code() {
        let body = "# Real\n\n```\n# Not a heading\n## Also not\n```\n\n## After\n";
        let hs = parse(body);
        assert_eq!(
            hs,
            vec![
                Heading { level: 1, text: "Real".into(), line: 1 },
                Heading { level: 2, text: "After".into(), line: 8 },
            ]
        );
    }

    #[test]
    fn ignores_non_atx_and_malformed() {
        // No space after `#` = not a heading; `#######` > 6 hashes = not a heading.
        let body = "#NoSpace\n####### TooDeep\n## Valid\n";
        let hs = parse(body);
        assert_eq!(hs.len(), 1);
        assert_eq!(hs[0].text, "Valid");
        assert_eq!(hs[0].level, 2);
    }

    #[test]
    fn trims_heading_text_whitespace() {
        let body = "##   Padded   \n";
        let hs = parse(body);
        assert_eq!(hs[0].text, "Padded");
    }

    #[test]
    fn empty_body_returns_empty() {
        assert!(parse("").is_empty());
    }
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p librarian-mcp preview::headings:: 2>&1 | tail -20`
Expected: tests fail with `not yet implemented` panic from `todo!()`.

- [ ] **Step 3: Implement `parse`**

Replace the `parse` function in `crates/librarian-mcp/src/preview/headings.rs`:

```rust
pub fn parse(body: &str) -> Vec<Heading> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for (idx, line) in body.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let bytes = trimmed.as_bytes();
        let mut level = 0u8;
        while level < bytes.len() as u8 && bytes[level as usize] == b'#' {
            level += 1;
        }
        if level == 0 || level > 6 {
            continue;
        }
        if bytes.get(level as usize) != Some(&b' ') {
            continue;
        }
        let text = trimmed[(level as usize + 1)..].trim().to_string();
        out.push(Heading {
            level,
            text,
            line: idx + 1,
        });
    }
    out
}
```

- [ ] **Step 4: Run tests to confirm pass**

Run: `cargo test -p librarian-mcp preview::headings:: 2>&1 | tail -10`
Expected: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/preview/headings.rs
git commit -m "feat(librarian-mcp): shared ATX heading parser for previews"
```

---

## Task 3: Shared summary extractor

**Files:**
- Modify: `crates/librarian-mcp/src/preview/summary.rs`

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/librarian-mcp/src/preview/summary.rs` with:

```rust
//! Shared first-paragraph extractor (see spec "Summary Extractor Rules").

const MAX_SUMMARY_CHARS: usize = 200;

/// Extract the first prose paragraph from a markdown body, trimmed to 200 chars.
///
/// Skips: leading H1 heading, blank lines, lines inside fenced code blocks.
/// Returns an empty string if no prose paragraph exists.
pub fn extract(body: &str) -> String {
    todo!("implement in next step")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_first_paragraph_after_h1() {
        let body = "# Title\n\nFirst paragraph of prose.\n\n## Next\n";
        assert_eq!(extract(body), "First paragraph of prose.");
    }

    #[test]
    fn skips_blank_lines_before_paragraph() {
        let body = "\n\n\nHello there.\n";
        assert_eq!(extract(body), "Hello there.");
    }

    #[test]
    fn collapses_internal_whitespace() {
        let body = "Line one.\nLine two.\n";
        assert_eq!(extract(body), "Line one. Line two.");
    }

    #[test]
    fn truncates_at_200_chars_with_ellipsis() {
        let body = "a".repeat(300);
        let out = extract(&body);
        assert!(out.ends_with('…'));
        assert!(out.chars().count() <= 201); // 200 chars + ellipsis
    }

    #[test]
    fn ignores_lines_inside_fenced_code() {
        let body = "```\ncode block\n```\n\nActual text here.\n";
        assert_eq!(extract(body), "Actual text here.");
    }

    #[test]
    fn empty_body_returns_empty() {
        assert_eq!(extract(""), "");
    }

    #[test]
    fn heading_only_returns_empty() {
        assert_eq!(extract("# Only a heading\n"), "");
    }

    #[test]
    fn stops_at_heading() {
        let body = "First line.\n## Next section\nShould not be included.\n";
        assert_eq!(extract(body), "First line.");
    }
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p librarian-mcp preview::summary:: 2>&1 | tail -20`
Expected: 8 tests fail with `not yet implemented`.

- [ ] **Step 3: Implement `extract`**

Replace the `extract` function in `crates/librarian-mcp/src/preview/summary.rs`:

```rust
pub fn extract(body: &str) -> String {
    let mut paragraph = String::new();
    let mut in_fence = false;
    let mut seen_h1 = false;
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if trimmed.is_empty() {
            if paragraph.is_empty() {
                continue;
            } else {
                break;
            }
        }
        // Heading line
        if trimmed.starts_with('#') {
            let hash_count = trimmed.chars().take_while(|c| *c == '#').count();
            if hash_count <= 6 && trimmed.chars().nth(hash_count) == Some(' ') {
                if !seen_h1 && hash_count == 1 && paragraph.is_empty() {
                    seen_h1 = true;
                    continue;
                }
                // Any other heading terminates (or pre-empts) the paragraph.
                break;
            }
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
    }

    truncate_with_ellipsis(&paragraph, MAX_SUMMARY_CHARS)
}

fn truncate_with_ellipsis(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    // Try to trim back to the previous word boundary for nicer output.
    if let Some(last_space) = out.rfind(' ') {
        if last_space > max / 2 {
            out.truncate(last_space);
        }
    }
    out.push('…');
    out
}
```

- [ ] **Step 4: Run tests to confirm pass**

Run: `cargo test -p librarian-mcp preview::summary:: 2>&1 | tail -10`
Expected: `test result: ok. 8 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/preview/summary.rs
git commit -m "feat(librarian-mcp): shared first-paragraph summary extractor"
```

---

## Task 4: Default preview extractor

**Files:**
- Modify: `crates/librarian-mcp/src/preview/default.rs`

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/librarian-mcp/src/preview/default.rs`:

```rust
//! Fallback preview for unknown artifact kinds.

use crate::catalog::artifact::ArtifactRow;
use crate::preview::{headings, summary};
use serde_json::{json, Value};

const MAX_HEADINGS: usize = 20;

pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    todo!("implement in next step")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_row() -> ArtifactRow {
        ArtifactRow {
            id: "x".into(),
            repo: "r".into(),
            rel_path: "x.md".into(),
            kind: "unknown".into(),
            status: "active".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        }
    }

    #[test]
    fn line_count_matches_body() {
        let body = "line1\nline2\nline3\n";
        let v = extract(&mk_row(), body);
        assert_eq!(v["shape"], "default");
        assert_eq!(v["line_count"], 3);
    }

    #[test]
    fn headings_are_extracted_and_capped() {
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("## H{i}\n"));
        }
        let v = extract(&mk_row(), &body);
        assert_eq!(v["headings"].as_array().unwrap().len(), 20);
    }

    #[test]
    fn summary_extracted_from_body() {
        let body = "# Title\n\nSome prose goes here.\n";
        let v = extract(&mk_row(), body);
        assert_eq!(v["summary"], "Some prose goes here.");
    }

    #[test]
    fn empty_body_has_empty_fields() {
        let v = extract(&mk_row(), "");
        assert_eq!(v["headings"].as_array().unwrap().len(), 0);
        assert_eq!(v["summary"], "");
        assert_eq!(v["line_count"], 0);
    }
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p librarian-mcp preview::default:: 2>&1 | tail -20`
Expected: 4 tests fail.

- [ ] **Step 3: Implement `extract`**

Replace the `extract` function:

```rust
pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    let mut headings = headings::parse(body);
    headings.truncate(MAX_HEADINGS);
    let line_count = if body.is_empty() {
        0
    } else {
        body.lines().count()
    };
    json!({
        "shape": "default",
        "headings": headings,
        "summary": summary::extract(body),
        "line_count": line_count,
    })
}
```

- [ ] **Step 4: Run tests to confirm pass**

Run: `cargo test -p librarian-mcp preview::default:: 2>&1 | tail -10`
Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/preview/default.rs
git commit -m "feat(librarian-mcp): default preview extractor for unknown kinds"
```

---

## Task 5: Plan preview extractor

**Files:**
- Modify: `crates/librarian-mcp/src/preview/plan.rs`

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/librarian-mcp/src/preview/plan.rs`:

```rust
//! `plan` artifact preview: heading map + checklist progress.

use crate::catalog::artifact::ArtifactRow;
use crate::preview::headings;
use serde_json::{json, Value};

const MAX_HEADINGS: usize = 20;
const OPEN_NEXT_LIMIT: usize = 3;
const TASK_TEXT_MAX: usize = 100;

pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    todo!("implement in next step")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_row() -> ArtifactRow {
        ArtifactRow {
            id: "p".into(),
            repo: "r".into(),
            rel_path: "p.md".into(),
            kind: "plan".into(),
            status: "draft".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        }
    }

    #[test]
    fn counts_tasks_total_and_done() {
        let body = "\
- [ ] First
- [x] Second
- [X] Third (upper X)
- [ ] Fourth
";
        let v = extract(&mk_row(), body);
        assert_eq!(v["shape"], "plan");
        assert_eq!(v["tasks"]["total"], 4);
        assert_eq!(v["tasks"]["done"], 2);
    }

    #[test]
    fn open_next_returns_first_three_unchecked() {
        let body = "\
- [ ] Alpha
- [x] Beta (done)
- [ ] Gamma
- [ ] Delta
- [ ] Epsilon
";
        let v = extract(&mk_row(), body);
        let open = v["tasks"]["open_next"].as_array().unwrap();
        assert_eq!(open.len(), 3);
        assert_eq!(open[0], "Alpha");
        assert_eq!(open[1], "Gamma");
        assert_eq!(open[2], "Delta");
    }

    #[test]
    fn empty_when_no_tasks() {
        let body = "Just prose, no checklist.\n";
        let v = extract(&mk_row(), body);
        assert_eq!(v["tasks"]["total"], 0);
        assert_eq!(v["tasks"]["done"], 0);
        assert_eq!(v["tasks"]["open_next"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn ignores_task_syntax_inside_fenced_code() {
        let body = "\
- [ ] Real task
```
- [ ] Fake task inside code
- [x] Another fake
```
- [ ] Second real task
";
        let v = extract(&mk_row(), body);
        assert_eq!(v["tasks"]["total"], 2);
        assert_eq!(v["tasks"]["done"], 0);
    }

    #[test]
    fn task_text_truncated_to_limit() {
        let long_task = "x".repeat(150);
        let body = format!("- [ ] {long_task}\n");
        let v = extract(&mk_row(), &body);
        let text = v["tasks"]["open_next"][0].as_str().unwrap();
        assert!(text.chars().count() <= TASK_TEXT_MAX);
    }

    #[test]
    fn headings_included_and_capped() {
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("## H{i}\n"));
        }
        let v = extract(&mk_row(), &body);
        assert_eq!(v["headings"].as_array().unwrap().len(), 20);
    }

    #[test]
    fn nested_indented_tasks_are_counted() {
        let body = "\
- [ ] Parent
  - [x] Nested done
  - [ ] Nested open
";
        let v = extract(&mk_row(), body);
        assert_eq!(v["tasks"]["total"], 3);
        assert_eq!(v["tasks"]["done"], 1);
    }
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p librarian-mcp preview::plan:: 2>&1 | tail -20`
Expected: 7 tests fail.

- [ ] **Step 3: Implement `extract`**

Replace the `extract` function:

```rust
pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    let mut hs = headings::parse(body);
    hs.truncate(MAX_HEADINGS);

    let mut total = 0u64;
    let mut done = 0u64;
    let mut open_next: Vec<String> = Vec::new();
    let mut in_fence = false;

    for line in body.lines() {
        let trimmed_start = line.trim_start();
        if trimmed_start.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let Some(rest) = trimmed_start.strip_prefix("- [") else {
            continue;
        };
        let (marker, after) = match rest.split_once("] ") {
            Some(parts) => parts,
            None => continue,
        };
        let marker = marker.trim();
        match marker {
            " " | "" => {
                total += 1;
                if open_next.len() < OPEN_NEXT_LIMIT {
                    open_next.push(truncate_task_text(after));
                }
            }
            "x" | "X" => {
                total += 1;
                done += 1;
            }
            _ => {}
        }
    }

    json!({
        "shape": "plan",
        "headings": hs,
        "tasks": {
            "total": total,
            "done": done,
            "open_next": open_next,
        },
    })
}

fn truncate_task_text(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= TASK_TEXT_MAX {
        return s.to_string();
    }
    s.chars().take(TASK_TEXT_MAX).collect()
}
```

- [ ] **Step 4: Run tests to confirm pass**

Run: `cargo test -p librarian-mcp preview::plan:: 2>&1 | tail -10`
Expected: `test result: ok. 7 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/preview/plan.rs
git commit -m "feat(librarian-mcp): plan preview extractor with checklist progress"
```

---

## Task 6: Spec preview extractor

**Files:**
- Modify: `crates/librarian-mcp/src/preview/spec.rs`

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/librarian-mcp/src/preview/spec.rs`:

```rust
//! `spec` artifact preview: heading map + summary.

use crate::catalog::artifact::ArtifactRow;
use crate::preview::{headings, summary};
use serde_json::{json, Value};

const MAX_HEADINGS: usize = 20;

pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    todo!("implement in next step")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_row() -> ArtifactRow {
        ArtifactRow {
            id: "s".into(),
            repo: "r".into(),
            rel_path: "s.md".into(),
            kind: "spec".into(),
            status: "draft".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        }
    }

    #[test]
    fn extracts_headings_and_summary() {
        let body = "\
# Spec Title

Overview paragraph here.

## Architecture

Details here.
";
        let v = extract(&mk_row(), body);
        assert_eq!(v["shape"], "spec");
        assert_eq!(v["summary"], "Overview paragraph here.");
        let hs = v["headings"].as_array().unwrap();
        assert_eq!(hs.len(), 2);
        assert_eq!(hs[0]["text"], "Spec Title");
        assert_eq!(hs[1]["text"], "Architecture");
    }

    #[test]
    fn summary_empty_for_headings_only() {
        let body = "# A\n## B\n### C\n";
        let v = extract(&mk_row(), body);
        assert_eq!(v["summary"], "");
    }

    #[test]
    fn caps_headings_at_limit() {
        let mut body = String::new();
        for i in 0..25 {
            body.push_str(&format!("## H{i}\n"));
        }
        let v = extract(&mk_row(), &body);
        assert_eq!(v["headings"].as_array().unwrap().len(), 20);
    }
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p librarian-mcp preview::spec:: 2>&1 | tail -20`
Expected: 3 tests fail.

- [ ] **Step 3: Implement `extract`**

Replace the `extract` function:

```rust
pub fn extract(_row: &ArtifactRow, body: &str) -> Value {
    let mut hs = headings::parse(body);
    hs.truncate(MAX_HEADINGS);
    json!({
        "shape": "spec",
        "headings": hs,
        "summary": summary::extract(body),
    })
}
```

- [ ] **Step 4: Run tests to confirm pass**

Run: `cargo test -p librarian-mcp preview::spec:: 2>&1 | tail -10`
Expected: `test result: ok. 3 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/preview/spec.rs
git commit -m "feat(librarian-mcp): spec preview extractor with headings + summary"
```

---

## Task 7: Memory preview extractor

**Files:**
- Modify: `crates/librarian-mcp/src/preview/memory.rs`

- [ ] **Step 1: Write the failing tests**

Replace the contents of `crates/librarian-mcp/src/preview/memory.rs`:

```rust
//! `memory` artifact preview: observation feed + summary.

use crate::catalog::artifact::ArtifactRow;
use crate::catalog::observations;
use crate::preview::summary;
use crate::tools::ToolContext;
use serde_json::{json, Value};

const LATEST_OBSERVATIONS: usize = 3;
const OBSERVATION_TEXT_MAX: usize = 200;

pub fn extract(row: &ArtifactRow, body: &str, ctx: &ToolContext) -> Value {
    todo!("implement in next step")
}

fn truncate_text(s: &str) -> String {
    if s.chars().count() <= OBSERVATION_TEXT_MAX {
        return s.to_string();
    }
    let mut out: String = s.chars().take(OBSERVATION_TEXT_MAX).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact;
    use crate::catalog::observations::ObservationRow;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_row(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: "r".into(),
            rel_path: format!("{id}.md"),
            kind: "memory".into(),
            status: "active".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        }
    }

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
        }
    }

    #[test]
    fn latest_observations_ordered_desc_by_created_at() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        for (i, ts) in [10i64, 30, 20, 40, 50].iter().enumerate() {
            observations::insert(
                &cat,
                &ObservationRow {
                    id: None,
                    artifact_id: "m".into(),
                    text: format!("obs{i}-{ts}"),
                    source: None,
                    created_at: *ts,
                },
            )
            .unwrap();
        }
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "", &ctx);
        assert_eq!(v["shape"], "memory");
        assert_eq!(v["observation_count"], 5);
        let latest = v["latest_observations"].as_array().unwrap();
        assert_eq!(latest.len(), 3);
        // Ordered by created_at DESC: 50, 40, 30
        assert_eq!(latest[0]["created_at"], 50);
        assert_eq!(latest[1]["created_at"], 40);
        assert_eq!(latest[2]["created_at"], 30);
    }

    #[test]
    fn no_observations_returns_zero_count_and_empty_list() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "", &ctx);
        assert_eq!(v["observation_count"], 0);
        assert_eq!(v["latest_observations"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn summary_falls_back_to_empty_when_body_empty() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "", &ctx);
        assert_eq!(v["summary"], "");
    }

    #[test]
    fn summary_uses_body_when_present() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "# Title\n\nMemory prose here.\n", &ctx);
        assert_eq!(v["summary"], "Memory prose here.");
    }

    #[test]
    fn observation_text_truncated_to_limit() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("m")).unwrap();
        let long = "y".repeat(300);
        observations::insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "m".into(),
                text: long,
                source: None,
                created_at: 1,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let v = extract(&mk_row("m"), "", &ctx);
        let text = v["latest_observations"][0]["text"].as_str().unwrap();
        assert!(text.ends_with('…'));
        assert!(text.chars().count() <= OBSERVATION_TEXT_MAX + 1);
    }
}
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p librarian-mcp preview::memory:: 2>&1 | tail -20`
Expected: 5 tests fail.

- [ ] **Step 3: Implement `extract`**

Replace the `extract` function:

```rust
pub fn extract(row: &ArtifactRow, body: &str, ctx: &ToolContext) -> Value {
    let cat = ctx.catalog.lock();
    let mut obs = observations::list_for_artifact(&cat, &row.id).unwrap_or_default();
    let observation_count = obs.len();
    obs.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    obs.truncate(LATEST_OBSERVATIONS);

    let latest: Vec<Value> = obs
        .into_iter()
        .map(|o| {
            json!({
                "text": truncate_text(&o.text),
                "created_at": o.created_at,
            })
        })
        .collect();

    json!({
        "shape": "memory",
        "observation_count": observation_count,
        "latest_observations": latest,
        "summary": summary::extract(body),
    })
}
```

- [ ] **Step 4: Run tests to confirm pass**

Run: `cargo test -p librarian-mcp preview::memory:: 2>&1 | tail -10`
Expected: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/preview/memory.rs
git commit -m "feat(librarian-mcp): memory preview extractor reads observations from catalog"
```

---

## Task 8: Verify dispatch in `preview/mod.rs`

**Files:**
- Modify: `crates/librarian-mcp/src/preview/mod.rs`

Dispatch is already wired from Task 1. Now add a test module to confirm routing.

- [ ] **Step 1: Add tests to `mod.rs`**

Append to `crates/librarian-mcp/src/preview/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::ArtifactRow;
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_row(kind: &str) -> ArtifactRow {
        ArtifactRow {
            id: "x".into(),
            repo: "r".into(),
            rel_path: "x.md".into(),
            kind: kind.into(),
            status: "draft".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        }
    }

    fn mk_ctx() -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
        }
    }

    #[test]
    fn routes_plan_to_plan_extractor() {
        let v = extract("plan", &mk_row("plan"), "- [ ] a\n", &mk_ctx());
        assert_eq!(v["shape"], "plan");
    }

    #[test]
    fn routes_spec_to_spec_extractor() {
        let v = extract("spec", &mk_row("spec"), "# T\n\nx.\n", &mk_ctx());
        assert_eq!(v["shape"], "spec");
    }

    #[test]
    fn routes_memory_to_memory_extractor() {
        let v = extract("memory", &mk_row("memory"), "", &mk_ctx());
        assert_eq!(v["shape"], "memory");
    }

    #[test]
    fn unknown_kind_falls_back_to_default() {
        let v = extract("adr", &mk_row("adr"), "text\n", &mk_ctx());
        assert_eq!(v["shape"], "default");
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p librarian-mcp preview::tests 2>&1 | tail -10`
Expected: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 3: Full preview suite + clippy check**

Run: `cargo test -p librarian-mcp preview:: 2>&1 | tail -10`
Expected: all preview tests pass (headings 5 + summary 8 + default 4 + plan 7 + spec 3 + memory 5 + dispatch 4 = 36).

Run: `cargo clippy -p librarian-mcp -- -D warnings 2>&1 | tail -10`
Expected: no errors/warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/librarian-mcp/src/preview/mod.rs
git commit -m "test(librarian-mcp): dispatch tests for preview extractor"
```

---

## Task 9: Revert interim `include_body` param, establish new `Args`

**Files:**
- Modify: `crates/librarian-mcp/src/tools/get.rs`

The previous session added `include_body` to `Args` and to the JSON schema. The spec replaces it with `full`, `heading`, `headings`, `start_line`, `end_line`. This task swaps the `Args` struct and input schema; body wiring happens in Task 10.

- [ ] **Step 1: Write the failing tests**

Open `crates/librarian-mcp/src/tools/get.rs`. Replace the existing `get_include_body_reads_file_body` test body (remove the body) and add three new tests at the bottom of the `mod tests` block:

```rust
    #[tokio::test]
    async fn include_body_param_returns_migration_error() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = ArtifactGet
            .call(&ctx, json!({"id": "a", "include_body": true}))
            .await;
        let err = res.expect_err("include_body must error");
        let msg = format!("{err}");
        assert!(
            msg.contains("include_body") && msg.contains("full"),
            "error should mention migration: got {msg}"
        );
    }

    #[tokio::test]
    async fn conflicting_body_selectors_error() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = ArtifactGet
            .call(&ctx, json!({"id": "a", "full": true, "heading": "X"}))
            .await;
        assert!(res.is_err(), "conflicting selectors must error");
    }

    #[tokio::test]
    async fn start_line_greater_than_end_line_errors() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = ArtifactGet
            .call(&ctx, json!({"id": "a", "start_line": 10, "end_line": 5}))
            .await;
        assert!(res.is_err(), "inverted line range must error");
    }
```

Also **delete** the old `get_include_body_reads_file_body` test entirely (superseded by Task 11's `full=true` test).

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p librarian-mcp tools::get 2>&1 | tail -20`
Expected: new tests fail (likely compile errors until `Args` + call logic updated).

- [ ] **Step 3: Replace the `Args` struct**

In `crates/librarian-mcp/src/tools/get.rs`, replace the entire `Args` struct definition:

```rust
#[derive(Deserialize)]
struct Args {
    id: String,
    #[serde(default)]
    include_observations: Option<bool>,
    #[serde(default)]
    include_links: Option<bool>,
    #[serde(default)]
    full: Option<bool>,
    #[serde(default)]
    heading: Option<String>,
    #[serde(default)]
    headings: Option<Vec<String>>,
    #[serde(default)]
    start_line: Option<usize>,
    #[serde(default)]
    end_line: Option<usize>,
}
```

- [ ] **Step 4: Replace the `input_schema` method**

Replace the body of `input_schema` in the `impl Tool for ArtifactGet` block:

```rust
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": {"type": "string"},
                "include_observations": {"type": "boolean", "default": false},
                "include_links": {"type": "boolean", "default": false},
                "full": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include full body (subject to soft cap)."
                },
                "heading": {
                    "type": "string",
                    "description": "Fetch one section by heading match (case-insensitive, trimmed)."
                },
                "headings": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Fetch multiple sections by heading match."
                },
                "start_line": {
                    "type": "integer",
                    "description": "1-indexed start of line slice. Pair with end_line."
                },
                "end_line": {
                    "type": "integer",
                    "description": "1-indexed inclusive end of line slice."
                }
            }
        })
    }
```

- [ ] **Step 5: Reject `include_body` before deserializing `Args`**

Replace the top of the `call` method:

```rust
    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        if args.get("include_body").is_some() {
            anyhow::bail!(
                "parameter `include_body` was removed; use `full: true` for the full body, or `heading=\"<section>\"` for a targeted section"
            );
        }
        let a: Args = serde_json::from_value(args)?;
        // ... existing logic ...
```

Leave the rest of `call` unchanged for now — full body wiring happens in Task 10.

Also validate mutual exclusion and line range immediately after the `Args` parse (still inside `call`, before touching the catalog):

```rust
        let body_selectors = [
            a.full.unwrap_or(false),
            a.heading.is_some(),
            a.headings.as_ref().map_or(false, |v| !v.is_empty()),
            a.start_line.is_some() || a.end_line.is_some(),
        ];
        if body_selectors.iter().filter(|b| **b).count() > 1 {
            anyhow::bail!(
                "at most one of `full`, `heading`, `headings`, `start_line`+`end_line` may be set"
            );
        }
        if let (Some(s), Some(e)) = (a.start_line, a.end_line) {
            if s > e {
                anyhow::bail!("start_line ({s}) must be <= end_line ({e})");
            }
        }
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p librarian-mcp tools::get 2>&1 | tail -20`
Expected: new validation tests pass (`include_body_param_returns_migration_error`, `conflicting_body_selectors_error`, `start_line_greater_than_end_line_errors`). Existing tests also still pass (`get_with_links_and_observations`, `get_missing_returns_null`).

- [ ] **Step 7: Commit**

```bash
git add crates/librarian-mcp/src/tools/get.rs
git commit -m "feat(librarian-mcp)!: replace include_body with full/heading/line selectors on artifact_get

Rejects include_body with a migration error. Adds input schema for the
new selectors and enforces mutual exclusion up front. Full body wiring
lands in the next commit."
```

---

## Task 10: Wire body selectors + soft cap + overflow hint

**Files:**
- Modify: `crates/librarian-mcp/src/tools/get.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block in `crates/librarian-mcp/src/tools/get.rs`:

```rust
    use crate::workspace::Root;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: build a context with one root pointing at a tempdir.
    fn mk_ctx_with_root(cat: Catalog) -> (ToolContext, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: dir.path().to_path_buf(),
                }],
                ignore: vec![],
                rules: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
        };
        (ctx, dir)
    }

    #[tokio::test]
    async fn full_true_returns_body_within_cap() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\nShort body.\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "full": true}))
            .await
            .unwrap();
        assert!(v["body"].as_str().unwrap().contains("Short body."));
        assert!(v.get("overflow").is_none(), "short body must not overflow");
    }

    #[tokio::test]
    async fn full_true_triggers_overflow_over_cap() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        let mut body = String::from("---\nkind: spec\n---\n\n");
        body.push_str("# Top\n\n");
        body.push_str("## Section One\n\n");
        for i in 0..600 {
            body.push_str(&format!("Line {i}\n"));
        }
        body.push_str("## Section Two\n");
        fs::write(dir.path().join("a.md"), body).unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "full": true}))
            .await
            .unwrap();
        let overflow = v["overflow"].as_object().expect("overflow present");
        assert!(overflow["total_lines"].as_u64().unwrap() > 500);
        assert_eq!(overflow["shown_lines"], 500);
        let hint = overflow["hint"].as_str().unwrap();
        assert!(hint.contains("heading="), "hint must suggest heading= usage");
        assert!(hint.contains("Top"), "hint lists top-level headings");
    }

    #[tokio::test]
    async fn heading_targeted_read_returns_single_section() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# Title\n\n## Alpha\n\nalpha body\n\n## Beta\n\nbeta body\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "heading": "Alpha"}))
            .await
            .unwrap();
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("alpha body"));
        assert!(!body.contains("beta body"));
    }

    #[tokio::test]
    async fn heading_missing_sets_meta_flag() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(dir.path().join("a.md"), "---\nkind: spec\n---\n\n# T\n\n## A\n\nx\n").unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "heading": "Nonexistent"}))
            .await
            .unwrap();
        assert_eq!(v["body"], "");
        assert_eq!(v["body_meta"]["heading_missing"], true);
    }

    #[tokio::test]
    async fn line_slice_returns_requested_range() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        // NOTE: no blank line between the closing `---` and the content so that
        // start_line=1 corresponds to L1 in the parsed body.
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\nL1\nL2\nL3\nL4\nL5\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(
                &ctx,
                json!({"id": "a", "start_line": 2, "end_line": 4}),
            )
            .await
            .unwrap();
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("L2"));
        assert!(body.contains("L3"));
        assert!(body.contains("L4"));
        assert!(!body.contains("L1"));
        assert!(!body.contains("L5"));
    }
```

- [ ] **Step 2: Run tests to confirm failure**

Run: `cargo test -p librarian-mcp tools::get 2>&1 | tail -20`
Expected: 5 new tests fail (body wiring not yet implemented).

- [ ] **Step 3: Add body-resolution helpers at the top of `get.rs`**

Just below the `use` statements in `crates/librarian-mcp/src/tools/get.rs`, add:

```rust
use crate::frontmatter;
use crate::preview::headings;
use std::path::PathBuf;

const SOFT_CAP_LINES: usize = 500;
const OVERFLOW_HEADING_LIMIT: usize = 10;

fn resolve_file_path(ctx: &ToolContext, row: &crate::catalog::artifact::ArtifactRow) -> Option<PathBuf> {
    ctx.workspace
        .roots
        .iter()
        .find(|r| r.name == row.repo)
        .map(|r| r.path.join(&row.rel_path))
}

fn find_heading_section<'a>(body: &'a str, query: &str) -> Option<String> {
    let hs = headings::parse(body);
    let normalized_query = normalize_heading(query);
    let idx = hs
        .iter()
        .position(|h| normalize_heading(&h.text) == normalized_query)?;
    let start_line = hs[idx].line;
    let start_level = hs[idx].level;
    let end_line = hs[idx + 1..]
        .iter()
        .find(|h| h.level <= start_level)
        .map(|h| h.line)
        .unwrap_or(usize::MAX);
    let lines: Vec<&str> = body.lines().collect();
    let slice_end = std::cmp::min(end_line.saturating_sub(1), lines.len());
    Some(lines[start_line - 1..slice_end].join("\n"))
}

fn normalize_heading(s: &str) -> String {
    s.trim()
        .trim_start_matches('#')
        .trim()
        .to_lowercase()
}

fn slice_lines(body: &str, start: usize, end: usize) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if start == 0 || start > lines.len() {
        return String::new();
    }
    let end = std::cmp::min(end, lines.len());
    lines[start - 1..end].join("\n")
}

fn apply_soft_cap(body: &str) -> (String, Option<(usize, usize, Vec<String>)>) {
    let lines: Vec<&str> = body.lines().collect();
    let total = lines.len();
    if total <= SOFT_CAP_LINES {
        return (body.to_string(), None);
    }
    let shown: String = lines[..SOFT_CAP_LINES].join("\n");
    let top_headings: Vec<String> = headings::parse(body)
        .into_iter()
        .filter(|h| h.level <= 2)
        .take(OVERFLOW_HEADING_LIMIT)
        .map(|h| h.text)
        .collect();
    (shown, Some((SOFT_CAP_LINES, total, top_headings)))
}
```

- [ ] **Step 4: Wire body routing into `call`**

Find the section of `call` that currently reads the body when `include_body` was true (the `if a.include_body.unwrap_or(false)` block). Replace that entire block with:

```rust
        let file_path = resolve_file_path(ctx, &row);
        let body_selected = a.full.unwrap_or(false)
            || a.heading.is_some()
            || a.headings.as_ref().map_or(false, |v| !v.is_empty())
            || a.start_line.is_some()
            || a.end_line.is_some();

        // Always attempt to compute a preview (even if body not selected).
        let file_content = match &file_path {
            Some(p) => match std::fs::read_to_string(p) {
                Ok(c) => Some(c),
                Err(e) => {
                    out["preview"] = Value::Null;
                    out["body_error"] = json!(e.to_string());
                    None
                }
            },
            None => {
                out["preview"] = Value::Null;
                out["body_error"] = json!(format!(
                    "repo {:?} not in workspace.roots",
                    row.repo
                ));
                None
            }
        };

        let parsed_body: Option<String> = file_content.as_ref().map(|content| {
            match frontmatter::parse(content) {
                Ok((_, b)) => b.to_string(),
                Err(_) => content.clone(),
            }
        });

        if let Some(body) = parsed_body.as_deref() {
            out["preview"] = crate::preview::extract(&row.kind, &row, body, ctx);

            if body_selected {
                let (final_body, overflow_meta, body_meta_extra) =
                    if let Some(ref name) = a.heading {
                        match find_heading_section(body, name) {
                            Some(section) => (section, None, json!({ "heading": name })),
                            None => (
                                String::new(),
                                None,
                                json!({ "heading": name, "heading_missing": true }),
                            ),
                        }
                    } else if let Some(ref list) = a.headings {
                        let mut parts = Vec::new();
                        let mut missing = Vec::new();
                        for name in list {
                            match find_heading_section(body, name) {
                                Some(s) => parts.push(s),
                                None => missing.push(name.clone()),
                            }
                        }
                        let joined = parts.join("\n\n");
                        let extra = if missing.is_empty() {
                            json!({ "headings": list })
                        } else {
                            json!({ "headings": list, "headings_missing": missing })
                        };
                        (joined, None, extra)
                    } else if let (Some(s), Some(e)) = (a.start_line, a.end_line) {
                        (
                            slice_lines(body, s, e),
                            None,
                            json!({ "start_line": s, "end_line": e }),
                        )
                    } else {
                        // full = true
                        let (shown, overflow) = apply_soft_cap(body);
                        (shown, overflow, json!({}))
                    };

                let total_lines = body.lines().count();
                let bytes = final_body.len();
                out["body"] = json!(final_body);
                let mut meta = json!({
                    "line_count": total_lines,
                    "bytes": bytes,
                });
                if let Some(extra) = body_meta_extra.as_object() {
                    for (k, v) in extra {
                        meta[k] = v.clone();
                    }
                }
                out["body_meta"] = meta;

                if let Some((shown, total, headings)) = overflow_meta {
                    let hint = format!(
                        "Body exceeds soft cap ({SOFT_CAP_LINES} lines). Narrow with heading=\"<section>\" or start_line=N, end_line=M. Top-level headings: {headings:?}"
                    );
                    out["overflow"] = json!({
                        "shown_lines": shown,
                        "total_lines": total,
                        "hint": hint,
                    });
                }
            }
        }
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p librarian-mcp tools::get 2>&1 | tail -30`
Expected: all 5 new tests pass plus the existing ones (`get_missing_returns_null`, `get_with_links_and_observations`, `include_body_param_returns_migration_error`, `conflicting_body_selectors_error`, `start_line_greater_than_end_line_errors`).

- [ ] **Step 6: Commit**

```bash
git add crates/librarian-mcp/src/tools/get.rs
git commit -m "feat(librarian-mcp): wire full/heading/line selectors + soft cap on artifact_get"
```

---

## Task 11: Ensure preview appears even without body selectors + `body_error` path

**Files:**
- Modify: `crates/librarian-mcp/src/tools/get.rs`

- [ ] **Step 1: Write the failing tests**

Append to the `mod tests` block:

```rust
    #[tokio::test]
    async fn preview_present_by_default() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = mk_row("a");
        row.kind = "spec".into();
        artifact::upsert(&cat, &row).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# A\n\nHello world.\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a"}))
            .await
            .unwrap();
        assert_eq!(v["preview"]["shape"], "spec");
        assert!(v.get("body").is_none(), "body absent when not selected");
    }

    #[tokio::test]
    async fn preview_null_when_file_missing() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, _dir) = mk_ctx_with_root(cat);
        // Note: file was never written.

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a"}))
            .await
            .unwrap();
        assert!(v["preview"].is_null());
        assert!(v["body_error"].as_str().is_some());
    }

    #[tokio::test]
    async fn preview_null_when_repo_not_in_roots() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat); // mk_ctx has roots: vec![]

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a"}))
            .await
            .unwrap();
        assert!(v["preview"].is_null());
        assert!(v["body_error"].as_str().unwrap().contains("workspace.roots"));
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p librarian-mcp tools::get 2>&1 | tail -20`

If any fail: review the body-routing block from Task 10 — the `preview` assignment must happen regardless of `body_selected`. Check that both the file-read failure paths (no root / IO error) set `preview: Null` and `body_error`.

Expected after fix: all 3 tests pass.

- [ ] **Step 3: Update existing `get_with_links_and_observations` test**

This test was written before preview existed. It still asserts shape correctly (doesn't check `preview`), but let's assert preview is present (null) for clarity. Find the test and add this assertion after the existing ones:

```rust
        // Preview is null here because mk_ctx has no roots configured.
        assert!(v["preview"].is_null());
```

- [ ] **Step 4: Run the whole `tools::get` suite**

Run: `cargo test -p librarian-mcp tools::get 2>&1 | tail -20`
Expected: all tests pass (12 total: 2 original + 3 validation from Task 9 + 5 body wiring from Task 10 + 3 preview visibility from this task — minus the deleted `get_include_body_reads_file_body`).

- [ ] **Step 5: Commit**

```bash
git add crates/librarian-mcp/src/tools/get.rs
git commit -m "feat(librarian-mcp): always return preview field on artifact_get; null on file errors"
```

---

## Task 12: End-to-end integration test

**Files:**
- Modify: `crates/librarian-mcp/src/tools/get.rs`

- [ ] **Step 1: Write the integration test**

Append to `mod tests`:

```rust
    #[tokio::test]
    async fn end_to_end_plan_across_all_modes() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = mk_row("pl");
        row.kind = "plan".into();
        artifact::upsert(&cat, &row).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("pl.md"),
            "---\nkind: plan\n---\n\n\
# Big Plan\n\n\
## Phase 1\n\n\
- [ ] Alpha task\n\
- [x] Beta done\n\
- [ ] Gamma task\n\n\
## Phase 2\n\n\
- [ ] Delta task\n",
        )
        .unwrap();

        // Mode 1: preview default
        let v = ArtifactGet
            .call(&ctx, json!({"id": "pl"}))
            .await
            .unwrap();
        assert_eq!(v["preview"]["shape"], "plan");
        assert_eq!(v["preview"]["tasks"]["total"], 4);
        assert_eq!(v["preview"]["tasks"]["done"], 1);
        let open = v["preview"]["tasks"]["open_next"].as_array().unwrap();
        assert_eq!(open[0], "Alpha task");
        assert!(v.get("body").is_none());

        // Mode 2: full body
        let v = ArtifactGet
            .call(&ctx, json!({"id": "pl", "full": true}))
            .await
            .unwrap();
        assert!(v["body"].as_str().unwrap().contains("Alpha task"));
        assert!(v["body"].as_str().unwrap().contains("Phase 2"));
        assert!(v.get("overflow").is_none());

        // Mode 3: heading-targeted read
        let v = ArtifactGet
            .call(&ctx, json!({"id": "pl", "heading": "Phase 1"}))
            .await
            .unwrap();
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("Alpha task"));
        assert!(body.contains("Gamma task"));
        assert!(!body.contains("Delta task"), "Phase 2 content must be excluded");
    }
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p librarian-mcp tools::get::tests::end_to_end_plan_across_all_modes 2>&1 | tail -10`
Expected: `test result: ok. 1 passed`.

- [ ] **Step 3: Commit**

```bash
git add crates/librarian-mcp/src/tools/get.rs
git commit -m "test(librarian-mcp): end-to-end artifact_get across preview/full/heading modes"
```

---

## Task 13: Full verification + release build

**Files:** none (verification only).

- [ ] **Step 1: Run the full test suite**

Run: `cargo test -p librarian-mcp 2>&1 | tail -20`
Expected: all tests pass. Total should be original 89 + new preview tests (36) + new get tests (net +8 after deleting the interim test) ≈ 133.

- [ ] **Step 2: Check formatting**

Run: `cargo fmt --all --check 2>&1 | tail -5`
Expected: no output (all formatted).

If output appears: `cargo fmt --all` and re-commit with a `chore(fmt):` message.

- [ ] **Step 3: Lint**

Run: `cargo clippy -p librarian-mcp --all-targets -- -D warnings 2>&1 | tail -10`
Expected: no warnings or errors.

- [ ] **Step 4: Release build (for live MCP testing)**

Run: `cargo build --release -p librarian-mcp 2>&1 | tail -5`
Expected: `Finished \`release\` profile [optimized] target(s) in X.XXs`.

Per `CLAUDE.md`, the MCP server runs the release binary — dev builds are not picked up. After this step the change is ready to exercise via `/mcp` restart.

- [ ] **Step 5: Final commit if anything outstanding**

If any fmt / clippy fixes were required, commit them now. Otherwise skip.

```bash
# Only if fmt/clippy produced edits:
git add -u crates/librarian-mcp/
git commit -m "chore(librarian-mcp): fmt + clippy after preview implementation"
```

- [ ] **Step 6: Live-smoke the MCP server**

Ask the operator to restart the MCP server with `/mcp`, then call `artifact_get` on a known plan with and without `full`/`heading` to confirm the new fields land in the live response. If the operator reports a mismatch, open a bug log entry per `docs/TODO-tool-misbehaviors.md` before proceeding.

---

## Self-Review Notes

Spec coverage cross-check:

| Spec requirement                               | Task |
|------------------------------------------------|------|
| Structured kind-specific previews              | 4–8  |
| `full` / `heading` / `headings` / `start_line`/`end_line` | 9–10 |
| Soft cap on `full` with overflow hint          | 10   |
| Mutual exclusion of body selectors             | 9    |
| `include_body` migration error                 | 9    |
| Preview null + body_error on file/root error   | 11   |
| On-the-fly (no schema change)                  | 1, 10 |
| Per-kind extractor files under `src/preview/`  | 1–7  |
| End-to-end integration test                    | 12   |
| Heading match: case-insensitive, hash-trimmed  | 10 (`normalize_heading`) |
| Summary: skip H1, skip fenced code, 200 chars  | 3    |
| Heading parser: ATX only, skip fenced code     | 2    |

No placeholders remain; all code blocks are concrete and copy-pasteable. Function and type names are consistent across tasks (`extract`, `parse`, `ArtifactRow`, `ToolContext`, `Value`). The `mk_ctx_with_root` helper is introduced in Task 10 and reused in Tasks 11–12; earlier tasks use the existing `mk_ctx` (no roots) since they test validation paths that don't require disk I/O.

## Post-Ship Fixes

Landed as part of the feature but not captured in the original 13-task plan.

### Caught during task execution

- **Task 3 review (code quality):** Summary extractor wasn't collapsing intra-line whitespace. Fixed with `split_whitespace().collect().join(" ")` pass. Regression test `collapses_intra_line_whitespace`. Commit `8fa032f`.

### Caught in final cross-commit review

- **Deadlock on memory-kind artifact_get:** `parking_lot::Mutex` is not reentrant. `call` held the catalog lock across `preview::extract`, which re-locks for memory-kind (reads observations). Tests missed it because `mk_row` hardcoded `kind: "spec"`. Fixed by narrowing the lock scope to a `{ }` block that ends before `preview::extract`. Added regression test `memory_kind_does_not_deadlock_on_preview` wrapping the call in `tokio::time::timeout(3s)`. Commit `ae03dd5`.

### Caught live on real data (2026-04-21)

- **Summary empty on real memory artifact (CLAUDE.md):** Summary extractor's heading-skip guard only allowed H1 to be skipped while the paragraph was empty. A file with H1 → blank → H2 → blank → prose returned `summary: ""` because the H2 triggered the paragraph-terminating `break` before any prose had been accumulated. Fix: skip any leading heading while `paragraph.is_empty()`; the heading-terminates-paragraph branch only fires once prose has been seen. Two regression tests: `skips_consecutive_leading_headings`, `skips_leading_h2_without_h1`. Commit `a1f5f73`.

### Non-blocking follow-ups (shipped 2026-04-21)

Identified during final review, implemented as a dedicated follow-up commit `1561661`:

- **I1** — `find_heading_section` was re-parsing the body's headings once per lookup. For `headings: [...]` selectors this was O(n·k). Changed to accept `&[Heading]`; parse once in the call path.
- **I3** — `body_meta.line_count` was ambiguous (total body lines on a heading/slice call). Changed to lines in the *returned* body, added `source_line_count` for the full-body total. Matches what callers actually want: "how much context is this costing me?" + "how much did I miss?".
- **M2** — Only the `default` preview shape had a `line_count` field. Added it to plan/spec/memory shapes for symmetry.

Tests: 145 pass (was 143 pre-follow-up). Clippy clean. Release built. Live-verified via `/mcp` restart.

## History

- 2026-04-20 — Plan created, brainstorm + spec approved, subagent-driven execution started.
- 2026-04-21 — All 13 tasks shipped via subagent-driven-development (commits `3d3e5f9` through `f6314db` on `experiments` branch). 2 bugs caught post-ship (deadlock, summary-empty). 3 non-blocking follow-ups landed same day. Status flipped to SHIPPED.
