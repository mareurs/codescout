# Library Scope Wiring Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the `scope` parameter into `find_symbol`, `list_symbols`, and `find_references` so they can search registered library directories, and add manual library registration.

**Architecture:** Libraries are external codebases (e.g. third-party crates, npm packages) whose roots live outside the project directory. The `LibraryRegistry` already tracks them, `Scope` enum already parses scope strings, and `goto_definition` already auto-discovers libraries. The missing piece is that the three LSP symbol tools parse `scope` into `_scope` (unused). We wire scope by: (1) extracting a shared helper that resolves which directories to search for a given scope, (2) modifying each tool's search loop to include library roots alongside (or instead of) the project root, and (3) displaying library file paths as `lib:<name>/relative/path`. We also add a `register_library` tool for manual registration and remove the dead `IndexLibrary` struct.

**Tech Stack:** Rust, LSP (document_symbols, workspace/symbol), tree-sitter fallback, ignore crate for directory walking.

**Key constraint — LSP workspace roots:** `workspace/symbol` only indexes the project workspace root, so it cannot find library symbols. For library directories, we always use the file-walking + `document_symbols` path. Each library file gets its `document_symbols` request sent through the project root's LSP server — `document_symbols` is a per-file operation that works on any file the server can parse, regardless of workspace root. This works for all 13 supported languages.

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/tools/symbol.rs` | Modify | Wire scope into `find_symbol`, `list_symbols`, `find_references`; add helpers |
| `src/tools/library.rs` | Modify | Add `RegisterLibrary` tool, remove dead `IndexLibrary` struct |
| `src/server.rs` | Modify | Register `RegisterLibrary` tool |
| `src/library/scope.rs` | No change | Already correct |
| `src/library/registry.rs` | No change | Already correct |

---

## Chunk 1: Shared Helpers and `list_symbols` Scope Wiring

### Task 1: Add helper to resolve library search roots

**Files:**
- Modify: `src/tools/symbol.rs` (add helper function near top, around line 46)

The helper takes a `Scope` and the `Agent` and returns a list of `(display_prefix, absolute_path)` tuples for library directories to search.

- [ ] **Step 1: Write the failing test**

In the `tests` module at the bottom of `src/tools/symbol.rs`, add:

```rust
#[tokio::test]
async fn resolve_library_roots_empty_when_no_libraries() {
    let dir = tempdir().unwrap();
    let agent = Agent::activate(dir.path().to_path_buf()).await.unwrap();
    let roots = resolve_library_roots(&Scope::Libraries, &agent).await;
    assert!(roots.is_empty());
}

#[tokio::test]
async fn resolve_library_roots_returns_registered_libraries() {
    let dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    let agent = Agent::activate(dir.path().to_path_buf()).await.unwrap();
    // Register a library
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project.as_mut().unwrap();
        project.library_registry.register(
            "mylib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
        );
    }
    let roots = resolve_library_roots(&Scope::Libraries, &agent).await;
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].0, "mylib");
    assert_eq!(roots[0].1, lib_dir.path().to_path_buf());
}

#[tokio::test]
async fn resolve_library_roots_filters_by_name() {
    let dir = tempdir().unwrap();
    let lib1 = tempdir().unwrap();
    let lib2 = tempdir().unwrap();
    let agent = Agent::activate(dir.path().to_path_buf()).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project.as_mut().unwrap();
        project.library_registry.register(
            "alpha".to_string(), lib1.path().to_path_buf(),
            "rust".to_string(), crate::library::registry::DiscoveryMethod::Manual,
        );
        project.library_registry.register(
            "beta".to_string(), lib2.path().to_path_buf(),
            "rust".to_string(), crate::library::registry::DiscoveryMethod::Manual,
        );
    }
    let roots = resolve_library_roots(&Scope::Library("alpha".to_string()), &agent).await;
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].0, "alpha");
}

#[tokio::test]
async fn resolve_library_roots_project_scope_returns_empty() {
    let dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    let agent = Agent::activate(dir.path().to_path_buf()).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project.as_mut().unwrap();
        project.library_registry.register(
            "mylib".to_string(), lib_dir.path().to_path_buf(),
            "rust".to_string(), crate::library::registry::DiscoveryMethod::Manual,
        );
    }
    let roots = resolve_library_roots(&Scope::Project, &agent).await;
    assert!(roots.is_empty());
}

