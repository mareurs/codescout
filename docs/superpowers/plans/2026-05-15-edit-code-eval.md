# Edit-Code Tool Eval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an adversarial library-level eval for `edit_code` — 14 cases across replace/insert/remove/rename, graded by composing tool return + on-disk content + `cargo check` exit. Catches BUG-054-class silent corruption.

**Architecture:** Standalone fixture crate at `tests/fixtures/edit-eval-rust/` mutated in-place per case; `git restore` resets between sequential cases; warm cargo target keeps `cargo check` fast (~1–2s/case). Shared harness primitives live in a new `tests/e2e/eval_common/` module which nav-eval is refactored onto first, then edit-eval builds against.

**Tech Stack:** Rust 2021, `serde_json`, `anyhow`, `tokio`, `cargo` (subprocess), `git` (subprocess), codescout `Tool` / `Agent` / `LspManager` / `ToolContext` / `RecoverableError` / `EditCode` types from this crate.

---

## Spec reference

- `docs/superpowers/specs/2026-05-15-edit-code-eval-design.md` — full design including verdict rubric, case catalogue, hard gates
- `docs/superpowers/specs/2026-05-15-nav-tool-eval-design.md` — companion eval (this plan refactors its runtime onto shared module)
- `docs/superpowers/plans/2026-05-15-nav-tool-eval.md` — predecessor plan whose patterns this one extends

## File map

| File | Responsibility |
|---|---|
| `tests/fixtures/edit-eval-rust/Cargo.toml` | Standalone crate manifest, `[workspace]` empty |
| `tests/fixtures/edit-eval-rust/src/lib.rs` | Declares 14 fixture modules |
| `tests/fixtures/edit-eval-rust/src/replace_*.rs` | 8 replace targets |
| `tests/fixtures/edit-eval-rust/src/insert_*.rs` | 3 insert targets |
| `tests/fixtures/edit-eval-rust/src/remove_*.rs` | 2 remove targets |
| `tests/fixtures/edit-eval-rust/src/rename_*.rs` | 1 rename target + 2 caller files |
| `tests/e2e/eval_common/mod.rs` | Re-exports shared primitives |
| `tests/e2e/eval_common/verdict.rs` | `Verdict` enum + `label()` |
| `tests/e2e/eval_common/report.rs` | `Report::new/push/render/assert_hard_gates`, `next_round_number()` |
| `tests/e2e/eval_common/proc.rs` | `git_restore`, `cargo_check`, `read_fixture_file` helpers |
| `tests/e2e/edit_eval/mod.rs` | Re-exports edit-specific types |
| `tests/e2e/edit_eval/types.rs` | `EditCase`, `EditAction`, `Expected`, `ContentInvariant` |
| `tests/e2e/edit_eval/matchers.rs` | `grade(case, return, disk, post_check) -> MatchResult` |
| `tests/e2e/edit_eval/runner.rs` | `edit_eval_context`, `run_one` |
| `tests/e2e/edit_eval/cases.rs` | `pub fn all() -> &'static [EditCase]` — 14 cases |
| `tests/e2e/edit_eval_harness.rs` | `#[ignore]` entrypoint |
| `tests/e2e/nav_eval/types.rs` | Modify: `Verdict` moves to `eval_common::Verdict` (re-export) |
| `tests/e2e/nav_eval/report.rs` | Modify: delegate to `eval_common::Report` |
| `tests/e2e/nav_eval/runner.rs` | Modify: use `eval_common::Verdict` |
| `tests/e2e/mod.rs` | Add `pub mod eval_common; pub mod edit_eval;` |

## Pattern notes the engineer must know

1. **Tool invocation pattern** (from `tests/e2e/nav_eval/runner.rs`):
   ```rust
   use codescout::tools::symbol::EditCode;
   use codescout::tools::Tool;
   EditCode.call(json!({ "action": "replace", "symbol": "...", "path": "...", "body": "..." }), ctx).await
   ```

