# Audit Doc Refs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `librarian(action="audit_doc_refs")` that scans markdown files for stale code references (file paths, line refs, symbols, link targets, dotted module paths) and emits findings as an `audit_issues` tracker.

**Architecture:** New module `src/librarian/tools/audit_doc_refs/` split into parser (pulldown-cmark walk), resolver (FS + LSP), severity, and merger units. Dispatched from the existing `Librarian::call` action enum. Tracker upserts via existing `artifact_augment(merge=true)` plumbing. Manual cadence; no CI integration in v1.

**Tech Stack:** Rust, pulldown-cmark 0.13 (already a dep), MiniJinja (already a dep, for tracker render template), existing `src/lsp/symbols.rs` symbol resolver, existing librarian `ToolContext`, `MockLspClient` for unit tests.

---

## Spec reference

This plan implements `docs/superpowers/specs/2026-05-16-audit-doc-refs-design.md`. Read the spec first if you have not; this plan assumes you have.

## Prerequisites

**Two upstream PRs must land before this plan executes:**

1. **PR-B `2026-05-16-artifact-cli-design.md`** — currently closer to merge; lands first against current `crates/librarian-mcp/` layout. Adds `codescout artifact <verb>` CLI surface.
2. **PR-A `dissolve-librarian-crate`** — mechanical move of `crates/librarian-mcp/src/**` → `src/librarian/**`. Folds Cargo deps into root. Keeps `librarian` feature flag as a module gate. No design doc — checklist only.

After both land, paths below are correct as written. **If you start before PR-A lands, every path that begins `src/librarian/` lives at `crates/librarian-mcp/src/` instead and imports are `librarian_mcp::...` rather than `crate::librarian::...`.** Do not start until at least PR-A has landed; the plan's resolver step (Task 6) cannot work without the dissolved layout.

## File structure

| File | Responsibility | Status |
|---|---|---|
| `src/librarian/tools/audit_doc_refs/mod.rs` | Types (`RefCandidate`, `RefKind`, `Finding`, `Resolution`, etc.), action dispatch entry, glue | Create |
| `src/librarian/tools/audit_doc_refs/parser.rs` | `parse_refs(text, path) -> (Vec<RefCandidate>, Vec<ParseWarning>)`; pulldown-cmark walk | Create |
| `src/librarian/tools/audit_doc_refs/resolver.rs` | `ResolveCtx`, `resolve_ref(candidate, ctx) -> Resolution`; FS + LSP checks | Create |
| `src/librarian/tools/audit_doc_refs/severity.rs` | Default severity map + drop rules + `severity_reason` assignment | Create |
| `src/librarian/tools/audit_doc_refs/merger.rs` | `merge_into_tracker(findings, prior, now, commit) -> TrackerParams` | Create |
| `src/librarian/tools/librarian.rs` | Extend `Action` enum + dispatch + `description` text | Modify |
| `src/librarian/tools/mod.rs` | `pub mod audit_doc_refs;` registration | Modify |
| `tests/librarian/audit_doc_refs/corpus.rs` | Tier-2 fixture-driven behavior tests | Create |
| `tests/librarian/audit_doc_refs/fixtures/*` | Six fixture repos: clean, drift, regression, wontfix, archive_drop, parse_recovery | Create |
| `tests/librarian/audit_doc_refs/eval_on_codescout_self.rs` | Tier-3 `#[ignore]`-marked eval | Create |
| `docs/manual/src/concepts/audit-doc-refs.md` | User-facing manual page | Create |
| `docs/superpowers/specs/2026-05-16-audit-doc-refs-design.md` | (unchanged) | — |

## Patterns to follow (read before starting)

- **Tool dispatch shape:** see `src/librarian/tools/refresh.rs` for the canonical `pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value>` shape and inline `#[cfg(test)] mod tests` with `mk_ctx` helper.
- **`RecoverableError`:** defined at `src/librarian/tools/mod.rs`. Use `RecoverableError::with_hint(...)` for expected, input-driven failures (bad glob, path outside project, repo without trackers dir). `anyhow::bail!` for genuine bugs.
- **`OutputGuard`:** at `src/tools/output.rs`. Use `cap_items` for the `findings` array; emit `OverflowInfo.by_file` distribution map. Project invariant — see `docs/PROGRESSIVE_DISCOVERABILITY.md`.
- **Tracker upsert:** existing `artifact_augment(merge=true)` path at `src/librarian/tools/augment.rs`. Pass JSON merge-patch via params; do not bypass.
- **MockLspClient:** at `src/lsp/mock.rs`. Constructors accept canned symbol-lookup responses for unit tests.
- **`audit_issues` archetype:** definition at `src/librarian/tools/tracker_design.rs::archetype_audit_issues` (line 155 of current crate file). Reuse without modification.

---

## Phase 1 — Parser

Parser is pure: `(text, md_path) -> (Vec<RefCandidate>, Vec<ParseWarning>)`. No I/O, no LSP, no FS. Easy to unit-test exhaustively.

### Task 1: Scaffold module + core types

**Files:**
- Create: `src/librarian/tools/audit_doc_refs/mod.rs`
- Create: `src/librarian/tools/audit_doc_refs/parser.rs`
- Modify: `src/librarian/tools/mod.rs` (add `pub mod audit_doc_refs;`)

- [ ] **Step 1: Create the module directory + register**

```rust
// src/librarian/tools/mod.rs — add to existing pub mod list
pub mod audit_doc_refs;
```

- [ ] **Step 2: Write `mod.rs` with core types**

```rust
// src/librarian/tools/audit_doc_refs/mod.rs
use serde::{Deserialize, Serialize};

pub mod parser;
// resolver, severity, merger added in later tasks

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefKind {
    FilePath,
    FileLine,
    FileSymbol,
    ModulePath,
    Link,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefPosition {
    InlineSpan,
    FencedBlock,
    LinkTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefCandidate {
    pub md_file: String,
    pub md_line: u32,
    pub raw_ref: String,
    pub ref_kind: RefKind,
    pub position: RefPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParseWarning {
    pub md_file: String,
    pub line: u32,
    pub reason: String,
}
```

- [ ] **Step 3: Stub parser.rs**

```rust
// src/librarian/tools/audit_doc_refs/parser.rs
use super::{ParseWarning, RefCandidate};
use std::path::Path;

pub fn parse_refs(_text: &str, _md_path: &Path) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
    (Vec::new(), Vec::new())
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p librarian-mcp` (pre-dissolve) or `cargo check` (post-dissolve).
Expected: clean, no errors, no new warnings.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/mod.rs src/librarian/tools/audit_doc_refs/
git commit -m "feat(audit_doc_refs): scaffold module + core types"
```

### Task 2: parser — file_path classifier

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/parser.rs`

- [ ] **Step 1: Write the failing tests**

```rust
// at the bottom of parser.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse(text: &str) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
        parse_refs(text, &PathBuf::from("test.md"))
    }

    #[test]
    fn parser_resolves_simple_file_path() {
        let (cands, _) = parse("See `src/foo.py` for the entry point.");
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].raw_ref, "src/foo.py");
        assert_eq!(cands[0].ref_kind, RefKind::FilePath);
        assert_eq!(cands[0].position, RefPosition::InlineSpan);
    }

    #[test]
    fn parser_ignores_prose_outside_code_spans() {
        let (cands, _) = parse("We use Pydantic for validation.");
        assert_eq!(cands.len(), 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p librarian-mcp audit_doc_refs::parser::tests`
Expected: both fail — first because `cands.len()` is 0, second passes accidentally (stub returns empty). Adjust expectation: the second test should pass at stub stage; the first fails.

- [ ] **Step 3: Implement minimal file_path classifier**

Replace the stub with:

```rust
// src/librarian/tools/audit_doc_refs/parser.rs
use super::{ParseWarning, RefCandidate, RefKind, RefPosition};
use pulldown_cmark::{Event, Options, Parser, Tag};
use std::path::Path;

pub fn parse_refs(text: &str, md_path: &Path) -> (Vec<RefCandidate>, Vec<ParseWarning>) {
    let md_file = md_path.to_string_lossy().to_string();
    let opts = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut candidates = Vec::new();
    let warnings = Vec::new(); // populated in Task 5

    let parser = Parser::new_ext(text, opts).into_offset_iter();
    for (event, span) in parser {
        let line = byte_offset_to_line(text, span.start);
        match event {
            Event::Code(content) => {
                if let Some(kind) = classify(content.as_ref()) {
                    candidates.push(RefCandidate {
                        md_file: md_file.clone(),
                        md_line: line,
                        raw_ref: content.into_string(),
                        ref_kind: kind,
                        position: RefPosition::InlineSpan,
                    });
                }
            }
            _ => {}
        }
    }
    (candidates, warnings)
}

fn classify(s: &str) -> Option<RefKind> {
    if looks_like_path(s) {
        Some(RefKind::FilePath)
    } else {
        None
    }
}

fn looks_like_path(s: &str) -> bool {
    // Heuristic: contains `/` or has a known file extension, no spaces.
    if s.contains(char::is_whitespace) {
        return false;
    }
    if s.contains('/') {
        return true;
    }
    matches!(
        s.rsplit_once('.').map(|(_, ext)| ext),
        Some("rs" | "py" | "ts" | "js" | "kt" | "java" | "go" | "md" | "toml" | "yaml" | "yml" | "json")
    )
}

fn byte_offset_to_line(text: &str, offset: usize) -> u32 {
    1 + text[..offset.min(text.len())].bytes().filter(|&b| b == b'\n').count() as u32
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::parser::tests`
Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/parser.rs
git commit -m "feat(audit_doc_refs): parse file_path candidates from inline code spans"
```

### Task 3: parser — file_line and file_symbol classifiers

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/parser.rs`

- [ ] **Step 1: Add failing tests**

