---
id: '2ad5c17769ff957b'
kind: plan
status: draft
title: Worktree semantic search — implementation plan
tags:
- worktree
- retrieval
- semantic-search
- plan
topic: worktree semantic search
---

# Worktree Semantic Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `semantic_search` work correctly inside a linked git worktree by reusing the main checkout's vectors for byte-identical files and indexing only the worktree's changed files.

**Architecture:** Query composition *above* the `CodeVectorStore` trait. A worktree query issues two `query()` calls — main's `project_id` with the worktree's dirty paths excluded, plus a per-worktree delta `project_id` holding only changed files — merged by score. The dirty set is derived by content hash, never by a git diff, so there is no base commit to choose and no staleness window.

**Tech Stack:** Rust, `async_trait`, `qdrant-client` 1.17, `rusqlite` + `sqlite-vec`, `serde`, `tempfile`.

**Spec:** `docs/superpowers/specs/2026-08-13-worktree-semantic-search-design.md` (artifact `cf3dee40ca22cbf2`). Read it before Task 1; it carries the rejected alternatives and the reasoning this plan does not repeat.

## Global Constraints

Every task's requirements implicitly include this section.

- **Branch is `experiments`.** `master` is protected. **Run `git branch --show-current` immediately before every commit** — this repo has concurrent sessions that switch the checkout's branch between turns, and it happened twice while this plan was being written.
- **Gate before every commit:** `cargo fmt` → `cargo clippy --all-targets -- -D warnings` → `cargo test`. Every task here touches retrieval, so **also** run `cargo clippy --features server-stack --all-targets -- -D warnings` and `cargo test --features server-stack`. `server-stack` is the configuration `cargo rb` ships and it has its own CI lane.
- **Mutation-verify every new test.** Reintroduce the defect, watch that *specific* test fail, revert. Then note which other tests stayed green — that set is what the suite was blind to. A test never seen to fail has an unknown failure mode.
- **Worktree fixtures are built by the test, never assumed.** Copy the on-disk fixture pattern from `src/prompts/mod.rs:774-796`, which writes `<tmp>/main/.git/worktrees/feat/HEAD` and `<tmp>/wt/.git`. No test may depend on a worktree existing on the host machine.
- **The payload/column/field name for a path is `file_path`** — Qdrant payload key, sqlite column, and `crate::retrieval::search::Hit.file_path`. Not `path`, not `rel_path`.
- **Do not generalize the exclusion lists into a filter struct.** Two axes stay two params; the spec's revisit-when is a *third* axis. `CodeVectorStore::query` already carries `#[allow(clippy::too_many_arguments)]`.
- **Error style:** these are state conditions, not malformed input — report them in the response payload, never `RecoverableError`. See `get_guide("error-handling")`.

## File Structure

| File | Responsibility | Task |
|---|---|---|
| `src/retrieval/drift.rs` | `ChunkRef`, `DriftAction`, `diff_chunks`, and the new `dirty_paths` — all pure set logic, no I/O | 1, 2 |
| `src/retrieval/qdrant.rs` | Qdrant impl: `must_not` on `file_path`; `file_path` in the scroll field list | 1, 3 |
| `src/retrieval/sqlite_code_store.rs` | lite impl: post-filter on `file_path`, `k` widening, `file_path` in `SELECT` | 1, 3 |
| `src/retrieval/code_store.rs` | trait signature + `InMemoryCodeStore` double + contract tests | 1, 3 |
| `src/retrieval/search.rs` | `SearchOpts.exclude_paths`; the two-source merge | 3, 6 |
| `src/retrieval/index_state.rs` | `IndexState.dirty_paths`, schema bump, back-compat | 4 |
| `src/retrieval/sync.rs` | `delta_project_id`; worktree sync mode | 5, 6 |
| `src/tools/semantic/semantic_search.rs` | worktree query path, hint, drift note | 7 |
| `../claude-plugins/codescout-companion/hooks/` | delete the false instruction; run `index` on worktree entry | 8 |

---

### Task 1: `ChunkRef` carries `file_path`

Needed only for the deletion half of the dirty set: a path present in main's index and **absent** from the worktree is never visited by the walk, so without main's path list main keeps serving a file you deleted. Parsing `chunk_id` is not an option — `src/retrieval/sqlite_code_store.rs:538-541` documents a real regression from that, because a `project_id` can itself contain colons (`lib:foo`).

**Files:**
- Modify: `src/retrieval/drift.rs:4-7`
- Modify: `src/retrieval/qdrant.rs:155` (`QdrantWrap::scroll_chunk_refs`) — and its `PayloadIncludeSelector` field list a few lines above
- Modify: `src/retrieval/sqlite_code_store.rs:148-160`
- Modify: `src/retrieval/code_store.rs:314` (`InMemoryCodeStore::chunk_refs`)
- Modify: `src/retrieval/sync.rs:465` (`RecordingStore::upsert_chunks`)
- Test: `src/retrieval/code_store.rs` (contract tests module)

**Interfaces:**
- Produces: `ChunkRef { chunk_id: String, content_hash: String, file_path: String }`

- [ ] **Step 1: Write the failing contract test**

Add to the contract-test module in `src/retrieval/code_store.rs`, beside `contract_delete_and_stats_and_refs`:

```rust
#[tokio::test]
async fn contract_chunk_refs_carry_file_path() {
    // The dirty-set derivation needs main's PATH list to notice files deleted in a
    // worktree. chunk_id cannot be parsed for it (project_id may contain colons —
    // see sqlite_code_store.rs:538), so ChunkRef must carry file_path directly.
    let store = InMemoryCodeStore::default();
    let payload = test_payload("proj", "src/a.rs", "fn a() {}");
    store
        .upsert_chunks("code_chunks", &[(payload.clone(), test_embedding())])
        .await
        .unwrap();

    let refs = store.chunk_refs("code_chunks", "proj").await.unwrap();
    assert_eq!(refs.len(), 1);
    assert_eq!(
        refs[0].file_path, "src/a.rs",
        "chunk_refs must expose file_path, not require parsing chunk_id"
    );
}
```