2. **ToolContext construction** (already factored into `nav_eval_context`; you'll factor a parallel `edit_eval_context`):
   ```rust
   let agent = Agent::new(Some(dir.clone())).await?;
   let lsp = LspManager::new_arc();
   Arc::new(ToolContext {
       agent, lsp,
       output_buffer: Arc::new(OutputBuffer::new(20)),
       progress: None, peer: None,
       section_coverage: Arc::new(Mutex::new(SectionCoverage::new())),
   })
   ```

3. **Mutation isolation rule.** Each case mutates the fixture on disk. The runner MUST `git restore -- tests/fixtures/edit-eval-rust/src/` before each case and verify pre-edit state compiles. If pre-edit doesn't compile, abort with a clear error — that's a fixture authoring bug, not an `edit_code` finding.

4. **cargo check oracle.** After the edit, run `cargo check --manifest-path tests/fixtures/edit-eval-rust/Cargo.toml`. The exit code is the disk oracle. Warm target dir → 1-2s; first run cold → ~30s. Run a warm-up check in `edit_eval_context` once before any case.

5. **Composite verdict.** A case declares its `Expected { return: ReturnExpected, disk: ContentInvariant, compiler: CompilerExpected }`. Grader compares observed triplet to expected; match = CORRECT, mismatch chooses worst-class verdict. This lets I-03 and M-02 declare `ok + faithful-content + build-fails` as their *expected* state and grade CORRECT.

6. **Recoverable vs fatal errors** (unchanged from nav-eval). `RecoverableError` via `err.downcast_ref::<RecoverableError>()`. Transient LSP "content modified" / `-32801` → retryable SilentWrong, not Panic.

7. **R-02 is the BUG-054 sentinel.** Until the trait-method stray-brace bug is fixed in `src/tools/symbol/edit_code.rs`, R-02 grades SILENT_WRONG and the matcher exempts it from H1 hard-gate. The exemption lives in `matchers.rs` with a `// LIMIT: BUG-054` comment so it can't drift.

8. **`#[ignore]` is mandatory.** Cargo target warm-up + 14 cases ≈ 30-60s. Run explicitly: `cargo test --test e2e_tests edit_eval_harness -- --ignored --nocapture`.

9. **Sequential execution required.** `git restore` operates on a shared working tree. Do NOT `tokio::join!` cases.

10. **Hands off user's in-progress edits.** `git restore` is scoped to `tests/fixtures/edit-eval-rust/src/` only. Never `git restore --` without a path; the user has unstaged edits in `src/retrieval/` that must not be touched.

---

## Task 1: Standalone fixture crate skeleton

**Files:**
- Create: `tests/fixtures/edit-eval-rust/Cargo.toml`
- Create: `tests/fixtures/edit-eval-rust/src/lib.rs`

- [ ] **Step 1: Create the manifest**

Create `tests/fixtures/edit-eval-rust/Cargo.toml`:

```toml
[package]
name = "edit-eval-rust"
version = "0.0.0"
edition = "2021"
publish = false

# Standalone — intentionally not a workspace member of code-explorer.
# This crate is mutated in-place per eval case; isolation depends on
# `git restore` between cases.
[workspace]

[lib]
path = "src/lib.rs"
```

- [ ] **Step 2: Create the lib root**

Create `tests/fixtures/edit-eval-rust/src/lib.rs`:

```rust
//! Adversarial fixtures for the codescout edit_code eval.
//!
//! See `docs/superpowers/specs/2026-05-15-edit-code-eval-design.md`
//! for the case catalogue and rubric.

// Fixture modules will be declared as files land.
```

- [ ] **Step 3: Verify the crate compiles**

Run: `cargo check --manifest-path tests/fixtures/edit-eval-rust/Cargo.toml`
Expected: `Finished` with no warnings.

- [ ] **Step 4: Commit**

```bash
git add tests/fixtures/edit-eval-rust/
git commit -m "test(edit-eval): standalone fixture crate skeleton"
```

---

## Task 2: Extract `eval_common::verdict`

**Files:**
- Create: `tests/e2e/eval_common/mod.rs`
- Create: `tests/e2e/eval_common/verdict.rs`
- Modify: `tests/e2e/mod.rs` — add `pub mod eval_common;` before existing modules

- [ ] **Step 1: Write the verdict module**

Create `tests/e2e/eval_common/verdict.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Correct,
    Partial,
    CleanError,
    SilentWrong,
    Corrupt,
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
            Verdict::Corrupt => "CORRUPT",
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
        assert_eq!(Verdict::Partial.label(), "PARTIAL");
        assert_eq!(Verdict::CleanError.label(), "CLEAN_ERROR");
        assert_eq!(Verdict::SilentWrong.label(), "SILENT_WRONG");
        assert_eq!(Verdict::Corrupt.label(), "CORRUPT");
        assert_eq!(Verdict::Hung.label(), "HUNG");
        assert_eq!(Verdict::Panic.label(), "PANIC");
    }
}
```

- [ ] **Step 2: Write the module root**

Create `tests/e2e/eval_common/mod.rs`:

```rust
pub mod verdict;

pub use verdict::Verdict;
```

- [ ] **Step 3: Register the module**

Edit `tests/e2e/mod.rs` — add the line `pub mod eval_common;` at the top of the file, before any existing `pub mod` declarations.

- [ ] **Step 4: Verify the new module compiles and its tests pass**

Run: `cargo test --test e2e_tests eval_common::verdict::tests`
Expected: `7 passed` from the seven assertions; `0 failed`.

- [ ] **Step 5: Commit**

```bash
git add tests/e2e/eval_common/ tests/e2e/mod.rs
git commit -m "test(eval-common): extract Verdict enum with Corrupt variant"
```

---

## Task 3: Refactor `nav_eval` onto `eval_common::Verdict`

**Files:**
- Modify: `tests/e2e/nav_eval/types.rs` — remove local `Verdict` enum, re-export from `eval_common`
- Modify: `tests/e2e/nav_eval/runner.rs` — update import path
- Modify: `tests/e2e/nav_eval/matchers.rs` — update import path
- Modify: `tests/e2e/nav_eval/report.rs` — update import path

The local `Verdict` enum currently has 6 variants (no Corrupt). The shared enum has 7. Nav-eval does not produce Corrupt verdicts — that's fine, the variant is simply never constructed from nav code.

- [ ] **Step 1: Delete the local Verdict and its test**

In `tests/e2e/nav_eval/types.rs`, remove the `pub enum Verdict { ... }` block, its `impl Verdict { pub fn label ... }` block, and the `#[cfg(test)] mod tests { ... }` block. Add at the top:

```rust
pub use crate::e2e::eval_common::Verdict;
```

The final `types.rs` keeps only `ToolUnderTest`, `SymbolRef`, `RefLoc`, `Expected`, `Case`, and the new re-export.

- [ ] **Step 2: Verify no other nav_eval site duplicates the Verdict**

Run: `grep -rn "enum Verdict" tests/e2e/nav_eval/`
Expected: no matches (the only definition is now in `eval_common`).

- [ ] **Step 3: Verify nav_eval still builds**

Run: `cargo check --tests --test e2e_tests`
Expected: clean build.

- [ ] **Step 4: Verify nav-eval test infra still compiles**

Run: `cargo test --test e2e_tests nav_eval -- --list`
Expected: lists the harness entries; no compile errors.

- [ ] **Step 5: Commit**

```bash
git add tests/e2e/nav_eval/
git commit -m "test(nav-eval): re-route Verdict through eval_common"
```

---

## Task 4: Extract `eval_common::proc` helpers

**Files:**
- Create: `tests/e2e/eval_common/proc.rs`
- Modify: `tests/e2e/eval_common/mod.rs`

- [ ] **Step 1: Write the helpers module**

Create `tests/e2e/eval_common/proc.rs`:

```rust
use std::path::Path;
use std::process::{Command, Output};

/// Reset a fixture subtree to HEAD. Scoped — never call without a path.
///
/// The user's working tree may carry unrelated in-progress edits elsewhere;
/// a bare `git restore --` would clobber them.
pub fn git_restore<P: AsRef<Path>>(fixture_src: P) -> std::io::Result<Output> {
    Command::new("git")
        .arg("restore")
        .arg("--")
        .arg(fixture_src.as_ref())
        .output()
}

/// Run `cargo check` on a fixture crate. Returns Ok(()) on exit 0,
/// Err with stderr-summary on non-zero. Inherits the calling process's
/// stdout/stderr environment but does not propagate them.
pub fn cargo_check<P: AsRef<Path>>(fixture_root: P) -> Result<(), String> {
    let manifest = fixture_root.as_ref().join("Cargo.toml");
    let out = Command::new("cargo")
        .arg("check")
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--quiet")
        .output()
        .map_err(|e| format!("spawn cargo: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        let tail: String = String::from_utf8_lossy(&out.stderr)
            .lines()
            .rev()
            .take(20)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        Err(tail)
    }
}

/// Read a file relative to a fixture root. Returns None on I/O error
/// so the grader can report "disk read failed" rather than panicking.
pub fn read_fixture_file<P: AsRef<Path>>(fixture_root: P, rel: &str) -> Option<String> {
    std::fs::read_to_string(fixture_root.as_ref().join(rel)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn read_fixture_file_returns_none_on_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(read_fixture_file(tmp.path(), "nope.rs").is_none());
    }

    #[test]
    fn read_fixture_file_returns_content() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.rs"), "fn x() {}").unwrap();
        assert_eq!(read_fixture_file(tmp.path(), "a.rs").as_deref(), Some("fn x() {}"));
    }
}
```

- [ ] **Step 2: Add `tempfile` to dev-dependencies if not already present**

Check `Cargo.toml`:

```bash
grep "^tempfile" Cargo.toml
```

If no match in the `[dev-dependencies]` section, add it. Edit `Cargo.toml` `[dev-dependencies]` and add:

```toml
tempfile = "3"
```

- [ ] **Step 3: Re-export from the module root**

Edit `tests/e2e/eval_common/mod.rs`. Replace the file contents with:

```rust
pub mod proc;
pub mod verdict;

pub use proc::{cargo_check, git_restore, read_fixture_file};
pub use verdict::Verdict;
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test --test e2e_tests eval_common::proc::tests`
Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
git add tests/e2e/eval_common/ Cargo.toml Cargo.lock
git commit -m "test(eval-common): extract git_restore/cargo_check/read_fixture helpers"
```

---

## Task 5: Extract `eval_common::report`

**Files:**
- Create: `tests/e2e/eval_common/report.rs`
- Modify: `tests/e2e/eval_common/mod.rs`

The existing `nav_eval/report.rs` will keep its public surface; this task adds a shared rendering helper that both evals can use.

- [ ] **Step 1: Write the report module**

Create `tests/e2e/eval_common/report.rs`:

```rust
use super::Verdict;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Report {
    pub eval_name: &'static str,
    pub round: u32,
    rows: Vec<Row>,
}

#[derive(Debug)]
struct Row {
    id: &'static str,
    verdict: Verdict,
    evidence: String,
}

impl Report {
    pub fn new(eval_name: &'static str, round: u32) -> Self {
        Self {
            eval_name,
            round,
            rows: Vec::new(),
        }
    }

    pub fn push(&mut self, id: &'static str, verdict: Verdict, evidence: impl Into<String>) {
        self.rows.push(Row {
            id,
            verdict,
            evidence: evidence.into(),
        });
    }

    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "# {} — Round {}\n", self.eval_name, self.round);
        let mut counts = std::collections::BTreeMap::<&str, u32>::new();
        for r in &self.rows {
            *counts.entry(r.verdict.label()).or_default() += 1;
        }
        let _ = writeln!(out, "## Tally\n");
        let _ = writeln!(out, "| Verdict | Count |");
        let _ = writeln!(out, "|---|---:|");
        for (k, v) in &counts {
            let _ = writeln!(out, "| {k} | {v} |");
        }
        let _ = writeln!(out, "\n## Cases\n");
        let _ = writeln!(out, "| ID | Verdict | Evidence |");
        let _ = writeln!(out, "|---|---|---|");
        for r in &self.rows {
            let ev = r.evidence.replace('|', "\\|").replace('\n', " ");
            let ev = if ev.len() > 200 {
                format!("{}…", &ev[..200])
            } else {
                ev
            };
            let _ = writeln!(out, "| {} | {} | {} |", r.id, r.verdict.label(), ev);
        }
        out
    }

    pub fn rows_by_verdict(&self, v: &Verdict) -> Vec<&'static str> {
        self.rows
            .iter()
            .filter(|r| &r.verdict == v)
            .map(|r| r.id)
            .collect()
    }

    pub fn write_to<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.render())
    }
}

