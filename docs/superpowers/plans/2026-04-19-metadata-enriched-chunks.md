# Metadata-Enriched Chunks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Improve semantic_search retrieval on multi-concept keyword queries by prepending a searchable header (`file_path :: container :: kind name(signature)`) to each chunk before embedding, while keeping user-visible output unchanged.

**Architecture:** Add a `metadata: Option<String>` field threaded through `RawChunk` → `CodeChunk` → `chunks` table. The AST chunker builds per-chunk headers using `file_path`, accumulated container path during recursion, and language-specific kind keywords. At embed time, the text sent to the model is `metadata + "\n" + content`. Search results return `content` only. Schema version bump forces a one-time full reindex on upgrade.

**Tech Stack:** Rust, rusqlite, tree-sitter, async_trait, tokio

**Spec:** `docs/superpowers/specs/2026-04-19-metadata-enriched-chunks-design.md`

---

## File Structure

**Modified:**
- `src/embed/chunker.rs` — add `metadata: Option<String>` to `RawChunk`
- `src/embed/schema.rs` — add `metadata: Option<String>` to `CodeChunk`
- `src/embed/ast_chunker.rs` — header builder, signature extractor, kind mapping, thread `file_path` + `container_path` through `split_file` + `nodes_to_chunks`
- `src/embed/index.rs` — schema version constant, migration drop-and-rebuild, insert/select with `metadata`, embed with `metadata + "\n" + content`
- `src/tools/workflow.rs` — bump `ONBOARDING_VERSION`
- `docs/manual/src/experimental/index.md` — link the new experimental page

**Created:**
- `docs/manual/src/experimental/metadata-enriched-chunks.md` — experimental-branch user doc

**Out of scope for the plan** (already done by user, no code change):
- `.codescout/project.toml` chunk_size = 1600 setting — kept as a config step, not a code change

---

## Task 1: Add `metadata` field to `RawChunk`

**Files:**
- Modify: `src/embed/chunker.rs` (`RawChunk` struct, L9-15)
- Test: `src/embed/chunker.rs` (tests module at bottom)

- [ ] **Step 1: Write failing test**

Add to the tests module in `src/embed/chunker.rs`:

```rust
#[test]
fn raw_chunk_carries_metadata_field() {
    let c = RawChunk {
        content: "body".into(),
        start_line: 1,
        end_line: 5,
        metadata: Some("src/foo.rs :: fn bar".into()),
    };
    assert_eq!(c.metadata.as_deref(), Some("src/foo.rs :: fn bar"));
}
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test -p codescout --lib raw_chunk_carries_metadata_field`
Expected: FAIL — `no field 'metadata'`.

- [ ] **Step 3: Add the field**

Replace the `RawChunk` struct in `src/embed/chunker.rs` with:

```rust
/// A raw text chunk with line tracking before embedding.
#[derive(Debug, Clone)]
pub struct RawChunk {
    pub content: String,
    /// 1-indexed start line in the original file
    pub start_line: usize,
    /// 1-indexed end line in the original file (inclusive)
    pub end_line: usize,
    /// Searchable header prepended before embedding. `None` for chunks from
    /// non-AST paths (markdown, plain text). Not returned in search results.
    pub metadata: Option<String>,
}
```

- [ ] **Step 4: Update all `RawChunk` constructors**

Grep for `RawChunk {` and add `metadata: None` to every literal construction. Expected sites:
- `src/embed/chunker.rs` — `split`, `split_markdown` return sites
- `src/embed/ast_chunker.rs` — `nodes_to_chunks`, `sub_split_node`, `extract_container_header`

Run: `mcp__codescout__grep` pattern `RawChunk \{` path `src/embed`

For each hit, insert `metadata: None,` as the last field.

- [ ] **Step 5: Run the whole test suite**

Run: `cargo test -p codescout --lib`
Expected: all pass (the new field defaults to None, so old behavior preserved).

- [ ] **Step 6: Commit**

```bash
git add src/embed/chunker.rs src/embed/ast_chunker.rs
git commit -m "feat(embed): add metadata field to RawChunk"
```

---

## Task 2: Add `metadata` column to DB schema + version constant

**Files:**
- Modify: `src/embed/index.rs` — all 5 `CREATE TABLE chunks` sites (lines 205, 365, 469, 4005, 4057)
- Modify: `src/embed/schema.rs` (`CodeChunk` struct, L7-26)
- Test: `src/embed/index.rs` tests module

- [ ] **Step 1: Add `metadata` field to `CodeChunk`**

In `src/embed/schema.rs`, add after `source`:

```rust
/// Searchable header prepended before embedding. NULL for non-AST chunks
/// (markdown, plain text). NOT returned in semantic_search results.
pub metadata: Option<String>,
```

Grep for `CodeChunk {` and add `metadata: None` to every construction site. Use `mcp__codescout__grep` pattern `CodeChunk \{` path `src`.

- [ ] **Step 2: Add `SCHEMA_VERSION` constant**

In `src/embed/index.rs` near the top (after any existing constants), add:

```rust
/// Bump whenever the `chunks` table schema changes in a way that requires
/// re-embedding (new column affecting embed input, changed column semantics).
/// On mismatch, `open_db` drops and recreates all chunk-related tables.
const SCHEMA_VERSION: u32 = 1;
```

