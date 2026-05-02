# Hybrid BM25 + Vector Retrieval Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add always-on Tantivy BM25 retrieval alongside sqlite-vec KNN, fused via RRF (k=60), so `semantic_search` rescues exact identifier/keyword queries that pure vector search fails.

**Architecture:** Tantivy index at `.codescout/tantivy/` is a derived artifact of the `chunks` SQLite table. `build_index` rebuilds it from scratch after `db_writer` completes. `semantic_search` runs both legs inside its existing `spawn_blocking`, fuses via RRF, then fetches BM25-only hits from SQLite before returning. BM25 applies to project scope only; library scopes remain vector-only.

**Tech Stack:** `tantivy 0.22` (BM25 + custom tokenizer), `rusqlite` (SQLite join for BM25-only hits), existing `sqlite-vec` vec0 (KNN leg, unchanged).

**Spec:** `docs/superpowers/specs/2026-05-02-hybrid-retrieval-design.md`

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `Cargo.toml` | Modify | Add `tantivy = "0.22"` |
| `src/embed/bm25.rs` | **Create** | `CodeTokenizer`, `tokenize_code_text`, `BM25Index` (open/build/search) |
| `src/embed/fusion.rs` | **Create** | `BM25Result`, `rrf_fuse` |
| `src/embed/schema.rs` | Modify | Add `pub id: u64` to `SearchResult` |
| `src/embed/index.rs` | Modify | Add `c.id` to SELECT in `search_scoped` + `search_scoped_vec0`; call `BM25Index::build` at end of `build_index` |
| `src/embed/mod.rs` | Modify | `pub mod bm25; pub mod fusion;` |
| `src/tools/semantic.rs` | Modify | Add BM25 leg + RRF fusion + BM25-only SQLite join in `call` |

---

## Task 1: Add tantivy + module scaffolding

**Files:**
- Modify: `Cargo.toml`
- Create: `src/embed/bm25.rs`
- Create: `src/embed/fusion.rs`
- Modify: `src/embed/mod.rs`

- [ ] **Step 1: Add tantivy to Cargo.toml**

In `Cargo.toml`, under `[dependencies]` (after the `sqlite-vec` line):
```toml
tantivy = "0.22"
```

- [ ] **Step 2: Create src/embed/bm25.rs**

```rust
use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use tantivy::{
    collector::TopDocs,
    directory::MmapDirectory,
    query::QueryParser,
    schema::{
        Field, IndexRecordOption, Schema, SchemaBuilder, TextFieldIndexing, TextOptions,
        FAST, STORED,
    },
    tokenizer::{Token, TokenStream, Tokenizer},
    Index, IndexSettings, IndexWriter, TantivyDocument,
};

pub use crate::embed::fusion::BM25Result;

// ── Tokenizer ────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct CodeTokenizer;

pub struct CodeTokenStream {
    tokens: Vec<Token>,
    index: usize,
}

impl TokenStream for CodeTokenStream {
    fn advance(&mut self) -> bool {
        if self.index < self.tokens.len() {
            self.index += 1;
            true
        } else {
            false
        }
    }
    fn token(&self) -> &Token {
        &self.tokens[self.index - 1]
    }
    fn token_mut(&mut self) -> &mut Token {
        &mut self.tokens[self.index - 1]
    }
}

impl Tokenizer for CodeTokenizer {
    type TokenStream<'a> = CodeTokenStream;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> CodeTokenStream {
        let raw = tokenize_code_text(text);
        let tokens = raw
            .into_iter()
            .enumerate()
            .map(|(pos, text)| Token {
                offset_from: 0,
                offset_to: 0,
                position: pos,
                text,
                position_length: 1,
            })
            .collect();
        CodeTokenStream { tokens, index: 0 }
    }
}

pub fn tokenize_code_text(_text: &str) -> Vec<String> {
    todo!("Task 2")
}

// ── Schema ───────────────────────────────────────────────────────────────────

fn build_schema() -> (Schema, Field, Field, Field, Field) {
    todo!("Task 3")
}

fn tantivy_dir(root: &Path) -> std::path::PathBuf {
    root.join(".codescout").join("tantivy")
}

// ── BM25Index ────────────────────────────────────────────────────────────────

pub struct BM25Index {
    index: Index,
    chunk_id: Field,
    content: Field,
    file_path: Field,
    metadata: Field,
}

impl BM25Index {
    pub fn open(root: &Path) -> Result<Option<Self>> {
        todo!("Task 3")
    }

    pub fn build(root: &Path, conn: &Connection) -> Result<Self> {
        todo!("Task 3")
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<BM25Result>> {
        todo!("Task 3")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
```