If `test_payload` / `test_embedding` helpers do not already exist in that module, use whatever the neighbouring `contract_*` tests use to build a `(CodePayload, EmbedOutput)` pair — read `contract_delete_and_stats_and_refs` and copy its construction verbatim rather than inventing helpers.

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test --features server-stack contract_chunk_refs_carry_file_path`
Expected: FAIL to compile — `no field 'file_path' on type 'ChunkRef'`.

- [ ] **Step 3: Add the field and fill it at all four construction sites**

`src/retrieval/drift.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ChunkRef {
    pub chunk_id: String,
    pub content_hash: String,
    /// Forward-slashed project-relative path, as stored in the payload's
    /// `file_path`. Present so the dirty-set derivation can find paths that exist
    /// in an index but not on disk without parsing `chunk_id`.
    pub file_path: String,
}
```

`src/retrieval/sqlite_code_store.rs:148-160` — widen the SELECT and the row mapping:

```rust
        let mut stmt = conn
            .prepare("SELECT chunk_id, content_hash, file_path FROM code_chunk WHERE project_id = ?1")?;
```

and in the `query_map` closure add `file_path: row.get(2)?,` to the `ChunkRef { .. }` literal.

`src/retrieval/qdrant.rs` — add `"file_path"` to the `PayloadIncludeSelector` field list used by `scroll_chunk_refs`, then populate the field at `:155`, reading it the same way the surrounding code reads `content_hash` from the point payload.

`src/retrieval/code_store.rs:314` and `src/retrieval/sync.rs:465` — both build the ref from a `CodePayload` they already hold, so add:

```rust
                file_path: p.file_path.clone(),
```

- [ ] **Step 4: Run the gate**

Run: `cargo test --features server-stack contract_chunk_refs_carry_file_path`
Expected: PASS.
Then: `cargo fmt && cargo clippy --features server-stack --all-targets -- -D warnings && cargo test --features server-stack`
Expected: all green. The compiler finds every construction site, so a missed one is a build error rather than a silent empty string.

- [ ] **Step 5: Mutation-verify**

Change the sqlite SELECT back to two columns and set `file_path: String::new()`. Confirm `contract_chunk_refs_carry_file_path` fails and note which other tests stay green. Revert.

- [ ] **Step 6: Commit**

```bash
git branch --show-current   # must print: experiments
git add src/retrieval/drift.rs src/retrieval/qdrant.rs src/retrieval/sqlite_code_store.rs src/retrieval/code_store.rs src/retrieval/sync.rs
git commit -m "feat(retrieval): ChunkRef carries file_path

The worktree dirty-set derivation needs main's path list to notice a file that
exists in the index but not on disk. Parsing chunk_id for it is unsafe --
sqlite_code_store.rs:538 documents the regression, since a project_id can itself
contain colons. Both stores already hold file_path; this exposes it."
```

---

### Task 2: `dirty_paths` — the pure function that carries the risk

The only place a decision can be wrong, and the only part CI verifies on both backends (see Task 3's note on Qdrant coverage). No I/O, no backend, no git, no embedder.

**Files:**
- Modify: `src/retrieval/drift.rs` (add below `diff_chunks`)
- Test: `src/retrieval/drift.rs` (its `#[cfg(test)]` module)

**Interfaces:**
- Consumes: `ChunkRef` from Task 1.
- Produces:
  ```rust
  pub struct LocalChunk { pub file_path: String, pub content_hash: String }
  pub struct DirtySet { pub paths: BTreeSet<String>, pub to_embed: Vec<usize> }
  pub fn dirty_paths(main_refs: &[ChunkRef], local: &[LocalChunk]) -> DirtySet
  ```
  `to_embed` holds **indices into `local`** so the caller keeps ownership of the heavy chunk bodies.

- [ ] **Step 1: Write the failing tests**

Add to `src/retrieval/drift.rs`:

```rust
#[cfg(test)]
mod dirty_tests {
    use super::*;

    fn r(path: &str, hash: &str) -> ChunkRef {
        ChunkRef {
            chunk_id: format!("main:{path}:{hash}"),
            content_hash: hash.into(),
            file_path: path.into(),
        }
    }
    fn l(path: &str, hash: &str) -> LocalChunk {
        LocalChunk { file_path: path.into(), content_hash: hash.into() }
    }

    #[test]
    fn unchanged_file_is_clean() {
        let d = dirty_paths(&[r("src/a.rs", "h1")], &[l("src/a.rs", "h1")]);
        assert!(d.paths.is_empty(), "byte-identical content must reuse main's vector");
        assert!(d.to_embed.is_empty());
    }

    #[test]
    fn modified_file_is_dirty_and_queued() {
        let d = dirty_paths(&[r("src/a.rs", "h1")], &[l("src/a.rs", "h2")]);
        assert!(d.paths.contains("src/a.rs"));
        assert_eq!(d.to_embed, vec![0], "changed content must be embedded into the delta");
    }

    #[test]
    fn file_absent_from_main_is_dirty_and_queued() {
        let d = dirty_paths(&[], &[l("src/new.rs", "h1")]);
        assert!(d.paths.contains("src/new.rs"));
        assert_eq!(d.to_embed, vec![0]);
    }

    #[test]
    fn file_in_main_but_absent_from_worktree_is_dirty_and_queues_nothing() {
        // The deletion case. Without this branch main keeps serving a file the
        // worktree deleted -- the exact confidently-stale outcome the design exists
        // to prevent, arriving through the back door.
        let d = dirty_paths(&[r("src/gone.rs", "h1")], &[]);
        assert!(
            d.paths.contains("src/gone.rs"),
            "a path in main but not on disk must be excluded from main's results"
        );
        assert!(d.to_embed.is_empty(), "there is nothing to embed for a deleted file");
    }

    #[test]
    fn one_changed_chunk_dirties_the_whole_file() {
        // A file is served by exactly one source. If any chunk differs, the delta
        // owns the file, so every chunk of it must be embedded.
        let main = [r("src/a.rs", "h1"), r("src/a.rs", "h2")];
        let local = [l("src/a.rs", "h1"), l("src/a.rs", "hX")];
        let d = dirty_paths(&main, &local);
        assert!(d.paths.contains("src/a.rs"));
        assert_eq!(d.to_embed, vec![0, 1], "a partially-changed file must be embedded whole");
    }
}
```

- [ ] **Step 2: Run them and confirm they fail**

Run: `cargo test --features server-stack dirty_tests`
Expected: FAIL to compile — `cannot find function 'dirty_paths'`.