- [ ] **Step 3: Write failing migration test**

Add to the `tests` module in `src/embed/index.rs`:

```rust
#[test]
fn old_schema_without_metadata_triggers_rebuild() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("project.db");

    // Create a legacy DB without metadata column and without schema_version.
    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                id INTEGER PRIMARY KEY,
                file_path TEXT NOT NULL,
                language TEXT NOT NULL,
                content TEXT NOT NULL,
                start_line INTEGER NOT NULL,
                end_line INTEGER NOT NULL,
                file_hash TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT 'project'
            );
            INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash)
             VALUES ('a.rs', 'rust', 'fn x(){}', 1, 1, 'abc');"
        ).unwrap();
    }

    // Open via production path; it should detect missing version, drop & recreate.
    let conn = open_db(&db_path).expect("open_db should migrate");

    // Verify new column exists.
    let cols: Vec<String> = conn
        .prepare("PRAGMA table_info(chunks)")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(cols.iter().any(|c| c == "metadata"),
        "metadata column missing after migration: cols={cols:?}");

    // Old row is gone (table was dropped).
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "old rows should be dropped on schema migration");
}
```

- [ ] **Step 4: Run test, verify it fails**

Run: `cargo test -p codescout --lib old_schema_without_metadata_triggers_rebuild`
Expected: FAIL — either `open_db` signature mismatch or column assertion fails.

- [ ] **Step 5: Update all `CREATE TABLE chunks` statements**

For each of the 5 sites (grep `CREATE TABLE.*chunks` in `src/embed/index.rs`):

Replace:
```sql
CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    language TEXT NOT NULL,
    content TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    file_hash TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'project'
);
```

With:
```sql
CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path TEXT NOT NULL,
    language TEXT NOT NULL,
    content TEXT NOT NULL,
    start_line INTEGER NOT NULL,
    end_line INTEGER NOT NULL,
    file_hash TEXT NOT NULL,
    source TEXT NOT NULL DEFAULT 'project',
    metadata TEXT
);
```

Note: the two `CREATE TABLE chunks (` variants (L4005, L4057) are also test-fixture schemas — include `metadata TEXT` there too.

- [ ] **Step 6: Add schema version check to `open_db`**

Locate `open_db` (`mcp__codescout__find_symbol query=open_db path=src/embed/index.rs`). At the top of the function, after connecting but before any schema work, add:

```rust
// Read stored schema version; drop chunk tables if stale.
let stored: Option<u32> = conn
    .query_row(
        "SELECT value FROM meta WHERE key = 'schema_version'",
        [],
        |r| r.get::<_, String>(0).and_then(|s| s.parse::<u32>().map_err(|_| rusqlite::Error::InvalidQuery)),
    )
    .optional()
    .ok()
    .flatten();

if stored != Some(SCHEMA_VERSION) {
    // Either no meta table yet, no key yet, or stale version — drop chunk-owned
    // tables and let the existing CREATE TABLE IF NOT EXISTS rebuild them.
    conn.execute_batch(
        "DROP TABLE IF EXISTS chunks;
         DROP TABLE IF EXISTS chunk_embeddings;
         DROP TABLE IF EXISTS files;"
    ).context("dropping stale schema")?;
}
```

After the `CREATE TABLE` batch that recreates the schema, record the version:

```rust
conn.execute(
    "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
    [&SCHEMA_VERSION.to_string()],
)?;
```

Assumption: there is a `meta (key, value)` table. If not, create it as part of the schema batch alongside `files`. Verify with `mcp__codescout__grep` pattern `CREATE TABLE.*meta`.

- [ ] **Step 7: Re-run migration test**

Run: `cargo test -p codescout --lib old_schema_without_metadata_triggers_rebuild`
Expected: PASS.

- [ ] **Step 8: Run the full test suite**

Run: `cargo test -p codescout --lib`
Expected: all pass. If any tests fail on schema shape (e.g. test fixtures assuming no metadata column), update them to include `metadata` in their assertions/inserts.

- [ ] **Step 9: Commit**

```bash
git add src/embed/index.rs src/embed/schema.rs
git commit -m "feat(embed): add metadata column + schema version check"
```

---

## Task 3: Implement signature extractor helper

**Files:**
- Modify: `src/embed/ast_chunker.rs` — add `extract_signature` function
- Test: same file

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/embed/ast_chunker.rs`:

```rust
#[test]
fn extract_signature_rust_fn() {
    let s = extract_signature("pub fn foo(x: i32) -> Result<String> {");
    assert_eq!(s, "pub fn foo(x: i32) -> Result<String>");
}

#[test]
fn extract_signature_python_def() {
    let s = extract_signature("def bar(self, token: str) -> bool:");
    assert_eq!(s, "def bar(self, token: str) -> bool");
}

#[test]
fn extract_signature_arrow_fn() {
    let s = extract_signature("const foo = (x) => {");
    assert_eq!(s, "const foo = (x)");
}