#[tokio::test]
async fn resolve_library_roots_all_scope_returns_all() {
    let dir = tempdir().unwrap();
    let lib1 = tempdir().unwrap();
    let lib2 = tempdir().unwrap();
    let agent = Agent::activate(dir.path().to_path_buf()).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project.as_mut().unwrap();
        project.library_registry.register(
            "alpha".to_string(), lib1.path().to_path_buf(),
            "rust".to_string(), crate::library::registry::DiscoveryMethod::Manual,
        );
        project.library_registry.register(
            "beta".to_string(), lib2.path().to_path_buf(),
            "python".to_string(), crate::library::registry::DiscoveryMethod::Manual,
        );
    }
    let roots = resolve_library_roots(&Scope::All, &agent).await;
    assert_eq!(roots.len(), 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test resolve_library_roots -- --nocapture`
Expected: FAIL — `resolve_library_roots` function does not exist.

- [ ] **Step 3: Write the implementation**

Add at the top of `src/tools/symbol.rs` (after the existing helper functions, around line 46):

```rust
use crate::library::scope::Scope;

/// Resolve which library directories to search for a given scope.
/// Returns `(library_name, absolute_root_path)` pairs.
async fn resolve_library_roots(
    scope: &Scope,
    agent: &crate::agent::Agent,
) -> Vec<(String, PathBuf)> {
    let registry = match agent.library_registry().await {
        Some(r) => r,
        None => return vec![],
    };
    registry
        .all()
        .iter()
        .filter(|entry| scope.includes_library(&entry.name))
        .map(|entry| (entry.name.clone(), entry.path.clone()))
        .collect()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test resolve_library_roots -- --nocapture`
Expected: All 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(library): add resolve_library_roots helper for scope-based searching"
```

---

### Task 2: Add helper to format library-relative file paths

**Files:**
- Modify: `src/tools/symbol.rs` (add helper near `resolve_library_roots`)

Library file paths need to display as `lib:name/relative/path.rs` instead of absolute paths.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn format_library_path_strips_root() {
    let lib_root = PathBuf::from("/home/user/.cargo/registry/src/serde-1.0");
    let file = PathBuf::from("/home/user/.cargo/registry/src/serde-1.0/src/lib.rs");
    let result = format_library_path("serde", &lib_root, &file);
    assert_eq!(result, "lib:serde/src/lib.rs");
}

#[test]
fn format_library_path_fallback_for_outside_root() {
    let lib_root = PathBuf::from("/home/user/.cargo/registry/src/serde-1.0");
    let file = PathBuf::from("/somewhere/else/lib.rs");
    let result = format_library_path("serde", &lib_root, &file);
    assert_eq!(result, "/somewhere/else/lib.rs");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test format_library_path -- --nocapture`
Expected: FAIL — function does not exist.

- [ ] **Step 3: Write the implementation**

```rust
/// Format a file path relative to a library root for display.
/// Returns `lib:<name>/<relative_path>` or the absolute path as fallback.
fn format_library_path(lib_name: &str, lib_root: &Path, file_path: &Path) -> String {
    file_path
        .strip_prefix(lib_root)
        .map(|rel| format!("lib:{}/{}", lib_name, rel.display()))
        .unwrap_or_else(|_| file_path.display().to_string())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test format_library_path -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(library): add format_library_path display helper"
```

---

### Task 3: Wire scope into `list_symbols` directory path

**Files:**
- Modify: `src/tools/symbol.rs` — the `ListSymbols::call` method (starts at line ~379)

When scope includes libraries and the path is `"."` or empty (project root), also walk library directories. When scope is `Libraries` only, skip the project root.

**Key design:** When `path` is explicit (a specific file or subdirectory), scope is ignored — the user asked for a specific location. Scope only affects the default "walk the project" behavior when path is `"."` or empty.

- [ ] **Step 1: Write the failing test**

Add a test that creates a temp project with a registered library directory containing a Rust file, calls `list_symbols` with `scope="libraries"`, and asserts the library file appears in results with `lib:name/` prefix.

```rust
#[tokio::test]
async fn list_symbols_scope_libraries_includes_library_files() {
    let project_dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    // Create a source file in the library
    let lib_src = lib_dir.path().join("src");
    std::fs::create_dir_all(&lib_src).unwrap();
    std::fs::write(lib_src.join("lib.rs"), "pub fn hello() {}\n").unwrap();

    let agent = Agent::activate(project_dir.path().to_path_buf()).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project.as_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = ListSymbols;
    let result = tool.call(json!({"scope": "libraries"}), &ctx).await.unwrap();

    // Should contain a file entry with lib:testlib/ prefix
    let files = result["files"].as_array().unwrap();
    assert!(!files.is_empty(), "should find library files");
    let first_file = files[0]["file"].as_str().unwrap();
    assert!(
        first_file.starts_with("lib:testlib/"),
        "library file should have lib: prefix, got: {}",
        first_file
    );
}

#[tokio::test]
async fn list_symbols_scope_project_excludes_libraries() {
    let project_dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    std::fs::create_dir_all(lib_dir.path().join("src")).unwrap();
    std::fs::write(lib_dir.path().join("src/lib.rs"), "pub fn hello() {}\n").unwrap();
    // Also create a project file
    std::fs::write(project_dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    let agent = Agent::activate(project_dir.path().to_path_buf()).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project.as_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(), lib_dir.path().to_path_buf(),
            "rust".to_string(), crate::library::registry::DiscoveryMethod::Manual,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = ListSymbols;
    let result = tool.call(json!({"scope": "project"}), &ctx).await.unwrap();

    let files = result["files"].as_array().unwrap_or(&vec![]);
    for f in files {
        let path = f["file"].as_str().unwrap();
        assert!(!path.starts_with("lib:"), "project scope should not include library files: {}", path);
    }
}
```

**Note:** These tests use tree-sitter fallback (no real LSP server), which is the standard pattern for unit tests in this file. The `test_ctx_with_agent` helper needs to exist — check if there's already a similar helper in the test module. If not, create one modeled on the existing `project_ctx` pattern in `library.rs` tests.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test list_symbols_scope -- --nocapture`
Expected: FAIL — scope is ignored, library files not found.

- [ ] **Step 3: Write the implementation**

In `ListSymbols::call`, modify the directory-walking branch (the `full_path.is_dir()` case starting around line 524). After collecting project files (when `scope.includes_project()`), also walk library roots (when scope includes libraries):

**Change 1:** Replace `let _scope = ...` with `let scope = ...` (remove underscore).

**Change 2:** In the `full_path.is_dir()` branch, wrap the existing directory walk in a `if scope.includes_project()` guard, then add a second loop for libraries:

```rust
// After the existing project file collection, add library files:
let lib_roots = resolve_library_roots(&scope, &ctx.agent).await;
for (lib_name, lib_root) in &lib_roots {
    if !lib_root.exists() {
        continue;
    }
    let lib_walker = ignore::WalkBuilder::new(lib_root)
        .hidden(true)
        .git_ignore(true)
        .build();
    for entry in lib_walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        dir_files.push((
            format_library_path(lib_name, lib_root, entry.path()),
            entry.path().to_path_buf(),
        ));
    }
}
```

**Important refactor needed:** The existing code collects `Vec<PathBuf>` for `dir_files` and later computes display paths by stripping the project root. For library files, the display path is different (`lib:name/...`). Refactor `dir_files` to be `Vec<(String, PathBuf)>` where the first element is the display path and the second is the absolute path. For project files, the display path is `path.strip_prefix(&root)`.

**Change 3:** When `scope` is `Libraries` or `Library(_)` and `rel_path` is `"."` or empty, skip the project directory walk entirely. When `scope` is `All`, do both.

**Change 4:** In the glob and single-file branches, scope is ignored (explicit path overrides scope). No changes needed there.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test list_symbols_scope -- --nocapture`
Expected: Both tests PASS.

- [ ] **Step 5: Run full test suite**

Run: `cargo test -- --nocapture 2>&1 | tail -5`
Expected: All existing tests still pass. The refactor from `Vec<PathBuf>` to `Vec<(String, PathBuf)>` must not break any existing test.

- [ ] **Step 6: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(library): wire scope into list_symbols for library directory searching"
```

---

## Chunk 2: `find_symbol` and `find_references` Scope Wiring

### Task 4: Wire scope into `find_symbol`

**Files:**
- Modify: `src/tools/symbol.rs` — the `FindSymbol::call` method (starts at line ~714)

`find_symbol` has two search paths:
1. **Path specified:** Walk files in that path + `document_symbols` per file. Scope is ignored (explicit path).
2. **No path:** Fast path via `workspace/symbol` (one LSP request per language), then tree-sitter fallback.

For scope wiring, we modify path 2:
- When `scope.includes_project()`: run the existing `workspace/symbol` + tree-sitter fallback (unchanged).
- When scope includes libraries: additionally walk each library directory and use `document_symbols` per file (same approach as `list_symbols`). Also extend the tree-sitter fallback to walk library directories.
- File display paths use `format_library_path` for library files.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn find_symbol_scope_libraries_searches_library_dirs() {
    let project_dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    std::fs::create_dir_all(lib_dir.path().join("src")).unwrap();
    std::fs::write(
        lib_dir.path().join("src/lib.rs"),
        "pub fn library_unique_symbol_xyz() {}\n",
    ).unwrap();

    let agent = Agent::activate(project_dir.path().to_path_buf()).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project.as_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(),
            lib_dir.path().to_path_buf(),
            "rust".to_string(),
            crate::library::registry::DiscoveryMethod::Manual,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = FindSymbol;
    let result = tool.call(json!({
        "pattern": "library_unique_symbol_xyz",
        "scope": "libraries"
    }), &ctx).await.unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    assert!(!symbols.is_empty(), "should find symbol in library");
    let file = symbols[0]["file"].as_str().unwrap();
    assert!(file.starts_with("lib:testlib/"), "file path should have lib: prefix: {}", file);
}

#[tokio::test]
async fn find_symbol_scope_all_searches_both() {
    let project_dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    // Project file
    std::fs::write(project_dir.path().join("main.rs"), "fn project_func() {}\n").unwrap();
    // Library file
    std::fs::create_dir_all(lib_dir.path().join("src")).unwrap();
    std::fs::write(lib_dir.path().join("src/lib.rs"), "pub fn lib_func() {}\n").unwrap();

    let agent = Agent::activate(project_dir.path().to_path_buf()).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project.as_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(), lib_dir.path().to_path_buf(),
            "rust".to_string(), crate::library::registry::DiscoveryMethod::Manual,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = FindSymbol;
    let result = tool.call(json!({
        "pattern": "func",
        "scope": "all"
    }), &ctx).await.unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    let files: Vec<&str> = symbols.iter().filter_map(|s| s["file"].as_str()).collect();
    assert!(files.iter().any(|f| f.starts_with("lib:testlib/")), "should include library symbol");
    assert!(files.iter().any(|f| !f.starts_with("lib:")), "should include project symbol");
}

#[tokio::test]
async fn find_symbol_scope_project_default_excludes_libraries() {
    let project_dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    std::fs::write(project_dir.path().join("main.rs"), "fn my_func() {}\n").unwrap();
    std::fs::create_dir_all(lib_dir.path().join("src")).unwrap();
    std::fs::write(lib_dir.path().join("src/lib.rs"), "pub fn my_func() {}\n").unwrap();

    let agent = Agent::activate(project_dir.path().to_path_buf()).await.unwrap();
    {
        let mut inner = agent.inner.write().await;
        let project = inner.active_project.as_mut().unwrap();
        project.library_registry.register(
            "testlib".to_string(), lib_dir.path().to_path_buf(),
            "rust".to_string(), crate::library::registry::DiscoveryMethod::Manual,
        );
    }

    let ctx = test_ctx_with_agent(agent);
    let tool = FindSymbol;
    let result = tool.call(json!({
        "pattern": "my_func",
        "scope": "project"
    }), &ctx).await.unwrap();

    let symbols = result["symbols"].as_array().unwrap();
    for s in symbols {
        let file = s["file"].as_str().unwrap();
        assert!(!file.starts_with("lib:"), "project scope should not include library: {}", file);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test find_symbol_scope -- --nocapture`
Expected: FAIL — scope is ignored.

- [ ] **Step 3: Write the implementation**

In `FindSymbol::call`:

**Change 1:** Replace `let _scope = ...` with `let scope = ...`.

**Change 2:** In the no-path branch (the `else` block starting around line 790), wrap the `workspace/symbol` + tree-sitter fallback in `if scope.includes_project() { ... }`.

**Change 3:** After the project search block, add library searching:

```rust
// Search library directories when scope includes them
let lib_roots = resolve_library_roots(&scope, &ctx.agent).await;
for (lib_name, lib_root) in &lib_roots {
    if !lib_root.exists() {
        continue;
    }
    let walker = ignore::WalkBuilder::new(lib_root)
        .hidden(true)
        .git_ignore(true)
        .build();
    for entry in walker.flatten() {
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path();
        let Some(lang) = ast::detect_language(path) else {
            continue;
        };

        // Try LSP document_symbols, fall back to tree-sitter
        let symbols = if let Ok(client) = ctx.lsp.get_or_start(lang, &root).await {
            let language_id = crate::lsp::servers::lsp_language_id(lang);
            client.document_symbols(path, language_id).await.unwrap_or_default()
        } else {
            crate::ast::extract_symbols(path).unwrap_or_default()
        };

        let source = if include_body {
            std::fs::read_to_string(path).ok()
        } else {
            None
        };

        // Collect matching symbols, rewriting file paths to lib: prefix
        for sym in &symbols {
            if name_ok(sym) && kind_filter.map_or(true, |f| matches_kind_filter(&sym.kind, f)) {
                let mut json = symbol_to_json(sym, include_body, source.as_deref(), depth, true);
                // Overwrite file path with library-relative display path
                if let Some(obj) = json.as_object_mut() {
                    obj.insert(
                        "file".to_string(),
                        json!(format_library_path(lib_name, lib_root, path)),
                    );
                }
                matches.push(json);
            }
        }

        if matches.len() > guard.max_results * 2 {
            break;
        }
    }
}
```

**CRITICAL — LSP workspace root invariant:** Always pass `&root` (project root) to `ctx.lsp.get_or_start(lang, &root)`, even for library files. The LSP manager caches one client per language. If you accidentally pass a library root as `workspace_root`, it will **kill and restart** the LSP server for that language (since workspace_root differs from the cached client's root), destroying all workspace indexing. `document_symbols` is a per-file operation that works on any file regardless of workspace root, so using the project root's LSP server is correct. Add this comment in the implementation:

```rust
// INVARIANT: Always use project root as workspace_root, not the library root.
// LspManager caches one client per language; passing a different root kills
// and restarts the server, destroying workspace indexing. document_symbols
// works on any file regardless of workspace root.
```

**Change 4:** For the file path in `symbol_to_json`, library symbols need their `file` field rewritten. The current `symbol_to_json` uses `sym.file` which is absolute. After calling `symbol_to_json`, overwrite the `"file"` key with the library-relative display path.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test find_symbol_scope -- --nocapture`
Expected: All 3 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(library): wire scope into find_symbol for library symbol searching"
```

---

### Task 5: Wire scope into `find_references`

**Files:**
- Modify: `src/tools/symbol.rs` — the `FindReferences::call` method (starts at line ~989)

For `find_references`, scope affects **output filtering**, not where to search. The LSP `textDocument/references` request returns all references the server knows about. Scope filters which references are included in the response:
- `project` (default): only references inside the project root
- `libraries`: only references inside registered library paths
- `all`: all references (current behavior minus build-artifact filtering)
- `lib:<name>`: only references inside that library

This is simpler than `find_symbol`/`list_symbols` because the LSP does the hard work.

**Known limitation:** The project root's LSP server generally only indexes the project workspace and its direct dependencies. `scope="libraries"` will only show library references that the LSP server happened to discover (e.g. via dependency resolution). It will NOT proactively search unrelated library directories. This is an inherent LSP limitation — document it in the tool description so agents don't expect cross-project reference discovery.

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn find_references_scope_is_parsed() {
    // This is a unit test verifying the scope variable is used (not _scope).
    // Full integration testing of reference filtering requires a running LSP.
    // We verify the scope is wired by checking that the function compiles
    // without the unused-variable warning (the _scope -> scope rename).
    // The actual filtering logic is tested via the helper function.
}
```

Since `find_references` filtering can't be easily unit-tested without LSP, test the filtering helper:

```rust
#[test]
fn classify_reference_path_project() {
    let root = PathBuf::from("/project");
    let libs = vec![("mylib".to_string(), PathBuf::from("/libs/mylib"))];
    let path = PathBuf::from("/project/src/main.rs");
    let (classification, display) = classify_reference_path(&path, &root, &libs);
    assert_eq!(classification, "project");
    assert_eq!(display, "src/main.rs");
}

#[test]
fn classify_reference_path_library() {
    let root = PathBuf::from("/project");
    let libs = vec![("mylib".to_string(), PathBuf::from("/libs/mylib"))];
    let path = PathBuf::from("/libs/mylib/src/lib.rs");
    let (classification, display) = classify_reference_path(&path, &root, &libs);
    assert_eq!(classification, "lib:mylib");
    assert_eq!(display, "lib:mylib/src/lib.rs");
}

#[test]
fn classify_reference_path_external() {
    let root = PathBuf::from("/project");
    let libs = vec![("mylib".to_string(), PathBuf::from("/libs/mylib"))];
    let path = PathBuf::from("/somewhere/else.rs");
    let (classification, display) = classify_reference_path(&path, &root, &libs);
    assert_eq!(classification, "external");
    assert_eq!(display, "/somewhere/else.rs");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test classify_reference_path -- --nocapture`
Expected: FAIL — function does not exist.

- [ ] **Step 3: Write the implementation**

Add helper:

```rust
/// Classify a reference path as project, library, or external.
/// Returns (classification_tag, display_path).
fn classify_reference_path(
    path: &Path,
    project_root: &Path,
    library_roots: &[(String, PathBuf)],
) -> (String, String) {
    if path.starts_with(project_root) {
        let rel = path.strip_prefix(project_root).unwrap_or(path);
        ("project".to_string(), rel.display().to_string())
    } else if let Some((name, lib_root)) = library_roots.iter().find(|(_, r)| path.starts_with(r)) {
        ("lib:".to_string() + name, format_library_path(name, lib_root, path))
    } else {
        ("external".to_string(), path.display().to_string())
    }
}
```

Then in `FindReferences::call`:

**Change 1:** Replace `let _scope = ...` with `let scope = ...`.

**Change 2:** After getting `refs` from LSP and before the build-artifact filter, resolve library roots and apply scope filtering:

```rust
let lib_roots = resolve_library_roots(&Scope::All, &ctx.agent).await;

// Scope-filter references
let refs: Vec<_> = refs
    .into_iter()
    .filter(|loc| {
        let Some(path) = uri_to_path(loc.uri.as_str()) else {
            return true; // keep references we can't resolve
        };
        let (classification, _) = classify_reference_path(&path, &root, &lib_roots);
        match &scope {
            Scope::Project => classification == "project",
            Scope::Libraries => classification.starts_with("lib:"),
            Scope::All => true,
            Scope::Library(name) => classification == format!("lib:{}", name),
        }
    })
    .collect();
```

**Change 3:** In the display mapping, use `classify_reference_path` for proper library path display:

```rust
let (_, display_path) = classify_reference_path(&path, &root, &lib_roots);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test classify_reference_path -- --nocapture`
Expected: PASS.

- [ ] **Step 5: Run clippy and full tests**

Run: `cargo clippy -- -D warnings && cargo test -- --nocapture 2>&1 | tail -5`
Expected: Clean clippy, all tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/tools/symbol.rs
git commit -m "feat(library): wire scope into find_references with path classification"
```

---

## Chunk 3: Manual Registration and Cleanup

### Task 6: Add `RegisterLibrary` tool

**Files:**
- Modify: `src/tools/library.rs` — add `RegisterLibrary` struct + Tool impl
- Modify: `src/server.rs` — register the tool
This tool allows manual registration of a library by providing a name and absolute path. The language is auto-detected from manifest files (Cargo.toml, package.json, etc.) via the existing `discover_library_root` function, with a fallback to a user-provided `language` parameter.

**Security note:** `register_library` does NOT need to be gated behind `file_write_enabled` in `check_tool_access`. It only writes to `.codescout/libraries.json` (tool metadata), not to user source files. Leaving it ungated (like `github_repo`) is correct — a user who disables file writes to protect source code should still be able to register libraries.

- [ ] **Step 1: Write the failing test**

In `src/tools/library.rs` tests module:

```rust
#[tokio::test]
async fn register_library_manual() {
    let dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();
    // Create a Cargo.toml so discovery works
    std::fs::write(
        lib_dir.path().join("Cargo.toml"),
        "[package]\nname = \"mylib\"\nversion = \"0.1.0\"\n",
    ).unwrap();

    let agent = Agent::activate(dir.path().to_path_buf()).await.unwrap();
    let ctx = project_ctx_with_agent(agent.clone());
    let tool = RegisterLibrary;
    let result = tool.call(json!({
        "path": lib_dir.path().display().to_string(),
    }), &ctx).await.unwrap();

    assert_eq!(result["status"], "ok");
    assert_eq!(result["name"], "mylib");
    assert_eq!(result["language"], "rust");

    // Verify it shows up in list_libraries
    let reg = agent.library_registry().await.unwrap();
    assert_eq!(reg.all().len(), 1);
    assert_eq!(reg.all()[0].name, "mylib");
}

#[tokio::test]
async fn register_library_with_explicit_name_and_language() {
    let dir = tempdir().unwrap();
    let lib_dir = tempdir().unwrap();

    let agent = Agent::activate(dir.path().to_path_buf()).await.unwrap();
    let ctx = project_ctx_with_agent(agent.clone());
    let tool = RegisterLibrary;
    let result = tool.call(json!({
        "path": lib_dir.path().display().to_string(),
        "name": "custom-name",
        "language": "python",
    }), &ctx).await.unwrap();

    assert_eq!(result["status"], "ok");
    assert_eq!(result["name"], "custom-name");
    assert_eq!(result["language"], "python");
}

#[tokio::test]
async fn register_library_fails_for_nonexistent_path() {
    let dir = tempdir().unwrap();
    let agent = Agent::activate(dir.path().to_path_buf()).await.unwrap();
    let ctx = project_ctx_with_agent(agent);
    let tool = RegisterLibrary;
    let result = tool.call(json!({
        "path": "/nonexistent/path/to/lib",
    }), &ctx).await;

    assert!(result.is_err() || result.unwrap().get("error").is_some());
}
```

**Note:** `project_ctx_with_agent` is a new test helper. Create it in the test module:

```rust
fn project_ctx_with_agent(agent: Agent) -> ToolContext {
    let lsp = Arc::new(crate::lsp::mock::MockLspClient::new());
    let output_buffer = Arc::new(crate::tools::output_buffer::OutputBuffer::new());
    let progress = crate::tools::progress::ProgressReporter::new(None);
    ToolContext {
        agent,
        lsp,
        output_buffer,
        progress,
    }
}
```

Also add this helper to the `src/tools/symbol.rs` test module (same implementation) — both files need it for their respective tests.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test register_library -- --nocapture`
Expected: FAIL — `RegisterLibrary` does not exist.

- [ ] **Step 3: Write the implementation**

In `src/tools/library.rs`:

```rust
pub struct RegisterLibrary;

#[async_trait]
impl Tool for RegisterLibrary {
    fn name(&self) -> &str {
        "register_library"
    }
    fn description(&self) -> &str {
        "Register an external library for searching with scope='lib:<name>'. \
         Auto-detects name and language from manifest files (Cargo.toml, package.json, etc.). \
         Use name/language params to override auto-detection."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute path to the library root directory"
                },
                "name": {
                    "type": "string",
                    "description": "Library name (auto-detected from manifest if omitted)"
                },
                "language": {
                    "type": "string",
                    "description": "Primary language (auto-detected if omitted)"
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> Result<Value> {
        let raw_path = super::require_str_param(&input, "path")?;
        let lib_path = PathBuf::from(raw_path);

        if !lib_path.exists() {
            return Err(super::RecoverableError::with_hint(
                format!("Path does not exist: {}", lib_path.display()),
                "Provide an absolute path to an existing directory.",
            ).into());
        }
        if !lib_path.is_dir() {
            return Err(super::RecoverableError::with_hint(
                format!("Path is not a directory: {}", lib_path.display()),
                "Provide a path to a directory, not a file.",
            ).into());
        }

        // Auto-detect from manifest, with user overrides.
        // IMPORTANT: discover_library_root expects a *file* path and calls .parent()
        // to start searching. Passing a directory would skip the directory itself.
        // We pass a synthetic file path inside the directory to work around this.
        let discovered = crate::library::discovery::discover_library_root(
            &lib_path.join("_probe"),
        );
        let name = input["name"].as_str()
            .map(String::from)
            .or_else(|| discovered.as_ref().map(|d| d.name.clone()))
            .unwrap_or_else(|| {
                lib_path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });
        let language = input["language"].as_str()
            .map(String::from)
            .or_else(|| discovered.as_ref().map(|d| d.language.clone()))
            .unwrap_or_else(|| "unknown".to_string());

        // Register and save
        {
            let mut inner = ctx.agent.inner.write().await;
            let project = inner.active_project.as_mut().ok_or_else(|| {
                super::RecoverableError::with_hint(
                    "No active project.",
                    "Call activate_project first.",
                )
            })?;
            project.library_registry.register(
                name.clone(),
                lib_path.clone(),
                language.clone(),
                crate::library::registry::DiscoveryMethod::Manual,
            );
            let registry_path = project.root.join(".codescout").join("libraries.json");
            project.library_registry.save(&registry_path)?;
        }

        Ok(json!({
            "status": "ok",
            "name": name,
            "language": language,
            "hint": format!("Use scope='lib:{}' in find_symbol/list_symbols/semantic_search. Run index_project(scope='lib:{}') to enable semantic search.", name, name),
        }))
    }
    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format!(
            "Registered library '{}' ({}) at {}",
            result["name"].as_str().unwrap_or("?"),
            result["language"].as_str().unwrap_or("?"),
            result["hint"].as_str().unwrap_or(""),
        ))
    }
}
```

- [ ] **Step 4: Register the tool in server.rs**

In `src/server.rs`, add to the imports:

```rust
library::RegisterLibrary,
```

Add to the tools vector in `from_parts`:

```rust
Arc::new(RegisterLibrary),
```

- [ ] **Step 5: Add to server_registers_all_tools test**

In `src/server.rs` tests, add `"register_library"` to the expected tool names set.

- [ ] **Step 6: Run tests to verify**

Run: `cargo test register_library -- --nocapture && cargo test server_registers_all_tools -- --nocapture`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add src/tools/library.rs src/server.rs
git commit -m "feat(library): add register_library tool for manual library registration"
```

---

### Task 7: Remove dead `IndexLibrary` struct

**Files:**
- Modify: `src/tools/library.rs` — remove `IndexLibrary` struct, its `impl Tool`, and related tests

- [ ] **Step 1: Remove the dead code**

Delete:
- `pub struct IndexLibrary;` (line 59)
- The entire `impl Tool for IndexLibrary` block (lines 62-150)
- `format_index_library` function (lines 176-180)
- Test `index_library_errors_for_unknown` (lines 249-260)
- Test `index_library_schema_is_valid` (lines 263-280)

Keep:
- Test `index_project_scope_lib_errors_for_unknown` — this tests the `index_project` tool, not `IndexLibrary`.

- [ ] **Step 2: Run clippy and tests**

Run: `cargo clippy -- -D warnings && cargo test library -- --nocapture`
Expected: Clean clippy (no dead code warnings), remaining library tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/tools/library.rs
git commit -m "chore: remove dead IndexLibrary struct (functionality lives in index_project)"
```

---

### Task 8: Update server instructions

**Files:**
- Modify: `src/prompts/server_instructions.md` — add `register_library` to tool reference

- [ ] **Step 1: Add register_library to the prompt**

In the Libraries section of `src/prompts/server_instructions.md`, add `register_library` tool documentation:

```markdown
- `register_library(path, name?, language?)` — manually register an external library.
  Auto-detects name and language from manifest files (Cargo.toml, package.json, pyproject.toml, go.mod).
  After registering, use `scope="lib:<name>"` in symbol/search tools, and `index_project(scope="lib:<name>")`
  for semantic search.
```

- [ ] **Step 2: Document scope behavior on explicit paths**

Add a note to the scope parameter descriptions in `server_instructions.md`:

```markdown
**Note:** When `path` is explicitly specified in `find_symbol` or `list_symbols`, the `scope`
parameter is ignored — the explicit path takes precedence. Scope only affects searches when
no path is given (or path is `"."`).
```

Also document the `find_references` limitation:

```markdown
**Note:** `find_references` scope filtering is limited to references the project's LSP server
already knows about. It cannot proactively discover references in unrelated library directories.
```

- [ ] **Step 3: Check onboarding prompt too**

Grep `src/prompts/onboarding_prompt.md` for library references and update if needed.

- [ ] **Step 4: Commit**

```bash
git add src/prompts/server_instructions.md src/prompts/onboarding_prompt.md
git commit -m "docs: add register_library to server instructions and update library guidance"
```

---

## Chunk 4: Final Verification

### Task 9: Full validation

- [ ] **Step 1: cargo fmt**

Run: `cargo fmt`

- [ ] **Step 2: cargo clippy**

Run: `cargo clippy -- -D warnings`
Expected: Clean — no warnings. Watch especially for unused `_scope` variables (should all be `scope` now).

- [ ] **Step 3: Full test suite**

Run: `cargo test`
Expected: All tests pass. New test count should be approximately original + 12 new tests.

- [ ] **Step 4: cargo build --release**

Run: `cargo build --release`
Expected: Clean build. This is the binary the MCP server runs.

- [ ] **Step 5: Manual smoke test**

After `/mcp` restart:
1. `register_library` with a known local crate/package path
2. `list_libraries` — verify it appears
3. `list_symbols(scope="libraries")` — verify library files show up
4. `find_symbol(pattern="...", scope="all")` — verify library symbols found
5. `index_project(scope="lib:<name>")` — verify indexing works
6. `semantic_search(query="...", scope="lib:<name>")` — verify semantic search works

- [ ] **Step 6: Final commit (if any fmt/clippy fixups)**

```bash
git add -A && git commit -m "chore: fmt and clippy fixups for library scope wiring"
```