```rust
// extend mod tests in parser.rs
#[test]
fn parser_classifies_file_line_over_file_path() {
    let (cands, _) = parse("at `scripts/eval_chunking.py:807` we see...");
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].ref_kind, RefKind::FileLine);
    assert_eq!(cands[0].raw_ref, "scripts/eval_chunking.py:807");
}

#[test]
fn parser_classifies_file_symbol_over_file_line() {
    let (cands, _) = parse("see `src/mrv/cli.py:cmd_generate` for...");
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].ref_kind, RefKind::FileSymbol);

    let (cands, _) = parse("see `src/foo.rs:Bar/baz` for...");
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].ref_kind, RefKind::FileSymbol);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p librarian-mcp audit_doc_refs::parser::tests`
Expected: both new tests fail — current classifier emits `FilePath` for all.

- [ ] **Step 3: Refine `classify`**

Replace `classify` with:

```rust
fn classify(s: &str) -> Option<RefKind> {
    if let Some((path_part, suffix)) = s.rsplit_once(':') {
        if looks_like_path(path_part) {
            // file_symbol if suffix contains `/` or starts with non-digit; file_line if all digits
            if suffix.chars().all(|c| c.is_ascii_digit()) && !suffix.is_empty() {
                return Some(RefKind::FileLine);
            }
            // Symbol patterns: `Class/method`, `fn_name`, `Class.method`
            if is_symbol_suffix(suffix) {
                return Some(RefKind::FileSymbol);
            }
        }
    }
    if looks_like_path(s) {
        return Some(RefKind::FilePath);
    }
    None
}

fn is_symbol_suffix(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '/' || c == '.')
        && s.chars().next().map(|c| !c.is_ascii_digit()).unwrap_or(false)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::parser::tests`
Expected: all four parser tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/parser.rs
git commit -m "feat(audit_doc_refs): classify file_line and file_symbol refs"
```

### Task 4: parser — module_path with code-context guard, link, fenced blocks

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/parser.rs`

- [ ] **Step 1: Add failing tests**

```rust
#[test]
fn parser_module_path_requires_code_context() {
    // Prose — must NOT classify
    let (cands, _) = parse("We import from mrv.chat_app in the runner.");
    assert!(
        cands.iter().all(|c| c.ref_kind != RefKind::ModulePath),
        "prose dotted-ident must not emit ModulePath"
    );

    // Code span — must classify
    let (cands, _) = parse("Use `mrv.chat_app` here.");
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].ref_kind, RefKind::ModulePath);
}

#[test]
fn parser_extracts_link_targets() {
    let (cands, _) = parse("[label](src/foo.py)");
    assert_eq!(cands.len(), 1);
    assert_eq!(cands[0].ref_kind, RefKind::Link);
    assert_eq!(cands[0].position, RefPosition::LinkTarget);
}

#[test]
fn parser_walks_fenced_code_blocks() {
    let text = "```\nimport mrv.chat_app\n```\n";
    let (cands, _) = parse(text);
    // expect at least one module_path candidate from the fenced block
    assert!(cands.iter().any(|c| c.ref_kind == RefKind::ModulePath));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p librarian-mcp audit_doc_refs::parser::tests`
Expected: three new tests fail (links and fenced blocks not yet walked; module_path not implemented).

- [ ] **Step 3: Implement**

Replace the event loop body in `parse_refs`:

```rust
let mut in_code_block = false;
let parser = Parser::new_ext(text, opts).into_offset_iter();
for (event, span) in parser {
    let line = byte_offset_to_line(text, span.start);
    match event {
        Event::Code(content) => {
            for raw in tokenize_code_span(content.as_ref()) {
                if let Some(kind) = classify(raw, /*in_code_context=*/ true) {
                    candidates.push(RefCandidate {
                        md_file: md_file.clone(),
                        md_line: line,
                        raw_ref: raw.to_string(),
                        ref_kind: kind,
                        position: RefPosition::InlineSpan,
                    });
                }
            }
        }
        Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
        Event::End(_) => in_code_block = false,
        Event::Text(content) if in_code_block => {
            for raw in tokenize_code_span(content.as_ref()) {
                if let Some(kind) = classify(raw, /*in_code_context=*/ true) {
                    candidates.push(RefCandidate {
                        md_file: md_file.clone(),
                        md_line: line,
                        raw_ref: raw.to_string(),
                        ref_kind: kind,
                        position: RefPosition::FencedBlock,
                    });
                }
            }
        }
        Event::Start(Tag::Link { dest_url, .. }) => {
            if let Some(kind) = classify(dest_url.as_ref(), /*in_code_context=*/ false) {
                candidates.push(RefCandidate {
                    md_file: md_file.clone(),
                    md_line: line,
                    raw_ref: dest_url.into_string(),
                    ref_kind: kind,
                    position: RefPosition::LinkTarget,
                });
            } else {
                // links always emit, even when classify returns None — they fall under RefKind::Link.
                candidates.push(RefCandidate {
                    md_file: md_file.clone(),
                    md_line: line,
                    raw_ref: dest_url.into_string(),
                    ref_kind: RefKind::Link,
                    position: RefPosition::LinkTarget,
                });
            }
        }
        _ => {}
    }
}
```

Helpers:

```rust
fn tokenize_code_span(s: &str) -> impl Iterator<Item = &str> {
    s.split_whitespace()
}

fn classify(s: &str, in_code_context: bool) -> Option<RefKind> {
    // existing file_line / file_symbol / file_path logic ...
    if let Some((path_part, suffix)) = s.rsplit_once(':') {
        if looks_like_path(path_part) {
            if suffix.chars().all(|c| c.is_ascii_digit()) && !suffix.is_empty() {
                return Some(RefKind::FileLine);
            }
            if is_symbol_suffix(suffix) {
                return Some(RefKind::FileSymbol);
            }
        }
    }
    if looks_like_path(s) {
        return Some(RefKind::FilePath);
    }
    if in_code_context && is_module_path(s) {
        return Some(RefKind::ModulePath);
    }
    None
}

fn is_module_path(s: &str) -> bool {
    s.contains('.')
        && !s.contains('/')
        && !s.contains(char::is_whitespace)
        && s.chars().all(|c| c.is_lowercase() || c.is_ascii_digit() || c == '.' || c == '_')
        && s.split('.').all(|part| !part.is_empty())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::parser::tests`
Expected: all 7 parser tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/parser.rs
git commit -m "feat(audit_doc_refs): module_path with code-context guard, link, fenced blocks"
```

### Task 5: parser — parse_warnings on malformed input

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/parser.rs`

- [ ] **Step 1: Add failing test**

```rust
#[test]
fn parser_recovers_from_unterminated_fence() {
    let text = "intro\n```\nsome code without close\n";
    let (cands, warns) = parse(text);
    // best-effort: cands may be empty or non-empty; warnings should be populated
    assert!(
        !warns.is_empty(),
        "expected at least one parse_warning for unterminated fence"
    );
    assert!(warns[0].reason.contains("fence") || warns[0].reason.contains("unterminated"));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p librarian-mcp audit_doc_refs::parser::tests::parser_recovers_from_unterminated_fence`
Expected: FAIL (`warns.is_empty()`).

- [ ] **Step 3: Implement fence-warning detection**

Pulldown-cmark does not error on unterminated fences — it implicitly closes at EOF. Detect by counting opening fences via a regex pre-pass:

```rust
// at top of parser.rs
use regex::Regex;
use std::sync::OnceLock;

fn fence_warnings(text: &str, md_file: &str) -> Vec<ParseWarning> {
    static FENCE_RE: OnceLock<Regex> = OnceLock::new();
    let re = FENCE_RE.get_or_init(|| Regex::new(r"(?m)^```").unwrap());
    let opens: Vec<_> = re.find_iter(text).collect();
    if opens.len() % 2 == 1 {
        let last = opens.last().unwrap();
        let line = 1 + text[..last.start()].bytes().filter(|&b| b == b'\n').count() as u32;
        vec![ParseWarning {
            md_file: md_file.to_string(),
            line,
            reason: "unterminated code fence".to_string(),
        }]
    } else {
        Vec::new()
    }
}
```

In `parse_refs`, replace the `let warnings = Vec::new();` line with:

```rust
let warnings = fence_warnings(text, &md_file);
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::parser::tests`
Expected: all parser tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/parser.rs
git commit -m "feat(audit_doc_refs): emit parse_warning for unterminated code fence"
```

---

## Phase 2 — Resolver

Resolver is stateful via `ResolveCtx` (catalog + LSP handle). Each candidate produces a `Resolution { verdict, severity, severity_reason, notes }`.

### Task 6: resolver scaffold + file_path / file_line verdicts

**Files:**
- Create: `src/librarian/tools/audit_doc_refs/resolver.rs`
- Create: `src/librarian/tools/audit_doc_refs/severity.rs`
- Modify: `src/librarian/tools/audit_doc_refs/mod.rs` (add `pub mod resolver; pub mod severity;` + types)

- [ ] **Step 1: Extend mod.rs with `Resolution`, `Finding`, `Severity`, `Verdict`**

```rust
// add to src/librarian/tools/audit_doc_refs/mod.rs
pub mod resolver;
pub mod severity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Resolved,
    Missing,
    FileMissing,
    SymbolMissing,
    LineOob,
    AnchorMissing,
    Unknown,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    High,
    Med,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub verdict: Verdict,
    pub severity: Severity,
    pub severity_reason: &'static str,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Finding {
    pub candidate: RefCandidate,
    pub resolution: Resolution,
}
```

- [ ] **Step 2: Stub severity.rs with the default map**