- [ ] **Step 3: Create src/embed/fusion.rs**

```rust
use crate::embed::schema::SearchResult;

#[derive(Debug, Clone)]
pub struct BM25Result {
    pub chunk_id: u64,
    pub score: f32,
    pub rank: usize,
}

/// Fuse vector and BM25 ranked lists via Reciprocal Rank Fusion.
/// Returns chunk_ids in descending RRF score order.
/// k=60 is the canonical constant — set it lower (e.g. 1.0) to amplify rank differences.
pub fn rrf_fuse(vector: &[SearchResult], bm25: &[BM25Result], k: f32) -> Vec<u64> {
    todo!("Task 5")
}

#[cfg(test)]
mod tests {
    use super::*;
}
```

- [ ] **Step 4: Register modules in src/embed/mod.rs**

Add two lines at the top of the existing module list (after `pub mod ast_chunker`):

```rust
pub mod bm25;
pub mod fusion;
```

- [ ] **Step 5: Verify scaffold compiles**

```bash
cargo build 2>&1 | grep -E "^error" | head -20
```

Expected: zero `error` lines. `todo!()` panics are fine.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/embed/bm25.rs src/embed/fusion.rs src/embed/mod.rs
git commit -m "chore(bm25): scaffold tantivy + fusion modules"
```

---

## Task 2: CodeTokenizer

**Files:**
- Modify: `src/embed/bm25.rs`

- [ ] **Step 1: Write failing tests**

Replace the empty `tests` module in `src/embed/bm25.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_splits_camel_case() {
        assert_eq!(
            tokenize_code_text("parseJsonObject"),
            vec!["parse", "json", "object"]
        );
    }

    #[test]
    fn tokenizer_splits_snake_case() {
        assert_eq!(tokenize_code_text("open_db"), vec!["open", "db"]);
    }

    #[test]
    fn tokenizer_splits_file_path() {
        assert_eq!(
            tokenize_code_text("src/embed/index.rs"),
            vec!["src", "embed", "index", "rs"]
        );
    }

    #[test]
    fn tokenizer_handles_mixed_ident() {
        assert_eq!(
            tokenize_code_text("impl Tool for SemanticSearch"),
            vec!["impl", "tool", "for", "semantic", "search"]
        );
    }

    #[test]
    fn tokenizer_strips_empty_tokens() {
        let tokens = tokenize_code_text("  spaces  and__double  ");
        assert!(!tokens.iter().any(|t| t.is_empty()), "no empty tokens");
    }
}
```

- [ ] **Step 2: Run tests — confirm FAIL**

```bash
cargo test embed::bm25::tests 2>&1 | tail -15
```

Expected: FAIL — `tokenize_code_text` is `todo!()`.

- [ ] **Step 3: Implement tokenize_code_text and split_camel_case**

Replace the `tokenize_code_text` stub in `src/embed/bm25.rs` (and add `split_camel_case` above it):

```rust
fn split_camel_case(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_uppercase() && chars[i - 1].is_lowercase() {
            if !current.is_empty() {
                parts.push(std::mem::take(&mut current));
            }
        }
        current.push(c);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts.into_iter().map(|s| s.to_lowercase()).collect()
}

pub fn tokenize_code_text(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        if word.is_empty() {
            continue;
        }
        for part in word.split('_').filter(|s| !s.is_empty()) {
            tokens.extend(split_camel_case(part));
        }
    }
    tokens
}
```

- [ ] **Step 4: Run tests — confirm PASS**

```bash
cargo test embed::bm25::tests 2>&1 | tail -15
```

Expected: 5 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/embed/bm25.rs
git commit -m "feat(bm25): implement CodeTokenizer with camelCase + snake_case splitting"
```

---

## Task 3: BM25Index — schema, open, build, search

