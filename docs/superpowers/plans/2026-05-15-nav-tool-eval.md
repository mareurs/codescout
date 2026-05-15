# Nav-Tool Eval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a library-level adversarial eval harness that grades action-correctness of `symbols`, `symbol_at`, `references`, `call_graph` on hand-authored Rust ambiguity fixtures and emits a committed verdict report per run.

**Architecture:** A standalone Rust fixture crate at `tests/fixtures/nav-eval-rust/` (not a workspace member) provides ambiguity traps. A harness module under `tests/e2e/nav_eval/` defines case types, per-tool matchers, a verdict enum, and a markdown report renderer. A single `#[ignore]`-marked `#[tokio::test]` in `tests/e2e/nav_eval.rs` loads the static case slice, runs each case, builds a `Report`, writes it to `docs/superpowers/specs/<date>-nav-eval-round-1.md`, and asserts hard gates.

**Tech Stack:** Rust, Tokio, `serde_json`, the existing `Tool` trait + `ToolContext` from codescout. Reuses `Agent::new` and `LspManager::new_arc` from the existing e2e harness (`tests/e2e/harness.rs`).

---

## Spec reference

Design spec: `docs/superpowers/specs/2026-05-15-nav-tool-eval-design.md`. Read it before starting — every decision in this plan derives from there.

## File map

| File | Responsibility |
|---|---|
| `tests/fixtures/nav-eval-rust/Cargo.toml` | Standalone crate manifest, not in any workspace |
| `tests/fixtures/nav-eval-rust/src/lib.rs` | Declares every fixture module |
| `tests/fixtures/nav-eval-rust/src/*.rs` | 12 fixture files, one per row in the catalogue |
| `tests/e2e/nav_eval.rs` | The single `#[ignore]` test entry point |
| `tests/e2e/nav_eval/mod.rs` | Module root, re-exports types/matchers/cases/report |
| `tests/e2e/nav_eval/types.rs` | `Case`, `Expected`, `Verdict`, `SymbolRef`, `RefLoc`, `ToolUnderTest` |
| `tests/e2e/nav_eval/matchers.rs` | `match_symbols`, `match_symbol_at_def`, `match_references`, `match_call_graph` |
| `tests/e2e/nav_eval/cases.rs` | `pub fn all() -> &'static [Case]` |
| `tests/e2e/nav_eval/report.rs` | `Report::new`, `Report::push`, `Report::render`, `Report::assert_hard_gates` |
| `tests/e2e/nav_eval/runner.rs` | `nav_eval_context`, `run_one`, timeout/panic wrappers |
| `tests/e2e/mod.rs` | Add `pub mod nav_eval;` |

## Pattern notes the engineer must know

1. **Tool invocation pattern** (from `tests/e2e/harness.rs:204`):
   ```rust
   Symbols.call(json!({ "name": "foo" }), ctx).await
   ```
   Tools are unit structs that implement `Tool`. Construct, call `.call(input, ctx)`.

2. **ToolContext construction** (from `tests/e2e/harness.rs:30`):
   ```rust
   let agent = Agent::new(Some(fixture_path)).await.unwrap();
   let lsp = LspManager::new_arc();
   Arc::new(ToolContext {
       agent, lsp,
       output_buffer: Arc::new(OutputBuffer::new(20)),
       progress: None, peer: None,
       section_coverage: Arc::new(Mutex::new(SectionCoverage::new())),
   })
   ```

3. **LSP indexing is async.** First call may return partial results. Pattern in `run_find_references` (`tests/e2e/harness.rs:204`): retry up to 8 times with 500ms × attempt backoff if the expected needle is missing. The eval reuses this retry only for reference-class lookups; symbol search and symbol_at are single-shot.

4. **Why a new context helper rather than reusing `fixture_context`:** `fixture_context(lang)` resolves to `tests/fixtures/<lang>-library/`. Our fixture lives at `tests/fixtures/nav-eval-rust/` (no `-library` suffix) because it is not a language fixture, it is an adversarial probe. Different semantics, different helper.

5. **Recoverable vs fatal errors.** `Tool::call` returns `anyhow::Result<Value>`. `RecoverableError` is a custom error inside that anyhow chain (`src/tools/core/types.rs`). Detect via downcasting: `err.downcast_ref::<RecoverableError>()`. Anything else is fatal.

6. **Sequential per-case execution.** Do not `tokio::join!` the cases. One LSP, one caller. Parallelism risks dirty state.

7. **`#[ignore]` is mandatory.** Rust-analyzer cold-start is 30+s; bundling this into `cargo test` is noise. Run explicitly: `cargo test -- --ignored run_nav_eval`.

---

## Task 1: Standalone fixture crate skeleton

**Files:**
- Create: `tests/fixtures/nav-eval-rust/Cargo.toml`
- Create: `tests/fixtures/nav-eval-rust/src/lib.rs`

- [ ] **Step 1: Create the manifest**

Create `tests/fixtures/nav-eval-rust/Cargo.toml`:

```toml
[package]
name = "nav-eval-rust"
version = "0.0.0"
edition = "2021"
publish = false

# Standalone — intentionally not a workspace member of code-explorer.
# This crate exists only to give rust-analyzer something to attach to
# when the nav-eval harness exercises navigation tools against
# hand-authored ambiguity traps.
[workspace]

[lib]
path = "src/lib.rs"
```

- [ ] **Step 2: Create the lib root**

Create `tests/fixtures/nav-eval-rust/src/lib.rs`:

```rust
//! Adversarial fixtures for the codescout nav-tool eval.
//!
//! Every module here is a hand-authored ambiguity trap. See
//! `docs/superpowers/specs/2026-05-15-nav-tool-eval-design.md`
//! for the catalogue and rationale.

// Modules will be declared as fixture files land.
```

- [ ] **Step 3: Verify the crate compiles**

Run: `cargo check --manifest-path tests/fixtures/nav-eval-rust/Cargo.toml`
Expected: `Finished` with no warnings.

- [ ] **Step 4: Commit**

```bash
git add tests/fixtures/nav-eval-rust/
git commit -m "test(nav-eval): standalone fixture crate skeleton"
```

---

## Task 2: Core types

**Files:**
- Create: `tests/e2e/nav_eval/mod.rs`
- Create: `tests/e2e/nav_eval/types.rs`
- Modify: `tests/e2e/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `tests/e2e/nav_eval/types.rs` with the types AND a unit test:

```rust
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolUnderTest {
    Symbols,
    SymbolAt,
    References,
    CallGraph,
}

#[derive(Debug, Clone)]
pub struct SymbolRef {
    pub name: &'static str,
    pub file: &'static str,
}

#[derive(Debug, Clone)]
pub struct RefLoc {
    pub file: &'static str,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub enum Expected {
    Symbols {
        must_include: Vec<SymbolRef>,
        must_not_include: Vec<SymbolRef>,
    },
    SymbolAtDef {
        file: &'static str,
        line: u32,
    },
    References {
        must_include: Vec<RefLoc>,
        must_not_include: Vec<RefLoc>,
        min_count: usize,
    },
    CallGraph {
        must_include_edges: Vec<(String, String)>,
        must_not_include_edges: Vec<(String, String)>,
    },
    NoResult,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub id: &'static str,
    pub tool: ToolUnderTest,
    pub input: Value,
    pub expected: Expected,
    pub rationale: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Correct,
    Partial,
    CleanError,
    SilentWrong,
    Hung,
    Panic,
}

