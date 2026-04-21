# Bash / Shell Full Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Elevate `.sh`/`.bash` files from "Detection only" to "Full support" — adding tree-sitter symbol extraction, AST-aware chunking, bash-language-server LSP, and companion-hook routing.

**Architecture:** Three independent layers: (1) server-side AST — add `tree-sitter-bash` grammar, symbol extractor, and chunker registry entry; (2) server-side LSP — wire `bash-language-server` into the existing server config system; (3) companion plugin — extend `SOURCE_EXT_PATTERN` and the Bash/Grep guards to block native reads on `.sh` files.

**Tech Stack:** Rust (tree-sitter-bash crate), bash-language-server (npm), tree-sitter node API, existing `extract_*_symbols` pattern in `src/ast/parser.rs`.

---

## Files touched

| File | Change |
|------|--------|
| `Cargo.toml` | Add `tree-sitter-bash` dependency |
| `src/ast/mod.rs` | Add `"bash"` arm in `get_ts_language` |
| `src/ast/parser.rs` | Add `extract_bash_symbols`, dispatch arm in `extract_symbols_from_source` |
| `src/embed/ast_chunker.rs` | Add bash entry to `LANGUAGE_REGISTRY` |
| `src/lsp/servers/mod.rs` | Add bash to `default_config`, `lsp_language_id`, `has_lsp_config` |
| `docs/manual/src/language-support.md` | Move bash to Full support table, add install section |
| `../claude-plugins/codescout-companion/hooks/detect-tools.sh` | Add `sh\|bash` to `SOURCE_EXT_PATTERN` |
| `../claude-plugins/codescout-companion/hooks/pre-tool-guard.sh` | Add `sh\|bash` to cat guard + Grep type guard |

---

## Task 1: tree-sitter-bash grammar + `get_ts_language` arm

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/ast/mod.rs`

- [ ] **Step 1: Write the failing test**

In `src/ast/parser.rs`, inside the existing `#[cfg(test)] mod tests` block, add:

```rust
#[test]
fn get_ts_language_bash() {
    assert!(
        crate::ast::get_ts_language("bash").is_some(),
        "bash should have a tree-sitter grammar"
    );
}
```

- [ ] **Step 2: Run it to confirm it fails**

```bash
cargo test get_ts_language_bash
```

Expected: `FAILED` — `assertion failed: crate::ast::get_ts_language("bash").is_some()`

- [ ] **Step 3: Add the dependency**

In `Cargo.toml`, after the `tree-sitter-css` line, add:

```toml
tree-sitter-bash = "0.23"
```

- [ ] **Step 4: Add the `get_ts_language` arm**

In `src/ast/mod.rs`, find `get_ts_language`. Add the bash arm before `_ => None`:

```rust
"bash" => Some(tree_sitter_bash::LANGUAGE.into()),
```

- [ ] **Step 5: Run test to confirm it passes**

```bash
cargo test get_ts_language_bash
```

Expected: `PASSED`

- [ ] **Step 6: Full test suite + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

Expected: all tests pass, no warnings.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock src/ast/mod.rs
git commit -m "feat(ast): add tree-sitter-bash grammar"
```

---

## Task 2: `extract_bash_symbols` + dispatch

**Files:**
- Modify: `src/ast/parser.rs`

- [ ] **Step 1: Write failing tests**

In `src/ast/parser.rs`, inside the existing `#[cfg(test)] mod tests` block, add:

