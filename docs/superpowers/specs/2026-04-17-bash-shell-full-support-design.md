# Bash / Shell Full Support Design

**Date:** 2026-04-17
**Status:** Approved

## Summary

Elevate `.sh`/`.bash` files from "Detection only" to "Full support" — adding
tree-sitter-bash for symbol extraction and AST-aware chunking, bash-language-server
for LSP navigation, and companion-hook routing to block native Read/Grep on shell files.

---

## Section 1: Server-side changes

### Cargo.toml

Add:

```toml
tree-sitter-bash = "0.23"
```

Same version family as all other tree-sitter grammar crates.

### `src/ast/mod.rs`

- `detect_language`: `.sh`/`.bash` already returns `"bash"` — no change needed.
- `get_ts_language`: add arm:
  ```rust
  "bash" => Some(tree_sitter_bash::LANGUAGE.into()),
  ```

### `src/ast/parser.rs`

- Add `extract_bash_symbols(node, source, file, prefix)`:
  - Extracts `function_definition` nodes (both `foo() { ... }` and `function foo { ... }` syntax).
  - Name extracted via the `name` child field of the grammar node.
  - Flat list only — no class hierarchy in bash.
  - Skip `variable_assignment` — too noisy in hook scripts; doesn't map cleanly to a symbol concept.
- Wire into `extract_symbols_from_source` match arm:
  ```rust
  "bash" => Ok(extract_bash_symbols(root, source, &file, "")),
  ```

### `src/embed/ast_chunker.rs`

Add bash entry to `LANGUAGE_REGISTRY`:

```rust
RegistryEntry {
    name: "bash",
    spec: LanguageSpec {
        node_types: &["function_definition"],
        doc_prefixes: &["#"],
        inner_node_types: &[],  // no containers in bash
    },
},
```

### `src/lsp/client.rs` (LSP server config)

Add `bash-language-server`:
- Binary: `bash-language-server`
- Args: `["start"]` (bash-language-server uses `start` subcommand, not `--stdio`)
- Language IDs: `["shellscript"]` (LSP languageId for bash/sh files)

### `docs/manual/src/language-support.md`

- Move Bash from "Detection only" table to the main Supported Languages table:

  | Bash | `.sh`, `.bash` | `bash-language-server` | Full |

- Add install snippet under "Installing LSP Servers":
  ```bash
  npm install -g bash-language-server
  ```

---

## Section 2: Companion plugin changes

**`hooks/detect-tools.sh`**

Add `sh|bash` to `SOURCE_EXT_PATTERN`:

```bash
SOURCE_EXT_PATTERN='\.(kt|kts|java|ts|tsx|js|jsx|py|go|rs|cs|rb|scala|swift|cpp|c|h|hpp|sh|bash)$'
```

**`hooks/pre-tool-guard.sh`**

Two additions:

1. The `cat` source-file regex — add `sh|bash` so `cat script.sh` triggers the symbol-nav hint:
   ```bash
   elif echo "$CMD" | grep -qE '^cat .*\.(rs|ts|tsx|...|sh|bash)'; then
   ```

2. The `Grep` type guard `case "$TYPE" in` — add `sh|bash|shellscript`:
   ```bash
   kotlin|kt|kts|java|...|sh|bash|shellscript)
     IS_SOURCE=true ;;
   ```

No change needed to the Read guard — it already uses `SOURCE_EXT_PATTERN` from detect-tools.sh.

---

## Section 3: Testing

### `src/ast/parser.rs`

Add to existing `tests` module:

- `extract_bash_symbols_function_def` — both `foo() { ... }` and `function foo { ... }` syntax produce a symbol.
- `extract_bash_symbols_no_functions` — script with only assignments/commands returns empty vec.
- `extract_bash_symbols_nested_ignored` — functions defined inside other functions not double-counted (take top-level only).

### `src/embed/ast_chunker.rs`

- `registry_lookup_all_languages` — add `"bash"` to the assertion list.
- `ast_split_bash_function` — two-function script produces two chunks, each containing the function body.

### LSP

No new test. Bash server config is a data change following the existing pattern;
LSP client tests use `MockLspClient` and don't test per-language config.

### Docs

Visual check only — no automated test for `language-support.md`.

---

## Out of scope

- `extract_bash_docstrings` — bash has no doc-comment convention; `#` comments above functions are already pulled in via `doc_prefixes` in the chunker.
- `variable_assignment` symbols — deferred; too noisy for hook-style scripts.
- `.env` / `Makefile` — separate decision; not shell scripts.
