# list_symbols: Progressive Directory Overview — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the shallow `max_depth(1)` directory walk in `list_symbols` with a budget-aware three-mode dispatch: full symbols for small trees (≤30 files), AST class overview for medium trees (31–80), and directory map for large trees (>80).

**Architecture:** Three new pure-function helpers (`find_split_point`, `count_files_by_subdir`, `ast_class_names_for_dir`) feed a mode-selection branch that replaces the current `is_project_root` depth asymmetry. `format_list_symbols` gains a new branch for the two new output shapes. A `force_mode` param lets callers bypass size-based switching. Server instructions updated for the new response shapes.

**Tech Stack:** Rust, `ignore` crate (WalkBuilder), `crate::ast::extract_symbols` (tree-sitter), `crate::lsp::symbols::SymbolKind`

---

## File Map

| File | Change |
|------|--------|
| `src/tools/symbol.rs` | 3 constants, 3 helpers, replace directory branch, update `format_list_symbols`, add `force_mode` param to schema |
| `src/prompts/server_instructions.md` | Document new response shapes and `force_mode` param |

**This is an API contract change.** The directory branch response schema changes from `{ "directory": ..., "files": [...] }` to one of three shapes. Callers parsing `result["files"]` will get `null` in overview modes. Server instructions update is required.

---

### Task 1: Add constants

**Files:**
- Modify: `src/tools/symbol.rs:499-505` (after existing constants block)

- [ ] **Step 1: Insert three new constants** after `LIST_SYMBOLS_SINGLE_FILE_FLAT_CAP` (~line 505):

```rust
/// File count below which directory mode returns full symbols (recursive walk).
const LIST_SYMBOLS_RECURSE_SMALL: usize = 30;
/// File count below which directory mode returns AST class names per subdir.
const LIST_SYMBOLS_RECURSE_MEDIUM: usize = 80;
/// Max immediate subdirectories shown in directory_map mode.
const LIST_SYMBOLS_MAX_SUBDIRS: usize = 15;
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check 2>&1 | head -20
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(list_symbols): add progressive directory threshold constants"
```

---

### Task 2: Implement find_split_point and count_files_by_subdir

**Files:**
- Modify: `src/tools/symbol.rs` — add two helper functions before `pub struct ListSymbols` (~line 515), add tests in `#[cfg(test)]` module

`find_split_point` collapses pass-through single-child dirs (e.g. `kotlin/edu/planner/` collapses to `planner/` which has real branching). `count_files_by_subdir` calls it before counting, so subdirectory grouping lands at the meaningful level — not a useless single-entry `edu/`.

- [ ] **Step 1: Write the failing tests** in the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn find_split_point_collapses_single_child_chain() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    // a/ → b/ → c/ (three files directly in c/) — should collapse to c/
    std::fs::create_dir_all(root.join("a/b/c")).unwrap();
    for i in 0..3 {
        std::fs::write(root.join(format!("a/b/c/file{i}.rs")), "").unwrap();
    }
    let split = find_split_point(root);
    assert_eq!(split, root.join("a/b/c"));
}

#[test]
fn find_split_point_stops_at_branch() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::create_dir_all(root.join("a/c")).unwrap();
    std::fs::write(root.join("a/b/file.rs"), "").unwrap();
    std::fs::write(root.join("a/c/file.rs"), "").unwrap();
    let split = find_split_point(root);
    assert_eq!(split, root.join("a"), "should stop at branching dir");
}

#[test]
fn find_split_point_stops_when_dir_has_direct_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    // a/ has one child b/ but also a direct source file — stop here
    std::fs::create_dir_all(root.join("a/b")).unwrap();
    std::fs::write(root.join("a/root.rs"), "").unwrap();
    std::fs::write(root.join("a/b/file.rs"), "").unwrap();
    let split = find_split_point(root);
    assert_eq!(split, root.join("a"), "mixed dir stops descent");
}