/// Determine the next round number for a given eval by counting existing
/// committed `2026-MM-DD-<eval>-round-N.md` files in `docs/superpowers/specs/`.
///
/// Returns 1 when no prior round file exists for `eval_slug`.
pub fn next_round_number(eval_slug: &str) -> u32 {
    let dir = PathBuf::from("docs/superpowers/specs");
    let mut max = 0u32;
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return 1;
    };
    let needle = format!("-{eval_slug}-round-");
    for e in entries.flatten() {
        let name = e.file_name();
        let Some(s) = name.to_str() else { continue };
        if let Some(idx) = s.find(&needle) {
            let tail = &s[idx + needle.len()..];
            let num: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = num.parse::<u32>() {
                if n > max {
                    max = n;
                }
            }
        }
    }
    max + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_has_header_and_empty_tally() {
        let r = Report::new("edit_eval", 1);
        let out = r.render();
        assert!(out.starts_with("# edit_eval — Round 1"));
        assert!(out.contains("## Tally"));
        assert!(out.contains("## Cases"));
    }

    #[test]
    fn report_groups_verdicts() {
        let mut r = Report::new("edit_eval", 1);
        r.push("R-01", Verdict::Correct, "ok");
        r.push("R-02", Verdict::SilentWrong, "stray }");
        let out = r.render();
        assert!(out.contains("| CORRECT | 1 |"));
        assert!(out.contains("| SILENT_WRONG | 1 |"));
        assert!(out.contains("| R-01 | CORRECT | ok |"));
    }

    #[test]
    fn report_escapes_pipes_and_newlines() {
        let mut r = Report::new("edit_eval", 1);
        r.push("R-03", Verdict::Correct, "left | right\nnext");
        let out = r.render();
        assert!(out.contains("left \\| right next"));
    }

    #[test]
    fn next_round_returns_one_when_no_dir() {
        // Run in a tmpdir cwd to ensure the docs path can't exist
        let tmp = tempfile::TempDir::new().unwrap();
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(tmp.path()).unwrap();
        let n = next_round_number("nonexistent-eval");
        std::env::set_current_dir(orig).unwrap();
        assert_eq!(n, 1);
    }
}
```

- [ ] **Step 2: Re-export from module root**

Edit `tests/e2e/eval_common/mod.rs`. Replace the file contents:

```rust
pub mod proc;
pub mod report;
pub mod verdict;

pub use proc::{cargo_check, git_restore, read_fixture_file};
pub use report::{next_round_number, Report};
pub use verdict::Verdict;
```

- [ ] **Step 3: Run the new tests**

Run: `cargo test --test e2e_tests eval_common::report::tests`
Expected: `4 passed`.

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/eval_common/
git commit -m "test(eval-common): extract Report with composite verdict rendering"
```

---

## Task 6: `edit_eval` types

**Files:**
- Create: `tests/e2e/edit_eval/mod.rs`
- Create: `tests/e2e/edit_eval/types.rs`
- Modify: `tests/e2e/mod.rs` — add `pub mod edit_eval;`

- [ ] **Step 1: Write the types**

Create `tests/e2e/edit_eval/types.rs`:

```rust
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditAction {
    Replace,
    Insert,
    Remove,
    Rename,
}

#[derive(Debug, Clone)]
pub enum ReturnExpected {
    Ok,
    CleanError, // RecoverableError downcast
}

#[derive(Debug, Clone)]
pub enum CompilerExpected {
    Builds,
    Breaks, // intentional — case demonstrates tool faithfulness, not semantic protection
    DontCare,
}

/// A content invariant the post-edit fixture file must satisfy.
/// Multiple invariants are AND-ed.
#[derive(Debug, Clone)]
pub enum ContentInvariant {
    /// The post-edit content of `file` must contain `needle` exactly `count` times.
    Contains { file: &'static str, needle: &'static str, count: usize },
    /// The post-edit content of `file` must NOT contain `needle`.
    NotContains { file: &'static str, needle: &'static str },
    /// A specific byte-range or line-range must equal exact text.
    /// Used sparingly — narrow assertions over broad ones.
    LineEquals { file: &'static str, line: u32, text: &'static str },
}

#[derive(Debug, Clone)]
pub struct Expected {
    pub return_: ReturnExpected,
    pub disk: Vec<ContentInvariant>,
    pub compiler: CompilerExpected,
}

#[derive(Debug, Clone)]
pub struct EditCase {
    pub id: &'static str,
    pub action: EditAction,
    pub input: Value,
    /// Fixture file the edit targets — used so the grader knows which file
    /// to read from disk. Relative to fixture src/.
    pub target_file: &'static str,
    pub expected: Expected,
    pub rationale: &'static str,
    /// If Some, this case is exempt from H1 hard-gate failure with the given
    /// LIMIT-comment reason. Used for BUG-054 sentinel R-02.
    pub h1_exempt: Option<&'static str>,
}
```

- [ ] **Step 2: Write the module root**

Create `tests/e2e/edit_eval/mod.rs`:

```rust
pub mod types;

// matchers, runner, cases land in later tasks.
```

- [ ] **Step 3: Register module**

Edit `tests/e2e/mod.rs` — add `pub mod edit_eval;` line, beside the existing `pub mod nav_eval;` (or wherever nav_eval is declared).

- [ ] **Step 4: Verify it compiles**

Run: `cargo check --tests --test e2e_tests`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add tests/e2e/edit_eval/ tests/e2e/mod.rs
git commit -m "test(edit-eval): module skeleton + EditCase/Expected types"
```

---

## Task 7: `edit_eval::matchers`

**Files:**
- Create: `tests/e2e/edit_eval/matchers.rs`
- Modify: `tests/e2e/edit_eval/mod.rs` — add `pub mod matchers;`

- [ ] **Step 1: Write the matcher**

Create `tests/e2e/edit_eval/matchers.rs`:

```rust
use crate::e2e::edit_eval::types::{
    CompilerExpected, ContentInvariant, EditCase, Expected, ReturnExpected,
};
use crate::e2e::eval_common::Verdict;

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub verdict: Verdict,
    pub evidence: String,
}

pub struct Observation<'a> {
    pub return_: ReturnObservation<'a>,
    pub disk: Option<&'a str>,
    pub compiler_ok: bool,
}