- [ ] **Step 3: Implement**

```rust
/// One chunk as it exists on disk right now.
#[derive(Debug, Clone)]
pub struct LocalChunk {
    pub file_path: String,
    pub content_hash: String,
}

/// Which paths a worktree must not inherit from the main index, and which local
/// chunks belong in the worktree's delta.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirtySet {
    /// Paths to pass as `exclude_paths` when querying main's `project_id`.
    pub paths: std::collections::BTreeSet<String>,
    /// Indices into the `local` slice: the chunks to embed under the delta id.
    pub to_embed: Vec<usize>,
}

/// Derive the worktree's dirty set by content, not by git.
///
/// A path is dirty when any of its chunks differs from the main index, when it is
/// absent from main entirely, or when main holds it and disk does not. A file is
/// served by exactly one source — main or the delta — never both and never
/// neither, so a partially-changed file is embedded whole.
///
/// `main_refs` must come from `chunk_refs(collection, main_project_id)`; comparison
/// is on `(file_path, content_hash)` so it needs no base commit and inherits no
/// staleness window.
pub fn dirty_paths(main_refs: &[ChunkRef], local: &[LocalChunk]) -> DirtySet {
    use std::collections::{BTreeSet, HashSet};

    let main_pairs: HashSet<(&str, &str)> = main_refs
        .iter()
        .map(|r| (r.file_path.as_str(), r.content_hash.as_str()))
        .collect();
    let local_paths: HashSet<&str> = local.iter().map(|c| c.file_path.as_str()).collect();

    let mut paths: BTreeSet<String> = BTreeSet::new();

    // Any local chunk whose exact bytes are not in main dirties its file.
    for c in local {
        if !main_pairs.contains(&(c.file_path.as_str(), c.content_hash.as_str())) {
            paths.insert(c.file_path.clone());
        }
    }
    // A path main holds but disk does not: exclude it, embed nothing.
    for r in main_refs {
        if !local_paths.contains(r.file_path.as_str()) {
            paths.insert(r.file_path.clone());
        }
    }
    // Every chunk of a dirty file goes to the delta, so the delta owns it whole.
    let to_embed = local
        .iter()
        .enumerate()
        .filter(|(_, c)| paths.contains(&c.file_path))
        .map(|(i, _)| i)
        .collect();

    DirtySet { paths, to_embed }
}
```

- [ ] **Step 4: Run and confirm they pass**

Run: `cargo test --features server-stack dirty_tests`
Expected: 5 passed.

- [ ] **Step 5: Mutation-verify each branch**

Delete the second `for` loop → `file_in_main_but_absent_from_worktree_...` must fail. Restore. Change `to_embed`'s filter to `!paths.contains(...)` → `modified_file_is_dirty_and_queued` must fail. Restore. Invert the first loop's `!` → `unchanged_file_is_clean` must fail. Restore. Record which other tests stayed green for each.

- [ ] **Step 6: Commit**

```bash
git branch --show-current   # must print: experiments
git add src/retrieval/drift.rs
git commit -m "feat(retrieval): derive a worktree's dirty set by content hash

dirty_paths compares (file_path, content_hash) against the main index, so a
worktree needs no base commit and inherits no staleness window: a main chunk is
reusable only when those exact bytes sit at that path on disk.

Covers the deletion case explicitly -- a path main holds and disk does not is
dirty and queues nothing. Without that branch main keeps serving a file the
worktree deleted, which is the failure this design exists to prevent."
```

---

### Task 3: `exclude_paths` through the query surface

The sibling of `exclude_languages`, which is already this shape in both stores with the backend divergence documented in-code at `src/retrieval/sqlite_code_store.rs:278-286`. Follow it exactly rather than inventing a mechanism.

**Files:**
- Modify: `src/retrieval/search.rs:32-48` (`SearchOpts`), and its two constructors around `:56` / `:67`, and the `query` call at `:109`
- Modify: `src/retrieval/code_store.rs:51-62` (trait `query`), `:183-205` (Qdrant forwarder), `:347-380` (`InMemoryCodeStore::query`)
- Modify: `src/retrieval/qdrant.rs:305-390` (`hybrid_query`)
- Modify: `src/retrieval/sqlite_code_store.rs:255-325` (`query`)
- Test: `src/retrieval/code_store.rs` contract module; `src/retrieval/sqlite_code_store.rs` tests

**Interfaces:**
- Produces: `query(..., exclude_languages: &[String], exclude_paths: &[String])` on `CodeVectorStore`; `SearchOpts.exclude_paths: Vec<String>`.

- [ ] **Step 1: Write the failing contract test**

In `src/retrieval/code_store.rs`, beside `contract_query_excludes_languages_and_scopes_project`:

```rust
#[tokio::test]
async fn contract_query_excludes_paths() {
    // The worktree design serves main's vectors for every path EXCEPT the ones the
    // worktree changed. That exclusion is a store-level contract, so both backends
    // must honour it.
    let store = InMemoryCodeStore::default();
    for path in ["src/keep.rs", "src/drop.rs"] {
        store
            .upsert_chunks("code_chunks", &[(test_payload("proj", path, "fn f() {}"), test_embedding())])
            .await
            .unwrap();
    }

    let hits = store
        .query("code_chunks", "proj", &dense_probe(), &SparseVector::default(),
               10, 1.0, true, &[], &["src/drop.rs".to_string()])
        .await
        .unwrap();

    assert!(
        hits.iter().all(|h| h.file_path != "src/drop.rs"),
        "excluded path must not appear in results"
    );
    assert!(
        hits.iter().any(|h| h.file_path == "src/keep.rs"),
        "exclusion must not empty the result set — the accepting case needs pinning too"
    );
}
```

Both assertions are required. A guard asserted only in the negative direction passes whether or not it discriminates.

Use the same helper names the neighbouring contract test uses for the payload, embedding and dense probe — read it and copy, do not invent.

- [ ] **Step 2: Run it and confirm it fails**

Run: `cargo test --features server-stack contract_query_excludes_paths`
Expected: FAIL to compile — `query` takes 9 arguments, 10 supplied.

- [ ] **Step 3: Thread the parameter**

Trait, `src/retrieval/code_store.rs`, extending the existing doc comment:

```rust
    /// Query: hybrid dense+sparse RRF, or pure-dense ANN when `disable_sparse`.
    /// `exclude_languages` drops hits whose payload `language` is in the list.
    /// `exclude_paths` drops hits whose payload `file_path` is in the list. Used by
    /// worktree search to suppress main's chunks for files the worktree changed;
    /// the worktree's delta project supplies those paths instead.
    #[allow(clippy::too_many_arguments)]
    async fn query(
        &self,
        collection: &str,
        project_id: &str,
        dense: &[f32],
        sparse: &SparseVector,
        limit: usize,
        bm25_boost: f32,
        disable_sparse: bool,
        exclude_languages: &[String],
        exclude_paths: &[String],
    ) -> Result<Vec<Hit>>;
```

Qdrant, `src/retrieval/qdrant.rs` `hybrid_query` — add the parameter and extend the `must_not` it already builds:

```rust
        let must = vec![Condition::matches("project_id", project_id.to_string())];
        let mut must_not: Vec<Condition> = exclude_languages
            .iter()
            .map(|l| Condition::matches("language", l.clone()))
            .collect();
        must_not.extend(
            exclude_paths
                .iter()
                .map(|p| Condition::matches("file_path", p.clone())),
        );
```

Nothing else in `hybrid_query` changes — `filter` is already cloned into both prefetch legs and the dense-only branch.

sqlite, `src/retrieval/sqlite_code_store.rs` — widen the `k` condition and add one post-filter clause:

```rust
        let k = if exclude_languages.is_empty() && exclude_paths.is_empty() {
            limit
        } else {
            limit.saturating_mul(4)
        };
```

and at the tail:

```rust
        Ok(rows
            .into_iter()
            .filter(|(_, hit, lang)| {
                !exclude_languages.contains(lang) && !exclude_paths.contains(&hit.file_path)
            })
            .map(|(_, hit, _)| hit)
            .collect())
```

Also extend the comment above `k` to say the widening now covers both exclusion lists.

`InMemoryCodeStore::query` (`code_store.rs:347-380`) already post-filters on `exclude_languages`; add `&& !exclude_paths.contains(&p.file_path)` to the same closure.

`SearchOpts` (`src/retrieval/search.rs`) gains the field, mirroring `exclude_languages`:

```rust
    /// Payload `file_path` values to exclude (Qdrant `must_not`; post-filtered in
    /// the lite store). Set by worktree search to paths the worktree changed;
    /// empty = no filter.
    pub exclude_paths: Vec<String>,
```

Initialise it to `Vec::new()` in both constructors, and pass `&opts.exclude_paths` at the `query` call site around `:109`.

- [ ] **Step 4: Run the gate**

Run: `cargo test --features server-stack contract_query_excludes_paths`
Expected: PASS.
Then the full gate, both feature sets.

- [ ] **Step 5: Add the lite-backend real test, and label the Qdrant gap**

Extend `real_vec0_refs_stats_delete_and_language_filter` (`src/retrieval/sqlite_code_store.rs:482`) — or add `real_vec0_path_filter` beside it — to assert both directions of the path exclusion against the real vec0 table. This one **does** run in CI.

Then extend the `#[ignore]`d Qdrant test at `src/retrieval/qdrant.rs:422` with the same assertions and run it manually:

```bash
cargo test --features server-stack -- --ignored qdrant_
```

Record in the commit body that the Qdrant half is manually verified. `qdrant.rs:422` is `#[ignore]`d, so CI never runs it — the backend most users of this feature run has no automatic coverage. Do not describe the two halves as equally covered.

- [ ] **Step 6: Mutation-verify**

Drop the `must_not.extend(...)` in Qdrant → the manual Qdrant test must fail. Drop the `exclude_paths.contains` clause in sqlite → `real_vec0_path_filter` must fail. Drop it in `InMemoryCodeStore` → `contract_query_excludes_paths` must fail. Restore each.

- [ ] **Step 7: Commit**

```bash
git branch --show-current   # must print: experiments
git add src/retrieval/search.rs src/retrieval/code_store.rs src/retrieval/qdrant.rs src/retrieval/sqlite_code_store.rs
git commit -m "feat(retrieval): exclude_paths on the query surface

The sibling of exclude_languages, which is already this shape in both stores with
the divergence documented in sqlite_code_store.rs:278-286: Qdrant applies a native
must_not, the lite store post-filters in Rust and widens k for headroom. Same
mechanism, one more field -- so worktree search works on both backends rather than
Qdrant only.

Not generalised into a filter struct: two exclusion axes stay two params. A third
earns the extraction.

Coverage is asymmetric and the asymmetry is not in our favour. sqlite's real_vec0
tests run in CI; qdrant.rs:422 is #[ignore]d, so the Qdrant must_not was verified
by hand (cargo test --features server-stack -- --ignored qdrant_) and by nothing
automatic."
```

---

### Task 4: `IndexState` records the dirty paths

**Files:**
- Modify: `src/retrieval/index_state.rs:27-34` (`IndexState`), `:55-64` (`write_index_state`), and `INDEX_STATE_SCHEMA_VERSION`
- Test: `src/retrieval/index_state.rs` tests module

**Interfaces:**
- Produces: `IndexState { last_indexed_commit, last_indexed_at, schema_version, dirty_paths: Vec<String> }` and `write_index_state_with_dirty(root: &Path, dirty: &[String]) -> std::io::Result<()>`.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn dirty_paths_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        write_index_state_with_dirty(root, &["src/a.rs".to_string()]).unwrap();
        let st = read_index_state(root).expect("sidecar should exist");
        assert_eq!(st.dirty_paths, vec!["src/a.rs".to_string()]);
    }

    #[test]
    fn sidecar_written_before_dirty_paths_existed_still_parses() {
        // Back-compat: an existing .codescout/index-state.json has no dirty_paths
        // key. It must read as an empty list, not fail the whole parse and silently
        // make every project look unindexed.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        std::fs::write(
            root.join(".codescout").join("index-state.json"),
            r#"{"last_indexed_commit":"abc","last_indexed_at":"2026-08-01T00:00:00Z","schema_version":1}"#,
        )
        .unwrap();
        let st = read_index_state(root).expect("old sidecar must still parse");
        assert!(st.dirty_paths.is_empty());
        assert_eq!(st.last_indexed_commit, "abc");
    }