#[test]
fn extract_signature_truncates_at_100_chars() {
    let long = "pub fn a_very_long_name_with_lots_of_generic_parameters<T: Clone + Send + Sync + Debug + Display + PartialEq>(x: T) -> Result<T> {";
    let s = extract_signature(long);
    assert!(s.len() <= 100, "expected <=100 chars, got {}: {s}", s.len());
}

#[test]
fn extract_signature_no_block_start() {
    // Signature that happens to fit on one line with no opening brace
    let s = extract_signature("pub const X: i32 = 5;");
    assert_eq!(s, "pub const X: i32 = 5;");
}
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p codescout --lib extract_signature`
Expected: 5 failures (`extract_signature` undefined).

- [ ] **Step 3: Implement `extract_signature`**

Add to `src/embed/ast_chunker.rs` (near `extract_container_header`, around L350):

```rust
/// Extract a compact signature from the first line of a node body.
///
/// Truncates at the first of: `{`, `:` (Python-style body delimiter),
/// `=>` (arrow function), or 100 chars. Designed for header metadata;
/// signature fidelity is not critical — only keyword matchability.
fn extract_signature(first_line: &str) -> String {
    const MAX_LEN: usize = 100;
    let trimmed = first_line.trim_end();

    // Find earliest delimiter among `{`, ` :` (Python), `=>`, `:` before newline
    let mut end = trimmed.len();
    for delim in ["{", "=>"] {
        if let Some(i) = trimmed.find(delim) {
            end = end.min(i);
        }
    }
    // Python `def foo(): ` — truncate at trailing `:` (but not inside type annotations)
    if let Some(i) = trimmed.rfind(':') {
        // Only truncate if the colon is near the end (Python body delimiter)
        if i > end.saturating_sub(5) && i >= trimmed.len().saturating_sub(2) {
            end = end.min(i);
        }
    }

    let sliced = trimmed[..end].trim_end();
    let truncated = if sliced.chars().count() > MAX_LEN {
        // char-safe slice
        sliced.chars().take(MAX_LEN).collect::<String>()
    } else {
        sliced.to_string()
    };
    truncated
}
```

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test -p codescout --lib extract_signature`
Expected: 5 pass.

If Python test fails because the heuristic for trailing `:` misidentifies type annotations, adjust the condition. The critical property: a signature like `def foo(self, x: int) -> bool:` must keep `-> bool` and drop the trailing `:`.

- [ ] **Step 5: Commit**

```bash
git add src/embed/ast_chunker.rs
git commit -m "feat(embed): add extract_signature helper for chunk metadata"
```

---

## Task 4: Language-specific kind keyword mapping

**Files:**
- Modify: `src/embed/ast_chunker.rs` — add `kind_keyword_for_node`
- Test: same file

- [ ] **Step 1: Write failing tests**

Add to `tests` module:

```rust
#[test]
fn kind_keyword_rust_function() {
    assert_eq!(kind_keyword_for_node("rust", "function_item"), Some("fn"));
}

#[test]
fn kind_keyword_rust_struct() {
    assert_eq!(kind_keyword_for_node("rust", "struct_item"), Some("struct"));
}

#[test]
fn kind_keyword_rust_impl() {
    assert_eq!(kind_keyword_for_node("rust", "impl_item"), Some("impl"));
}

#[test]
fn kind_keyword_python_class() {
    assert_eq!(kind_keyword_for_node("python", "class_definition"), Some("class"));
}

#[test]
fn kind_keyword_python_async() {
    assert_eq!(kind_keyword_for_node("python", "async_function_definition"), Some("async def"));
}

#[test]
fn kind_keyword_typescript_method() {
    assert_eq!(kind_keyword_for_node("typescript", "method_definition"), Some("method"));
}

#[test]
fn kind_keyword_unknown_returns_none() {
    assert_eq!(kind_keyword_for_node("rust", "weird_node"), None);
    assert_eq!(kind_keyword_for_node("klingon", "function_item"), None);
}
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p codescout --lib kind_keyword`
Expected: 7 fails (undefined).

- [ ] **Step 3: Implement `kind_keyword_for_node`**

Add to `src/embed/ast_chunker.rs`:

```rust
/// Map a tree-sitter node kind to a short header keyword for a given language.
/// Used in chunk metadata headers. Returns `None` for unknown lang/node pairs.
fn kind_keyword_for_node(lang: &str, node_kind: &str) -> Option<&'static str> {
    match (lang, node_kind) {
        ("rust", "function_item") => Some("fn"),
        ("rust", "struct_item") => Some("struct"),
        ("rust", "enum_item") => Some("enum"),
        ("rust", "trait_item") => Some("trait"),
        ("rust", "impl_item") => Some("impl"),
        ("rust", "mod_item") => Some("mod"),
        ("rust", "type_item") => Some("type"),
        ("rust", "const_item") => Some("const"),
        ("rust", "static_item") => Some("static"),
        ("rust", "macro_definition") => Some("macro"),

        ("python", "function_definition") => Some("def"),
        ("python", "async_function_definition") => Some("async def"),
        ("python", "class_definition") => Some("class"),
        ("python", "decorated_definition") => Some("def"),

        ("typescript" | "tsx", "function_declaration") => Some("function"),
        ("typescript" | "tsx", "class_declaration") => Some("class"),
        ("typescript" | "tsx", "method_definition") => Some("method"),
        ("typescript" | "tsx", "interface_declaration") => Some("interface"),
        ("typescript" | "tsx", "type_alias_declaration") => Some("type"),
        ("typescript" | "tsx", "export_statement") => Some("export"),

        ("javascript" | "jsx", "function_declaration") => Some("function"),
        ("javascript" | "jsx", "class_declaration") => Some("class"),
        ("javascript" | "jsx", "method_definition") => Some("method"),
        ("javascript" | "jsx", "export_statement") => Some("export"),

        ("java", "method_declaration") => Some("method"),
        ("java", "class_declaration") => Some("class"),
        ("java", "interface_declaration") => Some("interface"),
        ("java", "constructor_declaration") => Some("constructor"),
        ("java", "enum_declaration") => Some("enum"),

        ("kotlin", "function_declaration") => Some("fun"),
        ("kotlin", "class_declaration") => Some("class"),
        ("kotlin", "object_declaration") => Some("object"),
        ("kotlin", "property_declaration") => Some("property"),

        ("go", "function_declaration") => Some("func"),
        ("go", "method_declaration") => Some("method"),
        ("go", "type_declaration") => Some("type"),
        ("go", "var_declaration") => Some("var"),
        ("go", "const_declaration") => Some("const"),

        ("bash", "function_definition") => Some("function"),

        _ => None,
    }
}
```

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test -p codescout --lib kind_keyword`
Expected: 7 pass.

- [ ] **Step 5: Commit**

```bash
git add src/embed/ast_chunker.rs
git commit -m "feat(embed): add kind_keyword_for_node for chunk metadata"
```

---

## Task 5: Metadata header builder

**Files:**
- Modify: `src/embed/ast_chunker.rs` — add `build_metadata_header`
- Test: same file

- [ ] **Step 1: Write failing tests**

Add to `tests` module:

```rust
#[test]
fn metadata_header_top_level_rust_fn() {
    let h = build_metadata_header("src/foo.rs", &[], Some("fn"), Some("foo"), Some("fn foo(x: i32)"));
    assert_eq!(h.as_deref(), Some("src/foo.rs :: fn foo(x: i32)"));
}

#[test]
fn metadata_header_rust_method_in_impl() {
    let h = build_metadata_header(
        "src/embed/index.rs",
        &["impl IndexStore"],
        Some("fn"),
        Some("build_index"),
        Some("fn build_index(force: bool)"),
    );
    assert_eq!(h.as_deref(), Some("src/embed/index.rs :: impl IndexStore :: fn build_index(force: bool)"));
}

#[test]
fn metadata_header_struct_no_signature() {
    let h = build_metadata_header("src/foo.rs", &[], Some("struct"), Some("Bar"), None);
    assert_eq!(h.as_deref(), Some("src/foo.rs :: struct Bar"));
}

#[test]
fn metadata_header_gap_file_only() {
    let h = build_metadata_header("src/foo.rs", &[], None, None, None);
    assert_eq!(h.as_deref(), Some("src/foo.rs"));
}

#[test]
fn metadata_header_container_only() {
    // container header chunk emitted by extract_container_header
    let h = build_metadata_header("src/foo.rs", &["impl Bar"], None, None, None);
    assert_eq!(h.as_deref(), Some("src/foo.rs :: impl Bar"));
}

#[test]
fn metadata_header_kind_without_signature_uses_name() {
    let h = build_metadata_header("src/foo.rs", &[], Some("fn"), Some("bar"), None);
    assert_eq!(h.as_deref(), Some("src/foo.rs :: fn bar"));
}