pub enum ReturnObservation<'a> {
    Ok(&'a serde_json::Value),
    Recoverable(&'a str),
    Fatal(&'a str),
    TransientLsp(&'a str),
}

pub fn grade(case: &EditCase, obs: Observation<'_>) -> MatchResult {
    // Fatal / transient — short-circuit before considering disk.
    match &obs.return_ {
        ReturnObservation::Fatal(msg) => {
            return MatchResult {
                verdict: Verdict::Panic,
                evidence: format!("fatal: {msg}"),
            }
        }
        ReturnObservation::TransientLsp(msg) => {
            return MatchResult {
                verdict: Verdict::SilentWrong,
                evidence: format!("transient LSP (retryable): {msg}"),
            }
        }
        _ => {}
    }

    let return_ok = match (&case.expected.return_, &obs.return_) {
        (ReturnExpected::Ok, ReturnObservation::Ok(_)) => true,
        (ReturnExpected::CleanError, ReturnObservation::Recoverable(_)) => true,
        _ => false,
    };

    if !return_ok {
        let got = match &obs.return_ {
            ReturnObservation::Ok(_) => "Ok",
            ReturnObservation::Recoverable(_) => "RecoverableError",
            _ => "?",
        };
        let want = match case.expected.return_ {
            ReturnExpected::Ok => "Ok",
            ReturnExpected::CleanError => "RecoverableError",
        };
        return MatchResult {
            verdict: match (&case.expected.return_, &obs.return_) {
                (ReturnExpected::CleanError, ReturnObservation::Ok(_)) => Verdict::SilentWrong,
                (ReturnExpected::Ok, ReturnObservation::Recoverable(_)) => Verdict::CleanError,
                _ => Verdict::SilentWrong,
            },
            evidence: format!("return: want {want}, got {got}"),
        };
    }

    // Disk invariants — only checked when return matched.
    let disk_violation = match obs.disk {
        Some(content) => first_disk_violation(content, &case.expected.disk, case.target_file),
        None if case.expected.disk.is_empty() => None,
        None => Some(String::from("disk content unreadable; case expected invariants")),
    };

    if let Some(v) = disk_violation {
        return MatchResult {
            verdict: Verdict::SilentWrong,
            evidence: format!("disk: {v}"),
        };
    }

    // Compiler oracle — graded last.
    let compiler_match = match case.expected.compiler {
        CompilerExpected::Builds => obs.compiler_ok,
        CompilerExpected::Breaks => !obs.compiler_ok,
        CompilerExpected::DontCare => true,
    };

    if !compiler_match {
        let got = if obs.compiler_ok { "builds" } else { "breaks" };
        let want = match case.expected.compiler {
            CompilerExpected::Builds => "builds",
            CompilerExpected::Breaks => "breaks",
            CompilerExpected::DontCare => "n/a",
        };
        return MatchResult {
            verdict: Verdict::Corrupt,
            evidence: format!("compiler: want {want}, got {got}"),
        };
    }

    MatchResult {
        verdict: Verdict::Correct,
        evidence: String::from("triplet matched"),
    }
}

fn first_disk_violation(content: &str, invs: &[ContentInvariant], default_file: &str) -> Option<String> {
    for inv in invs {
        match inv {
            ContentInvariant::Contains { file, needle, count } => {
                let _ = file; // single-file fixtures common; multi-file rename overrides target_file
                let _ = default_file;
                let actual = content.matches(needle).count();
                if actual != *count {
                    return Some(format!("needle {needle:?} appears {actual}× (want {count}×)"));
                }
            }
            ContentInvariant::NotContains { needle, .. } => {
                if content.contains(needle) {
                    return Some(format!("forbidden needle {needle:?} present"));
                }
            }
            ContentInvariant::LineEquals { line, text, .. } => {
                let got = content.lines().nth((*line as usize).saturating_sub(1));
                if got.map(|l| l.trim_end()) != Some(text) {
                    return Some(format!(
                        "line {line}: want {text:?}, got {:?}",
                        got.unwrap_or("<missing>")
                    ));
                }
            }
        }
    }
    None
}

/// True when the case's H1 exemption fires for this verdict (the documented
/// LIMIT bug). Used by the harness to skip H1 hard-gate failure for that row.
pub fn h1_exempt_for(case: &EditCase, v: &Verdict) -> bool {
    match (&case.h1_exempt, v) {
        (Some(_), Verdict::SilentWrong) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2e::edit_eval::types::{EditAction, Expected};

    fn case_ok_disk(invs: Vec<ContentInvariant>) -> EditCase {
        EditCase {
            id: "T",
            action: EditAction::Replace,
            input: serde_json::json!({}),
            target_file: "x.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: invs,
                compiler: CompilerExpected::Builds,
            },
            rationale: "test",
            h1_exempt: None,
        }
    }

    #[test]
    fn correct_when_all_three_match() {
        let c = case_ok_disk(vec![ContentInvariant::Contains {
            file: "x.rs",
            needle: "fn foo",
            count: 1,
        }]);
        let v = serde_json::json!({"ok": true});
        let obs = Observation {
            return_: ReturnObservation::Ok(&v),
            disk: Some("fn foo() {}"),
            compiler_ok: true,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::Correct);
    }

    #[test]
    fn silent_wrong_when_expected_cleanerror_got_ok() {
        let mut c = case_ok_disk(vec![]);
        c.expected.return_ = ReturnExpected::CleanError;
        let v = serde_json::json!({});
        let obs = Observation {
            return_: ReturnObservation::Ok(&v),
            disk: None,
            compiler_ok: true,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::SilentWrong);
    }

    #[test]
    fn cleanerror_when_expected_ok_got_recoverable() {
        let c = case_ok_disk(vec![]);
        let obs = Observation {
            return_: ReturnObservation::Recoverable("dropped definition"),
            disk: None,
            compiler_ok: true,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::CleanError);
    }

    #[test]
    fn corrupt_when_return_ok_but_compiler_breaks_unexpectedly() {
        let c = case_ok_disk(vec![]);
        let v = serde_json::json!({});
        let obs = Observation {
            return_: ReturnObservation::Ok(&v),
            disk: None,
            compiler_ok: false,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::Corrupt);
    }

    #[test]
    fn breaks_expected_grades_correct_when_compiler_breaks() {
        let mut c = case_ok_disk(vec![]);
        c.expected.compiler = CompilerExpected::Breaks;
        let v = serde_json::json!({});
        let obs = Observation {
            return_: ReturnObservation::Ok(&v),
            disk: None,
            compiler_ok: false,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::Correct);
    }

    #[test]
    fn panic_when_fatal_error_short_circuits() {
        let c = case_ok_disk(vec![]);
        let obs = Observation {
            return_: ReturnObservation::Fatal("boom"),
            disk: None,
            compiler_ok: true,
        };
        assert_eq!(grade(&c, obs).verdict, Verdict::Panic);
    }

    #[test]
    fn h1_exempt_fires_only_for_silent_wrong() {
        let mut c = case_ok_disk(vec![]);
        c.h1_exempt = Some("BUG-054");
        assert!(h1_exempt_for(&c, &Verdict::SilentWrong));
        assert!(!h1_exempt_for(&c, &Verdict::Correct));
        c.h1_exempt = None;
        assert!(!h1_exempt_for(&c, &Verdict::SilentWrong));
    }
}
```

- [ ] **Step 2: Wire the module**

Edit `tests/e2e/edit_eval/mod.rs`. Replace contents:

```rust
pub mod matchers;
pub mod types;
```

- [ ] **Step 3: Run the unit tests**

Run: `cargo test --test e2e_tests edit_eval::matchers::tests`
Expected: `7 passed`.

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/edit_eval/
git commit -m "test(edit-eval): three-observable grader (return + disk + compiler)"
```

---

## Task 8: `edit_eval::runner`

**Files:**
- Create: `tests/e2e/edit_eval/runner.rs`
- Modify: `tests/e2e/edit_eval/mod.rs`

- [ ] **Step 1: Write the runner**

Create `tests/e2e/edit_eval/runner.rs`:

```rust
use crate::e2e::edit_eval::matchers::{grade, MatchResult, Observation, ReturnObservation};
use crate::e2e::edit_eval::types::EditCase;
use crate::e2e::eval_common::{cargo_check, git_restore, read_fixture_file, Verdict};
use codescout::agent::Agent;
use codescout::lsp::manager::LspManager;
use codescout::tools::symbol::EditCode;
use codescout::tools::{output_buffer::OutputBuffer, section_coverage::SectionCoverage, Tool, ToolContext};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const CASE_TIMEOUT: Duration = Duration::from_secs(45);

pub struct EditEvalCtx {
    pub tool_ctx: Arc<ToolContext>,
    pub fixture_root: PathBuf,
    pub fixture_src: PathBuf,
}

pub async fn edit_eval_context() -> EditEvalCtx {
    let fixture_root: PathBuf = std::env::current_dir()
        .expect("cwd")
        .join("tests/fixtures/edit-eval-rust");
    assert!(
        fixture_root.exists(),
        "edit-eval fixture missing: {}",
        fixture_root.display()
    );
    let fixture_src = fixture_root.join("src");

    // Warm the cargo target dir so per-case checks are fast.
    cargo_check(&fixture_root).expect("pre-flight cargo check on edit-eval fixture");

    let agent = Agent::new(Some(fixture_root.clone()))
        .await
        .expect("Agent::new for edit-eval");
    let lsp = LspManager::new_arc();

    let tool_ctx = Arc::new(ToolContext {
        agent,
        lsp,
        output_buffer: Arc::new(OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(Mutex::new(SectionCoverage::new())),
    });

    EditEvalCtx {
        tool_ctx,
        fixture_root,
        fixture_src,
    }
}

pub async fn run_one(ctx: &EditEvalCtx, case: &EditCase) -> MatchResult {
    let mut last = MatchResult {
        verdict: Verdict::SilentWrong,
        evidence: String::from("no attempts ran"),
    };
    for attempt in 0..6u64 {
        if attempt > 0 {
            tokio::time::sleep(Duration::from_millis(500 * attempt)).await;
            let _ = git_restore(&ctx.fixture_src);
        }
        let fut = invoke(&ctx.tool_ctx, case);
        let result = match tokio::time::timeout(CASE_TIMEOUT, fut).await {
            Err(_) => {
                return MatchResult {
                    verdict: Verdict::Hung,
                    evidence: format!("exceeded {}s", CASE_TIMEOUT.as_secs()),
                }
            }
            Ok(r) => r,
        };

        let disk = read_fixture_file(&ctx.fixture_src, case.target_file);
        let compiler_ok = cargo_check(&ctx.fixture_root).is_ok();

        let value_holder;
        let return_obs = match &result {
            Ok(v) => {
                value_holder = v.clone();
                ReturnObservation::Ok(unsafe { std::mem::transmute(&value_holder) })
            }
            Err(e) => {
                let msg = format!("{e}");
                if e.downcast_ref::<codescout::tools::RecoverableError>().is_some() {
                    ReturnObservation::Recoverable(Box::leak(msg.into_boxed_str()))
                } else if msg.contains("content modified") || msg.contains("-32801") {
                    ReturnObservation::TransientLsp(Box::leak(msg.into_boxed_str()))
                } else {
                    ReturnObservation::Fatal(Box::leak(msg.into_boxed_str()))
                }
            }
        };

        let obs = Observation {
            return_: return_obs,
            disk: disk.as_deref(),
            compiler_ok,
        };
        let candidate = grade(case, obs);
        match candidate.verdict {
            Verdict::Correct | Verdict::Partial | Verdict::CleanError | Verdict::Panic => {
                return candidate;
            }
            _ => last = candidate,
        }
    }
    last
}

async fn invoke(ctx: &ToolContext, case: &EditCase) -> anyhow::Result<serde_json::Value> {
    EditCode.call(case.input.clone(), ctx).await
}
```

Note on the unsafe transmute: `ReturnObservation::Ok` borrows a `&Value`. The owned `value_holder` lives for the full match scope and is dropped only after `grade` returns. The lifetime extension is sound for this call shape; if you'd rather avoid `unsafe`, switch `ReturnObservation::Ok` to own a `Value` (clone is cheap for our small payloads) — the choice is left to taste.

- [ ] **Step 2: Wire the module**

Edit `tests/e2e/edit_eval/mod.rs`:

```rust
pub mod matchers;
pub mod runner;
pub mod types;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check --tests --test e2e_tests`
Expected: clean. The runner has no unit tests of its own; it is exercised by the harness.

- [ ] **Step 4: Commit**

```bash
git add tests/e2e/edit_eval/
git commit -m "test(edit-eval): runner with cargo-check oracle and git-restore isolation"
```

---

## Task 9: Empty `cases` slice + harness entrypoint

**Files:**
- Create: `tests/e2e/edit_eval/cases.rs`
- Create: `tests/e2e/edit_eval_harness.rs`
- Modify: `tests/e2e/edit_eval/mod.rs`

- [ ] **Step 1: Empty cases slice**

Create `tests/e2e/edit_eval/cases.rs`:

```rust
use crate::e2e::edit_eval::types::EditCase;
use std::sync::OnceLock;

static CASES: OnceLock<Vec<EditCase>> = OnceLock::new();

pub fn all() -> &'static [EditCase] {
    CASES.get_or_init(Vec::new)
}
```

- [ ] **Step 2: Wire the module**

Edit `tests/e2e/edit_eval/mod.rs`:

```rust
pub mod cases;
pub mod matchers;
pub mod runner;
pub mod types;
```

- [ ] **Step 3: Write the harness entrypoint**

Create `tests/e2e/edit_eval_harness.rs`:

```rust
use crate::e2e::edit_eval::{cases, runner};
use crate::e2e::edit_eval::matchers::h1_exempt_for;
use crate::e2e::eval_common::{git_restore, next_round_number, Report, Verdict};

#[tokio::test]
#[ignore]
async fn edit_eval_harness() {
    let ctx = runner::edit_eval_context().await;
    let round = next_round_number("edit-eval");
    let mut report = Report::new("edit_eval", round);

    for case in cases::all() {
        let _ = git_restore(&ctx.fixture_src);
        let r = runner::run_one(&ctx, case).await;
        report.push(case.id, r.verdict.clone(), r.evidence.clone());
        // Always restore after a case so the next pre-edit check sees a clean tree.
        let _ = git_restore(&ctx.fixture_src);
    }

    let path = format!("docs/superpowers/specs/2026-05-15-edit-eval-round-{round}.md");
    report.write_to(&path).expect("write round file");
    println!("wrote {path}");

    // Hard gates
    let mut failures: Vec<String> = Vec::new();
    for case in cases::all() {
        // Recompute per-case verdict from rows
        let row_verdict = report.rows_by_verdict(&Verdict::SilentWrong).contains(&case.id);
        let row_corrupt = report.rows_by_verdict(&Verdict::Corrupt).contains(&case.id);
        let row_panic = report.rows_by_verdict(&Verdict::Panic).contains(&case.id);

        if row_panic {
            failures.push(format!("{}: PANIC (H2)", case.id));
            continue;
        }
        if (row_verdict || row_corrupt)
            && !h1_exempt_for(case, &Verdict::SilentWrong)
            && !h1_exempt_for(case, &Verdict::Corrupt)
        {
            failures.push(format!(
                "{}: unexpected destructive verdict (H1)",
                case.id
            ));
        }
    }

    if !failures.is_empty() {
        panic!(
            "edit_eval hard-gate failures:\n  {}\nFull round file: {path}",
            failures.join("\n  ")
        );
    }
}
```

- [ ] **Step 4: Register the harness file**

Edit `tests/e2e/mod.rs` and ensure the harness file is included. The pattern from nav-eval: harness files are siblings of the module, so `tests/e2e/mod.rs` needs:

```rust
pub mod edit_eval;
pub mod edit_eval_harness;
pub mod eval_common;
pub mod nav_eval;
pub mod nav_eval_harness;
// ... (existing entries below)
```

- [ ] **Step 5: Verify harness lists**

Run: `cargo test --test e2e_tests edit_eval_harness -- --list`
Expected: lists `edit_eval_harness::edit_eval_harness: test`. Does not fail on empty case slice.

- [ ] **Step 6: Run the harness with zero cases**

Run: `cargo test --test e2e_tests edit_eval_harness -- --ignored --nocapture`
Expected: writes `docs/superpowers/specs/2026-05-15-edit-eval-round-1.md` with empty tally + empty cases table; harness passes (no hard-gate failures since no cases).

Inspect the file: it should have the header and empty tables. Delete it afterwards — it will be regenerated when cases land.

```bash
rm docs/superpowers/specs/2026-05-15-edit-eval-round-1.md
```

- [ ] **Step 7: Commit**

```bash
git add tests/e2e/edit_eval/cases.rs tests/e2e/edit_eval_harness.rs tests/e2e/mod.rs
git commit -m "test(edit-eval): harness entrypoint with empty case slice"
```

---

## Task 10: Replace fixtures + cases (R-01..R-08)

This task is large because cases reference exact lines and content of fixture files. Each fixture must be authored to compile cleanly pre-edit, and each case must reference symbol paths/lines that exist.

**Files:**
- Create: `tests/fixtures/edit-eval-rust/src/replace_plain.rs`
- Create: `tests/fixtures/edit-eval-rust/src/replace_trait_impl.rs`
- Create: `tests/fixtures/edit-eval-rust/src/replace_generic.rs`
- Create: `tests/fixtures/edit-eval-rust/src/replace_tight_impl.rs`
- Create: `tests/fixtures/edit-eval-rust/src/replace_no_sig.rs`
- Create: `tests/fixtures/edit-eval-rust/src/replace_wrong_sig.rs`
- Create: `tests/fixtures/edit-eval-rust/src/replace_nested.rs`
- Create: `tests/fixtures/edit-eval-rust/src/replace_doc_adj.rs`
- Modify: `tests/fixtures/edit-eval-rust/src/lib.rs`
- Modify: `tests/e2e/edit_eval/cases.rs`

- [ ] **Step 1: Author replace fixtures**

Create `tests/fixtures/edit-eval-rust/src/replace_plain.rs`:

```rust
pub fn compute(x: i32) -> i32 {
    x + 1
}
```

Create `tests/fixtures/edit-eval-rust/src/replace_trait_impl.rs`:

```rust
pub trait Greeter {
    fn greet(&self) -> String;
}

pub struct Foo;

impl Greeter for Foo {
    fn greet(&self) -> String {
        String::from("hello")
    }
}
```

Create `tests/fixtures/edit-eval-rust/src/replace_generic.rs`:

```rust
use std::fmt::Display;
use std::str::FromStr;

pub fn parse<T>(s: &str) -> Option<T>
where
    T: FromStr + Display + 'static,
{
    s.parse().ok()
}
```

Create `tests/fixtures/edit-eval-rust/src/replace_tight_impl.rs`:

```rust
pub struct Counter(u32);

impl Counter {
    pub fn a(&self) -> u32 { self.0 }
    pub fn b(&self) -> u32 { self.0 * 2 }
    pub fn c(&self) -> u32 { self.0 * 3 }
}
```

Create `tests/fixtures/edit-eval-rust/src/replace_no_sig.rs`:

```rust
pub fn missing_sig() -> u32 {
    42
}
```

Create `tests/fixtures/edit-eval-rust/src/replace_wrong_sig.rs`:

```rust
pub fn foo() -> u32 {
    1
}
```

Create `tests/fixtures/edit-eval-rust/src/replace_nested.rs`:

```rust
pub fn outer() -> i32 {
    fn inner() -> i32 {
        7
    }
    inner()
}
```

Create `tests/fixtures/edit-eval-rust/src/replace_doc_adj.rs`:

```rust
/// Doc that lives immediately above the target with no blank line.
pub fn documented() -> &'static str {
    "before"
}
```

- [ ] **Step 2: Declare modules**

Edit `tests/fixtures/edit-eval-rust/src/lib.rs`. Replace contents:

```rust
//! Adversarial fixtures for the codescout edit_code eval.

pub mod replace_doc_adj;
pub mod replace_generic;
pub mod replace_nested;
pub mod replace_no_sig;
pub mod replace_plain;
pub mod replace_tight_impl;
pub mod replace_trait_impl;
pub mod replace_wrong_sig;
```

- [ ] **Step 3: Verify fixture compiles**

Run: `cargo check --manifest-path tests/fixtures/edit-eval-rust/Cargo.toml`
Expected: clean.

- [ ] **Step 4: Author the 8 replace cases**

Edit `tests/e2e/edit_eval/cases.rs`. Replace contents:

```rust
use crate::e2e::edit_eval::types::{
    CompilerExpected, ContentInvariant, EditAction, EditCase, Expected, ReturnExpected,
};
use serde_json::json;
use std::sync::OnceLock;

static CASES: OnceLock<Vec<EditCase>> = OnceLock::new();

pub fn all() -> &'static [EditCase] {
    CASES.get_or_init(|| vec![
        EditCase {
            id: "R-01",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "compute",
                "path": "src/replace_plain.rs",
                "body": "pub fn compute(x: i32) -> i32 {\n    x * 2\n}",
            }),
            target_file: "replace_plain.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_plain.rs", needle: "x * 2", count: 1 },
                    ContentInvariant::NotContains { file: "replace_plain.rs", needle: "x + 1" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "plain function body replace — happy-path baseline",
            h1_exempt: None,
        },
        EditCase {
            id: "R-02",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "impl Greeter for Foo/greet",
                "path": "src/replace_trait_impl.rs",
                "body": "    fn greet(&self) -> String {\n        String::from(\"hi\")\n    }",
            }),
            target_file: "replace_trait_impl.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_trait_impl.rs", needle: "String::from(\"hi\")", count: 1 },
                    // BUG-054 watchdog: stray `}` would produce three closing braces near EOF.
                    // A clean replace leaves exactly two: the impl method's `}` and the impl block's `}`.
                    ContentInvariant::NotContains { file: "replace_trait_impl.rs", needle: "}\n}\n}" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "trait-method body — BUG-054 stray-brace watchdog",
            h1_exempt: Some("BUG-054"),
        },
        EditCase {
            id: "R-03",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "parse",
                "path": "src/replace_generic.rs",
                "body": "pub fn parse<T>(s: &str) -> Option<T>\nwhere\n    T: FromStr + Display + 'static,\n{\n    let trimmed = s.trim();\n    trimmed.parse().ok()\n}",
            }),
            target_file: "replace_generic.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_generic.rs", needle: "s.trim()", count: 1 },
                    ContentInvariant::Contains { file: "replace_generic.rs", needle: "where", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "generic fn with where bounds — preserve signature shape",
            h1_exempt: None,
        },
        EditCase {
            id: "R-04",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "impl Counter/b",
                "path": "src/replace_tight_impl.rs",
                "body": "    pub fn b(&self) -> u32 { self.0 * 4 }",
            }),
            target_file: "replace_tight_impl.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_tight_impl.rs", needle: "self.0 * 4", count: 1 },
                    // Siblings must be intact.
                    ContentInvariant::Contains { file: "replace_tight_impl.rs", needle: "pub fn a(&self) -> u32 { self.0 }", count: 1 },
                    ContentInvariant::Contains { file: "replace_tight_impl.rs", needle: "pub fn c(&self) -> u32 { self.0 * 3 }", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "tight impl block — sibling methods must remain intact",
            h1_exempt: None,
        },
        EditCase {
            id: "R-05",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "missing_sig",
                "path": "src/replace_no_sig.rs",
                // Body lacks `fn missing_sig` line — must be refused.
                "body": "    99",
            }),
            target_file: "replace_no_sig.rs",
            expected: Expected {
                return_: ReturnExpected::CleanError,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_no_sig.rs", needle: "42", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "body omits signature — must produce CleanError",
            h1_exempt: None,
        },
        EditCase {
            id: "R-06",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "foo",
                "path": "src/replace_wrong_sig.rs",
                "body": "pub fn bar() -> u32 {\n    2\n}",
            }),
            target_file: "replace_wrong_sig.rs",
            expected: Expected {
                return_: ReturnExpected::CleanError,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_wrong_sig.rs", needle: "pub fn foo", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "body has different signature than target — must refuse",
            h1_exempt: None,
        },
        EditCase {
            id: "R-07",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "outer/inner",
                "path": "src/replace_nested.rs",
                "body": "    fn inner() -> i32 {\n        9\n    }",
            }),
            target_file: "replace_nested.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_nested.rs", needle: "9", count: 1 },
                    ContentInvariant::NotContains { file: "replace_nested.rs", needle: "7" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "nested function — symbol path resolves inside outer fn",
            h1_exempt: None,
        },
        EditCase {
            id: "R-08",
            action: EditAction::Replace,
            input: json!({
                "action": "replace",
                "symbol": "documented",
                "path": "src/replace_doc_adj.rs",
                "body": "pub fn documented() -> &'static str {\n    \"after\"\n}",
            }),
            target_file: "replace_doc_adj.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "replace_doc_adj.rs", needle: "\"after\"", count: 1 },
                    // Doc comment must survive.
                    ContentInvariant::Contains { file: "replace_doc_adj.rs", needle: "/// Doc that lives immediately above", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "doc-comment-adjacent function — doc must survive replace",
            h1_exempt: None,
        },
    ])
}
```