```rust
// src/librarian/tools/audit_doc_refs/severity.rs
use super::{Severity, Verdict};
use std::path::Path;

pub fn default_severity(verdict: Verdict) -> Severity {
    use Severity::*;
    use Verdict::*;
    match verdict {
        Missing | FileMissing | SymbolMissing => High,
        AnchorMissing | LineOob => Med,
        Unknown => Low,
        Resolved | External => Low,
    }
}

/// Apply path-based drop rules. Returns `(severity, reason)`.
pub fn apply_drops(
    md_file: &Path,
    verdict: Verdict,
    base: Severity,
    memory_globs: &[globset::Glob],
) -> (Severity, &'static str) {
    if matches_archive(md_file) {
        return (drop_one(base), "archive_drop");
    }
    if matches_memory(md_file, memory_globs) {
        return (drop_two(base), "memory_drop");
    }
    if matches_issues(md_file) {
        return (drop_one(base), "issues_drop");
    }
    (base, "policy_default")
}

fn drop_one(s: Severity) -> Severity {
    match s { Severity::High => Severity::Med, Severity::Med | Severity::Low => Severity::Low }
}
fn drop_two(s: Severity) -> Severity { drop_one(drop_one(s)) }

fn matches_archive(p: &Path) -> bool {
    let s = p.to_string_lossy();
    s.contains("docs/archive/") || s.ends_with(".archive.md")
}
fn matches_issues(p: &Path) -> bool { p.to_string_lossy().contains("docs/issues/") }
fn matches_memory(p: &Path, globs: &[globset::Glob]) -> bool {
    let mut builder = globset::GlobSetBuilder::new();
    for g in globs { builder.add(g.clone()); }
    builder.build().map(|set| set.is_match(p)).unwrap_or(false)
}

pub const DEFAULT_MEMORY_GLOBS: &[&str] = &[
    ".buddy/memory/**",
    "**/.buddy/memory/**",
    "**/buddy/memory/**",
    "**/projects/**/memory/**",
];
```

- [ ] **Step 3: Stub resolver.rs with `ResolveCtx` and `resolve_ref`**

```rust
// src/librarian/tools/audit_doc_refs/resolver.rs
use super::{Resolution, Severity, Verdict};
use super::{RefCandidate, RefKind};
use super::severity;
use std::path::{Path, PathBuf};

pub struct ResolveCtx<'a> {
    pub repo_root: &'a Path,
    pub memory_globs: &'a [globset::Glob],
    // LSP wired in Task 8
}

pub fn resolve_ref(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    match c.ref_kind {
        RefKind::FilePath => resolve_file_path(c, ctx),
        RefKind::FileLine => resolve_file_line(c, ctx),
        RefKind::FileSymbol => stub_unknown(c, ctx),
        RefKind::ModulePath => stub_unknown(c, ctx),
        RefKind::Link => resolve_link(c, ctx),
    }
}

fn resolve_file_path(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    let path = ctx.repo_root.join(&c.raw_ref);
    if path.exists() {
        let base = severity::default_severity(Verdict::Resolved);
        Resolution { verdict: Verdict::Resolved, severity: base, severity_reason: "policy_default", notes: None }
    } else {
        verdict_with_drops(Verdict::Missing, Path::new(&c.md_file), ctx.memory_globs)
    }
}

fn resolve_file_line(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    let (path_str, line_str) = c.raw_ref.rsplit_once(':').expect("file_line invariant");
    let path = ctx.repo_root.join(path_str);
    if !path.exists() {
        return verdict_with_drops(Verdict::Missing, Path::new(&c.md_file), ctx.memory_globs);
    }
    let line: u32 = line_str.parse().unwrap_or(0);
    let total = std::fs::read_to_string(&path).map(|s| s.lines().count() as u32).unwrap_or(0);
    if line == 0 || line > total {
        verdict_with_drops(Verdict::LineOob, Path::new(&c.md_file), ctx.memory_globs)
    } else {
        Resolution { verdict: Verdict::Resolved, severity: Severity::Low, severity_reason: "policy_default", notes: None }
    }
}

fn resolve_link(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    if c.raw_ref.starts_with("http://") || c.raw_ref.starts_with("https://") {
        return Resolution { verdict: Verdict::External, severity: Severity::Low, severity_reason: "policy_default", notes: None };
    }
    if c.raw_ref.starts_with('#') {
        // anchor — wired in a later task; stub as resolved for now
        return Resolution { verdict: Verdict::Resolved, severity: Severity::Low, severity_reason: "policy_default", notes: None };
    }
    // fs-scheme link → same as file_path
    let path = ctx.repo_root.join(&c.raw_ref);
    if path.exists() {
        Resolution { verdict: Verdict::Resolved, severity: Severity::Low, severity_reason: "policy_default", notes: None }
    } else {
        verdict_with_drops(Verdict::Missing, Path::new(&c.md_file), ctx.memory_globs)
    }
}

fn stub_unknown(_c: &RefCandidate, _ctx: &ResolveCtx<'_>) -> Resolution {
    Resolution { verdict: Verdict::Unknown, severity: Severity::Low, severity_reason: "policy_default", notes: None }
}

fn verdict_with_drops(verdict: Verdict, md_file: &Path, memory_globs: &[globset::Glob]) -> Resolution {
    let base = severity::default_severity(verdict);
    let (sev, reason) = severity::apply_drops(md_file, verdict, base, memory_globs);
    Resolution { verdict, severity: sev, severity_reason: reason, notes: None }
}
```

- [ ] **Step 4: Write failing tests**

```rust
// at the bottom of resolver.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::audit_doc_refs::{RefCandidate, RefKind, RefPosition};
    use tempfile::TempDir;

    fn cand(raw: &str, md: &str, kind: RefKind) -> RefCandidate {
        RefCandidate {
            md_file: md.to_string(), md_line: 1, raw_ref: raw.to_string(),
            ref_kind: kind, position: RefPosition::InlineSpan,
        }
    }
    fn ctx<'a>(root: &'a Path, globs: &'a [globset::Glob]) -> ResolveCtx<'a> {
        ResolveCtx { repo_root: root, memory_globs: globs }
    }

    #[test]
    fn resolver_resolved_for_existing_path() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.py"), "x = 1\n").unwrap();
        let r = resolve_ref(&cand("foo.py", "docs/spec.md", RefKind::FilePath), &ctx(tmp.path(), &[]));
        assert_eq!(r.verdict, Verdict::Resolved);
    }

    #[test]
    fn resolver_missing_for_absent_path() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_ref(&cand("gone.py", "docs/spec.md", RefKind::FilePath), &ctx(tmp.path(), &[]));
        assert_eq!(r.verdict, Verdict::Missing);
        assert_eq!(r.severity, Severity::High);
        assert_eq!(r.severity_reason, "policy_default");
    }

    #[test]
    fn resolver_line_oob_for_short_file() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.py"), "a\nb\nc\n").unwrap();
        let r = resolve_ref(&cand("foo.py:99", "docs/spec.md", RefKind::FileLine), &ctx(tmp.path(), &[]));
        assert_eq!(r.verdict, Verdict::LineOob);
        assert_eq!(r.severity, Severity::Med);
    }

    #[test]
    fn resolver_external_for_https_link() {
        let tmp = TempDir::new().unwrap();
        let r = resolve_ref(&cand("https://example.com", "docs/spec.md", RefKind::Link), &ctx(tmp.path(), &[]));
        assert_eq!(r.verdict, Verdict::External);
    }
}
```

- [ ] **Step 5: Run + verify pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::resolver::tests`
Expected: all 4 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/
git commit -m "feat(audit_doc_refs): resolver for file_path, file_line, link"
```

### Task 7: severity drop rules — archive, memory, issues

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/severity.rs` (already drafted in Task 6)
- Modify: `src/librarian/tools/audit_doc_refs/resolver.rs` (tests only)

- [ ] **Step 1: Add failing tests**

```rust
// extend mod tests in resolver.rs
#[test]
fn severity_drops_one_level_in_archive() {
    let tmp = TempDir::new().unwrap();
    let r = resolve_ref(
        &cand("gone.py", "docs/archive/old.md", RefKind::FilePath),
        &ctx(tmp.path(), &[]),
    );
    assert_eq!(r.verdict, Verdict::Missing);
    assert_eq!(r.severity, Severity::Med);
    assert_eq!(r.severity_reason, "archive_drop");
}

#[test]
fn severity_drops_two_levels_in_memory() {
    let tmp = TempDir::new().unwrap();
    let globs: Vec<_> = crate::tools::audit_doc_refs::severity::DEFAULT_MEMORY_GLOBS
        .iter().map(|g| globset::Glob::new(g).unwrap()).collect();
    let r = resolve_ref(
        &cand("gone.py", ".buddy/memory/foo.md", RefKind::FilePath),
        &ctx(tmp.path(), &globs),
    );
    assert_eq!(r.severity, Severity::Low);
    assert_eq!(r.severity_reason, "memory_drop");
}

#[test]
fn severity_reason_populated_for_every_finding() {
    let tmp = TempDir::new().unwrap();
    for kind in [RefKind::FilePath, RefKind::FileLine, RefKind::Link] {
        let r = resolve_ref(&cand("gone.py", "docs/spec.md", kind), &ctx(tmp.path(), &[]));
        assert!(!r.severity_reason.is_empty());
    }
}
```

- [ ] **Step 2: Run + verify pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::resolver::tests`
Expected: all severity tests pass (drop logic was implemented in Task 6).

- [ ] **Step 3: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/
git commit -m "test(audit_doc_refs): severity drop rules in archive and memory"
```

### Task 8: resolver — LSP-backed file_symbol and module_path, degraded flag

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/resolver.rs`
- Modify: `src/librarian/tools/audit_doc_refs/mod.rs` (add `ScanMeta` type)

- [ ] **Step 1: Extend types**

```rust
// src/librarian/tools/audit_doc_refs/mod.rs
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanMeta {
    pub last_scan_at: Option<String>,
    pub last_scan_commit: Option<String>,
    pub n_files_scanned: u32,
    pub n_refs_found: u32,
    pub degraded: bool,
    pub lsp_languages_offline: Vec<String>,
}
```

- [ ] **Step 2: Extend `ResolveCtx` with LSP trait**

```rust
// src/librarian/tools/audit_doc_refs/resolver.rs
use crate::lsp::ops::LspProvider; // assumes post-dissolve path

pub struct ResolveCtx<'a> {
    pub repo_root: &'a Path,
    pub memory_globs: &'a [globset::Glob],
    pub lsp: Option<&'a dyn LspProvider>,
    pub degraded_languages: std::cell::RefCell<Vec<String>>,
}
```

(If `LspProvider` does not exist or differs, look at `src/lsp/symbols.rs` and use whatever trait/struct accepts `(path, name) -> Option<SymbolHit>`. Adapt the call below accordingly.)