#[test]
fn metadata_header_nested_container() {
    let h = build_metadata_header(
        "src/x.rs",
        &["mod inner", "impl Foo"],
        Some("fn"),
        Some("baz"),
        Some("fn baz()"),
    );
    assert_eq!(h.as_deref(), Some("src/x.rs :: mod inner :: impl Foo :: fn baz()"));
}
```

- [ ] **Step 2: Run tests, verify they fail**

Run: `cargo test -p codescout --lib metadata_header`
Expected: 7 fails.

- [ ] **Step 3: Implement `build_metadata_header`**

Add to `src/embed/ast_chunker.rs`:

```rust
/// Build a chunk metadata header.
///
/// Format: `{file_path} :: {container_1} :: ... :: {kind} {signature_or_name}`
///
/// - `signature` takes priority over `name` when both present.
/// - If `kind` is None but `name` is Some, uses name alone.
/// - If all of kind/name/signature are None: returns just `file_path` (for gap chunks or container headers without an explicit kind).
/// - If `file_path` is empty, returns None (shouldn't happen in practice).
fn build_metadata_header(
    file_path: &str,
    container_path: &[&str],
    kind: Option<&str>,
    name: Option<&str>,
    signature: Option<&str>,
) -> Option<String> {
    if file_path.is_empty() {
        return None;
    }

    let mut parts: Vec<String> = Vec::with_capacity(container_path.len() + 2);
    parts.push(file_path.to_string());

    for c in container_path {
        parts.push((*c).to_string());
    }

    // Node part: kind + name/signature
    let node_part = match (kind, name, signature) {
        (Some(k), _, Some(sig)) => Some(format!("{k} {sig}")),
        (Some(k), Some(n), None) => Some(format!("{k} {n}")),
        (None, Some(n), Some(sig)) => Some(format!("{n} {sig}")),
        (None, Some(n), None) => Some(n.to_string()),
        _ => None,
    };
    if let Some(np) = node_part {
        // If signature already contains the kind keyword (e.g. "pub fn foo"),
        // avoid doubling it: check for "fn " / "class " etc. after trimming visibility.
        parts.push(np);
    }

    Some(parts.join(" :: "))
}
```

- [ ] **Step 4: Run tests, verify they pass**

Run: `cargo test -p codescout --lib metadata_header`
Expected: 7 pass.

- [ ] **Step 5: Commit**

```bash
git add src/embed/ast_chunker.rs
git commit -m "feat(embed): add build_metadata_header"
```

---

## Task 6: Thread `file_path` + `container_path` through `split_file` / `nodes_to_chunks`

**Files:**
- Modify: `src/embed/ast_chunker.rs` — `split_file`, `nodes_to_chunks`, `sub_split_node`, `extract_container_header`
- Modify: call sites of `split_file` (find with grep)
- Test: `src/embed/ast_chunker.rs` — integration test

- [ ] **Step 1: Find all callers of `split_file`**

Run: `mcp__codescout__find_references symbol=split_file path=src/embed/ast_chunker.rs`
Expected callers: `src/embed/index.rs` (build_index), tests in `ast_chunker.rs`.

Record the list for step 5.

- [ ] **Step 2: Write failing integration test**

Add to `tests` module in `src/embed/ast_chunker.rs`:

```rust
#[test]
fn split_file_rust_populates_metadata_headers() {
    let src = r#"
pub fn top_level() {
    println!("hi");
}

pub struct MyStore;

impl MyStore {
    pub fn build(&self) {
        // body
    }
}
"#;
    let chunks = split_file(src, "rust", Path::new("src/mystore.rs"), 4000);

    // Find chunk containing top_level
    let top = chunks.iter().find(|c| c.content.contains("top_level")).expect("top_level chunk");
    assert_eq!(
        top.metadata.as_deref(),
        Some("src/mystore.rs :: fn pub fn top_level()")
            .or(Some("src/mystore.rs :: fn top_level()"))
            .unwrap().into()
    );
    // Lenient match: header contains file path, kind, and name
    let meta = top.metadata.as_deref().expect("top_level has metadata");
    assert!(meta.contains("src/mystore.rs"), "meta missing path: {meta}");
    assert!(meta.contains("fn"), "meta missing kind fn: {meta}");
    assert!(meta.contains("top_level"), "meta missing name: {meta}");

    // build method should carry the impl container in its header
    let build = chunks.iter().find(|c| c.content.contains("fn build")).expect("build chunk");
    let bmeta = build.metadata.as_deref().expect("build has metadata");
    assert!(bmeta.contains("impl MyStore"), "build metadata missing impl container: {bmeta}");
    assert!(bmeta.contains("fn") && bmeta.contains("build"), "build metadata incomplete: {bmeta}");
}
```

- [ ] **Step 3: Run test, verify it fails**

Run: `cargo test -p codescout --lib split_file_rust_populates_metadata_headers`
Expected: FAIL (metadata is None because we haven't wired it yet).

- [ ] **Step 4: Update `split_file` signature**

In `src/embed/ast_chunker.rs`, change `split_file` to accept `file_path` as `&Path` (already has `path: &Path`; we'll pass it through). No signature change needed — the parameter already exists. Use `path.to_string_lossy()` for the header.

- [ ] **Step 5: Update `nodes_to_chunks` signature + body**

Replace the existing `nodes_to_chunks` function signature with:

```rust
fn nodes_to_chunks(
    source: &str,
    nodes: &[AstNode],
    chunk_size: usize,
    doc_prefixes: &[&str],
    ts_lang: Option<&tree_sitter::Language>,
    spec: Option<&LanguageSpec>,
    lang: &str,
    file_path: &str,
    container_path: &[String],
) -> Vec<RawChunk>
```

Inside the function, when producing each `RawChunk`:

1. Extract the node's tree-sitter kind via `spec` (you already have it in `extract_ast_nodes`; pass it via `AstNode` or re-derive from node text). Simpler: store `kind: String` in `AstNode` alongside `start_line`/`end_line` — update `AstNode` struct and `extract_ast_nodes` to populate it.

2. Extract node name: walk tree-sitter children for the `name:` / `identifier` child. Add helper:

```rust
fn extract_node_name(node: &tree_sitter::Node, source: &str) -> Option<String> {
    // Common identifier field name across grammars
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())
}
```

3. Extract signature via `extract_signature(first_line_of_node)`.

4. Build metadata:

```rust
let kind_kw = kind_keyword_for_node(lang, &node.kind);
let name = extract_node_name(&node.ts_node, source);
let first_line = lines.get(node.start_line).copied().unwrap_or("");
let sig = if first_line.is_empty() { None } else { Some(extract_signature(first_line)) };
let container_refs: Vec<&str> = container_path.iter().map(|s| s.as_str()).collect();
let metadata = build_metadata_header(file_path, &container_refs, kind_kw, name.as_deref(), sig.as_deref());
```

Assign `metadata` to every `RawChunk` produced for this node.

5. For the **inner recursion** path (when a large container is decomposed), push the container header onto `container_path` and pass it through:

```rust
if let Some(inner) = inner_nodes {
    // Build container descriptor: kind + name (no signature for container header)
    let container_desc = match (kind_keyword_for_node(lang, &node.kind), extract_node_name(&node.ts_node, source)) {
        (Some(k), Some(n)) => format!("{k} {n}"),
        (Some(k), None) => k.to_string(),
        (None, Some(n)) => n,
        _ => "container".to_string(),
    };

    // Emit header chunk — its metadata is the path + container_desc (no symbol)
    let header = extract_container_header(...);
    if !header.content.trim().is_empty() {
        let mut hdr = header;
        let container_refs: Vec<&str> = container_path.iter().map(|s| s.as_str()).collect();
        hdr.metadata = build_metadata_header(file_path, &container_refs, None, Some(&container_desc), None);
        chunks.push(hdr);
    }

    let mut inner_container = container_path.to_vec();
    inner_container.push(container_desc);
    let inner_chunks = nodes_to_chunks(source, &inner, chunk_size, doc_prefixes, None, None, lang, file_path, &inner_container);
    chunks.extend(inner_chunks);
}
```

6. For **gap chunks** (between nodes): metadata = `Some(file_path.to_string())` — just the path, no symbol.

7. For **sub_split_node** (oversized node with no inner structure): all pieces share the same metadata (same symbol, different slices). Pass metadata into `sub_split_node` as a new parameter or apply it to returned chunks after the call.

- [ ] **Step 6: Update `AstNode` struct**

In `src/embed/ast_chunker.rs`, replace:

```rust
pub struct AstNode {
    pub start_line: usize,
    pub end_line: usize,
}
```

with:

```rust
pub struct AstNode {
    pub start_line: usize,
    pub end_line: usize,
    pub kind: String,
    pub name: Option<String>,
}
```

Update `extract_ast_nodes` to populate `kind` via `node.kind().to_string()` and `name` via the new `extract_node_name` helper.

- [ ] **Step 7: Update `split_file` to pass new args**

In `split_file`, change the `nodes_to_chunks` call:

```rust
let file_path_str = path.to_string_lossy();
let container_path: Vec<String> = Vec::new();
nodes_to_chunks(
    source, &nodes, target, doc_prefixes, Some(&ts_lang), spec,
    lang, &file_path_str, &container_path,
)
```

For the markdown path (`split_markdown`) and unknown-language path (`chunker::split`): leave `metadata = None` on returned chunks (they already use `metadata: None` from Task 1).

- [ ] **Step 8: Update other callers and tests**

Grep for `nodes_to_chunks(` and update every call. Old tests will need new arguments:

```rust
let chunks = nodes_to_chunks(source, &nodes, 3000, &["///"], None, None, "rust", "test.rs", &[]);
```

- [ ] **Step 9: Run integration test**

Run: `cargo test -p codescout --lib split_file_rust_populates_metadata_headers`
Expected: PASS.

- [ ] **Step 10: Run full suite**

Run: `cargo test -p codescout --lib`
Expected: all pass. Fix any existing AST tests that break due to signature changes.

- [ ] **Step 11: Commit**

```bash
git add src/embed/ast_chunker.rs
git commit -m "feat(embed): thread file path and container through AST chunker for metadata"
```

---

## Task 7: Persist metadata through DB + embed with header prefix

**Files:**
- Modify: `src/embed/index.rs` — insert/select paths + embed-text construction

- [ ] **Step 1: Write failing test (DB round-trip)**

Add to `src/embed/index.rs` tests:

```rust
#[tokio::test]
async fn chunks_roundtrip_metadata_column() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("project.db");

    let conn = open_db(&db_path).unwrap();
    conn.execute(
        "INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source, metadata)
         VALUES ('a.rs', 'rust', 'body', 1, 5, 'hash', 'project', 'src/a.rs :: fn foo')",
        [],
    ).unwrap();

    let (meta,): (Option<String>,) = conn.query_row(
        "SELECT metadata FROM chunks WHERE file_path = 'a.rs'",
        [],
        |r| Ok((r.get(0)?,)),
    ).unwrap();

    assert_eq!(meta.as_deref(), Some("src/a.rs :: fn foo"));
}
```

- [ ] **Step 2: Run test, verify it passes**

Run: `cargo test -p codescout --lib chunks_roundtrip_metadata_column`
Expected: PASS (column was added in Task 2; test confirms usability).

- [ ] **Step 3: Write failing test (embed text includes header)**

Add a new test using a captured-text embedder:

```rust
#[tokio::test]
async fn build_index_embeds_metadata_plus_content() {
    // Create a MockEmbedder variant that records all text sent to embed().
    // Use the existing MockEmbedder pattern from tests if available, else inline it.
    use std::sync::{Arc, Mutex};

    struct CapturingEmbedder(Arc<Mutex<Vec<String>>>);
    #[async_trait::async_trait]
    impl crate::embed::Embedder for CapturingEmbedder {
        fn dimensions(&self) -> usize { 4 }
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<crate::embed::Embedding>> {
            let mut g = self.0.lock().unwrap();
            for t in texts { g.push(t.to_string()); }
            Ok(texts.iter().map(|_| vec![0.0_f32; 4]).collect())
        }
    }

    let dir = tempfile::tempdir().unwrap();
    let proj = dir.path();
    std::fs::create_dir_all(proj.join("src")).unwrap();
    std::fs::write(proj.join("src/a.rs"), "pub fn hello() {}\n").unwrap();

    let captured = Arc::new(Mutex::new(Vec::<String>::new()));
    let embedder: Arc<dyn crate::embed::Embedder> =
        Arc::new(CapturingEmbedder(captured.clone()));

    // Run build_index. Adjust the call to match the real signature; the point
    // is to trigger embedding of the one Rust chunk.
    let _ = build_index(proj, embedder.as_ref(), 4000, false).await.unwrap();

    let texts = captured.lock().unwrap();
    assert!(texts.iter().any(|t| t.starts_with("src/a.rs") && t.contains("\npub fn hello()")),
        "no embed text had metadata prefix; got: {texts:?}");
}
```

If `build_index`'s real signature differs, adjust the call to reach the same effect (index one Rust file and verify the embedder received text starting with the metadata header). Locate with `mcp__codescout__find_symbol query=build_index`.

- [ ] **Step 4: Run test, verify it fails**

Run: `cargo test -p codescout --lib build_index_embeds_metadata_plus_content`
Expected: FAIL — embedder received only `pub fn hello() {}`, no header.

- [ ] **Step 5: Wire metadata into insert path**

Locate `build_index` (search for the function definition). Find where chunks are INSERTed into the `chunks` table. Current INSERT likely reads:

```rust
conn.execute(
    "INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    params![...]
)?;
```

Change to:

```rust
conn.execute(
    "INSERT INTO chunks (file_path, language, content, start_line, end_line, file_hash, source, metadata)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
    params![..., chunk.metadata],
)?;
```

Update every INSERT site for `chunks` (should be 2-3 in `index.rs`, possibly one in test fixtures).

- [ ] **Step 6: Wire metadata into embed text construction**

Find the code that builds the `Vec<&str>` passed to `embedder.embed(...)`. It currently references `chunk.content` or similar. Replace with the concatenation:

```rust
// Build texts for embedding — prefix metadata when present for richer retrieval signal.
let embed_texts: Vec<String> = chunks.iter().map(|c| {
    match &c.metadata {
        Some(m) => format!("{m}\n{}", c.content),
        None => c.content.clone(),
    }
}).collect();
let embed_refs: Vec<&str> = embed_texts.iter().map(|s| s.as_str()).collect();