**Files:**
- Modify: `src/embed/bm25.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `src/embed/bm25.rs`:

```rust
    #[test]
    fn build_and_search_roundtrip() {
        use rusqlite::Connection;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let conn = Connection::open(":memory:").unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                id       INTEGER PRIMARY KEY,
                content  TEXT NOT NULL,
                file_path TEXT NOT NULL,
                metadata TEXT,
                source   TEXT NOT NULL DEFAULT 'project'
            );
            INSERT INTO chunks VALUES
              (1,'fn parse_model(name: &str) -> Result<EmbeddingModel>','src/embed/local.rs','fn parse_model','project'),
              (2,'struct SearchResult { file_path: String }','src/embed/schema.rs','struct SearchResult','project'),
              (3,'fn open_db(project_root: &Path) -> Result<Connection>','src/embed/index.rs','fn open_db','project');",
        ).unwrap();

        let idx = BM25Index::build(root, &conn).unwrap();
        let results = idx.search("parse model embedding", 10).unwrap();
        assert!(!results.is_empty(), "expected results");
        assert_eq!(results[0].chunk_id, 1, "parse_model chunk should rank first");
    }

    #[test]
    fn build_excludes_library_source() {
        use rusqlite::Connection;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let conn = Connection::open(":memory:").unwrap();
        conn.execute_batch(
            "CREATE TABLE chunks (
                id INTEGER PRIMARY KEY, content TEXT NOT NULL,
                file_path TEXT NOT NULL, metadata TEXT,
                source TEXT NOT NULL DEFAULT 'project'
            );
            INSERT INTO chunks VALUES (1,'project chunk','proj.rs',NULL,'project');
            INSERT INTO chunks VALUES (2,'library chunk','lib.rs',NULL,'library');",
        ).unwrap();

        let idx = BM25Index::build(dir.path(), &conn).unwrap();
        let results = idx.search("library chunk", 10).unwrap();
        // library source excluded from BM25 index — only project chunk indexed
        assert!(!results.iter().any(|r| r.chunk_id == 2), "library chunks must not appear");
    }

    #[test]
    fn open_returns_none_when_absent() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        let result = BM25Index::open(dir.path()).unwrap();
        assert!(result.is_none());
    }
```

- [ ] **Step 2: Run tests — confirm FAIL**

```bash
cargo test embed::bm25::tests 2>&1 | tail -20
```

Expected: 3 new tests FAIL — `build_schema`, `open`, `build`, `search` are all `todo!()`.

- [ ] **Step 3: Implement build_schema**

Replace the `build_schema` stub in `src/embed/bm25.rs`:

```rust
fn build_schema() -> (Schema, Field, Field, Field, Field) {
    let mut b = SchemaBuilder::new();
    let chunk_id = b.add_u64_field("chunk_id", FAST | STORED);
    let mk_text = |name: &str, builder: &mut SchemaBuilder| {
        builder.add_text_field(
            name,
            TextOptions::default()
                .set_indexing_options(
                    TextFieldIndexing::default()
                        .set_tokenizer("code")
                        .set_index_option(IndexRecordOption::WithFreqsAndPositions),
                )
                .set_stored(),
        )
    };
    let content = mk_text("content", &mut b);
    let file_path = mk_text("file_path", &mut b);
    let metadata = mk_text("metadata", &mut b);
    (b.build(), chunk_id, content, file_path, metadata)
}
```

- [ ] **Step 4: Implement BM25Index::open**

Replace the `open` stub:

```rust
    pub fn open(root: &Path) -> Result<Option<Self>> {
        let dir_path = tantivy_dir(root);
        if !dir_path.exists() {
            return Ok(None);
        }
        let dir = MmapDirectory::open(&dir_path)?;
        if !Index::exists(&dir)? {
            return Ok(None);
        }
        let (_, chunk_id, content, file_path, metadata) = build_schema();
        let index = Index::open(dir)?;
        index.tokenizers().register("code", CodeTokenizer::default());
        Ok(Some(Self { index, chunk_id, content, file_path, metadata }))
    }