#[test]
fn count_files_by_subdir_groups_and_sorts() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("sub_a")).unwrap();
    for i in 0..3 {
        std::fs::write(root.join(format!("sub_a/file{i}.rs")), "").unwrap();
    }
    std::fs::create_dir_all(root.join("sub_b")).unwrap();
    for i in 0..5 {
        std::fs::write(root.join(format!("sub_b/file{i}.rs")), "").unwrap();
    }
    // 1 file directly in root (counted in total, not in subdirs)
    std::fs::write(root.join("root.rs"), "").unwrap();

    let (total, subdirs) = count_files_by_subdir(root, root);

    assert_eq!(total, 9);
    assert_eq!(subdirs.len(), 2);
    assert!(subdirs[0].0.contains("sub_b"), "largest subdir first");
    assert_eq!(subdirs[0].1, 5);
    assert!(subdirs[1].0.contains("sub_a"));
    assert_eq!(subdirs[1].1, 3);
}

#[test]
fn count_files_by_subdir_collapses_passthrough() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    // kotlin/ → edu/ → planner/ → [api/(3), domain/(2)]
    for (sub, n) in &[("api", 3usize), ("domain", 2)] {
        std::fs::create_dir_all(root.join(format!("kotlin/edu/planner/{sub}"))).unwrap();
        for i in 0..*n {
            std::fs::write(root.join(format!("kotlin/edu/planner/{sub}/f{i}.rs")), "").unwrap();
        }
    }
    let (total, subdirs) = count_files_by_subdir(root, &root.join("kotlin"));
    assert_eq!(total, 5);
    assert_eq!(subdirs.len(), 2, "collapsed to planner/ children, not edu/");
    assert!(subdirs[0].0.contains("api"), "api (3) before domain (2)");
    assert_eq!(subdirs[0].1, 3);
}

#[test]
fn count_files_by_subdir_flat_dir_returns_empty_subdirs() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    for i in 0..4 {
        std::fs::write(root.join(format!("file{i}.rs")), "").unwrap();
    }
    let (total, subdirs) = count_files_by_subdir(root, root);
    assert_eq!(total, 4);
    assert!(subdirs.is_empty());
}