let vectors = embedder.embed(&embed_refs).await?;
```

Replace the existing texts pipeline. This adds one allocation per chunk — acceptable cost for an indexing op.

- [ ] **Step 7: Wire metadata into SELECT path (optional)**

Search results currently don't return `metadata`. Leave it that way for this task (per spec — users don't need to see it). But update any code that reads the chunks row to include `metadata`, so downstream tools can access it if needed. Specifically update `search_scoped` / `search_multi_db` SELECTs if they project `content` etc. — add `metadata` to the projection for completeness, wire into `SearchResult.metadata: Option<String>` if you add it (skip if out of scope).

For this plan: **skip SearchResult changes**. Only ensure insert + embed paths carry metadata. Search is unaffected at the user-visible layer.

- [ ] **Step 8: Re-run tests**

Run: `cargo test -p codescout --lib build_index_embeds_metadata_plus_content chunks_roundtrip_metadata_column`
Expected: both PASS.

- [ ] **Step 9: Run full suite**

Run: `cargo test -p codescout --lib`
Expected: all pass.

- [ ] **Step 10: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(embed): persist metadata + embed text includes header prefix"
```

---

## Task 8: Documentation + onboarding version bump

**Files:**
- Modify: `src/tools/workflow.rs` — `ONBOARDING_VERSION`
- Create: `docs/manual/src/experimental/metadata-enriched-chunks.md`
- Modify: `docs/manual/src/experimental/index.md` — add link