```rust
#[test]
fn extract_bash_symbols_function_def() {
    use std::path::Path;

    // POSIX style: foo() { }
    let src_posix = "foo() {\n  echo hi\n}\n";
    let syms = extract_symbols_from_source(src_posix, Some("bash"), Path::new("script.sh")).unwrap();
    assert_eq!(syms.len(), 1, "expected 1 symbol from POSIX-style function");
    assert_eq!(syms[0].name, "foo");

    // keyword style: function bar { }
    let src_keyword = "function bar {\n  echo hi\n}\n";
    let syms = extract_symbols_from_source(src_keyword, Some("bash"), Path::new("script.sh")).unwrap();
    assert_eq!(syms.len(), 1, "expected 1 symbol from keyword-style function");
    assert_eq!(syms[0].name, "bar");
}

#[test]
fn extract_bash_symbols_no_functions() {
    use std::path::Path;
    let src = "FOO=bar\nexport BAZ=qux\necho hello\n";
    let syms = extract_symbols_from_source(src, Some("bash"), Path::new("script.sh")).unwrap();
    assert!(syms.is_empty(), "plain script with no functions should yield no symbols");
}

#[test]
fn extract_bash_symbols_nested_not_double_counted() {
    use std::path::Path;
    // Nested function definition — only top-level should appear
    let src = "outer() {\n  inner() { echo nested; }\n  inner\n}\n";
    let syms = extract_symbols_from_source(src, Some("bash"), Path::new("script.sh")).unwrap();
    assert_eq!(syms.len(), 1, "only top-level function should be extracted");
    assert_eq!(syms[0].name, "outer");
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test "extract_bash_symbols"
```

Expected: `FAILED` — `"bash"` arm falls into `_ => Ok(vec![])`, tests for `len == 1` fail.

- [ ] **Step 3: Implement `extract_bash_symbols`**

In `src/ast/parser.rs`, add this function before `extract_rust_docstrings` (after the Kotlin extractors, around line 985):

```rust
fn extract_bash_symbols(
    node: Node,
    source: &str,
    file: &PathBuf,
    prefix: &str,
) -> Vec<SymbolInfo> {
    let mut symbols = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "function_definition" {
            if let Some(name) = child_name(child, source, "name") {
                symbols.push(SymbolInfo {
                    name_path: make_name_path(prefix, &name),
                    name,
                    kind: SymbolKind::Function,
                    file: file.clone(),
                    start_line: child.start_position().row as u32,
                    end_line: child.end_position().row as u32,
                    start_col: child.start_position().column as u32,
                    children: vec![],
                });
            }
        }
    }
    symbols
}
```

Note: iterating only direct children of root (`node.children`) ensures nested function definitions inside function bodies are not double-counted — the inner functions are children of `compound_statement`, not of root.

- [ ] **Step 4: Add dispatch arm**

In `extract_symbols_from_source`, find the `match lang { ... _ => Ok(vec![]) }` block. Add before `_ =>`:

```rust
"bash" => Ok(extract_bash_symbols(root, source, &file, "")),
```

- [ ] **Step 5: Run tests to confirm they pass**

```bash
cargo test "extract_bash_symbols"
```

Expected: all 3 tests `PASSED`.

- [ ] **Step 6: Full test suite + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

Expected: all pass.

- [ ] **Step 7: Commit**

```bash
git add src/ast/parser.rs
git commit -m "feat(ast): extract bash function_definition symbols"
```

---

## Task 3: Bash entry in `LANGUAGE_REGISTRY` + chunker test

**Files:**
- Modify: `src/embed/ast_chunker.rs`

- [ ] **Step 1: Extend the existing registry test**

In `src/embed/ast_chunker.rs`, find `registry_lookup_all_languages`. Add `"bash"` to the `languages` array:

```rust
let languages = [
    "rust", "python", "go", "typescript", "javascript",
    "tsx", "jsx", "java", "kotlin", "bash",  // <-- add "bash"
];
```

Add a new test below the existing ones:

```rust
#[test]
fn ast_split_bash_two_functions() {
    let source = "foo() {\n  echo foo\n}\n\nbar() {\n  echo bar\n}\n";
    let chunks = split_file(source, "bash", Path::new("script.sh"), 4000);
    // Two functions → at least 2 chunks (one per function)
    assert!(
        chunks.len() >= 2,
        "expected at least 2 chunks for a 2-function bash script, got {}",
        chunks.len()
    );
    assert!(chunks.iter().any(|c| c.content.contains("foo")));
    assert!(chunks.iter().any(|c| c.content.contains("bar")));
}
```