```

- [ ] **Step 5: Implement BM25Index::build**

Replace the `build` stub:

```rust
    pub fn build(root: &Path, conn: &Connection) -> Result<Self> {
        let dir_path = tantivy_dir(root);
        if dir_path.exists() {
            std::fs::remove_dir_all(&dir_path)?;
        }
        std::fs::create_dir_all(&dir_path)?;

        let (schema, chunk_id, content, file_path, metadata) = build_schema();
        let dir = MmapDirectory::open(&dir_path)?;
        let index = Index::create(dir, schema, IndexSettings::default())?;
        index.tokenizers().register("code", CodeTokenizer::default());

        let mut writer: IndexWriter = index.writer(50_000_000)?;

        let mut stmt = conn.prepare(
            "SELECT id, content, file_path, COALESCE(metadata, '') \
             FROM chunks WHERE source = 'project'",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)? as u64,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (id, cont, fp, meta) = row?;
            let mut doc = TantivyDocument::default();
            doc.add_u64(chunk_id, id);
            doc.add_text(content, &cont);
            doc.add_text(file_path, &fp);
            doc.add_text(metadata, &meta);
            writer.add_document(doc)?;
        }
        writer.commit()?;

        Ok(Self { index, chunk_id, content, file_path, metadata })
    }
```

- [ ] **Step 6: Implement BM25Index::search**

Replace the `search` stub:

```rust
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<BM25Result>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let mut parser = QueryParser::for_index(
            &self.index,
            vec![self.content, self.file_path, self.metadata],
        );
        parser.set_field_boost(self.content, 1.0);
        parser.set_field_boost(self.file_path, 1.5);
        parser.set_field_boost(self.metadata, 2.0);

        let (query, _) = parser.parse_query_lenient(query_str);
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        top_docs
            .iter()
            .enumerate()
            .map(|(rank, (score, addr))| {
                let doc: TantivyDocument = searcher.doc(*addr)?;
                let chunk_id = doc
                    .get_first(self.chunk_id)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                Ok(BM25Result { chunk_id, score: *score, rank: rank + 1 })
            })
            .collect()
    }
```

- [ ] **Step 7: Run all bm25 tests — confirm PASS**

```bash
cargo test embed::bm25::tests 2>&1 | tail -20
```

Expected: 8 tests PASS (5 tokenizer + 3 index).

- [ ] **Step 8: Commit**

```bash
git add src/embed/bm25.rs
git commit -m "feat(bm25): implement BM25Index — open, build, search with field boosting"
```

---

## Task 4: Add `id` to SearchResult + update search queries

`rrf_fuse` needs to join BM25 chunk_ids back to vector `SearchResult`s. `SearchResult` currently has no `id` field; the SQL queries don't SELECT `c.id`. This task adds both.

**Files:**
- Modify: `src/embed/schema.rs` (add `id` field)
- Modify: `src/embed/index.rs` (add `c.id` to SELECT in `search_scoped` and `search_scoped_vec0`, update `map_row` in both)

- [ ] **Step 1: Add `id: u64` to SearchResult in src/embed/schema.rs**

Use `edit_code` to add the field as the first member of `SearchResult`:

```rust
pub struct SearchResult {
    pub id: u64,          // ← add this line
    pub file_path: String,
    pub language: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f32,
    pub source: String,
    pub project_id: String,
}
```

- [ ] **Step 2: Fix compiler errors from the schema change**

```bash
cargo build 2>&1 | grep "^error" | head -30
```

All `SearchResult { ... }` struct literals now need `id: 0` (or real value). Fix each one:

- `src/embed/index.rs` — two `map_row` closures in `search_scoped` and `search_scoped_vec0`
- `src/embed/drift.rs` — if `SearchResult` is constructed there
- Any test helpers constructing `SearchResult` directly

For each location constructing `SearchResult` without a real id yet, add `id: 0` as a placeholder. The real fix (using actual `c.id`) happens in the next step.

- [ ] **Step 3: Update search_scoped SELECT to include c.id**

In `src/embed/index.rs`, find `search_scoped` (around line 1110). The current `sel` string is:
```rust
let sel = "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, c.source, \
           COALESCE(vec_distance_cosine(vec_f32(ce.embedding), vec_f32(?1)), 1.0) AS distance, \
           c.project_id \
           FROM chunks c JOIN chunk_embeddings ce ON c.id = ce.rowid";
```

Change to add `c.id` at position 8 (after `project_id`):
```rust
let sel = "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, c.source, \
           COALESCE(vec_distance_cosine(vec_f32(ce.embedding), vec_f32(?1)), 1.0) AS distance, \
           c.project_id, c.id \
           FROM chunks c JOIN chunk_embeddings ce ON c.id = ce.rowid";