#[test]
fn count_files_by_subdir_ignores_non_source_files() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("sub/README.md"), "").unwrap(); // ignored
    std::fs::write(root.join("sub/build.rs"), "").unwrap();  // counted
    let (total, subdirs) = count_files_by_subdir(root, root);
    assert_eq!(total, 1);
    assert_eq!(subdirs[0].1, 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test find_split_point count_files_by_subdir 2>&1 | tail -10
```
Expected: compile error — functions not found.

- [ ] **Step 3: Implement `find_split_point`** — add before `pub struct ListSymbols`:

```rust
/// Collapse single-child pass-through directories to find the first meaningful
/// branch point. A pass-through dir has zero direct source files and exactly one
/// immediate subdirectory. Stops when multiple children, direct files present,
/// or max depth (10) reached.
fn find_split_point(dir: &Path) -> PathBuf {
    fn inner(dir: &Path, depth: usize) -> PathBuf {
        if depth > 10 {
            return dir.to_path_buf();
        }
        let direct_files = ignore::WalkBuilder::new(dir)
            .max_depth(Some(1))
            .hidden(true)
            .git_ignore(true)
            .build()
            .flatten()
            .filter(|e| {
                e.file_type().map(|t| t.is_file()).unwrap_or(false)
                    && ast::detect_language(e.path()).is_some()
            })
            .count();

        if direct_files > 0 {
            return dir.to_path_buf();
        }

        let subdirs: Vec<PathBuf> = ignore::WalkBuilder::new(dir)
            .max_depth(Some(1))
            .hidden(true)
            .git_ignore(true)
            .build()
            .flatten()
            .filter(|e| e.depth() == 1 && e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| e.path().to_path_buf())
            .collect();

        if subdirs.len() == 1 {
            inner(&subdirs[0], depth + 1)
        } else {
            dir.to_path_buf()
        }
    }
    inner(dir, 0)
}
```

- [ ] **Step 4: Implement `count_files_by_subdir`** — add immediately after `find_split_point`:

```rust
/// Count source files in `dir` recursively, grouped by immediate subdirectory
/// of the meaningful split point (see `find_split_point`).
/// Returns `(total, Vec<(display_path, count)>)` sorted descending by count.
/// Files directly in the split point contribute to total but not to subdirs.
fn count_files_by_subdir(project_root: &Path, dir: &Path) -> (usize, Vec<(String, usize)>) {
    let split = find_split_point(dir);

    let walker = ignore::WalkBuilder::new(&split)
        .max_depth(None)
        .hidden(true)
        .git_ignore(true)
        .build();

    let mut total = 0usize;
    let mut subdir_counts: std::collections::HashMap<PathBuf, usize> =
        std::collections::HashMap::new();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if ast::detect_language(entry.path()).is_none() {
            continue;
        }
        total += 1;
        let abs = entry.path().to_path_buf();
        if let Ok(rel) = abs.strip_prefix(&split) {
            let components: Vec<_> = rel.components().collect();
            if components.len() > 1 {
                let first = split.join(components[0].as_os_str());
                *subdir_counts.entry(first).or_insert(0) += 1;
            }
        }
    }

    let mut subdirs: Vec<(String, usize)> = subdir_counts
        .into_iter()
        .map(|(abs_path, count)| {
            let display = abs_path
                .strip_prefix(project_root)
                .unwrap_or(&abs_path)
                .display()
                .to_string();
            (display, count)
        })
        .collect();
    subdirs.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    (total, subdirs)
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test find_split_point count_files_by_subdir 2>&1 | tail -15
```
Expected: all 7 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(list_symbols): add find_split_point + count_files_by_subdir helpers"
```

---

### Task 3: Implement ast_class_names_for_dir

**Files:**
- Modify: `src/tools/symbol.rs` — add helper after `count_files_by_subdir`, add tests

Scans **only immediate files** in a directory (depth 1) using tree-sitter AST (no LSP). Returns sorted, deduplicated class-like symbol names (kinds: `Class`, `Struct`, `Interface`, `Enum`, `Object`).

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn ast_class_names_for_dir_extracts_class_like_symbols() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("types.rs"),
        r#"
struct Foo { x: i32 }
struct Bar;
enum Baz { A, B }
fn not_a_class() {}
const SKIP: i32 = 1;
"#,
    )
    .unwrap();
    std::fs::write(dir.path().join("README.md"), "# hi").unwrap();

    let names = ast_class_names_for_dir(dir.path());

    assert!(names.contains(&"Foo".to_string()));
    assert!(names.contains(&"Bar".to_string()));
    assert!(names.contains(&"Baz".to_string()));
    assert!(!names.contains(&"not_a_class".to_string()));
    assert!(!names.contains(&"SKIP".to_string()));
    // sorted
    assert_eq!(names, { let mut v = names.clone(); v.sort(); v });
}

#[test]
fn ast_class_names_for_dir_does_not_recurse_into_subdirs() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/deep.rs"), "struct DeepClass;").unwrap();
    std::fs::write(dir.path().join("top.rs"), "struct TopClass;").unwrap();

    let names = ast_class_names_for_dir(dir.path());

    assert!(names.contains(&"TopClass".to_string()));
    assert!(!names.contains(&"DeepClass".to_string()));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test ast_class_names_for_dir 2>&1 | tail -10
```
Expected: compile error.

- [ ] **Step 3: Implement `ast_class_names_for_dir`** — add after `count_files_by_subdir`:

```rust
/// Extract top-level class-like symbol names from source files directly in `dir`
/// (depth 1, no recursion). Uses tree-sitter AST only — no LSP.
/// Kinds included: Class, Struct, Interface, Enum, Object.
/// Returns sorted, deduplicated names.
fn ast_class_names_for_dir(dir: &Path) -> Vec<String> {
    use crate::lsp::symbols::SymbolKind;

    let walker = ignore::WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(true)
        .git_ignore(true)
        .build();

    let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if ast::detect_language(entry.path()).is_none() {
            continue;
        }
        if let Ok(symbols) = ast::extract_symbols(entry.path()) {
            for sym in &symbols {
                match sym.kind {
                    SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Interface
                    | SymbolKind::Enum
                    | SymbolKind::Object => {
                        names.insert(sym.name.clone());
                    }
                    _ => {}
                }
            }
        }
    }

    let mut result: Vec<String> = names.into_iter().collect();
    result.sort();
    result
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test ast_class_names_for_dir 2>&1 | tail -10
```
Expected: both pass.

- [ ] **Step 5: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(list_symbols): add ast_class_names_for_dir helper"
```

---