- [ ] **Step 3: Write failing test using MockLspClient**

```rust
#[test]
fn resolver_symbol_missing_for_renamed_symbol() {
    use crate::lsp::mock::MockLspClient;
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("foo.rs"), "pub fn bar() {}\n").unwrap();
    let lsp = MockLspClient::new(); // returns None for any (path, name)
    let c = cand("foo.rs:renamed_baz", "docs/spec.md", RefKind::FileSymbol);
    let r = resolve_ref(&c, &ResolveCtx {
        repo_root: tmp.path(), memory_globs: &[], lsp: Some(&lsp),
        degraded_languages: Default::default(),
    });
    assert_eq!(r.verdict, Verdict::SymbolMissing);
}

#[test]
fn resolver_unknown_when_lsp_offline() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("foo.rs"), "pub fn bar() {}\n").unwrap();
    let c = cand("foo.rs:bar", "docs/spec.md", RefKind::FileSymbol);
    let ctx = ResolveCtx { repo_root: tmp.path(), memory_globs: &[], lsp: None, degraded_languages: Default::default() };
    let r = resolve_ref(&c, &ctx);
    assert_eq!(r.verdict, Verdict::Unknown);
    assert!(ctx.degraded_languages.borrow().iter().any(|l| l == "rust"));
}

#[test]
fn resolver_prefers_disk_truth_on_lsp_lag() {
    // file exists but LSP says no → symbol_missing, NOT unknown
    use crate::lsp::mock::MockLspClient;
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("foo.rs"), "pub fn bar() {}\n").unwrap();
    let lsp = MockLspClient::new(); // returns None
    let c = cand("foo.rs:bar", "docs/spec.md", RefKind::FileSymbol);
    let r = resolve_ref(&c, &ResolveCtx {
        repo_root: tmp.path(), memory_globs: &[], lsp: Some(&lsp),
        degraded_languages: Default::default(),
    });
    assert_eq!(r.verdict, Verdict::SymbolMissing); // NOT Unknown
}
```

- [ ] **Step 4: Implement `resolve_file_symbol` and `resolve_module_path`**

Replace `RefKind::FileSymbol` arm in `resolve_ref` and add real impls:

```rust
fn resolve_file_symbol(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    let (path_str, name) = c.raw_ref.rsplit_once(':').expect("file_symbol invariant");
    let path = ctx.repo_root.join(path_str);
    if !path.exists() {
        return verdict_with_drops(Verdict::FileMissing, Path::new(&c.md_file), ctx.memory_globs);
    }
    let lang = detect_language(path_str);
    let Some(lsp) = ctx.lsp else {
        ctx.degraded_languages.borrow_mut().push(lang.to_string());
        return Resolution { verdict: Verdict::Unknown, severity: Severity::Low, severity_reason: "policy_default", notes: None };
    };
    match lsp.lookup_symbol(&path, name) {
        Ok(Some(_)) => Resolution { verdict: Verdict::Resolved, severity: Severity::Low, severity_reason: "policy_default", notes: None },
        Ok(None) => verdict_with_drops(Verdict::SymbolMissing, Path::new(&c.md_file), ctx.memory_globs),
        Err(_) => {
            ctx.degraded_languages.borrow_mut().push(lang.to_string());
            Resolution { verdict: Verdict::Unknown, severity: Severity::Low, severity_reason: "policy_default", notes: None }
        }
    }
}

fn detect_language(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, ext)| ext) {
        Some("rs") => "rust", Some("py") => "python", Some("ts") => "typescript",
        Some("kt") => "kotlin", Some("java") => "java", Some("go") => "go",
        _ => "unknown",
    }
}
```

(`lookup_symbol(&Path, &str) -> Result<Option<SymbolHit>>` is the assumed `LspProvider` shape. Adapt to the real trait.)

- [ ] **Step 5: Run + verify pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::resolver::tests`
Expected: all resolver tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/
git commit -m "feat(audit_doc_refs): resolver LSP integration + degraded flag"
```

---

## Phase 3 — Merger + dispatch + tracker write

### Task 8b: Path-outside-project + anchor link resolution

Closes spec failure mode 4e (path outside active project → `unknown` with explanatory note) and the anchor-link resolution stubbed in Task 6.

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/resolver.rs`

- [ ] **Step 1: Add failing tests**

```rust
#[test]
fn resolver_unknown_for_path_outside_project() {
    let tmp = TempDir::new().unwrap();
    let r = resolve_ref(
        &cand("../other-repo/src/foo.py", "docs/spec.md", RefKind::FilePath),
        &ctx(tmp.path(), &[]),
    );
    assert_eq!(r.verdict, Verdict::Unknown);
    assert!(r.notes.as_deref().unwrap_or("").contains("outside active project"));
}

#[test]
fn resolver_anchor_resolved_when_heading_present() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("spec.md"), "# Top\n\n## Auth\n\nbody\n").unwrap();
    let r = resolve_ref(
        &cand("#auth", "docs/spec.md", RefKind::Link),
        &ctx(tmp.path(), &[]),
    );
    assert_eq!(r.verdict, Verdict::Resolved);
}

#[test]
fn resolver_anchor_missing_when_heading_absent() {
    let tmp = TempDir::new().unwrap();
    let docs = tmp.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("spec.md"), "# Top\n\n## Auth\n\nbody\n").unwrap();
    let r = resolve_ref(
        &cand("#missing-section", "docs/spec.md", RefKind::Link),
        &ctx(tmp.path(), &[]),
    );
    assert_eq!(r.verdict, Verdict::AnchorMissing);
    assert_eq!(r.severity, Severity::Med);
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p librarian-mcp audit_doc_refs::resolver::tests`
Expected: all three new tests fail.

- [ ] **Step 3: Patch `resolve_file_path` to detect path-outside-project**

In `resolver.rs`, before the `path.exists()` check in `resolve_file_path`, add:

```rust
fn resolve_file_path(c: &RefCandidate, ctx: &ResolveCtx<'_>) -> Resolution {
    if c.raw_ref.starts_with("../") || c.raw_ref.starts_with('/') {
        return Resolution {
            verdict: Verdict::Unknown,
            severity: Severity::Low,
            severity_reason: "policy_default",
            notes: Some("path outside active project; scope=umbrella required".to_string()),
        };
    }
    let path = ctx.repo_root.join(&c.raw_ref);
    // … existing logic
}
```

- [ ] **Step 4: Implement anchor resolution**

Replace the `c.raw_ref.starts_with('#')` arm in `resolve_link`:

```rust
if let Some(anchor) = c.raw_ref.strip_prefix('#') {
    let target_md = ctx.repo_root.join(&c.md_file);
    if let Ok(text) = std::fs::read_to_string(&target_md) {
        let slugs: std::collections::HashSet<String> = text
            .lines()
            .filter_map(|l| l.strip_prefix('#'))
            .map(|l| slugify(l.trim_start_matches('#').trim()))
            .collect();
        if slugs.contains(&slugify(anchor)) {
            return Resolution { verdict: Verdict::Resolved, severity: Severity::Low, severity_reason: "policy_default", notes: None };
        }
    }
    return verdict_with_drops(Verdict::AnchorMissing, Path::new(&c.md_file), ctx.memory_globs);
}

fn slugify(s: &str) -> String {
    s.to_lowercase().chars().filter_map(|c| match c {
        'a'..='z' | '0'..='9' => Some(c),
        ' ' | '-' | '_' => Some('-'),
        _ => None,
    }).collect()
}
```

- [ ] **Step 5: Run + verify pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::resolver::tests`
Expected: all resolver tests (Tasks 6, 7, 8, 8b) pass.

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/resolver.rs
git commit -m "feat(audit_doc_refs): path-outside-project + anchor link resolution"
```

### Task 9: `merge_into_tracker` with primary key + immutable `n`

**Files:**
- Create: `src/librarian/tools/audit_doc_refs/merger.rs`
- Modify: `src/librarian/tools/audit_doc_refs/mod.rs` (add `pub mod merger;` + `TrackerParams`, `Issue` types)

- [ ] **Step 1: Add types to mod.rs**

```rust
pub mod merger;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub n: u32,
    pub title: String,
    pub severity: Severity,
    pub severity_reason: String,
    pub status: String, // "open" | "in-progress" | "fixed" | "wontfix"
    pub owner: String,
    pub ref_kind: RefKind,
    pub md_file: String,
    pub md_line: u32,
    pub raw_ref: String,
    pub first_seen_commit: String,
    pub first_seen_at: String,
    pub last_verified_at: String,
    pub notes: String,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackerParams {
    pub issues: Vec<Issue>,
    pub scan_meta: ScanMeta,
    pub parse_warnings: Vec<ParseWarning>,
}
```

- [ ] **Step 2: Write merger.rs with key logic**

```rust
// src/librarian/tools/audit_doc_refs/merger.rs
use super::{Finding, Issue, TrackerParams, Verdict};
use chrono::{DateTime, Utc};