- [ ] **Step 5: Run the harness**

Run: `cargo test --test e2e_tests edit_eval_harness -- --ignored --nocapture`
Expected: 8 cases run, round file appears in `docs/superpowers/specs/`. Verdicts may include unexpected SILENT_WRONG / CORRUPT — that's the point. R-02 may grade SILENT_WRONG (BUG-054 sentinel) and the H1 exemption should keep the harness from panicking on it.

- [ ] **Step 6: Inspect the round file**

```bash
cat docs/superpowers/specs/2026-05-15-edit-eval-round-1.md
```

- [ ] **Step 7: Commit fixtures + cases + round file**

```bash
git add tests/fixtures/edit-eval-rust/src/replace_*.rs tests/fixtures/edit-eval-rust/src/lib.rs tests/e2e/edit_eval/cases.rs docs/superpowers/specs/2026-05-15-edit-eval-round-1.md
git commit -m "test(edit-eval): replace cluster (R-01..R-08) + round 1"
```

---

## Task 11: Insert fixtures + cases (I-01..I-03)

**Files:**
- Create: `tests/fixtures/edit-eval-rust/src/insert_before_first.rs`
- Create: `tests/fixtures/edit-eval-rust/src/insert_after_last.rs`
- Create: `tests/fixtures/edit-eval-rust/src/insert_bad_syntax.rs`
- Modify: `tests/fixtures/edit-eval-rust/src/lib.rs`
- Modify: `tests/e2e/edit_eval/cases.rs`