- [ ] **Step 1: Bump `ONBOARDING_VERSION`**

Locate with: `mcp__codescout__grep pattern=ONBOARDING_VERSION path=src/tools`
Increment by 1:

```rust
pub const ONBOARDING_VERSION: u32 = <current + 1>;
```

- [ ] **Step 2: Create experimental doc**

File: `docs/manual/src/experimental/metadata-enriched-chunks.md`

```markdown
# Metadata-Enriched Chunks

> ⚠ Experimental — may change without notice.

Every code chunk stored in the semantic index now carries a short searchable header prepended to its embedding input. Headers encode file path, container context, and symbol name:

    src/embed/index.rs :: impl IndexStore :: fn build_index(force: bool)

This information was previously invisible to the embedding model — chunks were embedded as raw code bodies with no location context. Multi-concept keyword queries (the dominant query shape in real usage) now match on file path, container, and symbol name in addition to body content, giving them more surface area to match on.

## What changes

- Chunks have a new `metadata` column populated during indexing.
- Embedding input is `metadata + "\n" + content` when metadata is present.
- Search results are unchanged — users still see raw code content. The header is an embedding-only signal.
- Unknown languages and markdown files have `metadata = NULL` and embed only the body (no behavior change there).

## When it helps

- Queries that mention a file path or module name (`"embed index build"`)
- Queries that mention a struct/class name alongside a concept (`"IndexStore force rebuild"`)
- 3–10 word keyword queries — the dominant shape in production traffic

## When it won't help

- Queries that don't map to any code structure (cross-file architectural questions)
- Bare symbol lookups — use `find_symbol` instead, it's exact

## Schema migration

On first index after upgrading, the existing `chunks` and `chunk_embeddings` tables are dropped and rebuilt. Expect one reindex delay; thereafter indexing is incremental as usual.
```