```

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test --features server-stack index_state`
Expected: FAIL to compile — no `dirty_paths` field, no `write_index_state_with_dirty`.

- [ ] **Step 3: Implement**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexState {
    pub last_indexed_commit: String,
    pub last_indexed_at: String,
    pub schema_version: u32,
    /// Project-relative paths this checkout must NOT inherit from the main index.
    /// Non-empty only for a worktree delta sync. `serde(default)` so sidecars
    /// written before this field existed keep parsing — a hard parse failure would
    /// read as "never indexed" for every existing project.
    #[serde(default)]
    pub dirty_paths: Vec<String>,
}
```

Bump `INDEX_STATE_SCHEMA_VERSION` by one. Then refactor `write_index_state` to delegate, so the existing three call sites keep their signature:

```rust
pub fn write_index_state(root: &Path) -> std::io::Result<()> {
    write_index_state_with_dirty(root, &[])
}

/// As [`write_index_state`], additionally recording the paths a worktree must not
/// inherit from the main index. See the worktree sync mode in `sync.rs`.
pub fn write_index_state_with_dirty(root: &Path, dirty: &[String]) -> std::io::Result<()> {
    let state = IndexState {
        last_indexed_commit: head_commit_full(root).unwrap_or_default(),
        last_indexed_at: chrono::Utc::now().to_rfc3339(),
        schema_version: INDEX_STATE_SCHEMA_VERSION,
        dirty_paths: dirty.to_vec(),
    };
    std::fs::create_dir_all(root.join(".codescout"))?;
    let body = serde_json::to_string_pretty(&state).map_err(std::io::Error::other)?;
    std::fs::write(state_path(root), body)
}
```

- [ ] **Step 4: Run and confirm pass**

Run: `cargo test --features server-stack index_state`
Expected: all pass, including the pre-existing tests at `:151`, `:168`, `:180`, `:190`.

- [ ] **Step 5: Mutation-verify**

Remove `#[serde(default)]` → `sidecar_written_before_dirty_paths_existed_still_parses` must fail. Restore.

- [ ] **Step 6: Commit**

```bash
git branch --show-current   # must print: experiments
git add src/retrieval/index_state.rs
git commit -m "feat(retrieval): index-state records a worktree's dirty paths

The list is written once by a worktree sync and read per query, which is exactly
how main's index already works -- your index is as fresh as your last sync, with no
second mental model and no per-query walk.

serde(default) is load-bearing: without it every sidecar written before this field
fails to parse, and a failed parse reads as 'never indexed' for the whole project."
```

---

### Task 5: `delta_project_id`, and proof it does not collide

**Files:**
- Modify: `src/retrieval/sync.rs` (beside `chunk_id` at `:77`)
- Test: `src/retrieval/sync.rs` tests module

**Interfaces:**
- Produces: `pub fn delta_project_id(main_project_id: &str, worktree_dir: &str) -> String`

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn delta_project_id_is_distinct_and_separator_is_not_a_colon() {
        // chunk_id joins on ':' (chunk_id() below), and sqlite_code_store.rs:538
        // documents a real regression from colon-bearing project ids. '@' keeps the
        // delta id unambiguous under that join.
        let id = delta_project_id("codescout", "peer-delegation");
        assert_eq!(id, "codescout@peer-delegation");
        assert!(!id.contains(':'), "delta id must not introduce a colon");
        assert_ne!(id, "codescout", "delta must not alias the main project");
    }

    #[test]
    fn delta_db_file_is_distinct_from_mains() {
        // The lite store maps project_id to a FILENAME (sqlite_code_store.rs:70 ->
        // sqlite_vec_ext::sanitize_db_name), and that map is not injective: every
        // non-alphanumeric char collapses to '_'. Pin that the delta still lands in
        // its own file, because sharing main's file would silently merge the two.
        use crate::sqlite_vec_ext::sanitize_db_name;
        let main = sanitize_db_name("codescout");
        let delta = sanitize_db_name(&delta_project_id("codescout", "peer-delegation"));
        assert_ne!(main, delta);
        assert_eq!(delta, "codescout_peer-delegation");
    }
```

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test --features server-stack delta_project_id`
Expected: FAIL to compile — `cannot find function 'delta_project_id'`.

- [ ] **Step 3: Implement**

```rust
/// Project id for a worktree's delta index: the changed files only.
///
/// `@` rather than `:` deliberately — [`chunk_id`] joins on `:` and
/// `sqlite_code_store.rs:538` documents a regression from colon-bearing project
/// ids. Note the lite store maps this to a filename via `sanitize_db_name`, which
/// is not injective, so the pair is pinned by test rather than assumed distinct.
pub fn delta_project_id(main_project_id: &str, worktree_dir: &str) -> String {
    format!("{main_project_id}@{worktree_dir}")
}
```

- [ ] **Step 4: Run and confirm pass**

Run: `cargo test --features server-stack delta_project_id`
Expected: 2 passed.

- [ ] **Step 5: Commit**

```bash
git branch --show-current   # must print: experiments
git add src/retrieval/sync.rs
git commit -m "feat(retrieval): delta project id for a worktree

'@' not ':' -- chunk_id joins on ':' and sqlite_code_store.rs:538 documents a real
regression from colon-bearing project ids.

The lite store turns a project id into a filename through a sanitizer that is not
injective (every non-alphanumeric collapses to '_'), so the delta's distinctness
from main's DB is pinned by test rather than assumed."
```

---

### Task 6: worktree sync mode — `index` builds the delta

`index` is the **only** thing that builds a delta. `semantic_search` never writes: that would put an ungated side effect on the hottest read path and surface embedder failures under the wrong verb.

**Files:**
- Modify: `src/retrieval/sync.rs` (the `stream_index` / `sync_project` path that consumes `chunk_refs`)
- Modify: `src/tools/semantic/index.rs:319` region (the `record_index_state: true` call site)
- Test: `src/retrieval/sync.rs` tests module

**Interfaces:**
- Consumes: `dirty_paths`/`LocalChunk`/`DirtySet` (Task 2), `delta_project_id` (Task 5), `write_index_state_with_dirty` (Task 4), `ChunkRef.file_path` (Task 1), `detect_worktree_info` → `WorktreeInfo { branch, main_repo }` (`src/prompts/mod.rs:183-237`).
- Produces: a worktree branch in the sync path that embeds only `DirtySet.to_embed` under `delta_project_id(...)` and records `DirtySet.paths`.