- [ ] **Step 1: Author fixtures**

Create `tests/fixtures/edit-eval-rust/src/insert_before_first.rs`:

```rust
pub struct Foo;

impl Foo {
    pub fn method_a(&self) -> u32 { 1 }
    pub fn method_b(&self) -> u32 { 2 }
}
```

Create `tests/fixtures/edit-eval-rust/src/insert_after_last.rs`:

```rust
pub struct Bar;

impl Bar {
    pub fn method_y(&self) -> u32 { 24 }
    pub fn method_z(&self) -> u32 { 25 }
}
```

Create `tests/fixtures/edit-eval-rust/src/insert_bad_syntax.rs`:

```rust
pub fn target() -> u32 {
    0
}
```

- [ ] **Step 2: Add module declarations**

Edit `tests/fixtures/edit-eval-rust/src/lib.rs`. Add the three new modules in alphabetical position:

```rust
//! Adversarial fixtures for the codescout edit_code eval.

pub mod insert_after_last;
pub mod insert_bad_syntax;
pub mod insert_before_first;
pub mod replace_doc_adj;
pub mod replace_generic;
pub mod replace_nested;
pub mod replace_no_sig;
pub mod replace_plain;
pub mod replace_tight_impl;
pub mod replace_trait_impl;
pub mod replace_wrong_sig;
```