pub fn merge_into_tracker(
    findings: Vec<Finding>,
    prior: &TrackerParams,
    now: DateTime<Utc>,
    commit: &str,
) -> TrackerParams {
    let now_str = now.to_rfc3339();
    let mut out = prior.clone();
    let mut seen_keys = std::collections::HashSet::new();

    for f in &findings {
        let key = (f.candidate.md_file.clone(), f.candidate.raw_ref.clone());
        seen_keys.insert(key.clone());

        if let Some(existing) = out.issues.iter_mut().find(|i| i.md_file == key.0 && i.raw_ref == key.1) {
            // Update existing
            existing.last_verified_at = now_str.clone();
            // verdict change → status transition (Task 10)
            if f.resolution.verdict == Verdict::Resolved && existing.status == "open" {
                existing.status = "fixed".to_string();
                existing.notes = format!("auto-resolved at {commit}");
            } else if f.resolution.verdict != Verdict::Resolved
                && f.resolution.verdict != Verdict::External
                && existing.status == "fixed"
            {
                existing.status = "open".to_string();
                existing.notes = format!("regression at {commit}; prior: {}", existing.notes);
            }
            // severity escalates only
            if severity_rank(f.resolution.severity) > severity_rank(existing.severity) {
                existing.severity = f.resolution.severity;
                existing.severity_reason = f.resolution.severity_reason.to_string();
            }
        } else if !matches!(f.resolution.verdict, Verdict::Resolved | Verdict::External) {
            // New finding — append with next n
            let next_n = out.issues.iter().map(|i| i.n).max().unwrap_or(0) + 1;
            out.issues.push(Issue {
                n: next_n,
                title: format!("{} — {:?}", f.candidate.raw_ref, f.resolution.verdict).to_lowercase(),
                severity: f.resolution.severity,
                severity_reason: f.resolution.severity_reason.to_string(),
                status: "open".to_string(),
                owner: String::new(),
                ref_kind: f.candidate.ref_kind,
                md_file: f.candidate.md_file.clone(),
                md_line: f.candidate.md_line,
                raw_ref: f.candidate.raw_ref.clone(),
                first_seen_commit: commit.to_string(),
                first_seen_at: now_str.clone(),
                last_verified_at: now_str.clone(),
                notes: String::new(),
                extra: serde_json::Map::new(),
            });
        }
    }
    out
}

fn severity_rank(s: super::Severity) -> u8 {
    match s {
        super::Severity::High => 3,
        super::Severity::Med => 2,
        super::Severity::Low => 1,
    }
}
```

- [ ] **Step 3: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::audit_doc_refs::*;
    use chrono::TimeZone;

    fn finding(md: &str, raw: &str, verdict: Verdict) -> Finding {
        Finding {
            candidate: RefCandidate {
                md_file: md.to_string(), md_line: 1, raw_ref: raw.to_string(),
                ref_kind: RefKind::FilePath, position: RefPosition::InlineSpan,
            },
            resolution: Resolution {
                verdict, severity: Severity::High, severity_reason: "policy_default", notes: None,
            },
        }
    }

    fn now() -> chrono::DateTime<chrono::Utc> { chrono::Utc.timestamp_opt(0, 0).unwrap() }

    #[test]
    fn merger_assigns_n_at_first_seen_and_preserves_it() {
        let a = finding("a.md", "x.py", Verdict::Missing);
        let r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
        assert_eq!(r1.issues.len(), 1);
        assert_eq!(r1.issues[0].n, 1);

        let b = finding("b.md", "y.py", Verdict::Missing);
        let r2 = merge_into_tracker(vec![a, b], &r1, now(), "c2");
        assert_eq!(r2.issues.len(), 2);
        // a still n=1, b is n=2
        assert!(r2.issues.iter().find(|i| i.raw_ref == "x.py").unwrap().n == 1);
        assert!(r2.issues.iter().find(|i| i.raw_ref == "y.py").unwrap().n == 2);
    }

    #[test]
    fn merger_first_seen_commit_immutable() {
        let a = finding("a.md", "x.py", Verdict::Missing);
        let r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
        let r2 = merge_into_tracker(vec![a], &r1, now(), "c2");
        assert_eq!(r2.issues[0].first_seen_commit, "c1");
    }
}
```

- [ ] **Step 4: Run + verify pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::merger::tests`
Expected: both tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/
git commit -m "feat(audit_doc_refs): merger with stable primary key and immutable n"
```

### Task 10: merger — open↔fixed lifecycle + wontfix preservation + severity escalation

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/merger.rs` (tests only — impl already done in Task 9)

- [ ] **Step 1: Add failing tests**

```rust
#[test]
fn lifecycle_open_to_fixed() {
    let a = finding("a.md", "x.py", Verdict::Missing);
    let r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
    assert_eq!(r1.issues[0].status, "open");

    let a_resolved = finding("a.md", "x.py", Verdict::Resolved);
    let r2 = merge_into_tracker(vec![a_resolved], &r1, now(), "c2");
    assert_eq!(r2.issues[0].status, "fixed");
    assert!(r2.issues[0].notes.contains("auto-resolved at c2"));
}

#[test]
fn lifecycle_fixed_to_open_regression() {
    let a = finding("a.md", "x.py", Verdict::Missing);
    let r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
    let a_ok = finding("a.md", "x.py", Verdict::Resolved);
    let r2 = merge_into_tracker(vec![a_ok], &r1, now(), "c2");
    assert_eq!(r2.issues[0].status, "fixed");

    let a_broken = finding("a.md", "x.py", Verdict::Missing);
    let r3 = merge_into_tracker(vec![a_broken], &r2, now(), "c3");
    assert_eq!(r3.issues[0].status, "open");
    assert!(r3.issues[0].notes.contains("regression at c3"));
}

#[test]
fn wontfix_never_auto_flipped() {
    let a = finding("a.md", "x.py", Verdict::Missing);
    let mut r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
    r1.issues[0].status = "wontfix".to_string();

    let a_ok = finding("a.md", "x.py", Verdict::Resolved);
    let r2 = merge_into_tracker(vec![a_ok], &r1, now(), "c2");
    assert_eq!(r2.issues[0].status, "wontfix");
}

#[test]
fn severity_escalates_only() {
    let mut low = finding("a.md", "x.py", Verdict::Missing);
    low.resolution.severity = Severity::Low;
    let r1 = merge_into_tracker(vec![low], &TrackerParams::default(), now(), "c1");
    assert_eq!(r1.issues[0].severity, Severity::Low);

    let mut high = finding("a.md", "x.py", Verdict::Missing);
    high.resolution.severity = Severity::High;
    let r2 = merge_into_tracker(vec![high], &r1, now(), "c2");
    assert_eq!(r2.issues[0].severity, Severity::High);

    // downgrade attempt — severity should NOT drop
    let mut med = finding("a.md", "x.py", Verdict::Missing);
    med.resolution.severity = Severity::Med;
    let r3 = merge_into_tracker(vec![med], &r2, now(), "c3");
    assert_eq!(r3.issues[0].severity, Severity::High);
}
```

- [ ] **Step 2: Run tests; some will fail**

Run: `cargo test -p librarian-mcp audit_doc_refs::merger::tests`
Expected: `wontfix_never_auto_flipped` fails — Task 9's impl flips any `fixed` ↔ `open`, including from `wontfix`.

- [ ] **Step 3: Patch merger to guard wontfix**

In merger.rs, the lifecycle blocks add an early guard:

```rust
if existing.status != "wontfix" {
    if f.resolution.verdict == Verdict::Resolved && existing.status == "open" {
        existing.status = "fixed".to_string();
        // ...
    } else if /* regression case */ existing.status == "fixed" {
        // ...
    }
}
```

- [ ] **Step 4: Re-run + verify all pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::merger::tests`
Expected: all 6 merger tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/merger.rs
git commit -m "feat(audit_doc_refs): lifecycle transitions + wontfix preservation + severity escalates only"
```

### Task 11: merger — idempotency + schema-lock additive merge

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/merger.rs` (tests only — both behaviors already correct from Task 9)

- [ ] **Step 1: Add failing tests**

```rust
#[test]
fn idempotent_merge() {
    let a = finding("a.md", "x.py", Verdict::Missing);
    let b = finding("b.md", "y.py", Verdict::Missing);
    let r1 = merge_into_tracker(vec![a.clone(), b.clone()], &TrackerParams::default(), now(), "c1");
    let r2 = merge_into_tracker(vec![a, b], &r1, now(), "c1");
    let s1 = serde_json::to_string(&r1).unwrap();
    let s2 = serde_json::to_string(&r2).unwrap();
    assert_eq!(s1, s2, "two back-to-back merges on unchanged input must be byte-identical");
}

#[test]
fn unknown_field_preserved_across_merge() {
    let a = finding("a.md", "x.py", Verdict::Missing);
    let mut r1 = merge_into_tracker(vec![a.clone()], &TrackerParams::default(), now(), "c1");
    r1.issues[0].extra.insert("custom".to_string(), serde_json::json!("user-edit"));
    let r2 = merge_into_tracker(vec![a], &r1, now(), "c2");
    assert_eq!(r2.issues[0].extra.get("custom"), Some(&serde_json::json!("user-edit")));
}
```

- [ ] **Step 2: Run + verify pass**

Run: `cargo test -p librarian-mcp audit_doc_refs::merger::tests`
Expected: both pass — Task 9's `out = prior.clone()` preserves `extra`; idempotency holds because every field updated by the merger is a deterministic function of inputs.

- [ ] **Step 3: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/merger.rs
git commit -m "test(audit_doc_refs): idempotency + schema-lock additive merge"
```

### Task 12: Action enum dispatch + Tool input schema

**Files:**
- Modify: `src/librarian/tools/librarian.rs`
- Modify: `src/librarian/tools/audit_doc_refs/mod.rs` (add `call` entry point)

- [ ] **Step 1: Sketch `call` entry in mod.rs**

```rust
// src/librarian/tools/audit_doc_refs/mod.rs
use crate::tools::ToolContext;
use anyhow::Result;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct AuditArgs {
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub paths: Option<Vec<String>>,
    #[serde(default = "default_true")]
    pub emit_tracker: bool,
    #[serde(default)]
    pub tracker_id: Option<String>,
    #[serde(default)]
    pub severity_overrides: Option<Value>,
    #[serde(default = "default_fail_on")]
    pub fail_on: String,
}

fn default_true() -> bool { true }
fn default_fail_on() -> String { "never".to_string() }

pub const DEFAULT_AUDIT_GLOBS: &[&str] = &[
    "docs/**/*.md",
    "CLAUDE.md",
    "**/CLAUDE.md",
    "**/README.md",
];