```

Update `map_row` in `search_scoped` to read column 8:
```rust
let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<SearchResult> {
    let distance: f64 = row.get(6)?;
    let score = (1.0_f32 - distance as f32).clamp(0.0, 1.0);
    Ok(SearchResult {
        id: row.get::<_, i64>(8)? as u64,   // ← new
        file_path: row.get(0)?,
        language: row.get(1)?,
        content: row.get(2)?,
        start_line: row.get::<_, i64>(3)? as usize,
        end_line: row.get::<_, i64>(4)? as usize,
        source: row.get(5)?,
        score,
        project_id: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
    })
};
```

- [ ] **Step 4: Update search_scoped_vec0 SELECT to include c.id**

In `src/embed/index.rs`, find `search_scoped_vec0` (around line 1179). Current `sel_exact`:
```rust
let sel_exact = format!(
    "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, c.source, \
     COALESCE(knn.distance, 1.0) AS distance, c.project_id \
     FROM chunks c JOIN ({knn_exact}) knn ON c.id = knn.rowid"
);
let sel_over = format!(
    "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, c.source, \
     COALESCE(knn.distance, 1.0) AS distance, c.project_id \
     FROM chunks c JOIN ({knn_over}) knn ON c.id = knn.rowid"
);
```

Add `c.id` at the end of both:
```rust
let sel_exact = format!(
    "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, c.source, \
     COALESCE(knn.distance, 1.0) AS distance, c.project_id, c.id \
     FROM chunks c JOIN ({knn_exact}) knn ON c.id = knn.rowid"
);
let sel_over = format!(
    "SELECT c.file_path, c.language, c.content, c.start_line, c.end_line, c.source, \
     COALESCE(knn.distance, 1.0) AS distance, c.project_id, c.id \
     FROM chunks c JOIN ({knn_over}) knn ON c.id = knn.rowid"
);
```

Update `map_row` in `search_scoped_vec0` to read column 8:
```rust
let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<SearchResult> {
    let distance: f64 = row.get(6)?;
    let score = (1.0_f32 - distance as f32).clamp(0.0, 1.0);
    Ok(SearchResult {
        id: row.get::<_, i64>(8)? as u64,   // ← new
        file_path: row.get(0)?,
        language: row.get(1)?,
        content: row.get(2)?,
        start_line: row.get::<_, i64>(3)? as usize,
        end_line: row.get::<_, i64>(4)? as usize,
        source: row.get(5)?,
        score,
        project_id: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
    })
};
```

- [ ] **Step 5: Run tests — confirm passing**

```bash
cargo test 2>&1 | grep -E "FAILED|error\[" | head -20
```

Expected: zero failures. All existing tests still pass with `id` populated from the DB.

- [ ] **Step 6: Commit**

```bash
git add src/embed/schema.rs src/embed/index.rs
git commit -m "feat(bm25): add id field to SearchResult, SELECT c.id in vector search queries"
```

---

## Task 5: Implement RRF fusion

**Files:**
- Modify: `src/embed/fusion.rs`

- [ ] **Step 1: Write failing tests**

Replace the empty `tests` module in `src/embed/fusion.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::schema::SearchResult;

    fn sr(id: u64, score: f32) -> SearchResult {
        SearchResult {
            id,
            file_path: format!("f{}.rs", id),
            language: "rust".into(),
            content: format!("c{}", id),
            start_line: 0,
            end_line: 1,
            score,
            source: "project".into(),
            project_id: "root".into(),
        }
    }

    fn bm(id: u64, rank: usize) -> BM25Result {
        BM25Result { chunk_id: id, score: 1.0, rank }
    }

    #[test]
    fn rrf_promotes_dual_hit_above_single_leg() {
        // vector: [1(rank1), 2(rank2)]; bm25: [2(rank1), 3(rank2)]
        // id=1: 1/(60+1)            = 0.01639 (vector only)
        // id=2: 1/(60+2)+1/(60+1)   = 0.01613+0.01639 = 0.03252 (both legs)
        // id=3: 1/(60+2)            = 0.01613 (bm25 only)
        // expected order: [2, 1, 3]
        let vector = vec![sr(1, 0.9), sr(2, 0.8)];
        let bm25 = vec![bm(2, 1), bm(3, 2)];
        let fused = rrf_fuse(&vector, &bm25, 60.0);
        assert_eq!(fused[0], 2, "dual hit should rank first");
        assert_eq!(fused[1], 1);
        assert_eq!(fused[2], 3);
    }

    #[test]
    fn rrf_bm25_only_hit_appears_in_output() {
        let vector = vec![sr(1, 0.9), sr(2, 0.8)];
        let bm25 = vec![bm(99, 1), bm(1, 2)];
        let fused = rrf_fuse(&vector, &bm25, 60.0);
        assert!(fused.contains(&99), "BM25-only chunk must appear");
    }

    #[test]
    fn rrf_empty_bm25_preserves_vector_order() {
        let vector = vec![sr(1, 0.9), sr(2, 0.8), sr(3, 0.7)];
        let fused = rrf_fuse(&vector, &[], 60.0);
        assert_eq!(fused, vec![1, 2, 3]);
    }

    #[test]
    fn rrf_empty_vector_preserves_bm25_order() {
        let bm25 = vec![bm(1, 1), bm(2, 2)];
        let fused = rrf_fuse(&[], &bm25, 60.0);
        assert_eq!(fused, vec![1, 2]);
    }
}
```

- [ ] **Step 2: Run tests — confirm FAIL**

```bash
cargo test embed::fusion::tests 2>&1 | tail -15
```

Expected: FAIL — `rrf_fuse` is `todo!()`.

- [ ] **Step 3: Implement rrf_fuse**

Replace the `rrf_fuse` stub in `src/embed/fusion.rs`:

```rust
pub fn rrf_fuse(vector: &[SearchResult], bm25: &[BM25Result], k: f32) -> Vec<u64> {
    use std::collections::HashMap;

    let mut scores: HashMap<u64, f32> = HashMap::new();

    // Vector leg: rank = 1-indexed position in the slice
    for (i, sr) in vector.iter().enumerate() {
        let rank = (i + 1) as f32;
        *scores.entry(sr.id).or_insert(0.0) += 1.0 / (k + rank);
    }

    // BM25 leg: rank already stored in BM25Result
    for r in bm25 {
        let rank = r.rank as f32;
        *scores.entry(r.chunk_id).or_insert(0.0) += 1.0 / (k + rank);
    }

    // Sort by RRF score descending, break ties by id for stability
    let mut ranked: Vec<(u64, f32)> = scores.into_iter().collect();
    ranked.sort_by(|(id_a, sa), (id_b, sb)| {
        sb.partial_cmp(sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| id_a.cmp(id_b))
    });

    ranked.into_iter().map(|(id, _)| id).collect()
}
```

- [ ] **Step 4: Run tests — confirm PASS**

```bash
cargo test embed::fusion::tests 2>&1 | tail -15
```

Expected: 4 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/embed/fusion.rs
git commit -m "feat(bm25): implement RRF fusion"
```

