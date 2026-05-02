# Auto-Reindex on Edit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** When `semantic_search` is called after write tools have modified files, automatically re-embed the changed files before searching so results are never stale.

**Architecture:** Write tools already call `ctx.agent.mark_file_dirty(path)` which populates `ActiveProject.dirty_files: Arc<Mutex<HashSet<PathBuf>>>`. We add (1) a `drain_dirty_files()` accessor on `Agent`, (2) a `reindex_files()` function that hash-gates and re-embeds a list of files, and (3) a pre-search drain step in `SemanticSearch::call()` that replaces the existing warn-only block.

**Tech Stack:** Rust, SQLite (rusqlite), `codescout_embed` crate (`Embedder` trait, `RawChunk`), `ast_chunker::split_file`, existing index primitives (`insert_chunk`, `delete_file_chunks`, `upsert_file_hash`, `is_file_changed_mtime_hash`).

---

### Task 1: Add `drain_dirty_files` to `Agent`

**Files:**
- Modify: `src/agent/mod.rs` (near `dirty_file_count` at line 625)

The `dirty_files` set already exists on `ActiveProject`. We need a drain accessor that clears the set and returns the paths for processing.

- [x] **Step 1: Write the failing test**

Add inside the `#[cfg(test)]` block of `src/agent/mod.rs` (or create one if absent):

```rust
#[tokio::test]
async fn drain_dirty_files_clears_set_and_returns_paths() {
    use std::path::PathBuf;
    use crate::agent::test_helpers::make_test_agent; // use the pattern for Agent construction in this file's tests

    let agent = make_test_agent().await;
    let a = PathBuf::from("/proj/src/a.rs");
    let b = PathBuf::from("/proj/src/b.rs");
    agent.mark_file_dirty(a.clone()).await;
    agent.mark_file_dirty(b.clone()).await;

    let mut drained = agent.drain_dirty_files().await;
    drained.sort();
    assert_eq!(drained, vec![a, b]);

    // Set must be empty after drain
    assert_eq!(agent.drain_dirty_files().await, vec![]);
}
```

> Note: Look at the existing Agent tests in this file for the correct helper to construct a minimal Agent for testing.

- [x] **Step 2: Run the test to verify it fails**

```bash
cargo test -p codescout drain_dirty_files -- --nocapture 2>&1
```

Expected: compile error — `drain_dirty_files` does not exist yet.

- [x] **Step 3: Implement `drain_dirty_files`**

In `src/agent/mod.rs`, add immediately after `dirty_file_count` (around line 636):

```rust
/// Drain all files marked dirty by write tools, returning them for re-indexing.
/// Clears the set so subsequent calls return only newly-dirtied files.
pub async fn drain_dirty_files(&self) -> Vec<PathBuf> {
    let inner = self.inner.read().await;
    inner
        .active_project()
        .map(|p| {
            let mut set = p.dirty_files.lock().unwrap_or_else(|e| e.into_inner());
            set.drain().collect()
        })
        .unwrap_or_default()
}
```

- [x] **Step 4: Run the test to verify it passes**

```bash
cargo test -p codescout drain_dirty_files -- --nocapture 2>&1
```

Expected: PASS.

- [x] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings
git add src/agent/mod.rs
git commit -m "feat(agent): add drain_dirty_files accessor"
```

---

### Task 2: Add `reindex_files` to `src/embed/index.rs`

**Files:**
- Modify: `src/embed/index.rs`

This function takes a list of absolute paths, hash-gates each one, chunks and embeds changed files, and writes the result to the SQLite index. It is called by `SemanticSearch` (Task 3).

- [x] **Step 1: Write the three unit tests**

Add inside the `#[cfg(test)]` module at the bottom of `src/embed/index.rs`:

```rust
// ── reindex_files tests ───────────────────────────────────────────────────────

#[test]
fn reindex_files_skips_empty_input() {
    // Trivially returns 0 without touching the DB
    let dir = tempdir().unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let embedder = std::sync::Arc::new(crate::embed::local::test_embedder());
    let result = rt.block_on(reindex_files(dir.path(), &[], &embedder));
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn reindex_files_skips_hash_unchanged() {
    // File already indexed with correct hash → hash gate fires, count = 0
    let dir = tempdir().unwrap();
    let conn = open_db(dir.path()).unwrap();

    let file = dir.path().join("mod.rs");
    std::fs::write(&file, "fn a() {}").unwrap();
    let hash = hash_file(&file).unwrap();
    let mtime = file_mtime(&file).unwrap();
    upsert_file_hash(&conn, "mod.rs", &hash, Some(mtime)).unwrap();
    drop(conn); // close before reindex_files opens its own

    let rt = tokio::runtime::Runtime::new().unwrap();
    let embedder = std::sync::Arc::new(crate::embed::local::test_embedder());
    let count = rt
        .block_on(reindex_files(dir.path(), &[file], &embedder))
        .unwrap();
    assert_eq!(count, 0, "hash gate should prevent re-embed");
}

#[test]
fn reindex_files_skips_unreadable_path() {
    // Non-existent path → gracefully skipped, count = 0
    let dir = tempdir().unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let embedder = std::sync::Arc::new(crate::embed::local::test_embedder());
    let ghost = dir.path().join("ghost.rs");
    let count = rt
        .block_on(reindex_files(dir.path(), &[ghost], &embedder))
        .unwrap();
    assert_eq!(count, 0);
}
```

> **Note on `test_embedder()`:** Look for how other async tests in this file create an `Embedder`. If a `test_embedder()` helper does not exist, use the same pattern as the nearest integration test that calls `build_index` or `embed_one`. A minimal stub that returns fixed-size zero-vectors is fine.

- [x] **Step 2: Run tests to verify they fail**

```bash
cargo test -p codescout reindex_files -- --nocapture 2>&1
```

Expected: compile error — `reindex_files` does not exist yet.

- [x] **Step 3: Implement `reindex_files`**

Add the function to `src/embed/index.rs` (near `build_index`, around line 1958):

```rust
/// Re-embed a specific set of files that were modified by write tools.
///
/// Hash-gated: files whose content matches the stored hash are skipped.
/// Called by `SemanticSearch` before executing a query to drain the dirty set.
pub async fn reindex_files(
    project_root: &Path,
    abs_paths: &[std::path::PathBuf],
    embedder: &std::sync::Arc<dyn codescout_embed::Embedder>,
) -> anyhow::Result<usize> {
    use crate::config::ProjectConfig;
    use crate::embed::schema::CodeChunk;

    if abs_paths.is_empty() {
        return Ok(0);
    }

    let config = ProjectConfig::load_or_default(project_root)?;
    let conn = open_db(project_root)?;
    let discovered = crate::workspace::discover_projects(project_root, 3, &[]);
    let mut count = 0;

    for abs_path in abs_paths {
        // Strip to relative path; skip if outside project root
        let rel_path = match abs_path.strip_prefix(project_root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let rel = rel_path.to_string_lossy().replace('\\', "/");

        // Hash gate: skip if content unchanged since last index
        match is_file_changed_mtime_hash(&conn, project_root, &rel) {
            Ok(false) => continue,
            Ok(true) => {}
            Err(_) => continue,
        }

        let Some(lang) = crate::ast::detect_language(abs_path) else {
            continue;
        };
        let source = match std::fs::read_to_string(abs_path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let hash = match hash_file(abs_path) {
            Ok(h) => h,
            Err(_) => continue,
        };
        let mtime = file_mtime(abs_path).unwrap_or(0);

        let chunks = super::ast_chunker::split_file(
            &source,
            lang,
            rel_path,
            config.embeddings.effective_chunk_size(),
        );
        if chunks.is_empty() {
            continue;
        }

        // Build embed texts: metadata header + content (mirrors embed_producer)
        let embed_texts: Vec<String> = chunks
            .iter()
            .map(|c| match &c.metadata {
                Some(m) => format!("{m}\n{}", c.content),
                None => c.content.clone(),
            })
            .collect();
        let text_refs: Vec<&str> = embed_texts.iter().map(|s| s.as_str()).collect();
        let vectors = embedder.embed(&text_refs).await?;

        // Resolve project_id for this file
        let project_id = discovered
            .iter()
            .find(|p| abs_path.starts_with(project_root.join(&p.relative_root)))
            .map(|p| p.id.clone())
            .unwrap_or_else(|| "project".to_string());

        // Atomic DB write: delete stale chunks, insert fresh, update hash
        conn.execute_batch("BEGIN")?;
        delete_file_chunks(&conn, &rel)?;
        for (chunk, vector) in chunks.iter().zip(vectors.iter()) {
            let code_chunk = CodeChunk {
                id: None,
                file_path: rel.clone(),
                language: lang.to_string(),
                content: chunk.content.clone(),
                start_line: chunk.start_line,
                end_line: chunk.end_line,
                file_hash: hash.clone(),
                source: "project".to_string(),
                project_id: project_id.clone(),
                metadata: chunk.metadata.clone(),
            };
            insert_chunk(&conn, &code_chunk, vector)?;
        }
        upsert_file_hash(&conn, &rel, &hash, Some(mtime))?;
        conn.execute_batch("COMMIT")?;
        count += 1;
    }

    Ok(count)
}
```