pub const MAX_FILES_DEFAULT: usize = 10_000;

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let _args: AuditArgs = serde_json::from_value(args).map_err(|e|
        crate::tools::RecoverableError::with_hint(
            format!("audit_doc_refs: bad args: {e}"),
            "see librarian(action=\"audit_doc_refs\") input schema",
        )
    )?;
    // Phase 3 Task 13 wires the actual scan; for now return a stub so the
    // dispatch arm compiles.
    Ok(json!({
        "n_files_scanned": 0,
        "n_refs_found": 0,
        "findings": [],
        "exit_code": 0,
    }))
}
```

- [ ] **Step 2: Extend `Librarian::call` dispatch**

Open `src/librarian/tools/librarian.rs`. Find the `match action` block in `call` (or whatever the dispatch shape is — check the existing code). Add:

```rust
"audit_doc_refs" => crate::tools::audit_doc_refs::call(ctx, args).await,
```

Add the new action to `input_schema` enum if the schema uses one. Update `description` text:

```rust
fn description(&self) -> &'static str {
    "Workspace artifact registry tools. Actions:\n\
     - find/get/create/update/move/link/graph/state_at: artifact CRUD\n\
     - context: pack topic/anchor neighbourhood into markdown bundle\n\
     - reindex: re-scan and classify markdown artifacts\n\
     - tracker_design: archetype library for tracker artifacts\n\
     - workspace_state_at: time-travel snapshot of all artifacts at a commit\n\
     - audit_doc_refs: scan markdown for stale code refs (file paths, symbols,\n\
       line refs, link targets, module paths). Surfaces broken references\n\
       against current filesystem + LSP symbol index. Manual cadence — run\n\
       when a doc-heavy PR is about to merge or when drift is suspected.\n\
       Output is an `audit_issues` tracker."
}
```

- [ ] **Step 3: Write smoke test**

```rust
// extend src/librarian/tools/librarian.rs::tests
#[tokio::test]
async fn audit_doc_refs_action_routes() {
    let ctx = mk_ctx();
    let result = crate::tools::audit_doc_refs::call(&ctx, json!({})).await.unwrap();
    assert_eq!(result["exit_code"], 0);
    assert!(result["findings"].is_array());
}
```

- [ ] **Step 4: Run + verify**

Run: `cargo test -p librarian-mcp librarian::tests::audit_doc_refs_action_routes`
Expected: PASS.

Also run: `cargo test -p librarian-mcp librarian::tests::prompt_surfaces_reference_only_real_tools` (if present at root). Verify no stale tool references.

- [ ] **Step 5: Commit**

```bash
git add src/librarian/tools/
git commit -m "feat(audit_doc_refs): wire Action enum dispatch + description"
```

### Task 13: End-to-end scan pipeline

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/mod.rs` (replace stubbed `call` with real scan)

- [ ] **Step 1: Implement the scan pipeline**

```rust
// src/librarian/tools/audit_doc_refs/mod.rs — replace stubbed call
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let args: AuditArgs = serde_json::from_value(args).map_err(|e|
        crate::tools::RecoverableError::with_hint(
            format!("audit_doc_refs: bad args: {e}"),
            "see librarian(action=\"audit_doc_refs\") input schema",
        )
    )?;

    let repo_root = ctx
        .current_project
        .as_ref()
        .ok_or_else(|| crate::tools::RecoverableError::new(
            "audit_doc_refs: no active project; activate one first",
        ))?
        .root
        .clone();

    let globs = args.paths.unwrap_or_else(||
        DEFAULT_AUDIT_GLOBS.iter().map(|s| s.to_string()).collect()
    );

    let files = collect_markdown_files(&repo_root, &globs)?;
    let max_files = std::env::var("LIBRARIAN_AUDIT_MAX_FILES")
        .ok().and_then(|s| s.parse().ok()).unwrap_or(MAX_FILES_DEFAULT);
    if files.len() > max_files {
        return Err(crate::tools::RecoverableError::with_hint(
            format!("audit_doc_refs: glob matched {} files (cap {})", files.len(), max_files),
            "tighten `paths` glob or set LIBRARIAN_AUDIT_MAX_FILES",
        ));
    }

    let memory_globs: Vec<_> = severity::DEFAULT_MEMORY_GLOBS.iter()
        .map(|g| globset::Glob::new(g).unwrap()).collect();
    let resolve_ctx = resolver::ResolveCtx {
        repo_root: &repo_root, memory_globs: &memory_globs,
        lsp: ctx.lsp.as_deref(), // requires ToolContext to carry an LSP handle; see note below
        degraded_languages: Default::default(),
    };

    let mut all_findings = Vec::new();
    let mut all_warnings = Vec::new();
    for md in &files {
        let text = std::fs::read_to_string(md)?;
        let (cands, warns) = parser::parse_refs(&text, md);
        for c in cands {
            let r = resolver::resolve_ref(&c, &resolve_ctx);
            all_findings.push(Finding { candidate: c, resolution: r });
        }
        all_warnings.extend(warns);
    }

    let now = chrono::Utc::now();
    let commit = ctx.head_commit().unwrap_or_else(|_| "unknown".to_string());

    let (tracker_id, tracker_path, tracker_params) =
        upsert_tracker(ctx, &args, all_findings.clone(), all_warnings.clone(), now, &commit).await?;

    let response = build_response(
        &all_findings,
        &all_warnings,
        &resolve_ctx.degraded_languages.borrow(),
        files.len(),
        tracker_id.as_deref(),
        tracker_path.as_deref(),
        &args.fail_on,
    );
    Ok(response)
}

fn collect_markdown_files(root: &Path, globs: &[String]) -> Result<Vec<PathBuf>> {
    use ignore::WalkBuilder;
    let mut set_builder = globset::GlobSetBuilder::new();
    for g in globs {
        set_builder.add(globset::Glob::new(g).map_err(|e|
            crate::tools::RecoverableError::with_hint(
                format!("bad glob {g}: {e}"),
                "fix glob syntax",
            )
        )?);
    }
    let set = set_builder.build()?;
    let mut out = Vec::new();
    for entry in WalkBuilder::new(root).build() {
        let entry = entry?;
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) { continue; }
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        if set.is_match(rel) { out.push(entry.path().to_path_buf()); }
    }
    Ok(out)
}
```

(Note: `ToolContext` likely does not currently carry an LSP handle — librarian's context is catalog-focused. You may need to (a) extend `ToolContext` to optionally carry an LSP provider, OR (b) skip LSP entirely when running inside the librarian tool and use `Verdict::Unknown` for all `file_symbol`/`module_path` candidates with `degraded=true`. Pick (b) for v1 simplicity; the spec's manual-cadence allowance covers this: when run from inside the librarian tool path, symbol lookups go through whatever symbol provider is reachable, falling back to `degraded`. Reassess after Task 14 ships and the cost of (a) is clearer.)

- [ ] **Step 2: Implement `upsert_tracker` and `build_response` (helpers)**

```rust
async fn upsert_tracker(
    ctx: &ToolContext,
    args: &AuditArgs,
    findings: Vec<Finding>,
    warnings: Vec<ParseWarning>,
    now: chrono::DateTime<chrono::Utc>,
    commit: &str,
) -> Result<(Option<String>, Option<String>, TrackerParams)> {
    if !args.emit_tracker { return Ok((None, None, TrackerParams::default())); }

    let tracker_id = match &args.tracker_id {
        Some(id) => id.clone(),
        None => ensure_default_tracker(ctx).await?, // creates docs/trackers/doc-ref-audit.md if absent
    };

    let prior = load_tracker_params(ctx, &tracker_id).await.unwrap_or_default();
    let mut new_params = merger::merge_into_tracker(findings, &prior, now, commit);
    new_params.scan_meta.last_scan_at = Some(now.to_rfc3339());
    new_params.scan_meta.last_scan_commit = Some(commit.to_string());
    new_params.scan_meta.n_files_scanned = /* passed in */ 0;
    new_params.parse_warnings = warnings;

    write_tracker_params(ctx, &tracker_id, &new_params).await?;
    let path = tracker_path_for(ctx, &tracker_id).await?;
    Ok((Some(tracker_id), Some(path), new_params))
}

fn build_response(
    findings: &[Finding],
    warnings: &[ParseWarning],
    offline: &[String],
    n_files: usize,
    tracker_id: Option<&str>,
    tracker_path: Option<&str>,
    fail_on: &str,
) -> Value {
    use crate::tools::output::OutputGuard; // adapt to real path
    let cap = 50;
    let shown: Vec<_> = findings.iter().take(cap).map(finding_to_json).collect();
    let total = findings.len();
    let overflow = if total > cap {
        json!({
            "shown": cap,
            "total": total,
            "by_file": findings_by_file(findings),
            "hint": format!("narrow with paths=[...] or read full tracker at {}",
                            tracker_path.unwrap_or("<no tracker>")),
        })
    } else { Value::Null };

    let n_broken = findings.iter()
        .filter(|f| matches!(f.resolution.verdict,
            Verdict::Missing | Verdict::FileMissing | Verdict::SymbolMissing | Verdict::LineOob | Verdict::AnchorMissing))
        .count();
    let n_unknown = findings.iter().filter(|f| f.resolution.verdict == Verdict::Unknown).count();
    let n_resolved = findings.iter().filter(|f| f.resolution.verdict == Verdict::Resolved).count();

    let exit_code = match fail_on {
        "high" if findings.iter().any(|f| f.resolution.severity == Severity::High
            && !matches!(f.resolution.verdict, Verdict::Resolved | Verdict::External)) => 1,
        "any" if n_broken + n_unknown > 0 => 1,
        _ => 0,
    };

    json!({
        "n_files_scanned": n_files,
        "n_refs_found": findings.len(),
        "n_refs_resolved": n_resolved,
        "n_refs_broken": n_broken,
        "n_refs_unknown": n_unknown,
        "tracker_id": tracker_id,
        "tracker_path": tracker_path,
        "findings": shown,
        "overflow": overflow,
        "parse_warnings": warnings,
        "scan_meta": {
            "degraded": !offline.is_empty(),
            "lsp_languages_offline": offline,
        },
        "exit_code": exit_code,
    })
}
```

- [ ] **Step 3: Compile**

Run: `cargo check -p librarian-mcp`
Expected: clean. Fix any type mismatches as you go.

- [ ] **Step 4: Write a smoke integration test**