- [ ] **Step 3: Verify fixture compiles**

Run: `cargo check --manifest-path tests/fixtures/edit-eval-rust/Cargo.toml`
Expected: clean.

- [ ] **Step 4: Add insert cases**

Edit `tests/e2e/edit_eval/cases.rs`. Inside the `vec![...]` literal, append after the R-08 case (before the closing `])`):

```rust
        EditCase {
            id: "I-01",
            action: EditAction::Insert,
            input: json!({
                "action": "insert",
                "symbol": "impl Foo/method_a",
                "path": "src/insert_before_first.rs",
                "position": "before",
                "body": "    pub fn method_zero(&self) -> u32 { 0 }",
            }),
            target_file: "insert_before_first.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "insert_before_first.rs", needle: "pub fn method_zero", count: 1 },
                    ContentInvariant::Contains { file: "insert_before_first.rs", needle: "pub fn method_a(&self) -> u32 { 1 }", count: 1 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "insert before first method of impl — sibling method_a intact",
            h1_exempt: None,
        },
        EditCase {
            id: "I-02",
            action: EditAction::Insert,
            input: json!({
                "action": "insert",
                "symbol": "impl Bar/method_z",
                "path": "src/insert_after_last.rs",
                "position": "after",
                "body": "    pub fn method_zz(&self) -> u32 { 26 }",
            }),
            target_file: "insert_after_last.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "insert_after_last.rs", needle: "method_zz", count: 1 },
                    // No duplicated/consumed impl closing brace.
                    ContentInvariant::Contains { file: "insert_after_last.rs", needle: "}\n", count: 3 },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "insert after last method at EOF — impl close brace preserved",
            h1_exempt: None,
        },
        EditCase {
            id: "I-03",
            action: EditAction::Insert,
            input: json!({
                "action": "insert",
                "symbol": "target",
                "path": "src/insert_bad_syntax.rs",
                "position": "after",
                "body": "this is not rust",
            }),
            target_file: "insert_bad_syntax.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "insert_bad_syntax.rs", needle: "this is not rust", count: 1 },
                ],
                compiler: CompilerExpected::Breaks,
            },
            rationale: "tool-faithful contract — disk gets exactly what was asked; compiler legitimately breaks",
            h1_exempt: None,
        },
```

- [ ] **Step 5: Run the harness**

Run: `cargo test --test e2e_tests edit_eval_harness -- --ignored --nocapture`
Expected: 11 cases now run; I-01 / I-02 likely CORRECT, I-03 should grade CORRECT (composite expected matches observed).

- [ ] **Step 6: Inspect updated round file**

```bash
cat docs/superpowers/specs/2026-05-15-edit-eval-round-2.md
```

A new round file appears because `next_round_number` saw round-1.

- [ ] **Step 7: Commit**

```bash
git add tests/fixtures/edit-eval-rust/src/insert_*.rs tests/fixtures/edit-eval-rust/src/lib.rs tests/e2e/edit_eval/cases.rs docs/superpowers/specs/2026-05-15-edit-eval-round-2.md
git commit -m "test(edit-eval): insert cluster (I-01..I-03) + round 2"
```

---

## Task 12: Remove + Rename fixtures + cases (M-01, M-02, N-01)

**Files:**
- Create: `tests/fixtures/edit-eval-rust/src/remove_clean.rs`
- Create: `tests/fixtures/edit-eval-rust/src/remove_referenced.rs`
- Create: `tests/fixtures/edit-eval-rust/src/rename_target.rs`
- Create: `tests/fixtures/edit-eval-rust/src/rename_caller_a.rs`
- Create: `tests/fixtures/edit-eval-rust/src/rename_caller_b.rs`
- Modify: `tests/fixtures/edit-eval-rust/src/lib.rs`
- Modify: `tests/e2e/edit_eval/cases.rs`

- [ ] **Step 1: Author fixtures**

Create `tests/fixtures/edit-eval-rust/src/remove_clean.rs`:

```rust
pub fn orphan() -> u32 {
    99
}
```

Create `tests/fixtures/edit-eval-rust/src/remove_referenced.rs`:

```rust
pub fn referenced() -> u32 {
    100
}

pub fn caller() -> u32 {
    referenced() + 1
}
```

Create `tests/fixtures/edit-eval-rust/src/rename_target.rs`:

```rust
pub fn target_fn() -> u32 {
    77
}
```

Create `tests/fixtures/edit-eval-rust/src/rename_caller_a.rs`:

```rust
use crate::rename_target::target_fn;

pub fn use_a() -> u32 {
    target_fn() + 1
}
```

Create `tests/fixtures/edit-eval-rust/src/rename_caller_b.rs`:

```rust
use crate::rename_target;

pub fn use_b() -> u32 {
    rename_target::target_fn() + 2
}
```

- [ ] **Step 2: Add module declarations**

Edit `tests/fixtures/edit-eval-rust/src/lib.rs`. Final contents:

```rust
//! Adversarial fixtures for the codescout edit_code eval.

pub mod insert_after_last;
pub mod insert_bad_syntax;
pub mod insert_before_first;
pub mod remove_clean;
pub mod remove_referenced;
pub mod rename_caller_a;
pub mod rename_caller_b;
pub mod rename_target;
pub mod replace_doc_adj;
pub mod replace_generic;
pub mod replace_nested;
pub mod replace_no_sig;
pub mod replace_plain;
pub mod replace_tight_impl;
pub mod replace_trait_impl;
pub mod replace_wrong_sig;
```

- [ ] **Step 3: Verify fixture compiles**

Run: `cargo check --manifest-path tests/fixtures/edit-eval-rust/Cargo.toml`
Expected: clean.

- [ ] **Step 4: Add the remove + rename cases**

Edit `tests/e2e/edit_eval/cases.rs`. Append after the I-03 case (before closing `])`):