- [x] **Step 4: Run tests to verify they pass**

```bash
cargo test -p codescout reindex_files -- --nocapture 2>&1
```

Expected: all 3 PASS.

- [x] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -- -D warnings
git add src/embed/index.rs
git commit -m "feat(embed): add reindex_files for lazy per-file re-embedding"
```

---

### Task 3: Pre-search drain in `SemanticSearch`

**Files:**
- Modify: `src/tools/semantic.rs` (lines 280–310, the `dirty_file_count` warn block)

Replace the existing warn-only block with a drain + reindex step. The embedder is already created earlier in the function (`let embedder = ctx.agent.get_or_create_embedder(&model).await?;`) — reuse it.

- [x] **Step 1: Locate the existing warn block**

Open `src/tools/semantic.rs`. Find the block starting around line 283:

```rust
// Warn if write tools have modified files in this session that haven't been re-indexed.
let dirty = ctx.agent.dirty_file_count().await;
if dirty > 0 {
    result["unindexed_writes"] = json!({
        "status": "warn",
        "file_count": dirty,
        "note": format!(
            "{} file{} modified in this session but not yet re-indexed — \
             run index(action='build') to include recent changes in semantic search.",
            dirty,
            if dirty == 1 { " was" } else { "s were" }
        )
    });
}
```

Confirm the lines visually — the exact text above is what will be replaced.

- [x] **Step 2: Identify where to insert the pre-search drain**

Find the line:

```rust
let query_embedding = codescout_embed::embed_one(embedder.as_ref(), query).await?;
```

The drain must happen AFTER this line (embedder is ready) and BEFORE the `tokio::task::spawn_blocking` block that runs the knn query. Insert the drain block between those two.

- [x] **Step 3: Replace warn block and add pre-search drain**

**Delete** the entire warn block found in Step 1 (the `dirty_file_count` + `unindexed_writes` section).

**Add** the pre-search drain immediately after the `query_embedding` line:

```rust
// Drain files written by tools in this session and re-embed them before searching.
let dirty_paths = ctx.agent.drain_dirty_files().await;
if !dirty_paths.is_empty() {
    match crate::embed::index::reindex_files(&root, &dirty_paths, &embedder).await {
        Ok(n) => {
            if n > 0 {
                tracing::info!("auto-reindexed {} file(s) before semantic_search", n);
            }
        }
        Err(e) => {
            tracing::warn!("auto-reindex failed, search proceeds on stale data: {e}");
            // Re-insert so next search can retry
            for path in dirty_paths {
                ctx.agent.mark_file_dirty(path).await;
            }
        }
    }
}
```

- [x] **Step 4: Run all tests**

```bash
cargo test 2>&1
```

Expected: all tests pass, no regressions.

- [x] **Step 5: Run clippy**

```bash
cargo clippy -- -D warnings 2>&1
```

Expected: no warnings.

- [x] **Step 6: Commit**

```bash
cargo fmt
git add src/tools/semantic.rs
git commit -m "feat(semantic): auto-reindex dirty files before search"
```

---

### Task 4: Build and verify end-to-end

- [x] **Step 1: Build release binary**

```bash
cargo build --release 2>&1
```

Expected: compiles clean.

- [x] **Step 2: Restart MCP server**

In Claude Code: `/mcp` → restart the `codescout` server.

- [x] **Step 3: Manual verification**

1. Edit a source file via `edit_code` or `edit_file`.
2. Immediately call `semantic_search` with a query relevant to the edited code.
3. Confirm: result reflects the new content, no `unindexed_writes` warning in the response.

- [x] **Step 4: Final commit if any fixups needed**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add -p && git commit -m "fix: <description of fixup>"
```
