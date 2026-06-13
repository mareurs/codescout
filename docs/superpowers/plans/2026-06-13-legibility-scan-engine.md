# Legibility Scan Engine (Phase 2a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the pure, testable scan engine that ranks code legibility refactor candidates from `usage.db` friction + the AST symbol index — returning a sorted `Vec<Candidate>`, with no MCP/tool/tracker coupling.

**Architecture:** A new `src/legibility/` module. A **recorder lane** queries the Phase-1 friction columns in `usage.db` (filtered by `project_root`), an **index lane** walks the tree-sitter symbol index for structural defects (over-budget bodies, name collisions, un-mappable files), and a **scorer** gates on structural defect, assigns tiers (biting-now vs latent), and ranks. Everything is a plain function over plain data — the engine is built the way the Dzo wants code built.

**Tech Stack:** Rust, `rusqlite`, `ignore::WalkBuilder`, `serde`. This is Phase 2a of `docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md`. Phase 2b (the `librarian(action="legibility_scan")` action + augmented-artifact tracker reconcile) is planned separately, against this engine's real `Candidate` API.

**Depends on:** Phase 1 (`docs/superpowers/plans/2026-06-13-honest-usage-db-logging.md`) — the columns `friction_target`, `overflow_tokens`, `err_family`, `project_root` must exist on `tool_calls`.

---

## Reused APIs (already in the codebase — verified)

- `crate::ast::detect_language(path: &Path) -> Option<&'static str>` (`src/ast/mod.rs:61`)
- `crate::ast::parser::extract_symbols_from_source(source: &str, language: Option<&'static str>, path: &Path) -> anyhow::Result<Vec<SymbolInfo>>` (`src/ast/parser.rs:50`)
- `crate::lsp::symbols::{SymbolInfo, SymbolKind}` — `SymbolInfo { name, name_path, kind, file, start_line (0-idx), end_line (0-idx), range_start_line: Option<u32>, start_col, children, detail }`; `SymbolKind` variants include `Function`, `Method`, `Constructor` (body-bearing).
- `crate::tools::MAX_INLINE_TOKENS` (= 2500) and `crate::tools::exceeds_inline_limit(text: &str) -> bool` (`src/tools/core/types.rs`)
- `ignore::WalkBuilder` — the project's standard gitignore-aware file walker (see `src/tools/symbol/references.rs:79`, `src/retrieval/sync.rs:68`).

## File structure

| File | Responsibility |
|---|---|
| `src/legibility/mod.rs` (new) | Types, both lanes, scorer, `scan()` orchestrator. One cohesive module (~350 lines); split if it overflows the symbols overview later — the probe will tell us. |
| `src/lib.rs` (modify) | Add `pub mod legibility;` |

---

### Task 1: Module skeleton + types

**Files:**
- Create: `src/legibility/mod.rs`
- Modify: `src/lib.rs` (add `pub mod legibility;` beside the other `pub mod` declarations)

- [ ] **Step 1: Write the failing test**

Create `src/legibility/mod.rs` with ONLY a `#[cfg(test)]` module first (and the type defs in Step 3 will follow). For TDD, write this test (it references types defined in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friction_score_and_emptiness() {
        let empty = Friction::default();
        assert!(empty.is_empty());
        assert_eq!(empty.score(), 0);

        let f = Friction { truncations: 14, retries: 0, code_class_edit_fails: 1, other: 2, sessions: 2 };
        assert!(!f.is_empty());
        // 3*14 + 2*0 + 2*1 + 1*2 = 46
        assert_eq!(f.score(), 46);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib legibility::tests::friction_score_and_emptiness`
Expected: FAIL — `Friction` not defined / `legibility` module not registered.

- [ ] **Step 3: Add the module registration and types**

In `src/lib.rs`, add `pub mod legibility;` next to the other module declarations.

At the TOP of `src/legibility/mod.rs` (above the test module):

```rust
//! Legibility scan engine — ranks refactor candidates from usage.db friction
//! and the AST symbol index. Pure: (db conn, project root) -> ranked candidates.
//! Phase 2a of docs/superpowers/specs/2026-06-13-dzo-friction-probes-design.md.

use crate::lsp::symbols::{SymbolInfo, SymbolKind};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

/// A structural legibility defect kind — the entry gate. A candidate must have one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Defect {
    OverBudgetBody,
    NameCollision,
    UnMappableFile,
}

/// Tier 1 = biting now (has recorder friction); Tier 2 = latent (structural only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    BitingNow,
    Latent,
}