```rust
// inline tests in mod.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn smoke_scan_yields_zero_on_clean_repo() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("foo.py"), "x = 1\n").unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/spec.md"), "See `foo.py`.\n").unwrap();

        // Build a minimal ToolContext anchored at tmp — adapt to your mk_ctx helper
        let ctx = mk_smoke_ctx(tmp.path());
        let result = call(&ctx, serde_json::json!({
            "emit_tracker": false,
            "paths": ["docs/**/*.md"],
        })).await.unwrap();

        assert_eq!(result["n_refs_broken"], 0);
        assert_eq!(result["exit_code"], 0);
    }
}
```

- [ ] **Step 5: Run + verify**

Run: `cargo test -p librarian-mcp audit_doc_refs::tests::smoke_scan_yields_zero_on_clean_repo`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/
git commit -m "feat(audit_doc_refs): end-to-end scan pipeline + smoke test"
```

### Task 14: Tier-2 fixture-driven behavior tests

**Files:**
- Create: `tests/librarian/audit_doc_refs/corpus.rs`
- Create: `tests/librarian/audit_doc_refs/fixtures/clean_repo/`
- Create: `tests/librarian/audit_doc_refs/fixtures/drift_repo/`
- Create: `tests/librarian/audit_doc_refs/fixtures/regression_repo/`
- Create: `tests/librarian/audit_doc_refs/fixtures/wontfix_repo/`
- Create: `tests/librarian/audit_doc_refs/fixtures/archive_drop_repo/`
- Create: `tests/librarian/audit_doc_refs/fixtures/parse_recovery_repo/`

- [ ] **Step 1: Build the clean_repo fixture**

```
tests/librarian/audit_doc_refs/fixtures/clean_repo/
├── src/
│   ├── foo.py
│   └── bar.rs
└── docs/
    └── spec.md       # references `src/foo.py` and `src/bar.rs` (both resolve)
```

```python
# tests/librarian/audit_doc_refs/fixtures/clean_repo/src/foo.py
def hello(): return 1
```

```rust
// tests/librarian/audit_doc_refs/fixtures/clean_repo/src/bar.rs
pub fn world() -> i32 { 2 }
```

```markdown
<!-- tests/librarian/audit_doc_refs/fixtures/clean_repo/docs/spec.md -->
Entry points: `src/foo.py` and `src/bar.rs`.
```

- [ ] **Step 2: Build drift_repo fixture**

```
fixtures/drift_repo/
├── src/keeper.py
└── docs/spec.md
```

```markdown
# drift_repo/docs/spec.md
Three missing paths: `src/gone1.py`, `src/gone2.rs`, `src/gone3.kt`.
Two missing symbols: `src/keeper.py:vanished`, `src/keeper.py:also_gone`.
One line OOB: `src/keeper.py:999`.
Two unknown modules: `unknown.module.one`, `unknown.module.two`.
```

```python
# drift_repo/src/keeper.py
def real_function(): pass
```

- [ ] **Step 3: Write the corpus driver**

```rust
// tests/librarian/audit_doc_refs/corpus.rs
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/librarian/audit_doc_refs/fixtures")
        .join(name)
}

#[tokio::test]
async fn clean_repo_yields_zero_findings() {
    let root = fixture("clean_repo");
    let ctx = build_ctx(&root); // helper that points ToolContext at this root
    let result = librarian_mcp::tools::audit_doc_refs::call(&ctx, serde_json::json!({
        "emit_tracker": false,
        "paths": ["docs/**/*.md"],
    })).await.unwrap();
    assert_eq!(result["n_refs_broken"], 0, "clean_repo should not surface findings");
}

#[tokio::test]
async fn drift_repo_yields_expected_distribution() {
    let root = fixture("drift_repo");
    let ctx = build_ctx(&root);
    let result = librarian_mcp::tools::audit_doc_refs::call(&ctx, serde_json::json!({
        "emit_tracker": false,
        "paths": ["docs/**/*.md"],
    })).await.unwrap();
    let findings = result["findings"].as_array().unwrap();
    let count = |verdict: &str| findings.iter().filter(|f| f["verdict"] == verdict).count();
    assert_eq!(count("missing"), 3, "three file_path:missing");
    // symbol_missing depends on whether LSP is available in tests; if degraded, accept unknown
    let symbol_missing = count("symbol_missing");
    let unknown = count("unknown");
    assert!(symbol_missing + unknown >= 2, "two file_symbol candidates resolved or marked unknown");
    assert_eq!(count("line_oob"), 1);
}
```

(Build out the same shape for `regression_repo`, `wontfix_repo`, `archive_drop_repo`, `parse_recovery_repo`. Each fixture is a few markdown files plus the source files referenced. The corpus tests assert the expected verdict and severity distribution.)

- [ ] **Step 4: Run + iterate until all fixture tests pass**

Run: `cargo test -p librarian-mcp --test audit_doc_refs`
Expected: all corpus tests pass. If a test fails, fix either the impl or the fixture; record the cause in `docs/TODO-tool-misbehaviors.md` per project rule if anything in codescout itself misbehaves.

- [ ] **Step 5: Commit**

```bash
git add tests/librarian/audit_doc_refs/
git commit -m "test(audit_doc_refs): Tier-2 fixture corpus across 6 scenarios"
```

### Task 15: Tracker auto-create + render template registration

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/mod.rs` (`ensure_default_tracker` impl)

- [ ] **Step 1: Implement `ensure_default_tracker`**

```rust
async fn ensure_default_tracker(ctx: &ToolContext) -> Result<String> {
    use crate::tools::artifact; // or wherever artifact create lives
    let path = "docs/trackers/doc-ref-audit.md";

    // Check if it exists in catalog
    let find_args = serde_json::json!({"action":"find","filter":{"rel_path":{"eq":path}}});
    if let Ok(v) = artifact::call(ctx, find_args).await {
        if let Some(arr) = v.as_array() {
            if let Some(first) = arr.first() {
                if let Some(id) = first["id"].as_str() {
                    return Ok(id.to_string());
                }
            }
        }
    }

    // Create the file + the augmented artifact
    let trackers_dir = ctx.current_project.as_ref().unwrap().root.join("docs/trackers");
    std::fs::create_dir_all(&trackers_dir)?;
    let file_path = trackers_dir.join("doc-ref-audit.md");
    if !file_path.exists() {
        std::fs::write(&file_path, "# Doc Ref Audit Tracker\n\nAuto-managed by `librarian(audit_doc_refs)`.\n")?;
    }

    let create_args = serde_json::json!({
        "action": "create",
        "kind": "tracker",
        "title": "Doc Ref Audit",
        "rel_path": path,
        "tags": ["doc-ref-audit"],
        "augment": {
            "prompt": include_str!("./render_prompt.md"),
            "params": { "issues": [], "scan_meta": {}, "parse_warnings": [] },
            "render_template": include_str!("./render_template.j2"),
        }
    });
    let created = artifact::call(ctx, create_args).await?;
    Ok(created["id"].as_str().unwrap().to_string())
}
```

- [ ] **Step 2: Add `render_prompt.md` next to the module**

```
src/librarian/tools/audit_doc_refs/render_prompt.md
```

```markdown
This tracker is auto-managed by `librarian(audit_doc_refs)`. Do not edit `issues[]` by hand —
the action overwrites `issues[]`, `scan_meta`, and `parse_warnings` on every run, preserving
your `status` overrides (notably `wontfix`) and any extra fields you add to an issue.

If you want to suppress a finding permanently, set its `status` to `wontfix`. The merger will
never auto-flip wontfix back to open.
```

- [ ] **Step 3: Add render template**

```
src/librarian/tools/audit_doc_refs/render_template.j2
```

```jinja
**Last scan:** {{ scan_meta.last_scan_at }} ({{ scan_meta.last_scan_commit }}) —
{{ issues|selectattr("status","equalto","open")|list|length }} open /
{{ issues|length }} total
{% if scan_meta.degraded %} — ⚠ degraded ({{ scan_meta.lsp_languages_offline|join(", ") }} offline){% endif %}

| # | severity | reason | status | ref | found in |
|---|---|---|---|---|---|
{% for i in issues %}| {{ i.n }} | {{ i.severity }} | {{ i.severity_reason }} | {{ i.status }} | `{{ i.raw_ref }}` | {{ i.md_file }}:{{ i.md_line }} |
{% endfor %}

{% if parse_warnings %}
### Parse warnings ({{ parse_warnings|length }})

| file | line | reason |
|---|---|---|
{% for w in parse_warnings %}| {{ w.md_file }} | {{ w.line }} | {{ w.reason }} |
{% endfor %}
{% endif %}
```

- [ ] **Step 4: Verify auto-create against tempdir**

Adapt the existing smoke test:

```rust
#[tokio::test]
async fn smoke_creates_tracker_when_absent() {
    let tmp = TempDir::new().unwrap();
    /* set up minimal markdown */
    let ctx = mk_smoke_ctx(tmp.path());
    let result = call(&ctx, serde_json::json!({"emit_tracker": true, "paths": ["docs/**/*.md"]})).await.unwrap();
    assert!(result["tracker_id"].as_str().is_some());
    assert!(tmp.path().join("docs/trackers/doc-ref-audit.md").exists());
}
```

- [ ] **Step 5: Run + verify**

Run: `cargo test -p librarian-mcp audit_doc_refs::tests::smoke_creates_tracker_when_absent`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/
git commit -m "feat(audit_doc_refs): auto-create tracker + render template"
```

### Task 16: OutputGuard wiring + glob explosion

**Files:**
- Modify: `src/librarian/tools/audit_doc_refs/mod.rs` (replace ad-hoc overflow with `OutputGuard::cap_items`)

- [ ] **Step 1: Replace inline cap logic with OutputGuard**

Look up the exact signature of `OutputGuard::cap_items` in `src/tools/output.rs`. Replace the ad-hoc cap block in `build_response` with the canonical call. The `OverflowInfo.by_file` map must populate from `findings_by_file(findings)`.

- [ ] **Step 2: Write failing test**

```rust
#[tokio::test]
async fn outputguard_caps_findings_inline() {
    let tmp = TempDir::new().unwrap();
    std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
    // emit 51 missing refs in one markdown file
    let mut body = String::new();
    for i in 0..51 { body.push_str(&format!("`src/gone{i}.py`\n")); }
    std::fs::write(tmp.path().join("docs/spec.md"), body).unwrap();

    let ctx = mk_smoke_ctx(tmp.path());
    let result = call(&ctx, serde_json::json!({"emit_tracker": false, "paths":["docs/**/*.md"]})).await.unwrap();
    assert_eq!(result["findings"].as_array().unwrap().len(), 50);
    assert_eq!(result["overflow"]["total"], 51);
    assert!(result["overflow"]["by_file"]["docs/spec.md"].is_number());
}