- [ ] **Step 1: Write the failing test using the existing `RecordingStore`**

`src/retrieval/sync.rs` already has a `RecordingStore` double (`:465`) and an incremental test (`stream_index_incremental_skips_unchanged_and_prunes_stale`, `:662`). Add beside them:

```rust
    #[tokio::test]
    async fn worktree_sync_embeds_only_dirty_files_and_records_them() {
        // Fixture is BUILT, never assumed: a worktree that exists only on the
        // developer's machine is not a test (see docs/trackers F-32). Pattern copied
        // from src/prompts/mod.rs:774-796.
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        let wt = tmp.path().join("wt");
        let meta = main.join(".git").join("worktrees").join("feat");
        std::fs::create_dir_all(&meta).unwrap();
        std::fs::write(meta.join("HEAD"), "ref: refs/heads/feat\n").unwrap();
        std::fs::create_dir_all(wt.join("src")).unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", meta.display())).unwrap();

        // main's index already holds src/same.rs with the SAME bytes the worktree has,
        // and src/gone.rs which the worktree does not have at all.
        std::fs::write(wt.join("src").join("same.rs"), "fn same() {}\n").unwrap();
        std::fs::write(wt.join("src").join("changed.rs"), "fn changed_v2() {}\n").unwrap();

        let store = RecordingStore::seeded_for_main("codescout", &[
            ("src/same.rs", "fn same() {}\n"),
            ("src/gone.rs", "fn gone() {}\n"),
            ("src/changed.rs", "fn changed_v1() {}\n"),
        ]);

        sync_worktree(&store, &wt, "codescout", &FakeEmbedder::default()).await.unwrap();

        let upserted = store.upserted_project_ids();
        assert!(
            upserted.iter().all(|p| p == "codescout@wt"),
            "a worktree sync must never write under main's project_id, got {upserted:?}"
        );
        let files = store.upserted_file_paths();
        assert!(files.contains(&"src/changed.rs".to_string()));
        assert!(!files.contains(&"src/same.rs".to_string()), "identical bytes must reuse main's vector");

        let st = crate::retrieval::index_state::read_index_state(&wt).unwrap();
        let dirty: std::collections::BTreeSet<_> = st.dirty_paths.iter().cloned().collect();
        assert!(dirty.contains("src/changed.rs"));
        assert!(dirty.contains("src/gone.rs"), "a file main holds and the worktree lacks must be excluded");
        assert!(!dirty.contains("src/same.rs"));
    }
```

`RecordingStore::seeded_for_main`, `upserted_project_ids` and `upserted_file_paths` are new helpers on the existing double — add them in this step, reading `RecordingStore`'s current shape at `src/retrieval/sync.rs:440-480` and following it. Reuse the existing `FakeEmbedder` (it already records what it was asked to embed).

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test --features server-stack worktree_sync_embeds_only_dirty`
Expected: FAIL to compile — `cannot find function 'sync_worktree'`.

- [ ] **Step 3: Implement `sync_worktree`**

Add to `src/retrieval/sync.rs`. Keep it a thin composition — the decisions live in `dirty_paths`:

```rust
/// Sync a linked worktree: reuse main's vectors for byte-identical files, embed
/// only what differs under the worktree's delta project id, and record the paths
/// main must not be asked for.
///
/// Called only from the `index` tool path. `semantic_search` must never call this —
/// a read tool that writes has no intent gate and surfaces embedder failures under
/// the wrong operation.
pub async fn sync_worktree<S, E>(
    store: &S,
    worktree_root: &Path,
    main_project_id: &str,
    embedder: &E,
) -> Result<()>
where
    S: crate::retrieval::code_store::CodeVectorStore + ?Sized,
    E: crate::retrieval::embedder::Embedder + ?Sized,
{
    let collection = /* same collection the caller already resolved */;
    let main_refs = store.chunk_refs(collection, main_project_id).await?;

    // Walk + chunk + hash the worktree exactly as a normal sync does; collect the
    // LocalChunk view alongside the payloads so indices line up with `to_embed`.
    let (payloads, local): (Vec<CodePayload>, Vec<crate::retrieval::drift::LocalChunk>) = /* walk */;

    let dirty = crate::retrieval::drift::dirty_paths(&main_refs, &local);

    let wt_dir = worktree_root
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "worktree".to_string());
    let delta_id = delta_project_id(main_project_id, &wt_dir);

    // Re-key the dirty payloads to the delta project before embedding, so their
    // chunk_ids and payload project_id are the delta's, never main's.
    let to_embed: Vec<CodePayload> = dirty
        .to_embed
        .iter()
        .map(|&i| {
            let mut p = payloads[i].clone();
            p.project_id = delta_id.clone();
            p.chunk_id = chunk_id(&delta_id, Path::new(&p.file_path), &p.content_hash);
            p
        })
        .collect();

    /* embed + upsert `to_embed` under `delta_id`, then prune delta chunks whose
       ids are absent from `to_embed` using the existing diff_chunks/delete path */

    let dirty_vec: Vec<String> = dirty.paths.iter().cloned().collect();
    if let Err(e) = crate::retrieval::index_state::write_index_state_with_dirty(worktree_root, &dirty_vec) {
        tracing::warn!("worktree index-state write failed: {e}");
    }
    Ok(())
}
```

For the two `/* ... */` regions, reuse the existing machinery rather than writing new code: the walk/chunk/hash loop is `stream_index`'s (`src/retrieval/sync.rs:190-220` computes `content_hash` then `chunk_id`), and the embed/upsert/prune sequence is the same one `stream_index` runs via `flush_pending` and `diff_chunks`. Extract the shared body into a helper both call if duplication would otherwise exceed a few lines — do not fork the chunker or the batching.

- [ ] **Step 4: Wire the `index` tool**

In `src/tools/semantic/index.rs`, at the sync call around `:319`: if `detect_worktree_info(root)` returns `Some(info)` with `info.main_repo == Some(main)`, resolve main's project id from `main`'s project config (the same resolution `src/config/project.rs:512-542` performs, so a `project.toml` name is honoured and the basename is the fallback) and call `sync_worktree` instead of the normal project sync. Report the delta's chunk count and the dirty-path count in the tool's response so the operator can see what happened.

- [ ] **Step 5: Run the gate**

Run: `cargo test --features server-stack worktree_sync_embeds_only_dirty`
Expected: PASS. Then the full gate on both feature sets.

- [ ] **Step 6: Mutation-verify**

Remove the `p.project_id = delta_id.clone()` re-key → the `upserted.iter().all(|p| p == "codescout@wt")` assertion must fail, proving the test catches a worktree writing into main's index. Restore. Then drop the `write_index_state_with_dirty` call → the dirty-path assertions must fail. Restore.

- [ ] **Step 7: Commit**

```bash
git branch --show-current   # must print: experiments
git add src/retrieval/sync.rs src/tools/semantic/index.rs
git commit -m "feat(retrieval): worktree sync embeds only what differs from main