impl Tier {
    /// 1 for biting-now, 2 for latent — the spec's numeric tier.
    pub fn rank(self) -> u8 {
        match self {
            Tier::BitingNow => 1,
            Tier::Latent => 2,
        }
    }
}

/// Observed cost from the recorder, per target. `retries` is reserved (0 in v1 —
/// same-input-repeat detection is a follow-up).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Friction {
    pub truncations: u32,
    pub retries: u32,
    pub code_class_edit_fails: u32,
    pub other: u32,
    pub sessions: u32,
}

impl Friction {
    pub fn is_empty(&self) -> bool {
        self.truncations == 0 && self.retries == 0 && self.code_class_edit_fails == 0 && self.other == 0
    }
    /// score = 3*truncations + 2*retries + 2*code_class_edit_fails + 1*other.
    /// Infra-class err_family never reaches `code_class_edit_fails` (excluded in the
    /// recorder query), so tool-class noise cannot inflate a code candidate.
    pub fn score(&self) -> u32 {
        3 * self.truncations + 2 * self.retries + 2 * self.code_class_edit_fails + self.other
    }
}

/// A raw structural finding from the index lane, before scoring/tiering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuralDefect {
    pub rel_file: String,
    pub name_path: String, // "(file)" for UnMappableFile
    pub defect: Defect,
    pub tokens: usize,
    pub lines: u32,
}

/// A parsed source file: its rel path, its source lines, and its symbol tree.
pub struct FileSymbols {
    pub rel_file: String,
    pub lines: Vec<String>,
    pub symbols: Vec<SymbolInfo>,
}

/// A ranked refactor candidate — the engine's output unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Candidate {
    pub key: String, // "<rel_file>::<name_path>"
    pub rel_file: String,
    pub name_path: String,
    pub defect: Defect,
    pub tier: Tier,
    pub tokens: usize,
    pub budget: usize,
    pub lines: u32,
    pub friction: Friction,
    pub score: u32,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib legibility::tests::friction_score_and_emptiness`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/legibility/mod.rs src/lib.rs
git commit -m "feat(legibility): scan-engine module skeleton + core types"
```

---

### Task 2: Over-budget-body detector

**Files:**
- Modify: `src/legibility/mod.rs`

- [ ] **Step 1: Write the failing test**

Add to the `tests` module. The `sym(...)` helper builds a `SymbolInfo` fixture concisely — add it once here:

```rust
    fn sym(name_path: &str, kind: SymbolKind, start: u32, end: u32) -> SymbolInfo {
        SymbolInfo {
            name: name_path.rsplit('/').next().unwrap_or(name_path).to_string(),
            name_path: name_path.to_string(),
            kind,
            file: std::path::PathBuf::from("x.rs"),
            start_line: start,
            end_line: end,
            range_start_line: None,
            start_col: 0,
            children: vec![],
            detail: None,
        }
    }

    fn file_with(rel: &str, body_lines: usize, syms: Vec<SymbolInfo>) -> FileSymbols {
        // each line is 200 bytes so `body_lines` lines ≈ body_lines*200 bytes
        FileSymbols {
            rel_file: rel.to_string(),
            lines: (0..body_lines).map(|_| "x".repeat(200)).collect(),
            symbols: syms,
        }
    }

    #[test]
    fn over_budget_bodies_flags_only_over_budget_functions() {
        // big fn spans lines 0..=70 → ~70*201 ≈ 14k bytes > 10k budget
        let big = sym("Foo/big", SymbolKind::Method, 0, 70);
        // small fn spans lines 0..=5 → ~6*201 ≈ 1.2k bytes < budget
        let small = sym("Foo/small", SymbolKind::Method, 0, 5);
        let files = vec![file_with("src/foo.rs", 71, vec![big, small])];
        let defects = over_budget_bodies(&files);
        assert_eq!(defects.len(), 1, "only the big body");
        assert_eq!(defects[0].name_path, "Foo/big");
        assert_eq!(defects[0].defect, Defect::OverBudgetBody);
        assert!(defects[0].tokens > crate::tools::MAX_INLINE_TOKENS);
    }

    #[test]
    fn over_budget_ignores_non_body_kinds() {
        // a Struct spanning many lines is NOT a body-bearing symbol
        let s = sym("BigStruct", SymbolKind::Struct, 0, 70);
        let files = vec![file_with("src/foo.rs", 71, vec![s])];
        assert!(over_budget_bodies(&files).is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib legibility::tests::over_budget`