### Task 4: Add force_mode param and replace directory branch

**Files:**
- Modify: `src/tools/symbol.rs:552-565` — `input_schema` method
- Modify: `src/tools/symbol.rs:718-850` — `else if full_path.is_dir()` branch in `call`

`force_mode: "auto" | "symbols"` — default `"auto"` uses size-based dispatch; `"symbols"` bypasses it, always returning full symbol output. Prevents silent param-ignoring when agents pass `depth` or `include_docs`.

- [ ] **Step 1: Write the integration test**

```rust
#[tokio::test]
async fn list_symbols_nested_dir_returns_overview_mode() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    // sub_a and sub_b each with 20 Rust files (total=40 > RECURSE_SMALL=30)
    for sub in &["sub_a", "sub_b"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
        for i in 0..20 {
            std::fs::write(root.join(format!("{sub}/f{i}.rs")), "pub struct S;").unwrap();
        }
    }
    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = ListSymbols
        .call(json!({ "path": "." }), &ctx)
        .await
        .unwrap();

    // 40 files in two subdirs → class_overview (31–80 range)
    assert_eq!(result["mode"].as_str(), Some("class_overview"));
    let subdirs = result["subdirectories"].as_array().unwrap();
    assert_eq!(subdirs.len(), 2);
    assert_eq!(result["total_files"].as_u64(), Some(40));
    let sub_a = subdirs
        .iter()
        .find(|s| s["path"].as_str().unwrap_or("").contains("sub_a"))
        .unwrap();
    assert!(
        sub_a["classes"].as_array().unwrap().iter().any(|c| c.as_str() == Some("S")),
        "AST class names extracted"
    );
}

#[tokio::test]
async fn list_symbols_force_mode_symbols_bypasses_threshold() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    for sub in &["sub_a", "sub_b"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
        for i in 0..20 {
            std::fs::write(root.join(format!("{sub}/f{i}.rs")), "pub struct S;").unwrap();
        }
    }
    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: buf(),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };

    let result = ListSymbols
        .call(json!({ "path": ".", "force_mode": "symbols" }), &ctx)
        .await
        .unwrap();

    // force_mode: "symbols" → no "mode" key, returns files array
    assert!(result["mode"].is_null(), "no mode field in symbols output");
    assert!(result["files"].is_array(), "files array present");
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test list_symbols_nested_dir_returns_overview_mode list_symbols_force_mode_symbols_bypasses_threshold 2>&1 | tail -10
```
Expected: compile or runtime failure.

- [ ] **Step 3: Add `force_mode` to `input_schema`** — in `impl Tool for ListSymbols`, find the `input_schema` method (~line 552) and add `force_mode` to the properties object:

```json
"force_mode": {
    "type": "string",
    "enum": ["auto", "symbols"],
    "description": "Override mode selection. 'symbols' forces full symbol output regardless of directory size. Default: 'auto'."
}
```

- [ ] **Step 4: Replace the directory branch** in `ListSymbols::call`. Find `} else if full_path.is_dir() {` (~line 718) and replace entire branch body:

```rust
} else if full_path.is_dir() {
    let root = ctx.agent.require_project_root().await?;
    let force_symbols = input["force_mode"].as_str() == Some("symbols");
    let (total_files, subdir_counts) = count_files_by_subdir(&root, &full_path);

    // Flat dir, small tree, or forced → full symbol mode
    let use_symbol_mode = force_symbols
        || total_files == 0
        || total_files <= LIST_SYMBOLS_RECURSE_SMALL
        || subdir_counts.is_empty();

    if use_symbol_mode {
        let mut dir_files: Vec<(String, PathBuf)> = vec![];

        if scope.includes_project() {
            let walker = ignore::WalkBuilder::new(&full_path)
                .max_depth(None)
                .hidden(true)
                .git_ignore(true)
                .build();
            for entry in walker.flatten() {
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    continue;
                }
                if ast::detect_language(entry.path()).is_none() {
                    continue;
                }
                let abs = entry.path().to_path_buf();
                let display = abs
                    .strip_prefix(&root)
                    .unwrap_or(&abs)
                    .display()
                    .to_string();
                dir_files.push((display, abs));
            }
        }

        let lib_roots = resolve_library_roots(&scope, &ctx.agent).await?;
        for (lib_name, lib_root) in &lib_roots {
            let walker = ignore::WalkBuilder::new(lib_root)
                .max_depth(None)
                .hidden(true)
                .git_ignore(false)
                .build();
            for entry in walker.flatten() {
                if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    continue;
                }
                if ast::detect_language(entry.path()).is_none() {
                    continue;
                }
                let abs = entry.path().to_path_buf();
                let display = format_library_path(lib_name, lib_root, &abs);
                dir_files.push((display, abs));
            }
        }

        let mut guard = guard;
        guard.max_files = guard.max_files.min(LIST_SYMBOLS_MAX_FILES);
        let (dir_files, file_overflow) =
            guard.cap_files(dir_files, "Narrow with a more specific glob or file path");
        let include_body = guard.should_include_body();

        let mut result = vec![];
        for (display_path, abs_path) in &dir_files {
            let Some(lang) = ast::detect_language(abs_path) else {
                continue;
            };
            let language_id = crate::lsp::servers::lsp_language_id(lang);

            let mut symbols = if let Ok(client) = ctx.lsp.get_or_start(lang, &root).await {
                let timer = LspTimer::start();
                let syms = client
                    .document_symbols(abs_path, language_id)
                    .await
                    .unwrap_or_default();
                if !syms.is_empty() {
                    timer.record(ctx, lang, &root).await;
                }
                syms
            } else {
                vec![]
            };

            if symbols.is_empty() {
                symbols = crate::ast::extract_symbols(abs_path).unwrap_or_default();
            }

            if symbols.is_empty() {
                continue;
            }

            let source = if include_body {
                std::fs::read_to_string(abs_path).ok()
            } else {
                None
            };
            let json_symbols: Vec<Value> = symbols
                .iter()
                .map(|s| {
                    symbol_to_json(
                        s,
                        include_body,
                        source.as_deref(),
                        depth.saturating_sub(1),
                        false,
                    )
                })
                .collect();
            let mut entry = json!({
                "file": display_path,
                "symbols": json_symbols,
            });
            if include_docs {
                entry["docstrings"] = json!(collect_docstrings(abs_path));
            }
            result.push(entry);
        }
        let mut result_json = json!({ "directory": rel_path, "files": result });
        if let Some(ov) = file_overflow {
            result_json["overflow"] = OutputGuard::overflow_json(&ov);
        }
        return Ok(result_json);
    }

    // class_overview mode: 31–80 files, has subdirs
    if total_files <= LIST_SYMBOLS_RECURSE_MEDIUM {
        let subdirs_json: Vec<Value> = subdir_counts
            .iter()
            .map(|(path, count)| {
                let subdir_abs = root.join(path);
                let classes = ast_class_names_for_dir(&subdir_abs);
                json!({
                    "path": path,
                    "file_count": count,
                    "classes": classes,
                })
            })
            .collect();
        let hint = format!(
            "Found {total_files} files across {} directories — showing top-level classes (AST). \
             Drill down with list_symbols('<subdir>') for full symbols, or \
             list_symbols('{rel_path}/**/*') to scan the full tree.",
            subdir_counts.len()
        );
        return Ok(json!({
            "directory": rel_path,
            "mode": "class_overview",
            "subdirectories": subdirs_json,
            "total_files": total_files,
            "hint": hint,
        }));
    }

    // directory_map mode: > 80 files
    let shown_subdirs: Vec<Value> = subdir_counts
        .iter()
        .take(LIST_SYMBOLS_MAX_SUBDIRS)
        .map(|(path, count)| json!({ "path": path, "file_count": count }))
        .collect();

    let overflow = if subdir_counts.len() > LIST_SYMBOLS_MAX_SUBDIRS {
        Some(json!({
            "shown": LIST_SYMBOLS_MAX_SUBDIRS,
            "total": subdir_counts.len(),
            "hint": format!(
                "Showing {} of {} directories (largest first).",
                LIST_SYMBOLS_MAX_SUBDIRS,
                subdir_counts.len()
            ),
        }))
    } else {
        None
    };

    let hint = format!(
        "Found {total_files} files across {} directories — too large for symbol overview. \
         Drill down with list_symbols('<subdir>') or use \
         list_symbols('{rel_path}/**/*') to scan the full tree with file cap.",
        subdir_counts.len()
    );

    let mut result = json!({
        "directory": rel_path,
        "mode": "directory_map",
        "subdirectories": shown_subdirs,
        "total_files": total_files,
        "hint": hint,
    });
    if let Some(ov) = overflow {
        result["overflow"] = ov;
    }
    Ok(result)
```