index in a worktree now reuses main's vectors for byte-identical files and embeds
only the delta, keyed under <main>@<worktree>. Cost is proportional to your diff,
not the corpus.

index is the ONLY thing that builds a delta. semantic_search stays a read: a search
that silently indexes has no intent gate, fires from the hottest call site, and
surfaces embedder failures under the wrong verb.

The regression test BUILDS its worktree fixture (pattern from prompts/mod.rs:774)
rather than assuming one exists -- a test that can only fail on a machine nobody
runs is not a guard."
```

---

### Task 7: the worktree query path

**Files:**
- Modify: `src/retrieval/search.rs` (the two-source merge)
- Modify: `src/tools/semantic/semantic_search.rs:200-270`
- Test: `src/retrieval/search.rs` tests; `src/tools/semantic/tests.rs` (or the existing tests module for that tool)

**Interfaces:**
- Consumes: `SearchOpts.exclude_paths` (Task 3), `IndexState.dirty_paths` (Task 4), `delta_project_id` (Task 5), `detect_worktree_info` (`src/prompts/mod.rs:204`).
- Produces: `pub fn merge_hits(a: Vec<Hit>, b: Vec<Hit>, limit: usize) -> Vec<Hit>`.

- [ ] **Step 1: Write the failing merge test**

```rust
    #[test]
    fn merge_at_limit_equals_the_true_global_top_k() {
        // Each source is queried at `limit`, and the merge is exact: a hit in the
        // global top-k is necessarily in its own source's top-k. So no over-fetch is
        // needed for the merge (the lite store's internal k-widening is separate).
        let a = vec![hit("a1", 0.90), hit("a2", 0.50)];
        let b = vec![hit("b1", 0.70), hit("b2", 0.60)];
        let merged = merge_hits(a, b, 3);
        let ids: Vec<&str> = merged.iter().map(|h| h.chunk_id.as_str()).collect();
        assert_eq!(ids, vec!["a1", "b1", "b2"]);
    }

    #[test]
    fn merge_returns_fewer_than_limit_when_sources_are_short() {
        let merged = merge_hits(vec![hit("a1", 0.9)], vec![], 5);
        assert_eq!(merged.len(), 1, "must not pad or panic when sources are short");
    }
```

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test --features server-stack merge_at_limit`
Expected: FAIL to compile — `cannot find function 'merge_hits'`.

- [ ] **Step 3: Implement the merge**

```rust
/// Merge two score-ordered hit lists and truncate to `limit`.
///
/// Exact for top-`limit` when each source was queried at `limit`: a hit in the
/// global top-`limit` is necessarily within its own source's top-`limit`. Scores
/// are cosine from the same model, so they are comparable across sources.
pub fn merge_hits(a: Vec<Hit>, b: Vec<Hit>, limit: usize) -> Vec<Hit> {
    let mut all: Vec<Hit> = a.into_iter().chain(b).collect();
    all.sort_by(|x, y| y.score.partial_cmp(&x.score).unwrap_or(std::cmp::Ordering::Equal));
    all.truncate(limit);
    all
}
```

- [ ] **Step 4: Wire `semantic_search`, with the hint and the drift note**

In `src/tools/semantic/semantic_search.rs`, after `project_id` is resolved (`:208-213`):

1. `detect_worktree_info(root)` → not a worktree: unchanged behaviour, stop here.
2. `read_index_state(worktree_root)`: `None`, or `dirty_paths` empty *and* the delta project has no chunks (`project_has_chunks(collection, &delta_id)`) → return an **empty result set carrying a hint**, and issue no queries:

```
No index for worktree project `<delta_id>`. A worktree's files differ from the main
checkout, so main's vectors are not served for changed files. Run index(action="build")
here to index them — it only embeds what differs. `symbols`, `grep` and `references`
are computed from the filesystem and are already correct in this worktree.
```

3. Otherwise run both queries and `merge_hits` them:
   - main: `opts.exclude_paths = state.dirty_paths.clone()`
   - delta: `opts.exclude_paths = Vec::new()`, `project_id = delta_id`
4. Then `read_index_state(main_repo)`. If its `last_indexed_at` parses later than the worktree state's, attach a note:

```
Note: the main checkout was re-indexed after this worktree's delta was built, so
results for unchanged files may reflect main's newer content. Re-run
index(action="build") here to refresh.
```

If main has no sidecar, attach nothing and claim nothing — undetectable drift is reported as silence, not as reassurance.

Note that `classify_search_error` (`:19-26`) cannot serve step 2: its "Qdrant collection is missing" branch is unreachable when the collection is global and present.

- [ ] **Step 5: Test the hint in both directions**

```rust
    #[test]
    fn worktree_hint_names_the_delta_project_and_both_exits() {
        let h = worktree_no_index_hint("codescout@wt");
        assert!(h.contains("codescout@wt"), "hint must name the resolved project id");
        assert!(h.contains("index(action=\"build\")"));
        assert!(h.contains("grep"), "hint must offer the tools that ARE correct here");
    }

    #[test]
    fn no_hint_when_the_delta_is_indexed() {
        // The negative direction. A hint that fires unconditionally passes the
        // positive test while telling the user nothing.
        assert!(worktree_hint_for(/* delta present */ true, "codescout@wt").is_none());
    }
```

Shape the helper so both directions are testable without a live store — a small `fn worktree_hint_for(delta_present: bool, delta_id: &str) -> Option<String>`.