Expected: FAIL — `over_budget_bodies` / `collect_bodies` not defined.

- [ ] **Step 3: Implement**

Add to `src/legibility/mod.rs` (above the test module):

```rust
fn is_body_bearing(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::Function | SymbolKind::Method | SymbolKind::Constructor)
}

/// Recurse the symbol tree, collecting body-bearing symbols.
fn collect_bodies<'a>(syms: &'a [SymbolInfo], out: &mut Vec<&'a SymbolInfo>) {
    for s in syms {
        if is_body_bearing(s.kind) {
            out.push(s);
        }
        collect_bodies(&s.children, out);
    }
}

/// The source text of a symbol's body, plus its line count. Empty when the range
/// is degenerate or out of bounds.
fn body_text(lines: &[String], sym: &SymbolInfo) -> (String, u32) {
    if lines.is_empty() {
        return (String::new(), 0);
    }
    let start = sym.range_start_line.unwrap_or(sym.start_line) as usize;
    let end = (sym.end_line as usize).min(lines.len() - 1);
    if start > end {
        return (String::new(), 0);
    }
    (lines[start..=end].join("\n"), (end - start + 1) as u32)
}

/// Index-lane detector: function/method bodies that exceed the inline budget
/// (so `symbols(include_body=true)` truncates them).
pub fn over_budget_bodies(files: &[FileSymbols]) -> Vec<StructuralDefect> {
    let mut out = Vec::new();
    for f in files {
        let mut bodies = Vec::new();
        collect_bodies(&f.symbols, &mut bodies);
        for sym in bodies {
            let (body, lines) = body_text(&f.lines, sym);
            if !body.is_empty() && crate::tools::exceeds_inline_limit(&body) {
                out.push(StructuralDefect {
                    rel_file: f.rel_file.clone(),
                    name_path: sym.name_path.clone(),
                    defect: Defect::OverBudgetBody,
                    tokens: body.len() / 4,
                    lines,
                });
            }
        }
    }
    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib legibility::tests::over_budget`
Expected: PASS (both tests).

- [ ] **Step 5: Commit**

```bash
git add src/legibility/mod.rs
git commit -m "feat(legibility): over-budget-body index-lane detector"
```

---

### Task 3: Name-collision detector

**Files:**
- Modify: `src/legibility/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn name_collisions_flags_same_file_duplicate_name_path() {
        // inherent impl + trait impl both expose "LspManager/get_or_start"
        let a = sym("LspManager/get_or_start", SymbolKind::Method, 0, 5);
        let b = sym("LspManager/get_or_start", SymbolKind::Method, 10, 15);
        let unique = sym("LspManager/do_start", SymbolKind::Method, 20, 25);
        let files = vec![file_with("src/lsp/manager.rs", 30, vec![a, b, unique])];
        let defects = name_collisions(&files);
        assert_eq!(defects.len(), 1);
        assert_eq!(defects[0].name_path, "LspManager/get_or_start");
        assert_eq!(defects[0].defect, Defect::NameCollision);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib legibility::tests::name_collisions_flags_same_file_duplicate_name_path`
Expected: FAIL — `name_collisions` not defined.

- [ ] **Step 3: Implement**

```rust
/// Recurse the symbol tree, collecting ALL symbols (any kind).
fn collect_all<'a>(syms: &'a [SymbolInfo], out: &mut Vec<&'a SymbolInfo>) {
    for s in syms {
        out.push(s);
        collect_all(&s.children, out);
    }
}

/// Index-lane detector: a name_path that resolves to more than one symbol within
/// the same file (e.g. an inherent impl + a trait impl method) — the ambiguity
/// that hard-fails `edit_code`. Cross-file ambiguity is a softer signal, deferred.
pub fn name_collisions(files: &[FileSymbols]) -> Vec<StructuralDefect> {
    let mut out = Vec::new();
    for f in files {
        let mut all = Vec::new();
        collect_all(&f.symbols, &mut all);
        let mut counts: HashMap<&str, u32> = HashMap::new();
        for s in &all {
            *counts.entry(s.name_path.as_str()).or_insert(0) += 1;
        }
        let mut keys: Vec<&str> = counts.iter().filter(|(_, c)| **c > 1).map(|(k, _)| *k).collect();
        keys.sort_unstable(); // deterministic output order
        for np in keys {
            out.push(StructuralDefect {
                rel_file: f.rel_file.clone(),
                name_path: np.to_string(),
                defect: Defect::NameCollision,
                tokens: 0,
                lines: 0,
            });
        }
    }
    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib legibility::tests::name_collisions_flags_same_file_duplicate_name_path`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/legibility/mod.rs