---

## Task 6: Wire BM25 build into build_index

After `db_writer` completes, `conn` has been moved into the writer. Re-open to build the Tantivy index.

**Files:**
- Modify: `src/embed/index.rs`

- [ ] **Step 1: Write a staleness test**

Add to the `tests` module in `src/embed/index.rs`:

```rust
#[test]
fn bm25_index_built_after_build_index() {
    // Verify that build_index creates .codescout/tantivy/ alongside the DB.
    // Uses the existing test helpers to set up a minimal project.
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    // Minimal project: one Rust file
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/lib.rs"), "pub fn hello() {}").unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"t\"\nversion=\"0.1.0\"").unwrap();

    // build_index requires an active embedder — skip if none available
    // (this test is intentionally light; the roundtrip is covered by bm25::tests)
    let tantivy_dir = root.join(".codescout").join("tantivy");
    // After a successful build_index the dir must exist
    // We can't run build_index in unit tests (needs network/ONNX), so assert the
    // helper builds the dir when called directly.
    crate::embed::bm25::BM25Index::build(root, &crate::embed::index::open_db(root).unwrap()).unwrap();
    assert!(tantivy_dir.exists(), ".codescout/tantivy/ must be created by build");
}
```

- [ ] **Step 2: Run test — confirm PASS**

```bash
cargo test embed::index::tests::bm25_index_built 2>&1 | tail -10
```

Expected: PASS — `BM25Index::build` already works from Task 3.

- [ ] **Step 3: Add BM25 build call to build_index**