- [ ] **Step 2: Run to confirm they fail**

```bash
cargo test "registry_lookup_all_languages" && cargo test "ast_split_bash_two_functions"
```

Expected: both `FAILED` — bash not in registry.

- [ ] **Step 3: Add bash entry to `LANGUAGE_REGISTRY`**

In `src/embed/ast_chunker.rs`, find `LANGUAGE_REGISTRY`. Add at the end, before the closing `];`:

```rust
RegistryEntry {
    name: "bash",
    spec: LanguageSpec {
        node_types: &["function_definition"],
        doc_prefixes: &["#"],
        inner_node_types: &[],
    },
},
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo test "registry_lookup_all_languages" && cargo test "ast_split_bash_two_functions"
```

Expected: both `PASSED`.

- [ ] **Step 5: Full suite + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add src/embed/ast_chunker.rs
git commit -m "feat(embed): add bash to AST chunker registry"
```

---

## Task 4: bash-language-server LSP config

**Files:**
- Modify: `src/lsp/servers/mod.rs`

- [ ] **Step 1: Update the existing coverage test**

In `src/lsp/servers/mod.rs`, find `has_lsp_config_covers_all_configured_languages`. Change:

```rust
assert!(!has_lsp_config("bash"));
```

to:

```rust
assert!(has_lsp_config("bash"));
```

- [ ] **Step 2: Run test to confirm it fails**

```bash
cargo test has_lsp_config_covers_all_configured_languages
```

Expected: `FAILED` — `has_lsp_config("bash")` returns false.

- [ ] **Step 3: Add bash to `has_lsp_config`**

In `has_lsp_config`, add `"bash"` to the `matches!` macro list:

```rust
pub fn has_lsp_config(lang: &str) -> bool {
    matches!(
        lang,
        "rust"
            | "python"
            | "typescript"
            | "javascript"
            | "tsx"
            | "jsx"
            | "go"
            | "java"
            | "kotlin"
            | "c"
            | "cpp"
            | "csharp"
            | "ruby"
            | "html"
            | "css"
            | "scss"
            | "less"
            | "bash"  // <-- add this
    )
}
```

- [ ] **Step 4: Add bash to `default_config`**

In `default_config`, add before `_ => None`:

```rust
"bash" => Some(LspServerConfig {
    command: crate::platform::lsp_binary_name("bash-language-server"),
    args: vec!["start".into()],
    workspace_root: root,
    init_timeout: None,
    mux: false,
    env: vec![],
}),
```

Note: `bash-language-server` uses `start` as a positional subcommand (not `--stdio`), matching how solargraph uses `stdio`. This is the standard invocation per the npm package.

- [ ] **Step 5: Add bash to `lsp_language_id`**

Bash uses `"shellscript"` as its LSP `languageId` (VS Code / LSP convention). Add before `other => other`:

```rust
"bash" => "shellscript",
```

- [ ] **Step 6: Run test to confirm it passes**

```bash
cargo test has_lsp_config_covers_all_configured_languages
```

Expected: `PASSED`.

- [ ] **Step 7: Full suite + clippy**

```bash
cargo test && cargo clippy -- -D warnings
```

- [ ] **Step 8: Commit**

```bash
git add src/lsp/servers/mod.rs
git commit -m "feat(lsp): add bash-language-server config"
```

---

## Task 5: Update `language-support.md`

**Files:**
- Modify: `docs/manual/src/language-support.md`

- [ ] **Step 1: Move bash from Detection-only to Full support**

In the main **Supported Languages** table, add a row:

```
| Bash       | `.sh`, `.bash`          | `bash-language-server`       | Full          |
```

In the **Detection-Only Languages** table, remove the Bash row:

```
| Bash     | `.sh`, `.bash`  |
```

- [ ] **Step 2: Add install snippet**

Under "Installing LSP Servers", add a new section after the HTML/CSS section:

```markdown
### Bash