#[tokio::test]
async fn glob_explosion_returns_recoverable() {
    let tmp = TempDir::new().unwrap();
    /* set LIBRARIAN_AUDIT_MAX_FILES=2 via test env override or pass a tighter cap */
    let ctx = mk_smoke_ctx(tmp.path());
    std::env::set_var("LIBRARIAN_AUDIT_MAX_FILES", "1");
    for i in 0..5 { std::fs::write(tmp.path().join(format!("doc{i}.md")), "x").unwrap(); }
    let err = call(&ctx, serde_json::json!({"paths":["*.md"]})).await.unwrap_err();
    assert!(format!("{err}").contains("cap"));
    std::env::remove_var("LIBRARIAN_AUDIT_MAX_FILES");
}
```

- [ ] **Step 3: Run + verify**

Run: `cargo test -p librarian-mcp audit_doc_refs::tests::outputguard_caps_findings_inline audit_doc_refs::tests::glob_explosion_returns_recoverable`
Expected: both PASS.

- [ ] **Step 4: Commit**

```bash
git add src/librarian/tools/audit_doc_refs/
git commit -m "feat(audit_doc_refs): OutputGuard wiring + glob explosion recovery"
```

### Task 17: Cargo fmt/clippy/test sweep + experiments push

**Files:** none new — quality gate

- [ ] **Step 1: Format**

Run: `cargo fmt`
Expected: clean.

- [ ] **Step 2: Clippy**

Run: `cargo clippy -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Full test**

Run: `cargo test`
Expected: ALL pass.

- [ ] **Step 4: Live MCP smoke (optional but recommended)**

Run: `cargo build --release`, restart MCP via `/mcp`, invoke `librarian(action="audit_doc_refs", scope="project")` against codescout itself, sanity-check output.

- [ ] **Step 5: Commit any fixes from this sweep**

```bash
git add -A
git commit -m "chore(audit_doc_refs): fmt + clippy + test sweep"
```

---

## Phase 4 — Eval + docs

### Task 18: Tier-3 eval on codescout self

**Files:**
- Create: `tests/librarian/audit_doc_refs/eval_on_codescout_self.rs`
- Create: `tests/librarian/audit_doc_refs/eval_golden.json`

- [ ] **Step 1: Write the `#[ignore]`-marked eval test**

```rust
// tests/librarian/audit_doc_refs/eval_on_codescout_self.rs
use std::path::PathBuf;

#[tokio::test]
#[ignore = "run on demand: cargo test --test audit_doc_refs -- --ignored"]
async fn eval_on_codescout_self() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let ctx = build_ctx(&root);
    let result = librarian_mcp::tools::audit_doc_refs::call(&ctx, serde_json::json!({
        "scope": "project",
        "emit_tracker": false,
        "paths": ["docs/**/*.md", "CLAUDE.md"],
    })).await.unwrap();

    let n_broken = result["n_refs_broken"].as_u64().unwrap();
    let n_high: usize = result["findings"].as_array().unwrap().iter()
        .filter(|f| f["severity"] == "high").count();

    eprintln!("eval result: {} broken, {} high-severity", n_broken, n_high);
    eprintln!("findings: {}", serde_json::to_string_pretty(&result["findings"]).unwrap());

    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/librarian/audit_doc_refs/eval_golden.json");
    if !golden_path.exists() {
        std::fs::write(&golden_path, serde_json::to_string_pretty(&result["findings"]).unwrap()).unwrap();
        panic!("golden file did not exist — wrote current findings to {golden_path:?}; review and commit");
    }
    let golden: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&golden_path).unwrap()).unwrap();
    assert_eq!(&result["findings"], &golden, "audit findings drift vs golden");

    assert!(n_high <= 5, "acceptance threshold: ≤5 high-severity findings on master, got {n_high}");
}
```

- [ ] **Step 2: Run the eval to seed the golden**

Run: `cargo test --test audit_doc_refs -- --ignored eval_on_codescout_self`
Expected: first run panics with "golden file did not exist — wrote current findings"; review the seeded golden file by hand.

- [ ] **Step 3: Audit the seeded golden manually**

Read `tests/librarian/audit_doc_refs/eval_golden.json`. Each `findings[*]` is either:
- A real drift case → fix the doc in a follow-up PR, then re-seed.
- A false positive → file a tool-misbehavior log entry, then re-seed.
- A legitimate finding the team accepts → leave in golden, document why in the doc itself if needed.

- [ ] **Step 4: Re-run to verify pass against committed golden**

Run: `cargo test --test audit_doc_refs -- --ignored eval_on_codescout_self`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/librarian/audit_doc_refs/eval_on_codescout_self.rs tests/librarian/audit_doc_refs/eval_golden.json
git commit -m "test(audit_doc_refs): Tier-3 eval against codescout self + seeded golden"
```

### Task 19: Manual page

**Files:**
- Create: `docs/manual/src/concepts/audit-doc-refs.md`

- [ ] **Step 1: Write the manual page**

Content covers: what the action does, when to run it (manual cadence), the input schema with one example invocation, the verdict glossary, severity drop policy with the four memory globs, how to suppress findings via `wontfix`, how to override severity per ref-kind.

```markdown
<!-- docs/manual/src/concepts/audit-doc-refs.md -->
# Audit Doc Refs

`librarian(action="audit_doc_refs", …)` scans markdown files for stale code
references and emits findings as an `audit_issues` tracker at
`docs/trackers/doc-ref-audit.md` (auto-created on first run).

## When to use

Manual cadence in v1 — run when a doc-heavy PR is about to merge or when you
suspect drift. No CI integration; the `fail_on` flag is present for downstream
repos that wire their own gates.

## What it scans

Only inside code-spans, fenced blocks, and link targets. Plain prose is never
parsed.

## Verdicts

| verdict | meaning | default severity |
|---|---|---|
| resolved | reference matches current code/filesystem | n/a |
| missing | file path does not exist | high |
| symbol_missing | LSP returned no match for symbol | high |
| file_missing | file_symbol's path component is gone | high |
| line_oob | cited line past EOF | med |
| anchor_missing | `#section` link target does not exist in target md | med |
| unknown | parser identified candidate but resolution ambiguous | low |
| external | http/https link — informational, dropped from tracker | n/a |

## Severity drops

| Location | Drop | Why |
|---|---|---|
| `docs/archive/**` or `*.archive.md` | one level | archive is meant to rot |
| Memory files (see globs below) | two levels | memory is temporally pinned by design |
| `docs/issues/**` | one level | issue trackers document historical state |

Memory globs: `.buddy/memory/**`, `**/.buddy/memory/**`, `**/buddy/memory/**`,
`**/projects/**/memory/**`. Override via `severity_overrides.memory_globs`.

## Suppression

Set an issue's `status` to `wontfix` in the tracker. The merger never
auto-flips wontfix back to open.

## Example

```jsonc
librarian({
  "action": "audit_doc_refs",
  "scope": "project",
  "paths": ["docs/**/*.md", "CLAUDE.md"],
  "emit_tracker": true,
  "fail_on": "never"
})
```
```

- [ ] **Step 2: Add to manual's SUMMARY.md (if mdBook)**

If `docs/manual/src/SUMMARY.md` exists, insert a link to the new page in the appropriate section.

- [ ] **Step 3: Build the manual to verify**

Run: `mdbook build docs/manual` (or whatever the existing manual build command is — check `scripts/` or CI).
Expected: clean build.

- [ ] **Step 4: Commit**

```bash
git add docs/manual/
git commit -m "docs(audit_doc_refs): manual page + concept guide"
```

### Task 20: Release notes + final test sweep

- [ ] **Step 1: Update CHANGELOG or release notes draft**

Add a bullet under the next minor version section noting the new action, manual cadence default, and the tracker path.

- [ ] **Step 2: Final pre-completion gate**

Run all three:

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```

Expected: all clean.

- [ ] **Step 3: Live-MCP smoke**

```bash
cargo build --release
# restart MCP via /mcp
# invoke librarian(action="audit_doc_refs", scope="project") against codescout
# verify the tracker file appears at docs/trackers/doc-ref-audit.md
# verify response shape matches spec
```

- [ ] **Step 4: Cherry-pick to master (per project's git workflow)**

Per `CLAUDE.md § Standard Ship Sequence`:

```bash
git checkout master
git cherry-pick <range-of-commits-on-experiments>
git push
git checkout experiments
git rebase master
```

Do not push to master until you have personally verified the live-MCP smoke.

- [ ] **Step 5: Final commit (release notes only — others already committed)**

```bash
git add CHANGELOG.md
git commit -m "chore: release notes for audit_doc_refs action"
```

---

## Self-review notes

Tracker-archetype reuse: verified at design time (`tracker_design.rs:155`). If
the archetype shape changes between now and execution, update the
`augment.params` payload in Task 15.

LSP integration via `ToolContext`: not yet present in librarian's context.
Phase 3 Task 13 carries a NOTE explaining the v1 fallback (return `Unknown`
+ `degraded=true` when no LSP is reachable). If the executing engineer
chooses to plumb LSP through `ToolContext` instead, that is acceptable and
arguably better — but adds plan scope; prefer the v1 fallback unless the
plumbing is a one-line change.

Path drift between `crates/librarian-mcp/` and `src/librarian/`: all task
paths assume post-dissolve. If PR-A has not landed when execution starts,
adjust mentally — but prefer to wait, since the resolver step requires the
in-crate LSP call that the dissolve unlocks.