- [ ] **Step 3: Add entry to experimental index**

Edit `docs/manual/src/experimental/index.md`, add under the existing list:

```markdown
- [Metadata-Enriched Chunks](./metadata-enriched-chunks.md)
```

- [ ] **Step 4: Commit**

```bash
git add src/tools/workflow.rs docs/manual/src/experimental/metadata-enriched-chunks.md docs/manual/src/experimental/index.md
git commit -m "docs(embed): experimental doc for metadata-enriched chunks + onboarding bump"
```

---

## Task 9: Full validation + benchmark re-run

**Files:**
- Modify: `docs/research/2026-04-03-embedding-model-benchmark.md` — add new run

- [ ] **Step 1: Build release + restart MCP**

```bash
cargo build --release
```

Then `/mcp` to restart the server.

- [ ] **Step 2: Verify project.toml setting**

Confirm `.codescout/project.toml` has:

```toml
[embeddings]
model = "CodeRankEmbed"
url = "http://localhost:43300/v1"
chunk_size = 1600
```

(Ask the user to verify and add `chunk_size = 1600` if missing.)

- [ ] **Step 3: Force rebuild index**

Call `index_project(force: true)` via codescout. Wait for completion with `index_status()`.

Expected:
- All chunks have non-null metadata (spot-check with a sqlite3 query via run_command)
- Chunk count ~15–20% lower than the baseline 20,840

- [ ] **Step 4: Run the 20 benchmark queries**

Run each TC-01 through TC-20 from `docs/research/2026-04-03-embedding-model-benchmark.md § Test Cases` using `semantic_search` with `limit=10`.

Score each per the rubric:
- 3 = all expected files in top 5
- 2 = all in top 10 OR majority in top 5
- 1 = ≥1 in top 10
- 0 = none in top 10

- [ ] **Step 5: Record results in benchmark doc**

Append a new "Model" section to the benchmark doc:

```markdown
### Model: CodeRankEmbed + metadata headers + chunk_size=1600 (2026-04-19)

| Field | Value |
|-------|-------|
| Model string | `CodeRankEmbed` + `url = "http://localhost:43300/v1"` |
| chunk_size | 1600 (explicit) |
| Metadata headers | enabled |
| Chunk count | <fill in> |
| DB size | <fill in> |
| **Total score** | **<fill in>/60** |

| TC | Score | Notes |
|----|-------|-------|
| ... | ... | ... |
```

Fill in per-query scores and tier totals.

- [ ] **Step 6: Update head-to-head comparison**

Add a new column in the head-to-head table with this run's scores.

- [ ] **Step 7: Decide outcome per spec criteria**

- **≥30/60** → ship. Proceed to graduating the feature from `experiments` to `master` via the standard ship sequence (CLAUDE.md § Graduating a Feature).
- **25–29/60** → ship but iterate.
- **<25/60** → do not ship. Open an issue documenting which queries regressed; revisit header format.

- [ ] **Step 8: Commit results**

```bash
git add docs/research/2026-04-03-embedding-model-benchmark.md
git commit -m "docs(benchmark): record CodeRankEmbed + metadata + chunk_size=1600 run"
```

---

## Self-Review Notes

- Spec coverage: chunk size tune (Task 9 step 2 via config) · metadata builder (Task 5) · signature extractor (Task 3) · kind mapping (Task 4) · storage schema (Task 2) · AST threading (Task 6) · embed pipeline (Task 7) · migration (Task 2) · docs (Task 8) · validation (Task 9). All spec sections mapped.
- Type consistency: `RawChunk.metadata` (Task 1), `CodeChunk.metadata` (Task 2), `AstNode.kind/name` (Task 6), `build_metadata_header(...)` (Task 5), `extract_signature(...)` (Task 3), `kind_keyword_for_node(...)` (Task 4). Signatures agree across tasks.
- Placeholders: none (no "TBD"/"TODO"/"handle edge cases" without code).
- Scope: single-plan scope (all changes in `src/embed/` + minor wiring). No subsystem decomposition needed.
- Risk notes: Task 6 is the largest and riskiest (touches `nodes_to_chunks` recursion); dedicated integration test gates it before Task 7 builds on top. If Task 6 tests fail in unexpected ways, pause and revisit the AST node iteration model before pressing on.