impl Verdict {
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Correct => "CORRECT",
            Verdict::Partial => "PARTIAL",
            Verdict::CleanError => "CLEAN_ERROR",
            Verdict::SilentWrong => "SILENT_WRONG",
            Verdict::Hung => "HUNG",
            Verdict::Panic => "PANIC",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_labels_are_stable() {
        assert_eq!(Verdict::Correct.label(), "CORRECT");
        assert_eq!(Verdict::SilentWrong.label(), "SILENT_WRONG");
        assert_eq!(Verdict::CleanError.label(), "CLEAN_ERROR");
        assert_eq!(Verdict::Hung.label(), "HUNG");
        assert_eq!(Verdict::Panic.label(), "PANIC");
        assert_eq!(Verdict::Partial.label(), "PARTIAL");
    }
}
```

Create `tests/e2e/nav_eval/mod.rs`:

```rust
pub mod types;
```

Modify `tests/e2e/mod.rs` — add after the existing `pub mod harness;`-style declarations:

```rust
pub mod nav_eval;
```

- [ ] **Step 2: Run the test to verify it passes**

Run: `cargo test --test e2e nav_eval::types::tests::verdict_labels_are_stable`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/nav_eval/ tests/e2e/mod.rs
git commit -m "test(nav-eval): Case/Expected/Verdict types"
```

---

## Task 3: Matchers — `match_symbols`

**Files:**
- Create: `tests/e2e/nav_eval/matchers.rs`
- Modify: `tests/e2e/nav_eval/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `tests/e2e/nav_eval/matchers.rs`:

```rust
use crate::e2e::nav_eval::types::{Expected, RefLoc, SymbolRef, Verdict};
use serde_json::Value;

/// Outcome of comparing a tool response to an `Expected`.
/// `evidence` is a human-readable line that lands under `**Got:**` in the
/// report. Keep it short (one or two lines).
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub verdict: Verdict,
    pub evidence: String,
}

/// Walk the `matches` array of a `symbols` response and grade against
/// `must_include` (every required `SymbolRef` must appear with the right
/// file + name) and `must_not_include` (none of the forbidden refs may
/// appear).
pub fn match_symbols(
    value: &Value,
    must_include: &[SymbolRef],
    must_not_include: &[SymbolRef],
) -> MatchResult {
    let empty = vec![];
    let matches: &Vec<Value> = value
        .get("matches")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let contains = |needle: &SymbolRef| -> bool {
        matches.iter().any(|m| {
            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let file = m.get("file").and_then(|v| v.as_str()).unwrap_or("");
            name == needle.name && file.ends_with(needle.file)
        })
    };

    let missing: Vec<&SymbolRef> = must_include.iter().filter(|n| !contains(n)).collect();
    let forbidden_hit: Vec<&SymbolRef> = must_not_include.iter().filter(|n| contains(n)).collect();

    let summary: Vec<String> = matches
        .iter()
        .map(|m| {
            let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            let file = m.get("file").and_then(|v| v.as_str()).unwrap_or("?");
            format!("{name}@{file}")
        })
        .collect();
    let evidence = format!("matches=[{}]", summary.join(", "));

    if !missing.is_empty() {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — missing {missing:?}"),
        };
    }
    if !forbidden_hit.is_empty() {
        return MatchResult {
            verdict: Verdict::Partial,
            evidence: format!("{evidence} — forbidden present {forbidden_hit:?}"),
        };
    }
    MatchResult {
        verdict: Verdict::Correct,
        evidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sym(name: &'static str, file: &'static str) -> SymbolRef {
        SymbolRef { name, file }
    }

    #[test]
    fn correct_when_all_required_present_and_no_forbidden() {
        let v = json!({"matches": [
            {"name": "new", "file": "src/overload.rs"},
            {"name": "new", "file": "src/other.rs"},
        ]});
        let r = match_symbols(
            &v,
            &[sym("new", "overload.rs"), sym("new", "other.rs")],
            &[],
        );
        assert_eq!(r.verdict, Verdict::Correct);
    }

    #[test]
    fn silent_wrong_when_required_missing() {
        let v = json!({"matches": []});
        let r = match_symbols(&v, &[sym("new", "overload.rs")], &[]);
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }

    #[test]
    fn partial_when_forbidden_present() {
        let v = json!({"matches": [
            {"name": "new", "file": "src/overload.rs"},
            {"name": "new", "file": "src/tests_module.rs"},
        ]});
        let r = match_symbols(
            &v,
            &[sym("new", "overload.rs")],
            &[sym("new", "tests_module.rs")],
        );
        assert_eq!(r.verdict, Verdict::Partial);
    }
}
```

Modify `tests/e2e/nav_eval/mod.rs`:

```rust
pub mod matchers;
pub mod types;
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --test e2e nav_eval::matchers::tests`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/nav_eval/matchers.rs tests/e2e/nav_eval/mod.rs
git commit -m "test(nav-eval): match_symbols + unit coverage"
```

---

## Task 4: Matchers — `match_symbol_at_def`

**Files:**
- Modify: `tests/e2e/nav_eval/matchers.rs`

- [ ] **Step 1: Add the matcher and tests**

Append to `tests/e2e/nav_eval/matchers.rs` (after `match_symbols`):

```rust
/// Extracts the first `def.location` from a symbol_at response and compares
/// against the expected file + line. File comparison uses `ends_with` to be
/// independent of absolute path prefixes.
pub fn match_symbol_at_def(
    value: &Value,
    expected_file: &str,
    expected_line: u32,
) -> MatchResult {
    let def = value.get("def");
    let first = def.and_then(|d| d.get("locations")).and_then(|l| l.as_array()).and_then(|a| a.first());
    let Some(loc) = first else {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("def empty; raw={}", value),
        };
    };
    let file = loc.get("file").and_then(|v| v.as_str()).unwrap_or("");
    let line = loc.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;

    let evidence = format!("def={file}:{line}");
    if file.ends_with(expected_file) && line == expected_line {
        MatchResult { verdict: Verdict::Correct, evidence }
    } else {
        MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — expected {expected_file}:{expected_line}"),
        }
    }
}
```

Append to the existing `#[cfg(test)] mod tests` block in the same file:

```rust
    #[test]
    fn def_correct_when_file_and_line_match() {
        let v = json!({"def": {"locations": [{"file": "/a/b/src/foo.rs", "line": 42}]}});
        let r = match_symbol_at_def(&v, "foo.rs", 42);
        assert_eq!(r.verdict, Verdict::Correct);
    }

    #[test]
    fn def_silent_wrong_when_line_off_by_one() {
        let v = json!({"def": {"locations": [{"file": "/a/b/src/foo.rs", "line": 41}]}});
        let r = match_symbol_at_def(&v, "foo.rs", 42);
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }

    #[test]
    fn def_silent_wrong_when_empty() {
        let v = json!({"def": {"locations": []}});
        let r = match_symbol_at_def(&v, "foo.rs", 42);
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --test e2e nav_eval::matchers::tests`
Expected: 6 tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/nav_eval/matchers.rs
git commit -m "test(nav-eval): match_symbol_at_def"
```

---

## Task 5: Matchers — `match_references` and `match_call_graph`

**Files:**
- Modify: `tests/e2e/nav_eval/matchers.rs`

- [ ] **Step 1: Add both matchers and tests**

Append to `tests/e2e/nav_eval/matchers.rs` after `match_symbol_at_def`:

```rust
pub fn match_references(
    value: &Value,
    must_include: &[RefLoc],
    must_not_include: &[RefLoc],
    min_count: usize,
) -> MatchResult {
    let empty = vec![];
    let refs: &Vec<Value> = value
        .get("references")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let contains = |needle: &RefLoc| -> bool {
        refs.iter().any(|r| {
            let file = r.get("file").and_then(|v| v.as_str()).unwrap_or("");
            let line = r.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            file.ends_with(needle.file) && line == needle.line
        })
    };

    let missing: Vec<&RefLoc> = must_include.iter().filter(|n| !contains(n)).collect();
    let forbidden_hit: Vec<&RefLoc> = must_not_include.iter().filter(|n| contains(n)).collect();

    let evidence = format!("references.len()={}, min_required={min_count}", refs.len());

    if refs.len() < min_count {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — below min_count"),
        };
    }
    if !missing.is_empty() {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — missing {missing:?}"),
        };
    }
    if !forbidden_hit.is_empty() {
        return MatchResult {
            verdict: Verdict::Partial,
            evidence: format!("{evidence} — forbidden present {forbidden_hit:?}"),
        };
    }
    MatchResult { verdict: Verdict::Correct, evidence }
}

pub fn match_call_graph(
    value: &Value,
    must_include_edges: &[(String, String)],
    must_not_include_edges: &[(String, String)],
) -> MatchResult {
    let empty = vec![];
    let edges: &Vec<Value> = value
        .get("edges")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty);

    let edge_pairs: Vec<(String, String)> = edges
        .iter()
        .map(|e| {
            let src = e.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let dst = e.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string();
            (src, dst)
        })
        .collect();

    let missing: Vec<&(String, String)> = must_include_edges
        .iter()
        .filter(|e| !edge_pairs.contains(e))
        .collect();
    let forbidden_hit: Vec<&(String, String)> = must_not_include_edges
        .iter()
        .filter(|e| edge_pairs.contains(e))
        .collect();

    let evidence = format!("edges={edge_pairs:?}");

    if !missing.is_empty() {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("{evidence} — missing {missing:?}"),
        };
    }
    if !forbidden_hit.is_empty() {
        return MatchResult {
            verdict: Verdict::Partial,
            evidence: format!("{evidence} — forbidden present {forbidden_hit:?}"),
        };
    }
    MatchResult { verdict: Verdict::Correct, evidence }
}
```

Append to the test module:

```rust
    fn rloc(file: &'static str, line: u32) -> RefLoc {
        RefLoc { file, line }
    }

    #[test]
    fn refs_silent_wrong_below_min_count() {
        let v = json!({"references": [{"file": "src/a.rs", "line": 1}]});
        let r = match_references(&v, &[], &[], 3);
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }

    #[test]
    fn refs_correct_with_required_and_no_forbidden() {
        let v = json!({"references": [
            {"file": "src/a.rs", "line": 10},
            {"file": "src/b.rs", "line": 20},
        ]});
        let r = match_references(
            &v,
            &[rloc("a.rs", 10), rloc("b.rs", 20)],
            &[],
            2,
        );
        assert_eq!(r.verdict, Verdict::Correct);
    }

    #[test]
    fn refs_partial_when_forbidden_present() {
        let v = json!({"references": [
            {"file": "src/a.rs", "line": 10},
            {"file": "src/tests.rs", "line": 99},
        ]});
        let r = match_references(
            &v,
            &[rloc("a.rs", 10)],
            &[rloc("tests.rs", 99)],
            1,
        );
        assert_eq!(r.verdict, Verdict::Partial);
    }

    #[test]
    fn cg_correct_with_required_edges() {
        let v = json!({"edges": [
            {"from": "a", "to": "b"},
            {"from": "b", "to": "c"},
        ]});
        let r = match_call_graph(
            &v,
            &[("a".to_string(), "b".to_string())],
            &[],
        );
        assert_eq!(r.verdict, Verdict::Correct);
    }

    #[test]
    fn cg_silent_wrong_missing_edge() {
        let v = json!({"edges": []});
        let r = match_call_graph(
            &v,
            &[("a".to_string(), "b".to_string())],
            &[],
        );
        assert_eq!(r.verdict, Verdict::SilentWrong);
    }
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --test e2e nav_eval::matchers::tests`
Expected: 11 tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/nav_eval/matchers.rs
git commit -m "test(nav-eval): match_references + match_call_graph"
```

---

## Task 6: Report renderer

**Files:**
- Create: `tests/e2e/nav_eval/report.rs`
- Modify: `tests/e2e/nav_eval/mod.rs`

- [ ] **Step 1: Add the renderer + tests**

Create `tests/e2e/nav_eval/report.rs`:

```rust
use crate::e2e::nav_eval::matchers::MatchResult;
use crate::e2e::nav_eval::types::{Case, ToolUnderTest, Verdict};

pub struct Row {
    pub case_id: String,
    pub tool: ToolUnderTest,
    pub rationale: String,
    pub verdict: Verdict,
    pub evidence: String,
}

pub struct Report {
    rows: Vec<Row>,
}

impl Report {
    pub fn new() -> Self { Self { rows: vec![] } }

    pub fn push(&mut self, case: &Case, m: MatchResult) {
        self.rows.push(Row {
            case_id: case.id.to_string(),
            tool: case.tool,
            rationale: case.rationale.to_string(),
            verdict: m.verdict,
            evidence: m.evidence,
        });
    }

    pub fn render(&self, round: usize, date_iso: &str) -> String {
        let mut counts = [0usize; 6];
        for r in &self.rows {
            counts[match r.verdict {
                Verdict::Correct => 0,
                Verdict::Partial => 1,
                Verdict::CleanError => 2,
                Verdict::SilentWrong => 3,
                Verdict::Hung => 4,
                Verdict::Panic => 5,
            }] += 1;
        }

        let mut out = String::new();
        out.push_str(&format!("# Nav-tool Eval — Round {round} ({date_iso})\n\n"));
        out.push_str("## Summary\n\n");
        out.push_str(&format!(
            "- Cases: {}  Correct: {}  Partial: {}  Clean-error: {}  Silent-wrong: {}  Hung: {}  Panic: {}\n\n",
            self.rows.len(), counts[0], counts[1], counts[2], counts[3], counts[4], counts[5],
        ));

        out.push_str("## Hard gates\n\n");
        out.push_str(&format!("- [{}] H1 — Zero SILENT_WRONG\n", if counts[3] == 0 { "x" } else { " " }));
        out.push_str(&format!("- [{}] H2 — Zero HUNG\n", if counts[4] == 0 { "x" } else { " " }));
        out.push_str(&format!("- [{}] H3 — Zero PANIC\n", if counts[5] == 0 { "x" } else { " " }));
        out.push_str("\n## Per-case detail\n\n");

        for r in &self.rows {
            out.push_str(&format!(
                "### {} — `{:?}` — {}\n**Verdict:** {}\n**Got:** {}\n\n",
                r.case_id, r.tool, r.rationale, r.verdict.label(), r.evidence,
            ));
        }
        out
    }

    pub fn assert_hard_gates(&self) {
        let mut failures = vec![];
        for r in &self.rows {
            match r.verdict {
                Verdict::SilentWrong => failures.push(format!("{} SILENT_WRONG: {}", r.case_id, r.evidence)),
                Verdict::Hung => failures.push(format!("{} HUNG", r.case_id)),
                Verdict::Panic => failures.push(format!("{} PANIC: {}", r.case_id, r.evidence)),
                _ => {}
            }
        }
        assert!(failures.is_empty(), "Hard gate failures:\n{}", failures.join("\n"));
    }

    pub fn counts(&self) -> (usize, usize, usize, usize, usize, usize) {
        let mut c = [0usize; 6];
        for r in &self.rows {
            c[match r.verdict {
                Verdict::Correct => 0, Verdict::Partial => 1, Verdict::CleanError => 2,
                Verdict::SilentWrong => 3, Verdict::Hung => 4, Verdict::Panic => 5,
            }] += 1;
        }
        (c[0], c[1], c[2], c[3], c[4], c[5])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2e::nav_eval::matchers::MatchResult;
    use crate::e2e::nav_eval::types::{Case, Expected, ToolUnderTest};
    use serde_json::json;

    fn fake_case(id: &'static str) -> Case {
        Case {
            id,
            tool: ToolUnderTest::Symbols,
            input: json!({}),
            expected: Expected::NoResult,
            rationale: "test",
        }
    }

    #[test]
    fn render_includes_summary_and_per_case_headers() {
        let mut r = Report::new();
        r.push(&fake_case("C-01"), MatchResult { verdict: Verdict::Correct, evidence: "ok".into() });
        let md = r.render(1, "2026-05-15");
        assert!(md.contains("# Nav-tool Eval — Round 1 (2026-05-15)"));
        assert!(md.contains("### C-01"));
        assert!(md.contains("CORRECT"));
        assert!(md.contains("**Got:** ok"));
    }

    #[test]
    fn assert_hard_gates_fails_on_silent_wrong() {
        let mut r = Report::new();
        r.push(&fake_case("C-02"), MatchResult { verdict: Verdict::SilentWrong, evidence: "wrong".into() });
        let result = std::panic::catch_unwind(|| r.assert_hard_gates());
        assert!(result.is_err());
    }

    #[test]
    fn assert_hard_gates_passes_on_clean_error_alone() {
        let mut r = Report::new();
        r.push(&fake_case("C-03"), MatchResult { verdict: Verdict::CleanError, evidence: "x".into() });
        r.assert_hard_gates(); // must not panic
    }
}
```

Modify `tests/e2e/nav_eval/mod.rs`:

```rust
pub mod matchers;
pub mod report;
pub mod types;
```

- [ ] **Step 2: Run the tests**

Run: `cargo test --test e2e nav_eval::report::tests`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/nav_eval/report.rs tests/e2e/nav_eval/mod.rs
git commit -m "test(nav-eval): Report renderer + hard gate assert"
```

---

## Task 7: Runner and context helper

**Files:**
- Create: `tests/e2e/nav_eval/runner.rs`
- Modify: `tests/e2e/nav_eval/mod.rs`

- [ ] **Step 1: Write the runner**

Create `tests/e2e/nav_eval/runner.rs`:

```rust
use crate::e2e::nav_eval::matchers::{
    match_call_graph, match_references, match_symbol_at_def, match_symbols, MatchResult,
};
use crate::e2e::nav_eval::types::{Case, Expected, ToolUnderTest, Verdict};
use codescout::agent::Agent;
use codescout::lsp::LspManager;
use codescout::tools::output_buffer::OutputBuffer;
use codescout::tools::section_coverage::SectionCoverage;
use codescout::tools::ToolContext;
use codescout::tools::symbol::{call_graph::CallGraph, references::References, symbol_at::SymbolAt, symbols::Symbols};
use codescout::tools::core::types::Tool;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CASE_TIMEOUT: Duration = Duration::from_secs(30);