```bash
npm install -g bash-language-server
```

Binary: `bash-language-server`, invoked with `start` (positional argument — not `--stdio`).
```

- [ ] **Step 3: Commit**

```bash
git add docs/manual/src/language-support.md
git commit -m "docs(language-support): bash promoted to full support"
```

---

## Task 6: Companion plugin routing

**Files:**
- Modify: `../claude-plugins/codescout-companion/hooks/detect-tools.sh`
- Modify: `../claude-plugins/codescout-companion/hooks/pre-tool-guard.sh`

- [ ] **Step 1: Update `SOURCE_EXT_PATTERN` in detect-tools.sh**

Find the line:

```bash
SOURCE_EXT_PATTERN='\.(kt|kts|java|ts|tsx|js|jsx|py|go|rs|cs|rb|scala|swift|cpp|c|h|hpp)$'
```

Replace with:

```bash
SOURCE_EXT_PATTERN='\.(kt|kts|java|ts|tsx|js|jsx|py|go|rs|cs|rb|scala|swift|cpp|c|h|hpp|sh|bash)$'
```

- [ ] **Step 2: Update the `cat` guard pattern in pre-tool-guard.sh**

Find:

```bash
elif echo "$CMD" | grep -qE '^cat .*\.(rs|ts|tsx|js|jsx|py|go|kt|kts|java|cs|rb|swift|cpp|c|h|hpp)'; then
```

Add `sh|bash` to the extension list:

```bash
elif echo "$CMD" | grep -qE '^cat .*\.(rs|ts|tsx|js|jsx|py|go|kt|kts|java|cs|rb|swift|cpp|c|h|hpp|sh|bash)'; then
```

Also update the `SRC_FILE` extraction regex on the line below to match:

```bash
SRC_FILE=$(echo "$CMD" | grep -oE '[^ ]+\.(rs|ts|tsx|js|jsx|py|go|kt|kts|java|cs|rb|swift|cpp|c|h|hpp|sh|bash)' | head -1)
```

- [ ] **Step 3: Update the Grep `$TYPE` guard in pre-tool-guard.sh**

Find the `case "$TYPE" in` block:

```bash
case "$TYPE" in
  kotlin|kt|kts|java|ts|typescript|js|javascript|py|python|go|rust|cs|csharp|rb|ruby|scala|swift|cpp|c)
    IS_SOURCE=true ;;
esac
```

Add `sh|bash|shellscript`:

```bash
case "$TYPE" in
  kotlin|kt|kts|java|ts|typescript|js|javascript|py|python|go|rust|cs|csharp|rb|ruby|scala|swift|cpp|c|sh|bash|shellscript)
    IS_SOURCE=true ;;
esac
```

- [ ] **Step 4: Smoke test the companion hook manually**

Open a shell in this project and verify:

```bash
# Should now be blocked (pre-tool-guard should intercept)
# Test by attempting: Read on detect-tools.sh from Claude — it should block and redirect
echo "Manual test: open a new Claude Code session and attempt Read on a .sh file"
echo "Expected: blocked with codescout routing hint"
```

- [ ] **Step 5: Commit**

```bash
cd ../claude-plugins/codescout-companion
git add hooks/detect-tools.sh hooks/pre-tool-guard.sh
git commit -m "feat(routing): block native reads on .sh/.bash files"
cd -
```

---

## Final verification

- [ ] `cargo fmt && cargo clippy -- -D warnings && cargo test` — all green in codescout repo
- [ ] `cargo build --release` — binary builds clean
- [ ] Restart MCP server with `/mcp` and run `list_symbols` on any `.sh` file in this repo — should return function symbols
- [ ] Run `semantic_search("enforce hook guard")` — should return results from hook scripts with function-boundary chunks