- [ ] **Step 6: Mutation-verify**

Make the hint unconditional → `no_hint_when_the_delta_is_indexed` must fail. Flip the drift timestamp comparison → the drift test must fail. Pass `state.dirty_paths` to the delta query instead of main's → the merge would drop the delta's own results; assert that case explicitly if it is not already covered.

- [ ] **Step 7: Commit**

```bash
git branch --show-current   # must print: experiments
git add src/retrieval/search.rs src/tools/semantic/semantic_search.rs
git commit -m "feat(semantic_search): serve a worktree from main's vectors plus its delta

Two queries -- main with the worktree's dirty paths excluded, the delta without --
merged by score. Each source is queried at limit and the merge is exact, so no
over-fetch is needed.

The reported bug was silence, not absence: an unindexed worktree returned
{results: [], total: 0} with no hint, indistinguishable from a query that matched
nothing. It now names the resolved project id and both exits, including that
symbols/grep/references are already correct here.

Drift is reported, never assumed away: if main was re-indexed after the delta was
built, results carry a note. If main has no sidecar the drift is undetectable and
we say nothing rather than implying freshness we cannot establish."
```

---

### Task 8: retire the contradiction, and the docs

Two shipped surfaces currently disagree: the companion hook says *"Do NOT run index in worktrees — the shared index is read-only here"*, while activation says *"Run index(action='build') to enable semantic_search."* Task 6 makes the hook's line false, so it is deleted rather than argued with.

**Files:**
- Modify: `/home/marius/work/claude/claude-plugins/codescout-companion/hooks/worktree-activate.mjs`
- Modify: `/home/marius/work/claude/claude-plugins/codescout-companion/docs/` — whichever page documents the hook inventory
- Modify: `docs/architecture/companion-plugin.md` (codescout repo)
- Modify: `CHANGELOG.md` (`[Unreleased]` → `### Added`)
- Modify: `docs/issues/2026-08-13-enter-worktree-desyncs-codescout-and-strands-semantic-search.md`

- [ ] **Step 1: Edit the hook**

Delete the `Do NOT run index in worktrees — the shared index is read-only here` sentence from the `additionalContext` string. Add an instruction to run `index(action="build")` after `workspace(action="activate", …)`, noting it embeds only changed files. Also drop the `.codescout/embeddings` symlink *for this purpose*: it links the legacy sqlite store, which both activations observed on 2026-08-13 flagged as `legacy_semantic_index`, and it is a no-op for Qdrant-backed semantic search. Leave the marker file and the write-guard alone — the read-tool coverage gap is half 1 and out of scope here.

- [ ] **Step 2: Correct the two stale plugin docs**

`docs/architecture/companion-plugin.md` names hooks as `.sh` that are now `.mjs` invoked via `node` (only `il3-deny-hook.sh`, `detect-tools.sh` and `*.test.sh` remain shell), and describes the worktree symlink as covering `.codescout/` when the real-dir fallback links only `embeddings`. Fix both while you are in the file.

- [ ] **Step 3: CHANGELOG entry**

Add under `[Unreleased]` → `### Added`, following the surrounding entries' voice: what changed, the mechanism, and what is explicitly not covered (halves 1 and 3, the basename collision hazard, Qdrant CI coverage). Link the spec, not a restatement of it.

- [ ] **Step 4: Update the bug file, then archive it through the librarian**

Fill `## Fix` with the commit SHAs and mark half 2 fixed; leave halves 1 and 3 open with their own `## Resume` lines. Archive **only if** halves 1 and 3 have been split into their own bug files first — otherwise the file stays open, because archiving it would retire two unfixed problems. Use `artifact(action="move", …)`, never `git mv`: `id = sha256(abs_path)` and a bare move orphans the catalog row's events.

- [ ] **Step 5: Verify the doc links**

Run: `librarian(action="audit_doc_refs")`
Expected: no new `missing` findings. Note that this gate does **not** scan `CHANGELOG.md` (`docs/issues/2026-08-08-audit-doc-refs-never-scans-changelog-or-contributing.md`), so check the CHANGELOG's spec link by hand.

- [ ] **Step 6: Commit**

```bash
git branch --show-current   # must print: experiments
git add CHANGELOG.md docs/architecture/companion-plugin.md docs/issues/
git commit -m "docs: retire the worktree index contradiction

worktree index is now cheap, so the hook's 'do NOT run index in worktrees' is
false and is deleted rather than reconciled -- two shipped surfaces disagreed and
one of them is now simply wrong.

Also corrects companion-plugin.md, which names hooks as .sh that are .mjs, and
overstates the worktree symlink: the real-dir fallback links only embeddings, the
legacy sqlite store, which is a no-op for Qdrant-backed search."
```

Commit the plugin-repo changes separately in that repo, prefixed `codescout-companion:` per the cross-repo SHA discipline in memory `gotchas`.

---

## Self-Review

**Spec coverage.** `exclude_paths` on the query surface → Task 3. Content-hash dirty set incl. deletions → Tasks 1–2. Delta project id and its `@` separator → Task 5. `index`-only lifecycle → Task 6. Two-query merge, hint, drift note → Task 7. `ChunkRef.file_path` → Task 1. `IndexState` dirty list → Task 4. Plugin lifecycle edits and the contradiction → Task 8. Testing discipline → Global Constraints plus per-task mutation steps. Out-of-scope items are restated in Task 8 step 4 so they are not archived by accident.

**Deliberate gaps, named rather than hidden.** Task 6's walk/chunk/embed regions point at existing machinery instead of reproducing `stream_index` — reproducing it would invite a forked chunker, which is a worse failure than a plan that says "reuse this". Task 3's contract-test helpers are named as "read the neighbouring test and copy" because inventing helper signatures that do not exist is the placeholder this plan is trying to avoid.

**Type consistency.** `ChunkRef.file_path`, `LocalChunk.file_path`, `Hit.file_path`, the Qdrant payload key and the sqlite column are all `file_path`. `DirtySet.paths` is `BTreeSet<String>` throughout and is converted to `Vec<String>` exactly once, at the `write_index_state_with_dirty` boundary. `exclude_paths` is `&[String]` at the trait and `Vec<String>` on `SearchOpts`, matching `exclude_languages` in both positions.