git commit -m "feat(legibility): same-file name-collision detector"
```

---

### Task 4: Un-mappable-file detector

**Files:**
- Modify: `src/legibility/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn un_mappable_files_flags_overview_over_budget_not_line_count() {
        // Many symbols → estimated overview exceeds the budget.
        let many: Vec<SymbolInfo> = (0..400)
            .map(|i| sym(&format!("Mod/sym_{i:04}_with_a_longish_name"), SymbolKind::Function, i, i))
            .collect();
        let big_map = file_with("src/huge.rs", 400, many);

        // A long file (1500 lines) with FEW symbols maps cleanly → NOT flagged.
        // (Encodes the verified longer-files-better finding: line count is not a trigger.)
        let long_clean = file_with("src/long_clean.rs", 1500, vec![
            sym("A/f", SymbolKind::Function, 0, 700),
            sym("A/g", SymbolKind::Function, 701, 1499),
        ]);

        let defects = un_mappable_files(&[big_map, long_clean]);
        assert_eq!(defects.len(), 1, "only the many-symbol file");
        assert_eq!(defects[0].rel_file, "src/huge.rs");
        assert_eq!(defects[0].name_path, "(file)");
        assert_eq!(defects[0].defect, Defect::UnMappableFile);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib legibility::tests::un_mappable_files_flags_overview_over_budget_not_line_count`
Expected: FAIL — `un_mappable_files` not defined.

- [ ] **Step 3: Implement**

```rust
/// Estimated byte size of a `symbols(path)` overview: ~one line per symbol,
/// dominated by the name_path + the optional detail (signature), plus a fixed
/// per-line overhead (kind label, line range, indentation).
fn overview_bytes(files_syms: &[&SymbolInfo]) -> usize {
    const PER_SYMBOL_OVERHEAD: usize = 24;
    files_syms
        .iter()
        .map(|s| PER_SYMBOL_OVERHEAD + s.name_path.len() + s.detail.as_deref().map_or(0, str::len))
        .sum()
}

/// Index-lane detector: a file whose `symbols(path)` overview would exceed the
/// inline budget (can't be mapped in one call). Driven by symbol count/size, NOT
/// line count — a cleanly-mapped long file is left alone (verified: longer files
/// comprehend better; the hazard is total context, not within-file length).
pub fn un_mappable_files(files: &[FileSymbols]) -> Vec<StructuralDefect> {
    let mut out = Vec::new();
    for f in files {
        let mut all = Vec::new();
        collect_all(&f.symbols, &mut all);
        let bytes = overview_bytes(&all);
        if bytes > crate::tools::MAX_INLINE_TOKENS * 4 {
            out.push(StructuralDefect {
                rel_file: f.rel_file.clone(),
                name_path: "(file)".to_string(),
                defect: Defect::UnMappableFile,
                tokens: bytes / 4,
                lines: f.lines.len() as u32,
            });
        }
    }
    out
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib legibility::tests::un_mappable_files_flags_overview_over_budget_not_line_count`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/legibility/mod.rs
git commit -m "feat(legibility): un-mappable-file detector (overview-overflow, not line count)"
```

---

### Task 5: `parse_project` + `index_lane`

**Files:**
- Modify: `src/legibility/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn index_lane_finds_over_budget_body_in_real_file() {
        let dir = tempfile::tempdir().unwrap();
        // a Rust file with one huge function (each line padded so the body exceeds budget)
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 {
            src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
        }
        src.push_str("}\n");
        std::fs::write(dir.path().join("huge.rs"), src).unwrap();

        let defects = index_lane(dir.path());
        assert!(
            defects.iter().any(|d| d.defect == Defect::OverBudgetBody && d.name_path.contains("huge")),
            "expected an over-budget-body defect for `huge`, got: {defects:?}"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib legibility::tests::index_lane_finds_over_budget_body_in_real_file`
Expected: FAIL — `parse_project` / `index_lane` not defined.

- [ ] **Step 3: Implement**

```rust
/// Walk the project (gitignore-aware), parse every recognized source file's symbols.
pub fn parse_project(root: &Path) -> Vec<FileSymbols> {
    let mut out = Vec::new();
    for entry in ignore::WalkBuilder::new(root).hidden(true).git_ignore(true).build().flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let Some(lang) = crate::ast::detect_language(path) else {
            continue;
        };
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(symbols) = crate::ast::parser::extract_symbols_from_source(&source, Some(lang), path) else {
            continue; // unparseable / unsupported → skip, never fail the whole scan
        };
        let rel_file = path.strip_prefix(root).unwrap_or(path).to_string_lossy().to_string();
        let lines = source.lines().map(str::to_string).collect();
        out.push(FileSymbols { rel_file, lines, symbols });
    }
    out
}

/// Run all three structural detectors over the parsed project.
pub fn index_lane(root: &Path) -> Vec<StructuralDefect> {
    let files = parse_project(root);
    let mut defects = over_budget_bodies(&files);
    defects.extend(name_collisions(&files));
    defects.extend(un_mappable_files(&files));
    defects
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib legibility::tests::index_lane_finds_over_budget_body_in_real_file`
Expected: PASS.

> If `extract_symbols_from_source` does not surface a top-level free `fn` as `Function` for the fixture (some extractors only emit container members), wrap the function in an `impl` or adjust the fixture to a struct-method; assert on whatever body-bearing symbol the extractor yields. Confirm by reading the parsed symbols in a scratch assertion first.

- [ ] **Step 5: Commit**

```bash
git add src/legibility/mod.rs
git commit -m "feat(legibility): parse_project walker + index_lane orchestrator"
```

---

### Task 6: Recorder lane (usage.db friction query)

**Files:**
- Modify: `src/legibility/mod.rs`

- [ ] **Step 1: Write the failing test**

This test builds a temp `usage.db` via the Phase-1 `open_db`/`write_record` API and asserts the friction aggregation AND the `project_root` filter (the F-1 contamination regression — a foreign-project row must be excluded):

```rust
    #[test]
    fn recorder_lane_aggregates_friction_and_filters_by_project_root() {
        use crate::usage::db::{open_db, write_record};
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let conn = open_db(dir.path()).unwrap();

        // 2 truncations on the same target, this repo
        for _ in 0..2 {
            write_record(&conn, "symbols", 1, "success", true, None,
                "cs", None, "s1", None, None, Some("ccs1"),
                Some("Foo/bar"), Some(1000), None, Some("/repo")).unwrap();
        }
        // 1 code-class edit fail on the same target, this repo
        write_record(&conn, "edit_code", 1, "error", Some("ambiguous name_path \"Foo/bar\" matches 2 symbols"),
            "cs", None, "s1", None, None, Some("ccs1"),
            Some("Foo/bar"), None, Some("ambiguous_name_path"), Some("/repo")).unwrap();
        // a FOREIGN-project row for the same target — must be excluded (F-1)
        write_record(&conn, "symbols", 1, "success", true, None,
            "cs", None, "s9", None, None, Some("ccs9"),
            Some("Foo/bar"), Some(9999), None, Some("/other-repo")).unwrap();

        let map = recorder_lane(&conn, "/repo").unwrap();
        let fr = map.get("Foo/bar").expect("Foo/bar present");
        assert_eq!(fr.truncations, 2, "foreign-repo truncation must be excluded");
        assert_eq!(fr.code_class_edit_fails, 1);
        assert_eq!(fr.sessions, 1);
        assert_eq!(fr.score(), 3 * 2 + 2 * 1); // 8
        assert!(!map.contains_key(""), "empty friction_target excluded");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib legibility::tests::recorder_lane_aggregates_friction_and_filters_by_project_root`
Expected: FAIL — `recorder_lane` not defined.

- [ ] **Step 3: Implement**

```rust
/// `err_family` values that indicate a code/extractor-shape problem (count toward
/// the score). Infra families (lsp_disconnect, …) are tool-class and excluded.
const CODE_FAMILIES: &str = "'ast_extent_fail','ambiguous_name_path','replace_dropped_sibling'";
const INFRA_FAMILIES: &str = "'lsp_disconnect','lsp_index_locked','mux_startup_fail','lsp_not_running'";

/// Recorder lane: aggregate per-`friction_target` cost from usage.db, scoped to
/// this repo by `project_root` (the F-1 cross-project contamination fix).
pub fn recorder_lane(
    conn: &rusqlite::Connection,
    project_root: &str,
) -> rusqlite::Result<HashMap<String, Friction>> {
    let sql = format!(
        "SELECT friction_target,
                SUM(CASE WHEN overflowed = 1 THEN 1 ELSE 0 END),
                SUM(CASE WHEN err_family IN ({code}) THEN 1 ELSE 0 END),
                SUM(CASE WHEN outcome != 'success'
                          AND (err_family IS NULL OR err_family NOT IN ({code}, {infra}))
                         THEN 1 ELSE 0 END),
                COUNT(DISTINCT cc_session_id)
         FROM tool_calls
         WHERE project_root = ?1 AND friction_target IS NOT NULL AND friction_target != ''
         GROUP BY friction_target",
        code = CODE_FAMILIES,
        infra = INFRA_FAMILIES,
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([project_root], |r| {
        Ok((
            r.get::<_, String>(0)?,
            Friction {
                truncations: r.get::<_, i64>(1)? as u32,
                retries: 0,
                code_class_edit_fails: r.get::<_, i64>(2)? as u32,
                other: r.get::<_, i64>(3)? as u32,
                sessions: r.get::<_, i64>(4)? as u32,
            },
        ))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (k, fr) = row?;
        map.insert(k, fr);
    }
    Ok(map)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib legibility::tests::recorder_lane_aggregates_friction_and_filters_by_project_root`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/legibility/mod.rs
git commit -m "feat(legibility): recorder lane — friction query, project_root-filtered (F-1)"
```

---

### Task 7: Scorer + `scan()` orchestrator + integration

**Files:**
- Modify: `src/legibility/mod.rs`

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn score_and_rank_tiers_and_orders() {
        let structural = vec![
            // biting-now: has friction
            StructuralDefect { rel_file: "src/a.rs".into(), name_path: "A/hot".into(),
                defect: Defect::OverBudgetBody, tokens: 4000, lines: 242 },
            // latent: no friction, bigger body
            StructuralDefect { rel_file: "src/b.rs".into(), name_path: "B/cold".into(),
                defect: Defect::OverBudgetBody, tokens: 6000, lines: 331 },
        ];
        let mut friction = HashMap::new();
        friction.insert("A/hot".to_string(), Friction { truncations: 5, ..Default::default() });

        let ranked = score_and_rank(structural, &friction);
        assert_eq!(ranked.len(), 2);
        // biting-now (A/hot) ranks above latent (B/cold) despite smaller body
        assert_eq!(ranked[0].name_path, "A/hot");
        assert_eq!(ranked[0].tier, Tier::BitingNow);
        assert_eq!(ranked[0].key, "src/a.rs::A/hot");
        assert_eq!(ranked[0].score, 15); // 3*5
        assert_eq!(ranked[1].name_path, "B/cold");
        assert_eq!(ranked[1].tier, Tier::Latent);
    }

    #[test]
    fn scan_end_to_end_ranks_a_real_over_budget_body() {
        use crate::usage::db::{open_db, write_record};
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();

        // a real over-budget function in a source file
        let mut src = String::from("fn huge() {\n");
        for i in 0..200 {
            src.push_str(&format!("    let v{i} = \"{}\";\n", "x".repeat(80)));
        }
        src.push_str("}\n");
        std::fs::write(dir.path().join("huge.rs"), src).unwrap();

        let conn = open_db(dir.path()).unwrap();
        // friction targeting the function's name_path (whatever the extractor calls it: "huge")
        write_record(&conn, "symbols", 1, "success", true, None, "cs", None, "s1",
            None, None, Some("ccs1"), Some("huge"), Some(3500), None,
            Some(&dir.path().to_string_lossy())).unwrap();

        let cands = scan(&conn, dir.path(), &dir.path().to_string_lossy()).unwrap();
        assert!(
            cands.iter().any(|c| c.defect == Defect::OverBudgetBody && c.name_path.contains("huge")),
            "expected ranked over-budget candidate for huge: {cands:?}"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib legibility::tests::score_and_rank legibility::tests::scan_end_to_end`
Expected: FAIL — `score_and_rank` / `scan` not defined.

- [ ] **Step 3: Implement**

```rust
/// Combine structural defects with recorder friction → tiered, scored, ranked
/// candidates. A candidate's friction is matched by its name_path, falling back to
/// its rel_file (for un-mappable files, whose `friction_target` is the path).
pub fn score_and_rank(
    structural: Vec<StructuralDefect>,
    friction: &HashMap<String, Friction>,
) -> Vec<Candidate> {
    let mut cands: Vec<Candidate> = structural
        .into_iter()
        .map(|d| {
            let fr = friction
                .get(&d.name_path)
                .or_else(|| friction.get(&d.rel_file))
                .cloned()
                .unwrap_or_default();
            let tier = if fr.is_empty() { Tier::Latent } else { Tier::BitingNow };
            let score = fr.score();
            Candidate {
                key: format!("{}::{}", d.rel_file, d.name_path),
                rel_file: d.rel_file,
                name_path: d.name_path,
                defect: d.defect,
                tier,
                tokens: d.tokens,
                budget: crate::tools::MAX_INLINE_TOKENS,
                lines: d.lines,
                friction: fr,
                score,
            }
        })
        .collect();

    // Tier 1 before Tier 2; within tier 1 by score desc; ties and tier 2 by tokens
    // over budget (proxy: tokens) desc; final tie-break by key for determinism.
    cands.sort_by(|a, b| {
        a.tier
            .rank()
            .cmp(&b.tier.rank())
            .then(b.score.cmp(&a.score))
            .then(b.tokens.cmp(&a.tokens))
            .then(a.key.cmp(&b.key))
    });
    cands
}

/// The engine entry point: index lane + recorder lane → ranked candidates.
pub fn scan(
    conn: &rusqlite::Connection,
    root: &Path,
    project_root: &str,
) -> anyhow::Result<Vec<Candidate>> {
    let structural = index_lane(root);
    let friction = recorder_lane(conn, project_root)?;
    Ok(score_and_rank(structural, &friction))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib legibility::tests`
Expected: PASS (all engine tests).

- [ ] **Step 5: Full gate**

Run: `cargo test --lib legibility:: && cargo clippy --lib -- -D warnings`
Expected: green, clippy clean. (Then `rustfmt --edition 2021 src/legibility/mod.rs` if `cargo fmt --check` flags it; format only this file.)

- [ ] **Step 6: Commit**

```bash
git add src/legibility/mod.rs
git commit -m "feat(legibility): scorer (gate/tier/rank) + scan() orchestrator"
```

---

## Self-Review

**Spec coverage** (against `2026-06-13-dzo-friction-probes-design.md` § "Deliverable 2"):
- Recorder lane (biting-now, structured columns, `project_root` filter) → Task 6. ✓
- Index lane: over-budget bodies (Task 2), name collisions (Task 3), un-mappable files (Task 4), orchestrated (Task 5). ✓
- Scorer: structural-defect entry gate (index lane only emits structural defects → gate is implicit), two tiers, observed-cost rank, infra excluded from score (Task 1 `Friction::score` doc + Task 6 query) → Task 7. ✓
- Candidate key `<rel_file>::<name_path>` → Task 7. ✓
- Budget = single source of truth (`crate::tools::MAX_INLINE_TOKENS`) → Tasks 2/4/7. ✓
- F-1 regression (project_root exclusion) → Task 6 test. ✓
- No-line-count / longer-files-better (un-mappable keyed on overview size, not lines) → Task 4 test. ✓

**Out of scope (this plan — belongs to Phase 2b or later):** the `librarian(action="legibility_scan")` action + params schema; the augmented-artifact tracker reconcile (params/body, auto-close by re-measurement); output formatting; the `retries` friction dimension (same-input-repeat detection); cross-file name ambiguity. The reconcile **`merge=true`** discipline and the auto-close semantics are 2b's, planned against this engine's `Candidate` API.

**Placeholder scan:** Task 5 Step 4 carries a conditional fallback about the extractor's free-`fn` behavior — a genuine extractor-shape unknown with a concrete in-task resolution (read the parsed symbols, adjust the fixture), not a content placeholder. All code blocks are complete.

**Type consistency:** `Friction`/`Candidate`/`StructuralDefect`/`FileSymbols`/`Defect`/`Tier` defined in Task 1 and used identically through Task 7; `over_budget_bodies`/`name_collisions`/`un_mappable_files`/`parse_project`/`index_lane`/`recorder_lane`/`score_and_rank`/`scan` signatures are stable across their defining and calling tasks; the `write_record` 16-arg order in Task 6's test matches the Phase-1 signature.