- [ ] **Step 5: Compile check**

```bash
cargo check 2>&1 | head -30
```
Expected: no errors.

- [ ] **Step 6: Run integration tests**

```bash
cargo test list_symbols_nested_dir_returns_overview_mode list_symbols_force_mode_symbols_bypasses_threshold 2>&1 | tail -15
```
Expected: both pass.

- [ ] **Step 7: Run all list_symbols tests for regressions**

```bash
cargo test list_symbols 2>&1 | tail -30
```
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(list_symbols): three-mode progressive directory dispatch + force_mode param"
```

---

### Task 5: Update format_list_symbols for new output shapes

**Files:**
- Modify: `src/tools/symbol.rs:2802–2854` — `format_list_symbols` function

The current function falls through to directory/pattern mode using `val["files"]`. The new `class_overview` and `directory_map` shapes have no `"files"` key — they render as `"0 symbols"` today. Add a branch for `val["mode"]`.

- [ ] **Step 1: Write the failing unit tests**

```rust
#[test]
fn format_list_symbols_class_overview_mode() {
    let val = serde_json::json!({
        "directory": "src/main/kotlin",
        "mode": "class_overview",
        "subdirectories": [
            { "path": "src/main/kotlin/api",    "file_count": 12, "classes": ["CourseController", "PlannerApi"] },
            { "path": "src/main/kotlin/domain", "file_count": 8,  "classes": ["Course", "Student"] }
        ],
        "total_files": 45,
        "hint": "Found 45 files — drill down with list_symbols('<subdir>')."
    });
    let result = format_list_symbols(&val);
    assert!(result.contains("src/main/kotlin"));
    assert!(result.contains("45 files"));
    assert!(result.contains("api"));
    assert!(result.contains("12"));
    assert!(result.contains("CourseController"));
    assert!(result.contains("domain"));
    assert!(result.contains("Course"));
    assert!(result.contains("drill down"), "hint shown");
}

#[test]
fn format_list_symbols_directory_map_mode() {
    let val = serde_json::json!({
        "directory": "ktor-server/src",
        "mode": "directory_map",
        "subdirectories": [
            { "path": "ktor-server/src/main", "file_count": 80 },
            { "path": "ktor-server/src/test", "file_count": 40 }
        ],
        "total_files": 120,
        "hint": "Found 120 files — too large for symbol overview."
    });
    let result = format_list_symbols(&val);
    assert!(result.contains("ktor-server/src"));
    assert!(result.contains("120 files"));
    assert!(result.contains("src/main"));
    assert!(result.contains("80"));
    assert!(result.contains("too large"));
}