```rust
        EditCase {
            id: "M-01",
            action: EditAction::Remove,
            input: json!({
                "action": "remove",
                "symbol": "orphan",
                "path": "src/remove_clean.rs",
            }),
            target_file: "remove_clean.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::NotContains { file: "remove_clean.rs", needle: "pub fn orphan" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "remove function with no callers — clean removal",
            h1_exempt: None,
        },
        EditCase {
            id: "M-02",
            action: EditAction::Remove,
            input: json!({
                "action": "remove",
                "symbol": "referenced",
                "path": "src/remove_referenced.rs",
            }),
            target_file: "remove_referenced.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::NotContains { file: "remove_referenced.rs", needle: "pub fn referenced" },
                    // Caller remains; it now references a missing fn — compiler will fail.
                    ContentInvariant::Contains { file: "remove_referenced.rs", needle: "pub fn caller", count: 1 },
                ],
                compiler: CompilerExpected::Breaks,
            },
            rationale: "remove with same-file caller — tool faithful, compile legitimately breaks",
            h1_exempt: None,
        },
        EditCase {
            id: "N-01",
            action: EditAction::Rename,
            input: json!({
                "action": "rename",
                "symbol": "target_fn",
                "path": "src/rename_target.rs",
                "new_name": "renamed_fn",
            }),
            target_file: "rename_target.rs",
            expected: Expected {
                return_: ReturnExpected::Ok,
                disk: vec![
                    ContentInvariant::Contains { file: "rename_target.rs", needle: "pub fn renamed_fn", count: 1 },
                    ContentInvariant::NotContains { file: "rename_target.rs", needle: "target_fn" },
                ],
                compiler: CompilerExpected::Builds,
            },
            rationale: "cross-file rename — LSP updates all callsites; project still builds",
            h1_exempt: None,
        },
```

Note: N-01's content invariants check `rename_target.rs` only. The grader's `disk` field reads `target_file`, so cross-file invariants aren't directly checkable from the matcher today. The compiler oracle is what proves the rename propagated to `rename_caller_a.rs` and `rename_caller_b.rs` — if the rename failed to update them, the crate doesn't build, and the case grades CORRUPT.

- [ ] **Step 5: Run the harness**

Run: `cargo test --test e2e_tests edit_eval_harness -- --ignored --nocapture`
Expected: 14 cases run. M-02 should grade CORRECT (Breaks-expected matches Breaks-observed). N-01 should grade CORRECT if rename works; CORRUPT if any callsite was missed.

- [ ] **Step 6: Inspect round file**

```bash
cat docs/superpowers/specs/2026-05-15-edit-eval-round-3.md
```

- [ ] **Step 7: Commit**

```bash
git add tests/fixtures/edit-eval-rust/src/remove_*.rs tests/fixtures/edit-eval-rust/src/rename_*.rs tests/fixtures/edit-eval-rust/src/lib.rs tests/e2e/edit_eval/cases.rs docs/superpowers/specs/2026-05-15-edit-eval-round-3.md
git commit -m "test(edit-eval): remove + rename cluster (M-01, M-02, N-01) + round 3"
```

---

## Task 13: Diagnostic iteration

This task is open-ended: the goal is to iterate on the eval — or on `src/tools/symbol/edit_code.rs` — until verdicts match expectations (R-02 stays SILENT_WRONG-by-design with H1 exemption; everything else CORRECT).

- [ ] **Step 1: Read the latest round file**

```bash
ls -1 docs/superpowers/specs/2026-05-15-edit-eval-round-*.md
cat docs/superpowers/specs/2026-05-15-edit-eval-round-$(ls docs/superpowers/specs/ | grep "edit-eval-round" | sed 's/.*round-\([0-9]*\).*/\1/' | sort -n | tail -1).md
```

- [ ] **Step 2: For each unexpected verdict, classify the cause**

For each case in CORRUPT / SILENT_WRONG / PANIC (excluding R-02's exempt SILENT_WRONG):
- Is the case's `Expected` wrong? → fix the case
- Is a matcher invariant wrong? → fix the invariant
- Is `edit_code` itself wrong? → file an entry in `docs/TODO-tool-misbehaviors.md` AND fix in `src/tools/symbol/edit_code.rs` (with regression test alongside)

Use this priority order: matcher/case bugs first (cheap), then tool bugs (require a full src commit). Do NOT touch `src/` to make the eval pass faster — that loses signal.

- [ ] **Step 3: Re-run after each fix**

```bash
cargo test --test e2e_tests edit_eval_harness -- --ignored --nocapture
```

Each run produces a new round file. Commit each round with a message describing what changed:

```bash
git add docs/superpowers/specs/2026-05-15-edit-eval-round-N.md <fixed-files>
git commit -m "test(edit-eval): round N — <what changed>"
```

- [ ] **Step 4: Stop conditions**

You are done with this task when ONE of:
- All cases CORRECT except R-02 (which remains SILENT_WRONG-by-design with H1 exemption), AND `assert_hard_gates` passes inside the harness (no panic on completion)
- Any remaining non-CORRECT verdicts are documented as new LIMIT entries in `docs/TODO-tool-misbehaviors.md` with H1 exemptions added to their cases

- [ ] **Step 5: Final cleanup**

Ensure all of:
```bash
cargo fmt
cargo clippy -- -D warnings
cargo test --test e2e_tests edit_eval_harness -- --ignored
```
pass.

- [ ] **Step 6: Final commit (only after Step 5 is green)**

```bash
git add -u
git commit -m "test(edit-eval): final round; eval converged"
```

---

## Task 14: CHANGELOG + TODO-tool-misbehaviors update

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/TODO-tool-misbehaviors.md` (if any new LIMIT entries were filed during Task 13)

- [ ] **Step 1: Add CHANGELOG entry**

Edit `CHANGELOG.md`. Under the current Unreleased / next-version heading, add a `### Testing` subsection if it doesn't exist, then:

```markdown
### Testing
- Adversarial library-level eval for `edit_code` — 14 cases across replace/insert/remove/rename, graded via composite (return + on-disk content + cargo check exit). Includes BUG-054 regression sentinel (R-02) and shared `eval_common/` module factored out of nav-eval.
```

- [ ] **Step 2: Update TODO-tool-misbehaviors if needed**

If Task 13 surfaced new bugs, ensure each has a `LIMIT-NNN` (or `BUG-NNN`) entry in `docs/TODO-tool-misbehaviors.md` and that the corresponding case's `h1_exempt` field references it.

- [ ] **Step 3: Commit**

```bash
git add CHANGELOG.md docs/TODO-tool-misbehaviors.md
git commit -m "docs: changelog + tool-misbehaviors for edit_code eval"
```

---

## Self-review notes (executed before this plan was saved)

1. **Spec coverage:**
   - Decisions table → Tasks 1, 4, 7, 8, 10–12 (oracle, isolation, action mix, case count, fixture, harness, rubric)
   - Architecture file layout → File map at top
   - Verdict rubric (incl. composite) → Tasks 6 and 7
   - Case catalogue 14 entries → Tasks 10/11/12 (8 + 3 + 3)
   - Hamsa-shape mitigation (compiler oracle + narrow content invariants + three observables) → Tasks 4, 5, 7
   - Harness runtime pseudocode → Task 9 (entrypoint)
   - Hard gates H1/H2/H3 → Task 9 Step 3 (gate check); R-02 exemption woven into matcher
   - Out-of-scope items → not in any task (correct)
   - Definition-of-Done → Task 13's stop conditions + Task 14's CHANGELOG

2. **Placeholder scan:** No "TBD", "TODO", "appropriate", "fill in" outside of the live `TODO-tool-misbehaviors.md` filename reference.

3. **Type consistency:** `EditCase`, `Expected { return_, disk, compiler }`, `ReturnExpected`, `CompilerExpected`, `ContentInvariant`, `Observation`, `ReturnObservation`, `MatchResult`, `Verdict::{Correct, Partial, CleanError, SilentWrong, Corrupt, Hung, Panic}` are used consistently across tasks 6–9. `Report::push(id, verdict, evidence)` signature matches both Task 5 (definition) and Task 9 (call site). `next_round_number("edit-eval")` slug matches the round file naming pattern `2026-05-15-edit-eval-round-N.md`.

## Non-goals (this plan)

- LSP-side bugs that surface during edits (e.g., did_change cascades). Out of scope; would need a separate harness.
- Non-Rust edit_code coverage. Each language needs its own fixture crate and LSP warmup.
- Performance / latency grading. `cargo check` variance is too high to gate on.
- LLM-in-the-loop grading (subagent-driven adversarial generation). Future Approach C.
- Edits to `edit_file`, `edit_markdown`, `create_file`. Each is its own eval design.