In `src/embed/index.rs`, find the section after `let (indexed, drift_results) = writer_result?;` (around line 2105) and before the `tracing::info!` call.

Add these lines:

```rust
    let (indexed, drift_results) = writer_result?;

    // BM25 index: full rebuild from the freshly-written chunks table.
    // conn was moved into db_writer — re-open for the BM25 pass.
    {
        let bm25_conn = open_db(project_root)?;
        crate::embed::bm25::BM25Index::build(project_root, &bm25_conn)?;
    }

    tracing::info!(
```

- [ ] **Step 4: Run full test suite**

```bash
cargo test 2>&1 | grep -E "FAILED|^error" | head -20
```

Expected: zero failures.

- [ ] **Step 5: Commit**

```bash
git add src/embed/index.rs
git commit -m "feat(bm25): rebuild Tantivy index at end of build_index"
```

---

## Task 7: Wire hybrid search into semantic_search

**Files:**
- Modify: `src/tools/semantic.rs`

The `spawn_blocking` closure in `SemanticSearch::call` currently (around line 185 in the closure body):

```rust
let results = crate::embed::index::search_multi_db(...)?;
let results = apply_file_diversity_cap(results, MAX_CHUNKS_PER_FILE);
let results: Vec<_> = results.into_iter().take(limit).collect();
```

Replace those three lines with the hybrid pipeline.

- [ ] **Step 1: Write smoke test**

Add to `tests` module in `src/tools/semantic.rs`:

```rust
#[test]
fn rrf_fuse_integration_empty_bm25_returns_vector_order() {
    // When BM25 returns empty (no tantivy dir), output matches vector order.
    use crate::embed::fusion;
    use crate::embed::schema::SearchResult;

    let vector = vec![
        SearchResult { id: 1, file_path: "a.rs".into(), language: "rust".into(),
            content: "a".into(), start_line: 0, end_line: 1, score: 0.9,
            source: "project".into(), project_id: "root".into() },
        SearchResult { id: 2, file_path: "b.rs".into(), language: "rust".into(),
            content: "b".into(), start_line: 0, end_line: 1, score: 0.8,
            source: "project".into(), project_id: "root".into() },
    ];
    let fused_ids = fusion::rrf_fuse(&vector, &[], 60.0);
    assert_eq!(fused_ids, vec![1, 2]);
}
```

- [ ] **Step 2: Run test — confirm PASS**

```bash
cargo test tools::semantic::tests::rrf_fuse_integration 2>&1 | tail -10
```

Expected: PASS.

- [ ] **Step 3: Add imports to src/tools/semantic.rs**

At the top of `src/tools/semantic.rs`, add:

```rust
use std::collections::HashMap;
```

- [ ] **Step 4: Clone query string before spawn_blocking**

`query` is a `&str` borrowed from `input`; it can't be captured by the `move ||` closure. Find the block of variable clones before `spawn_blocking` (where `root2`, `model2`, `scope2` are declared) and add:

```rust
let query2 = query.to_string();
```

Then in all subsequent steps, use `&query2` inside the closure wherever the BM25 search passes the query string.

- [ ] **Step 5: Replace the three-line search block in spawn_blocking**

Locate this block in the `spawn_blocking` closure (the exact content to replace):

```rust
            let results = crate::embed::index::search_multi_db(
                &root2,
                &query_embedding,
                search_limit,
                &scope2,
                &library_registry2,
                project_filter.as_deref(),
            )?;
            let results = apply_file_diversity_cap(results, MAX_CHUNKS_PER_FILE);
            let results: Vec<_> = results.into_iter().take(limit).collect();
```

Replace with:

```rust
            let vector_results = crate::embed::index::search_multi_db(
                &root2,
                &query_embedding,
                search_limit,
                &scope2,
                &library_registry2,
                project_filter.as_deref(),
            )?;

            // BM25 leg — project scope only; other scopes fall back to pure vector
            let bm25_results = if matches!(scope2, crate::library::scope::Scope::Project) {
                match crate::embed::bm25::BM25Index::open(&root2)? {
                    Some(idx) => idx.search(&query2, search_limit).unwrap_or_default(),
                    None => vec![],
                }
            } else {
                vec![]
            };

            // RRF fusion: re-rank when BM25 has results, else preserve vector order
            let results = if bm25_results.is_empty() {
                vector_results
            } else {
                let fused_ids =
                    crate::embed::fusion::rrf_fuse(&vector_results, &bm25_results, 60.0);

                // Build lookup from vector results (already have full data)
                let mut sr_map: HashMap<u64, crate::embed::schema::SearchResult> =
                    vector_results.into_iter().map(|r| (r.id, r)).collect();

                // Fetch BM25-only hits that vector search didn't return
                let bm25_only: Vec<u64> = fused_ids
                    .iter()
                    .filter(|id| !sr_map.contains_key(id))
                    .copied()
                    .collect();
                if !bm25_only.is_empty() {
                    let conn2 = crate::embed::index::open_db(&root2)?;
                    let placeholders = bm25_only
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("?{}", i + 1))
                        .collect::<Vec<_>>()
                        .join(",");
                    let sql = format!(
                        "SELECT id, file_path, language, content, start_line, \
                         end_line, source, project_id FROM chunks WHERE id IN ({})",
                        placeholders
                    );
                    let mut stmt = conn2.prepare(&sql)?;
                    let params =
                        rusqlite::params_from_iter(bm25_only.iter().map(|id| *id as i64));
                    let rows = stmt.query_map(params, |row| {
                        Ok(crate::embed::schema::SearchResult {
                            id: row.get::<_, i64>(0)? as u64,
                            file_path: row.get(1)?,
                            language: row.get(2)?,
                            content: row.get(3)?,
                            start_line: row.get::<_, i64>(4)? as usize,
                            end_line: row.get::<_, i64>(5)? as usize,
                            source: row.get(6)?,
                            score: 0.0,
                            project_id: row.get(7)?,
                        })
                    })?;
                    for row in rows.flatten() {
                        sr_map.insert(row.id, row);
                    }
                }

                // Reconstruct Vec<SearchResult> in fused order
                fused_ids
                    .into_iter()
                    .filter_map(|id| sr_map.remove(&id))
                    .collect()
            };

            let results = apply_file_diversity_cap(results, MAX_CHUNKS_PER_FILE);
            let results: Vec<_> = results.into_iter().take(limit).collect();
```

- [ ] **Step 6: Run full test suite**

```bash
cargo test 2>&1 | grep -E "FAILED|^error" | head -20
```

Expected: zero failures.

- [ ] **Step 7: Check clippy**

```bash
cargo clippy -- -D warnings 2>&1 | grep "^error" | head -20
```

Expected: zero errors.

- [ ] **Step 8: Commit**

```bash
git add src/tools/semantic.rs
git commit -m "feat(bm25): wire hybrid BM25+vector search with RRF fusion into semantic_search"
```

---

## Task 8: Build release binary + run benchmark

- [ ] **Step 1: Build release binary**

```bash
cargo build --release 2>&1 | tail -5
```

Expected: `Finished release [optimized]`.

- [ ] **Step 2: Restart MCP server**

In Claude Code: run `/mcp` to restart the MCP server and pick up the new release binary.

- [ ] **Step 3: Activate project and rebuild index**

```
workspace(action="activate", path=".")
index(action="build", force=true)
```

Expected: completes without error. `.codescout/tantivy/` directory created.

- [ ] **Step 4: Run all 20 benchmark queries**

Open `docs/research/2026-04-03-embedding-model-benchmark.md` and run each of the 20 test cases (TC-01 through TC-20) against `semantic_search`. Record results in a new section:

```markdown
### Model: <current model> — hybrid BM25+vector (2026-05-02)
```

Score each using the existing 0–3 rubric.

- [ ] **Step 5: Compare against baseline**

Calculate tier scores. Target: total ≥ 40/60 (up from 36/60 best baseline).

Primary regression targets (all currently 0/3):
- **TC-10** — "output cap overflow hint" (vocabulary mismatch)
- **TC-19** — "activation wiring agent project" (vocabulary mismatch)
- **TC-20** — "prompt surface tool name consistency" (vocabulary mismatch)

At least one of these three must improve to ≥ 1/3 for the hybrid feature to be considered successful.

- [ ] **Step 6: Commit benchmark results**

```bash
git add docs/research/2026-04-03-embedding-model-benchmark.md
git commit -m "docs(benchmark): add hybrid BM25+vector results 2026-05-02"
```

- [ ] **Step 7: Final checks**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

Expected: all pass.