/// Build a ToolContext rooted at the nav-eval fixture crate.
///
/// Distinct from the language-fixture helper because our fixture does not
/// follow the `<lang>-library` naming convention — it is an adversarial
/// probe, not a language sample.
pub async fn nav_eval_context() -> Arc<ToolContext> {
    let dir: PathBuf = std::env::current_dir()
        .expect("cwd")
        .join("tests/fixtures/nav-eval-rust");
    assert!(dir.exists(), "Nav-eval fixture missing: {}", dir.display());

    // Ensure rust-analyzer has build artifacts to attach to. Stdout/stderr are
    // discarded — `cargo check` failure here will surface as LSP misses, which
    // is the failure mode we want to make visible in the report rather than
    // a panic before the report is written.
    let _ = std::process::Command::new("cargo")
        .args(["check", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .status();

    let agent = Agent::new(Some(dir.clone()))
        .await
        .expect("Agent::new for nav-eval");
    let lsp = LspManager::new_arc();

    Arc::new(ToolContext {
        agent,
        lsp,
        output_buffer: Arc::new(OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(Mutex::new(SectionCoverage::new())),
    })
}

pub async fn run_one(ctx: &ToolContext, case: &Case) -> MatchResult {
    let fut = invoke(ctx, case);
    match tokio::time::timeout(CASE_TIMEOUT, fut).await {
        Err(_) => MatchResult {
            verdict: Verdict::Hung,
            evidence: format!("exceeded {}s", CASE_TIMEOUT.as_secs()),
        },
        Ok(result) => grade(case, result),
    }
}

async fn invoke(ctx: &ToolContext, case: &Case) -> anyhow::Result<serde_json::Value> {
    match case.tool {
        ToolUnderTest::Symbols => Symbols.call(case.input.clone(), ctx).await,
        ToolUnderTest::SymbolAt => SymbolAt.call(case.input.clone(), ctx).await,
        ToolUnderTest::References => References.call(case.input.clone(), ctx).await,
        ToolUnderTest::CallGraph => CallGraph.call(case.input.clone(), ctx).await,
    }
}

fn grade(case: &Case, result: anyhow::Result<serde_json::Value>) -> MatchResult {
    match result {
        Err(e) => {
            let is_recoverable = e
                .downcast_ref::<codescout::tools::RecoverableError>()
                .is_some();
            if is_recoverable {
                MatchResult {
                    verdict: Verdict::CleanError,
                    evidence: format!("RecoverableError: {e}"),
                }
            } else {
                MatchResult {
                    verdict: Verdict::Panic,
                    evidence: format!("fatal: {e}"),
                }
            }
        }
        Ok(value) => match &case.expected {
            Expected::Symbols { must_include, must_not_include } => {
                match_symbols(&value, must_include, must_not_include)
            }
            Expected::SymbolAtDef { file, line } => {
                match_symbol_at_def(&value, file, *line)
            }
            Expected::References { must_include, must_not_include, min_count } => {
                match_references(&value, must_include, must_not_include, *min_count)
            }
            Expected::CallGraph { must_include_edges, must_not_include_edges } => {
                match_call_graph(&value, must_include_edges, must_not_include_edges)
            }
            Expected::NoResult => MatchResult {
                verdict: Verdict::SilentWrong,
                evidence: format!("expected RecoverableError; got Ok: {value}"),
            },
        },
    }
}
```

Modify `tests/e2e/nav_eval/mod.rs`:

```rust
pub mod matchers;
pub mod report;
pub mod runner;
pub mod types;
```

- [ ] **Step 2: Compile-check**

Run: `cargo check --tests`
Expected: clean compile. Resolve any missing imports against the real paths in `src/tools/symbol/` and `src/tools/mod.rs` (`RecoverableError` is re-exported from `codescout::tools`).

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/nav_eval/runner.rs tests/e2e/nav_eval/mod.rs
git commit -m "test(nav-eval): runner with timeout + RecoverableError detection"
```

---

## Task 8: Empty `cases` slice + harness entrypoint

**Files:**
- Create: `tests/e2e/nav_eval/cases.rs`
- Create: `tests/e2e/nav_eval.rs`
- Modify: `tests/e2e/nav_eval/mod.rs`
- Modify: `tests/e2e/mod.rs`

- [ ] **Step 1: Empty cases slice**

Create `tests/e2e/nav_eval/cases.rs`:

```rust
use crate::e2e::nav_eval::types::Case;

/// All eval cases. Fixture tasks append entries here one file at a time.
pub fn all() -> &'static [Case] {
    &[]
}
```

Modify `tests/e2e/nav_eval/mod.rs`:

```rust
pub mod cases;
pub mod matchers;
pub mod report;
pub mod runner;
pub mod types;
```

- [ ] **Step 2: Harness entrypoint**

Create `tests/e2e/nav_eval.rs`:

```rust
use crate::e2e::nav_eval::cases;
use crate::e2e::nav_eval::report::Report;
use crate::e2e::nav_eval::runner::{nav_eval_context, run_one};
use chrono::Local;
use std::path::PathBuf;

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn run_nav_eval() {
    let ctx = nav_eval_context().await;
    let mut report = Report::new();

    for case in cases::all() {
        let result = run_one(&ctx, case).await;
        report.push(case, result);
    }

    let date = Local::now().format("%Y-%m-%d").to_string();
    let round = next_round_number(&date);
    let out = PathBuf::from(format!(
        "docs/superpowers/specs/{date}-nav-eval-round-{round}.md"
    ));
    std::fs::write(&out, report.render(round, &date)).expect("write report");
    eprintln!("Nav-eval report → {}", out.display());

    report.assert_hard_gates();
}

fn next_round_number(date: &str) -> usize {
    let dir = PathBuf::from("docs/superpowers/specs");
    let prefix = format!("{date}-nav-eval-round-");
    let mut max_seen = 0usize;
    let Ok(entries) = std::fs::read_dir(&dir) else { return 1 };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(|s| s.to_string()) else { continue };
        let Some(rest) = name.strip_prefix(&prefix) else { continue };
        let Some(num_str) = rest.strip_suffix(".md") else { continue };
        if let Ok(n) = num_str.parse::<usize>() {
            if n > max_seen { max_seen = n; }
        }
    }
    max_seen + 1
}
```

Modify `tests/e2e/mod.rs` — add at the bottom:

```rust
pub mod nav_eval_harness;
```

Wait — naming clash: we already declared `pub mod nav_eval;` in Task 2 for the *module directory*. The entrypoint test file at `tests/e2e/nav_eval.rs` collides. Fix: rename the entrypoint to `tests/e2e/nav_eval_harness.rs` to keep the module dir at `tests/e2e/nav_eval/`. Edit the file path in this task accordingly.

Replace the create above with: **Create `tests/e2e/nav_eval_harness.rs`** (same contents).

`tests/e2e/mod.rs` final lines:

```rust
pub mod nav_eval;
pub mod nav_eval_harness;
```

- [ ] **Step 3: Run the empty eval to verify wiring**

Run: `cargo test --test e2e -- --ignored run_nav_eval`
Expected: PASS, with an empty round file written to `docs/superpowers/specs/<date>-nav-eval-round-1.md` containing "Cases: 0" and all hard gates checked.

- [ ] **Step 4: Inspect the emitted file, then delete it before commit**

The empty report is not the artifact we want to keep — the first meaningful round is committed after cases land (Task 13).

```bash
rm docs/superpowers/specs/*-nav-eval-round-1.md
git add tests/e2e/nav_eval/cases.rs tests/e2e/nav_eval_harness.rs tests/e2e/nav_eval/mod.rs tests/e2e/mod.rs
git commit -m "test(nav-eval): harness entrypoint + empty cases slice"
```

---

## Task 9: Fixtures — overload, trait_dispatch, generics + cases

**Files:**
- Create: `tests/fixtures/nav-eval-rust/src/overload.rs`
- Create: `tests/fixtures/nav-eval-rust/src/trait_dispatch.rs`
- Create: `tests/fixtures/nav-eval-rust/src/generics.rs`
- Modify: `tests/fixtures/nav-eval-rust/src/lib.rs`
- Modify: `tests/e2e/nav_eval/cases.rs`

- [ ] **Step 1: Write `overload.rs`**

Create `tests/fixtures/nav-eval-rust/src/overload.rs`:

```rust
//! Three structs each define `fn new` with different signatures.
//! Trap: which `new` does a name-only search resolve to?

pub struct Foo;
pub struct Bar;
pub struct Baz;

impl Foo {
    pub fn new() -> Foo { Foo }
}

impl Bar {
    pub fn new(_label: &str) -> Bar { Bar }
}

impl Baz {
    pub fn new(_n: usize, _flag: bool) -> Baz { Baz }
}
```

- [ ] **Step 2: Write `trait_dispatch.rs`**

Create `tests/fixtures/nav-eval-rust/src/trait_dispatch.rs`:

```rust
//! Inherent `Counter::next` AND `Iterator::next` impl on the same type.
//! Trap: call `counter.next()` and ask symbol_at which `next` it resolves to.

pub struct Counter { value: u32 }

impl Counter {
    pub fn new() -> Self { Counter { value: 0 } }
    /// Inherent method — same name as Iterator::next.
    pub fn next(&mut self) -> u32 {
        self.value += 1;
        self.value
    }
}

impl Iterator for Counter {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        let v = self.value + 100;
        self.value = v;
        Some(v)
    }
}

pub fn use_counter() {
    let mut c = Counter::new();
    let _ = c.next();
}
```

- [ ] **Step 3: Write `generics.rs`**

Create `tests/fixtures/nav-eval-rust/src/generics.rs`:

```rust
//! Two `parse<T>` functions in different submodules, identical bounds.
//! Trap: symbol-name search must return BOTH, not just one.

pub mod left {
    use std::str::FromStr;
    pub fn parse<T: FromStr>(s: &str) -> Option<T> { s.parse().ok() }
}

pub mod right {
    use std::str::FromStr;
    pub fn parse<T: FromStr>(s: &str) -> Option<T> { s.parse().ok() }
}

pub fn use_both() {
    let _: Option<i32> = left::parse("1");
    let _: Option<u64> = right::parse("2");
}
```

- [ ] **Step 4: Register the modules**

Replace `tests/fixtures/nav-eval-rust/src/lib.rs`:

```rust
//! Adversarial fixtures for the codescout nav-tool eval.

pub mod generics;
pub mod overload;
pub mod trait_dispatch;
```

- [ ] **Step 5: Verify fixture compiles**

Run: `cargo check --manifest-path tests/fixtures/nav-eval-rust/Cargo.toml`
Expected: clean compile.

- [ ] **Step 6: Append cases**

Replace `tests/e2e/nav_eval/cases.rs`:

```rust
use crate::e2e::nav_eval::types::{Case, Expected, SymbolRef, ToolUnderTest};
use serde_json::json;
use std::sync::OnceLock;

static CASES: OnceLock<Vec<Case>> = OnceLock::new();

pub fn all() -> &'static [Case] {
    CASES.get_or_init(|| vec![
        // ---------------- overload.rs ----------------
        Case {
            id: "C-01",
            tool: ToolUnderTest::Symbols,
            input: json!({ "name": "new", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "new", file: "overload.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "three impls of `new` — search must return them all",
        },
        // ---------------- trait_dispatch.rs ----------------
        Case {
            id: "C-02",
            tool: ToolUnderTest::Symbols,
            input: json!({ "name": "next", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "next", file: "trait_dispatch.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "inherent next + Iterator::next on the same struct",
        },
        Case {
            // line 24 is `let _ = c.next();` inside `use_counter`.
            // We expect symbol_at on column of `next` to resolve to the
            // inherent method (line 9). If LSP returns Iterator::next (line 16)
            // that's wrong-target SILENT_WRONG.
            id: "C-03",
            tool: ToolUnderTest::SymbolAt,
            input: json!({
                "path": "src/trait_dispatch.rs",
                "line": 24,
                "identifier": "next",
            }),
            expected: Expected::SymbolAtDef {
                file: "trait_dispatch.rs",
                line: 9,
            },
            rationale: "ambiguous call site — identifier-on-line vs trait-impl",
        },
        // ---------------- generics.rs ----------------
        Case {
            id: "C-04",
            tool: ToolUnderTest::Symbols,
            input: json!({ "name": "parse", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "parse", file: "generics.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "two parse<T> in different submodules — both must appear",
        },
    ])
}
```

- [ ] **Step 7: Run the harness and inspect verdicts**

Run: `cargo test --test e2e -- --ignored run_nav_eval`
Expected: test runs to completion; round file written; hard gates may or may not pass at this point. Open the report file and check that C-01..C-04 each have a verdict and a `Got:` line.

- [ ] **Step 8: Commit the fixtures + cases (not the round file)**

```bash
rm docs/superpowers/specs/*-nav-eval-round-1.md
git add tests/fixtures/nav-eval-rust/src/ tests/e2e/nav_eval/cases.rs
git commit -m "test(nav-eval): fixtures + cases — overload/trait_dispatch/generics"
```

---

## Task 10: Fixtures — cross_module, shadowing, re_export, closure_vs_fn + cases

**Files:**
- Create: `tests/fixtures/nav-eval-rust/src/cross_module.rs`
- Create: `tests/fixtures/nav-eval-rust/src/shadowing.rs`
- Create: `tests/fixtures/nav-eval-rust/src/re_export.rs`
- Create: `tests/fixtures/nav-eval-rust/src/closure_vs_fn.rs`
- Modify: `tests/fixtures/nav-eval-rust/src/lib.rs`
- Modify: `tests/e2e/nav_eval/cases.rs`

- [ ] **Step 1: Write `cross_module.rs`**

Create `tests/fixtures/nav-eval-rust/src/cross_module.rs`:

```rust
//! Two modules each define `validate`. `a::validate` is called from
//! `use_a`; `b::validate` is dead. References for `a::validate` must NOT
//! include `b::validate`'s definition site.

pub mod a {
    pub fn validate(s: &str) -> bool { !s.is_empty() }
}

pub mod b {
    pub fn validate(s: &str) -> bool { !s.is_empty() }
}

pub fn use_a() {
    let _ = a::validate("hi");
}
```

- [ ] **Step 2: Write `shadowing.rs`**

Create `tests/fixtures/nav-eval-rust/src/shadowing.rs`:

```rust
//! Top-level `fn parse` shadowed by a local binding inside `caller`.
//! symbol_at on `parse(s)` inside `caller` must resolve to the local
//! binding (line 11), not the top-level fn (line 6).

pub fn parse(s: &str) -> usize { s.len() }

pub fn caller(s: &str) -> usize {
    let parse = |x: &str| x.len() * 2;
    parse(s)
}
```

- [ ] **Step 3: Write `re_export.rs`**

Create `tests/fixtures/nav-eval-rust/src/re_export.rs`:

```rust
//! Same item exposed under two names via `pub use ... as ...`.
//! references for `Bar` must include the def site; references for `Baz`
//! must include the re-export site, both ultimately pointing to the same
//! type but via different name resolutions.

pub mod inner {
    pub struct Bar;
}

pub use inner::Bar as Baz;

pub fn make_bar() -> inner::Bar { inner::Bar }
pub fn make_baz() -> Baz { Baz }
```

- [ ] **Step 4: Write `closure_vs_fn.rs`**

Create `tests/fixtures/nav-eval-rust/src/closure_vs_fn.rs`:

```rust
//! Top-level `fn handle` plus a local closure `let handle = ...` inside
//! another fn. Name-only search must return the top-level fn; the closure
//! binding is a local, not a top-level symbol.

pub fn handle(_x: u32) -> u32 { 0 }

pub fn caller() {
    let handle = |x: u32| x + 1;
    let _ = handle(2);
}
```

- [ ] **Step 5: Register modules**

Replace `tests/fixtures/nav-eval-rust/src/lib.rs`:

```rust
//! Adversarial fixtures for the codescout nav-tool eval.

pub mod closure_vs_fn;
pub mod cross_module;
pub mod generics;
pub mod overload;
pub mod re_export;
pub mod shadowing;
pub mod trait_dispatch;
```

- [ ] **Step 6: Verify fixture still compiles**

Run: `cargo check --manifest-path tests/fixtures/nav-eval-rust/Cargo.toml`
Expected: clean compile.

- [ ] **Step 7: Append cases**

In `tests/e2e/nav_eval/cases.rs`, append inside the `vec![...]` before the closing `])` — keep all existing cases:

```rust
        // ---------------- cross_module.rs ----------------
        Case {
            id: "C-05",
            tool: ToolUnderTest::References,
            input: json!({
                "symbol": "a/validate",
                "path": "src/cross_module.rs",
            }),
            expected: Expected::References {
                must_include: vec![],
                must_not_include: vec![],
                min_count: 2,
            },
            rationale: "validate-in-a is called once; min_count 2 covers def + call",
        },
        // ---------------- shadowing.rs ----------------
        Case {
            // Line 11: `parse(s)` inside `caller`. Local `parse` defined line 10.
            id: "C-06",
            tool: ToolUnderTest::SymbolAt,
            input: json!({
                "path": "src/shadowing.rs",
                "line": 11,
                "identifier": "parse",
            }),
            expected: Expected::SymbolAtDef {
                file: "shadowing.rs",
                line: 10,
            },
            rationale: "local binding must win over top-level fn",
        },
        // ---------------- re_export.rs ----------------
        Case {
            id: "C-07",
            tool: ToolUnderTest::References,
            input: json!({
                "symbol": "Bar",
                "path": "src/re_export.rs",
            }),
            expected: Expected::References {
                must_include: vec![],
                must_not_include: vec![],
                min_count: 2,
            },
            rationale: "Bar referenced via direct path and via re-export Baz",
        },
        // ---------------- closure_vs_fn.rs ----------------
        Case {
            id: "C-08",
            tool: ToolUnderTest::Symbols,
            input: json!({ "name": "handle", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "handle", file: "closure_vs_fn.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "top-level fn handle visible; closure binding is not a top-level symbol",
        },
```

- [ ] **Step 8: Run + commit**

```bash
cargo test --test e2e -- --ignored run_nav_eval
rm docs/superpowers/specs/*-nav-eval-round-*.md
git add tests/fixtures/nav-eval-rust/src/ tests/e2e/nav_eval/cases.rs
git commit -m "test(nav-eval): cross_module/shadowing/re_export/closure_vs_fn"
```

---

## Task 11: Fixtures — macro_expansion, tests_module + cases

**Files:**
- Create: `tests/fixtures/nav-eval-rust/src/macro_expansion.rs`
- Create: `tests/fixtures/nav-eval-rust/src/tests_module.rs`
- Modify: `tests/fixtures/nav-eval-rust/src/lib.rs`
- Modify: `tests/e2e/nav_eval/cases.rs`

- [ ] **Step 1: Write `macro_expansion.rs`**

Create `tests/fixtures/nav-eval-rust/src/macro_expansion.rs`:

```rust
//! Top-level `fn run` plus a `fn run` generated by a macro_rules invocation.
//! Trap: does symbol search see the macro-generated body?

macro_rules! make_run {
    ($name:ident) => {
        pub fn $name() -> u32 { 42 }
    };
}

pub fn run() -> u32 { 1 }

make_run!(run_generated);
```

- [ ] **Step 2: Write `tests_module.rs`**

Create `tests/fixtures/nav-eval-rust/src/tests_module.rs`:

```rust
//! Top-level `fn add` plus a `fn add` inside a `#[cfg(test)] mod tests`
//! helper. Default search scope must include the top-level fn; whether it
//! includes the test-module helper depends on tool semantics — we encode
//! the current expected behavior and let the report reveal drift.

pub fn add(a: i32, b: i32) -> i32 { a + b }

#[cfg(test)]
mod tests {
    fn add(_x: i32) -> i32 { 0 }

    #[test]
    fn smoke() {
        let _ = add(1);
        assert_eq!(super::add(1, 2), 3);
    }
}
```

- [ ] **Step 3: Register modules**

Replace `tests/fixtures/nav-eval-rust/src/lib.rs`:

```rust
//! Adversarial fixtures for the codescout nav-tool eval.

pub mod closure_vs_fn;
pub mod cross_module;
pub mod generics;
pub mod macro_expansion;
pub mod overload;
pub mod re_export;
pub mod shadowing;
pub mod tests_module;
pub mod trait_dispatch;
```

- [ ] **Step 4: Verify fixture compiles**

Run: `cargo check --manifest-path tests/fixtures/nav-eval-rust/Cargo.toml`
Expected: clean compile.

- [ ] **Step 5: Append cases**

In `tests/e2e/nav_eval/cases.rs`, append inside `vec![...]`:

```rust
        // ---------------- macro_expansion.rs ----------------
        Case {
            id: "C-09",
            tool: ToolUnderTest::Symbols,
            input: json!({ "name": "run", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "run", file: "macro_expansion.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "macro-generated fn coexists with hand-written fn",
        },
        // ---------------- tests_module.rs ----------------
        Case {
            id: "C-10",
            tool: ToolUnderTest::Symbols,
            input: json!({ "name": "add", "scope": "project" }),
            expected: Expected::Symbols {
                must_include: vec![
                    SymbolRef { name: "add", file: "tests_module.rs" },
                ],
                must_not_include: vec![],
            },
            rationale: "top-level add must be discoverable; mod tests helper drift recorded in report",
        },
```

- [ ] **Step 6: Run + commit**

```bash
cargo test --test e2e -- --ignored run_nav_eval
rm docs/superpowers/specs/*-nav-eval-round-*.md
git add tests/fixtures/nav-eval-rust/src/ tests/e2e/nav_eval/cases.rs
git commit -m "test(nav-eval): macro_expansion + tests_module"
```

---

## Task 12: Fixtures — call_graph_cycle, call_graph_trait, cold_path + cases

**Files:**
- Create: `tests/fixtures/nav-eval-rust/src/call_graph_cycle.rs`
- Create: `tests/fixtures/nav-eval-rust/src/call_graph_trait.rs`
- Create: `tests/fixtures/nav-eval-rust/src/cold_path.rs`
- Modify: `tests/fixtures/nav-eval-rust/src/lib.rs`
- Modify: `tests/e2e/nav_eval/cases.rs`

- [ ] **Step 1: Write `call_graph_cycle.rs`**

Create `tests/fixtures/nav-eval-rust/src/call_graph_cycle.rs`:

```rust
//! Cycle a -> b -> c -> a. BFS callees from `a` at depth 5 must terminate
//! and deduplicate.

pub fn a() { b() }
pub fn b() { c() }
pub fn c() {
    if false { a() }
}
```

- [ ] **Step 2: Write `call_graph_trait.rs`**

Create `tests/fixtures/nav-eval-rust/src/call_graph_trait.rs`:

```rust
//! Trait `Worker::run` with three impls. `impl_alpha::run` calls
//! `impl_beta::run` via a `Worker` reference (trait dispatch).
//! Does callees crossing trait dispatch resolve?

pub trait Worker { fn run(&self); }

pub struct Alpha;
pub struct Beta;
pub struct Gamma;

impl Worker for Alpha {
    fn run(&self) {
        let b = Beta;
        let w: &dyn Worker = &b;
        w.run();
    }
}

impl Worker for Beta {
    fn run(&self) {
        let g = Gamma;
        g.run();
    }
}

impl Worker for Gamma {
    fn run(&self) {}
}
```

- [ ] **Step 3: Write `cold_path.rs`**

Create `tests/fixtures/nav-eval-rust/src/cold_path.rs`:

```rust
//! `cold` is only reachable from `#[cfg(test)]` code. references must
//! return at least one ref (the cfg-test caller) — confirms the scope
//! includes test-config code.

pub fn cold() -> u32 { 7 }

#[cfg(test)]
mod tests {
    use super::cold;
    #[test]
    fn smoke() { assert_eq!(cold(), 7); }
}
```

- [ ] **Step 4: Register modules**

Replace `tests/fixtures/nav-eval-rust/src/lib.rs`:

```rust
//! Adversarial fixtures for the codescout nav-tool eval.

pub mod call_graph_cycle;
pub mod call_graph_trait;
pub mod closure_vs_fn;
pub mod cold_path;
pub mod cross_module;
pub mod generics;
pub mod macro_expansion;
pub mod overload;
pub mod re_export;
pub mod shadowing;
pub mod tests_module;
pub mod trait_dispatch;
```

- [ ] **Step 5: Verify fixture compiles**

Run: `cargo check --manifest-path tests/fixtures/nav-eval-rust/Cargo.toml`
Expected: clean compile (may emit dead-code warnings for unused fns — that is fine; suppress with `#[allow(dead_code)]` per-file if noisy).

- [ ] **Step 6: Append cases**

In `tests/e2e/nav_eval/cases.rs`, append inside `vec![...]`:

```rust
        // ---------------- call_graph_cycle.rs ----------------
        Case {
            id: "C-11",
            tool: ToolUnderTest::CallGraph,
            input: json!({
                "symbol": "a",
                "path": "src/call_graph_cycle.rs",
                "direction": "callees",
                "max_depth": 5,
            }),
            expected: Expected::CallGraph {
                must_include_edges: vec![
                    ("a".to_string(), "b".to_string()),
                    ("b".to_string(), "c".to_string()),
                ],
                must_not_include_edges: vec![],
            },
            rationale: "cycle must terminate; deduped edges only",
        },
        // ---------------- call_graph_trait.rs ----------------
        Case {
            id: "C-12",
            tool: ToolUnderTest::CallGraph,
            input: json!({
                "symbol": "impl Worker for Alpha/run",
                "path": "src/call_graph_trait.rs",
                "direction": "callees",
                "max_depth": 3,
            }),
            expected: Expected::CallGraph {
                must_include_edges: vec![],
                must_not_include_edges: vec![],
            },
            rationale: "dynamic dispatch crossing trait — current behavior recorded, not asserted",
        },
        // ---------------- cold_path.rs ----------------
        Case {
            id: "C-13",
            tool: ToolUnderTest::References,
            input: json!({
                "symbol": "cold",
                "path": "src/cold_path.rs",
            }),
            expected: Expected::References {
                must_include: vec![],
                must_not_include: vec![],
                min_count: 2,
            },
            rationale: "cfg(test)-only caller must be reachable from references",
        },
        // ---------------- LIMIT-001: callees without LSP fallback ----------------
        Case {
            id: "C-14",
            tool: ToolUnderTest::CallGraph,
            input: json!({
                "symbol": "a",
                "path": "src/call_graph_cycle.rs",
                "direction": "callees",
                "max_depth": 1,
            }),
            expected: Expected::CallGraph {
                must_include_edges: vec![
                    ("a".to_string(), "b".to_string()),
                ],
                must_not_include_edges: vec![],
            },
            rationale: "smoke for callees one-hop — if LSP callHierarchy is unavailable, clean-error is acceptable",
        },
```

- [ ] **Step 7: Run + commit**

```bash
cargo test --test e2e -- --ignored run_nav_eval
rm docs/superpowers/specs/*-nav-eval-round-*.md
git add tests/fixtures/nav-eval-rust/src/ tests/e2e/nav_eval/cases.rs
git commit -m "test(nav-eval): call_graph cycle/trait + cold_path"
```

---

## Task 13: First round verdict, committed

**Files:**
- Generated: `docs/superpowers/specs/<date>-nav-eval-round-1.md`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Verify formatting / lint cleanliness**

Run: `cargo fmt --all`
Run: `cargo clippy --tests -- -D warnings`
Expected: no diffs, no warnings.

- [ ] **Step 2: Run the eval producing the first committed round**

Run: `cargo test --test e2e -- --ignored run_nav_eval --nocapture`
Expected: report path printed to stderr, test passes (hard gates green) OR fails with a clear list of SILENT_WRONG/HUNG/PANIC entries.

- [ ] **Step 3: If hard gates fail, do NOT fix tools in this plan**

This plan's deliverable is the eval, not tool fixes. Any SILENT_WRONG, HUNG, or PANIC entries are surfaced as findings in the round file, and each becomes its own follow-up spec. Mark the round file's hard-gate boxes accordingly and document the failures in the report's per-case detail (already done automatically by `Report::render`).

If hard gates fail, the test fails. The verdict file is still written *before* the assertion, so it lands on disk. Commit it as-is and open issues per failing case.

- [ ] **Step 4: Add CHANGELOG entry**

Insert at the top of the `## Unreleased` section in `CHANGELOG.md` (create the section if absent):

```markdown
### Added
- Nav-tool eval harness (`tests/e2e/nav_eval_harness.rs`, `tests/fixtures/nav-eval-rust/`).
  Library-level adversarial eval grading action-correctness of `symbols`,
  `symbol_at`, `references`, and `call_graph` on hand-authored Rust ambiguity
  traps. Run via `cargo test --test e2e -- --ignored run_nav_eval`. First
  round verdict committed at `docs/superpowers/specs/<date>-nav-eval-round-1.md`.
  See spec at `docs/superpowers/specs/2026-05-15-nav-tool-eval-design.md`.
```

- [ ] **Step 5: Commit the round + CHANGELOG**

```bash
git add docs/superpowers/specs/*-nav-eval-round-1.md CHANGELOG.md
git commit -m "test(nav-eval): round 1 verdict + CHANGELOG"
```

---

## Self-review notes (executed before this plan was saved)

1. **Spec coverage check.** Every row in spec §"Decisions settled" is implemented: tools (T7 runner), failure axis (matchers grade correctness only), failure target (T9-T12 cases are all disambiguation traps), ground truth (T9-T12 hand-authored), grading verdict enum (T2), language (T9-T12 are Rust), approach library-level (T7 calls `Tool::call` directly), case count 14 (T9-T12: 4+4+2+4), harness location (T8 `nav_eval_harness.rs`), verdict location (T8 path computation), `#[ignore]` marker (T8), iteration via `next_round_number` (T8). Hard gates implemented in T6 `Report::assert_hard_gates`.
2. **Placeholder scan.** No TBDs. Every step has full code. Two places use "current behavior recorded" in rationale (C-10, C-12) — intentional: those cases are observational, not assertive. Their `must_*` lists are intentionally empty so they only fail on SILENT_WRONG. The report carries the observed result regardless.
3. **Type consistency.** `SymbolRef { name, file }`, `RefLoc { file, line }`, `Verdict::Correct/Partial/CleanError/SilentWrong/Hung/Panic`, `Expected::{Symbols, SymbolAtDef, References, CallGraph, NoResult}` referenced consistently in T2 (definition), T3-T5 (matchers), T6 (report rows), T7 (runner), T8 (cases sentinel), T9-T12 (case bodies).
4. **Naming clash fixed inline.** T2 declared `pub mod nav_eval;` (a directory module); T8 originally wanted `tests/e2e/nav_eval.rs` for the entrypoint test — Rust does not allow a file and a directory with the same name at the same level. Renamed entrypoint to `nav_eval_harness.rs`. Both T8 and the CHANGELOG line in T13 use the new name.
5. **`scope: "project"` in case inputs.** `symbols` accepts a `scope` arg (`"project"` | `"libraries"` | `"all"` | `"lib:<name>"`); we pin to project so cases are deterministic and not affected by registered libraries on the host.
6. **`references` `min_count` floors.** Set per case based on the fixture's actual reference structure. If a fixture is edited later, `min_count` must be updated alongside.

## Non-goals (this plan)

- No code changes to `symbols`, `symbol_at`, `references`, or `call_graph`. The eval measures; it does not fix.
- No CI integration. Test is `#[ignore]` — opt-in. Follow-up if round results justify wiring it into a periodic job.
- No language other than Rust. Follow-up specs per language.