#[test]
fn format_list_symbols_directory_map_with_overflow() {
    let subdirs: Vec<serde_json::Value> = (0..15)
        .map(|i| serde_json::json!({ "path": format!("sub/{i}"), "file_count": 10 }))
        .collect();
    let val = serde_json::json!({
        "directory": "big",
        "mode": "directory_map",
        "subdirectories": subdirs,
        "total_files": 300,
        "overflow": { "shown": 15, "total": 23, "hint": "Showing 15 of 23 directories (largest first)." },
        "hint": "Found 300 files."
    });
    let result = format_list_symbols(&val);
    assert!(result.contains("Showing 15 of 23"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test format_list_symbols_class_overview format_list_symbols_directory_map 2>&1 | tail -15
```
Expected: FAIL — output is empty or "0 symbols".

- [ ] **Step 3: Add overview branch to `format_list_symbols`** — insert after the file-mode `return out;` (~line 2820), before the directory/pattern branch:

```rust
    // class_overview / directory_map mode
    if let Some(mode) = val["mode"].as_str() {
        let dir = val["directory"].as_str().unwrap_or(".");
        let total = val["total_files"].as_u64().unwrap_or(0);
        let empty: Vec<Value> = vec![];
        let subdirs = val["subdirectories"].as_array().unwrap_or(&empty);

        let mut out = format!("{dir} — {total} files\n");

        for subdir in subdirs {
            let path = subdir["path"].as_str().unwrap_or("?");
            let count = subdir["file_count"].as_u64().unwrap_or(0);
            out.push_str(&format!("\n  {path} ({count} files)"));
            if mode == "class_overview" {
                let empty_arr: Vec<Value> = vec![];
                let classes = subdir["classes"].as_array().unwrap_or(&empty_arr);
                if !classes.is_empty() {
                    let names: Vec<&str> = classes.iter().filter_map(|v| v.as_str()).collect();
                    out.push_str(&format!("\n    {}", names.join(", ")));
                }
            }
        }

        if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
            out.push('\n');
            out.push_str(&format_overflow(overflow));
        }

        if let Some(hint) = val["hint"].as_str() {
            out.push_str(&format!("\n\n{hint}"));
        }

        return out;
    }
```

- [ ] **Step 4: Run new tests**

```bash
cargo test format_list_symbols_class_overview format_list_symbols_directory_map 2>&1 | tail -15
```
Expected: all pass.

- [ ] **Step 5: Run all format_list_symbols tests**

```bash
cargo test format_list_symbols 2>&1 | tail -20
```
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(list_symbols): render class_overview and directory_map in format_compact"
```

---

### Task 6: Update server instructions

**Files:**
- Modify: `src/prompts/server_instructions.md`

This is an API contract change. Agents parsing `result["files"]` will get `null` in overview modes. Server instructions must tell agents how to detect the mode and use `force_mode`.

- [ ] **Step 1: Read the current list_symbols section**

```bash
grep -n "list_symbols" src/prompts/server_instructions.md | head -20
```

- [ ] **Step 2: Add documentation for the new response shapes**

Find the `list_symbols` entry in server_instructions.md and append (or add nearby) a note like:

```markdown
**`list_symbols` directory responses vary by tree size:**
- Small tree (≤30 files) or `force_mode: "symbols"`: `{ "directory": ..., "files": [...] }` — existing shape
- Medium tree (31–80 files): `{ "mode": "class_overview", "subdirectories": [...], "total_files": N, "hint": "..." }`
- Large tree (>80 files): `{ "mode": "directory_map", "subdirectories": [...], "total_files": N, "hint": "..." }`

Check `result["mode"]` to detect which shape was returned. Use `force_mode: "symbols"` to always get the `files` array (e.g. when you need full symbols regardless of tree size).
```

- [ ] **Step 3: Compile check (server_instructions is embedded at build time — verify no issues)**

```bash
cargo check 2>&1 | head -10
```

- [ ] **Step 4: Commit**

```bash
git add src/prompts/server_instructions.md
git commit -m "docs(server_instructions): document list_symbols progressive directory response shapes"
```

---

### Task 7: Final verification

- [ ] **Step 1: cargo fmt**

```bash
cargo fmt
git diff --stat
```
If changes: `git add -u && git commit -m "style: cargo fmt"`

- [ ] **Step 2: cargo clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep -E "^error|warning\[" | head -20
```
Expected: no errors, no new warnings. Fix any before continuing.

- [ ] **Step 3: Full test suite**

```bash
cargo test 2>&1 | tail -30
```
Expected: all pass.

- [ ] **Step 4: Release build**

```bash
cargo build --release 2>&1 | tail -5
```
Expected: `Finished release [optimized]`.

- [ ] **Step 5: Smoke test via MCP**

Run `/mcp` to restart, then call:
```
list_symbols("ktor-server/src/main/kotlin")    # expect class_overview or directory_map
list_symbols("src")                             # small tree → symbols mode
list_symbols("src/main.rs")                    # file mode unchanged
list_symbols("src/**/*.rs")                    # glob mode unchanged
list_symbols("src", force_mode="symbols")      # force_mode bypass
```

- [ ] **Step 6: Cherry-pick to master**

```bash
git log --oneline experiments ^master | head -10  # review commits to cherry-pick
git checkout master
git cherry-pick <sha1> <sha2> <sha3> <sha4> <sha5> <sha6>
git push
git checkout experiments
git rebase master
```
