use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::util::fs::to_forward_slash;

/// Directories the code index never walks, regardless of `.gitignore` or
/// user-supplied ignore patterns.
///
/// These hold *tool state derived from the project*, not project content, so
/// embedding them makes the corpus self-referential: `semantic_search` starts
/// returning codescout's own memories and catalog rows as if they were source.
/// `.git` additionally carries every historical blob, which no search should see.
///
/// Deliberately not listed: `.claude`, `.serena`, `.buddy`. Those can hold real
/// project documentation — skills, command definitions, prompts — that a caller
/// may legitimately want indexed. They belong in per-project `ignore_patterns`,
/// which is a decision, not a default.
pub(crate) const ALWAYS_SKIP_DIRS: &[&str] = &[".git", ".codescout"];

#[derive(Debug, Clone, Default)]
pub struct SyncOpts {
    pub languages: Option<Vec<String>>,
    pub force_reindex: bool,
    /// When true, `sync_project` records the indexed git HEAD to
    /// `.codescout/index-state.json` on success (the freshness sidecar that
    /// external consumers and `index(action="status")` read). Set by *project*
    /// syncs; left false by *library* syncs so library checkouts aren't polluted.
    pub record_index_state: bool,
    /// Glob/gitignore-style patterns to exclude from the index walk. Sourced from
    /// `config.ignored_paths.patterns`; an empty vec ignores nothing.
    pub ignore_patterns: Vec<String>,
    /// Directory to site the per-project index lock in. `None` — every production
    /// caller — resolves `per_user_runtime_dir()`.
    ///
    /// A test seam, and the only one available here: `sync_project` takes the lock
    /// internally, so a test that drives it end-to-end otherwise writes into the
    /// real runtime dir, and lock files are deliberately never unlinked. See
    /// docs/issues/2026-07-28-index-lock-tests-pollute-runtime-dir.md.
    pub index_lock_dir: Option<std::path::PathBuf>,
}

#[derive(Debug, Default)]
pub struct SyncReport {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
    pub elapsed_ms: u128,
}

impl std::fmt::Display for SyncReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "added={} updated={} deleted={} elapsed_ms={}",
            self.added, self.updated, self.deleted, self.elapsed_ms
        )
    }
}

pub fn content_hash(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("{:x}", h.finalize())
}

/// Build the vector-store chunk id for a file chunk.
///
/// The path component MUST be forward-slash normalized. `rel_path` is OS-derived
/// (it comes from a `strip_prefix` of a filesystem-walk path), so on Windows a raw
/// `.display()` yields `src\lib.rs` — which would (a) persist native separators as
/// the vector store's primary key, (b) make the id disagree with the `file_path`
/// field of its own payload (which normalizes one line below), and (c) make the id
/// platform-dependent, so the `local_ids` / `server_ids` delete-set diff cannot be
/// compared across hosts. See
/// `docs/issues/2026-07-07-display-audit-scope-gap-non-to-string-sites.md`.
pub fn chunk_id(project_id: &str, rel_path: &Path, content_hash: &str) -> String {
    format!("{project_id}:{}:{content_hash}", to_forward_slash(rel_path))
}

/// Embed `pending`'s chunk content and upsert it, then clear `pending` so the
/// content + embeddings are dropped — keeping peak memory at O(flush_batch).
async fn flush_pending(
    embedder: &dyn crate::retrieval::embedder::BatchEmbedder,
    store: &dyn crate::retrieval::code_store::CodeVectorStore,
    collection: &str,
    pending: &mut Vec<crate::retrieval::payload::CodePayload>,
) -> Result<usize> {
    use crate::retrieval::embedder::EmbedOutput;
    use crate::retrieval::payload::CodePayload;
    if pending.is_empty() {
        return Ok(0);
    }
    let texts: Vec<String> = pending.iter().map(|p| p.content.clone()).collect();
    let embeds = embedder.embed_batch_dyn(&texts).await?;
    let n = pending.len();
    let chunks: Vec<(CodePayload, EmbedOutput)> = pending.drain(..).zip(embeds).collect();
    store.upsert_chunks(collection, &chunks).await?;
    Ok(n)
}

/// Whether a walk entry is tool state the code index must never descend into.
///
/// Directory-only by design: a *file* named `.git` is a worktree pointer and a
/// file named `.codescout` is just a file — neither is a state tree, and neither
/// should be skipped on the strength of its name alone.
pub(crate) fn is_always_skipped(name: &str, is_dir: bool) -> bool {
    is_dir && ALWAYS_SKIP_DIRS.contains(&name)
}

/// Walk `root`, diff against `server` chunk refs, and embed+upsert changed chunks
/// in bounded batches so peak memory is O(flush_batch), not O(all_files).
///
/// Split out of [`RetrievalClient::sync_project`] both as a test seam (driven by
/// `&dyn BatchEmbedder` + `&dyn CodeVectorStore`) and to bound the index pass: the
/// previous whole-tree materialisation grew to 68 GB and OOM-killed the host
/// (docs/issues/2026-06-19-mcp-server-oom-68gb.md). `chunk_id` encodes the content
/// hash, so the delete-set needs only the cheap id sets — never the chunk content.
///
/// Returns `(added, deleted)`.
#[allow(clippy::too_many_arguments)]
async fn stream_index(
    root: &Path,
    project_id: &str,
    collection: &str,
    server: &[crate::retrieval::drift::ChunkRef],
    embedder: &dyn crate::retrieval::embedder::BatchEmbedder,
    store: &dyn crate::retrieval::code_store::CodeVectorStore,
    force_reindex: bool,
    chunk_target: usize,
    flush_batch: usize,
    ignore_patterns: &[String],
) -> Result<(usize, usize)> {
    use crate::embed::ast_chunker::split_file;
    use crate::retrieval::payload::CodePayload;
    use std::collections::HashSet;

    let server_ids: HashSet<&str> = server.iter().map(|c| c.chunk_id.as_str()).collect();
    let mut local_ids: HashSet<String> = HashSet::new();
    let mut pending: Vec<CodePayload> = Vec::new();
    let mut added = 0usize;

    let ignore_matcher = crate::embed::build_ignore_matcher(root, ignore_patterns);
    for entry in ignore::WalkBuilder::new(root)
        // Index tracked dotfiles (`.github/`, `.cargo/config.toml`), which means
        // turning off the crate's hidden-entry filter -- and that filter is what
        // normally keeps `.git/` out of a walk. `.gitignore` does not cover it:
        // git has no reason to ignore its own directory. So the denylist below is
        // load-bearing, not belt-and-braces.
        .hidden(false)
        .filter_entry(move |e| {
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if e.file_name()
                .to_str()
                .is_some_and(|n| is_always_skipped(n, is_dir))
            {
                return false;
            }
            !ignore_matcher.matched(e.path(), is_dir).is_ignore()
        })
        .build()
        .filter_map(|e| e.ok())
    {
        let Some(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let Some(lang) = crate::embed::lang_for_ext(ext) else {
            continue;
        };
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let rel_path = path.strip_prefix(root).unwrap_or(path);
        for c in split_file(&source, lang, path, chunk_target) {
            // Skip empty/whitespace-only chunks — embedders reject empty inputs.
            if c.content.trim().is_empty() {
                continue;
            }
            let hash = content_hash(&c.content);
            let chunk_id = chunk_id(project_id, rel_path, &hash);
            // Every local chunk id participates in the delete-set diff, even when
            // it is already indexed and skipped for re-embedding.
            local_ids.insert(chunk_id.clone());
            // chunk_id encodes the content hash, so a content change yields a new
            // id; skip re-embedding ids the server already has unless force_reindex.
            if !force_reindex && server_ids.contains(chunk_id.as_str()) {
                continue;
            }
            pending.push(CodePayload {
                project_id: project_id.into(),
                file_path: to_forward_slash(rel_path),
                language: lang.into(),
                start_line: c.start_line as i64,
                end_line: c.end_line as i64,
                ast_kind: String::new(),
                ast_header: String::new(),
                content: c.content,
                content_hash: hash,
                last_indexed_commit: String::new(),
                chunk_id,
            });
            // Flush when the buffer fills so peak memory stays O(flush_batch), not
            // O(all_files) — the whole-tree materialisation grew to 68 GB and
            // OOM-killed the host (docs/issues/2026-06-19-mcp-server-oom-68gb.md).
            if pending.len() >= flush_batch {
                added += flush_pending(embedder, store, collection, &mut pending).await?;
            }
        }
    }
    // Flush the tail.
    if !pending.is_empty() {
        added += flush_pending(embedder, store, collection, &mut pending).await?;
    }

    // Delete server chunks that are no longer present locally.
    let to_delete: Vec<String> = server
        .iter()
        .filter(|c| !local_ids.contains(c.chunk_id.as_str()))
        .map(|c| c.chunk_id.clone())
        .collect();
    let deleted = to_delete.len();
    if !to_delete.is_empty() {
        store
            .delete_chunks(collection, project_id, &to_delete)
            .await?;
    }

    Ok((added, deleted))
}

impl crate::retrieval::client::RetrievalClient {
    pub async fn sync_project(
        &self,
        project_id: &str,
        root: &Path,
        opts: SyncOpts,
    ) -> Result<SyncReport> {
        // chunk=1200 was the universal sweet spot in the Phase 5.5 chunk×model matrix
        // (see docs/research/2026-05-06-retrieval-stack-benchmark.md). Override with
        // CODESCOUT_CHUNK_TARGET when retuning.
        const STACK_CHUNK_TARGET: usize = 1200;
        let chunk_target: usize = std::env::var("CODESCOUT_CHUNK_TARGET")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(STACK_CHUNK_TARGET);
        // Flush the embed/upsert buffer every FLUSH_BATCH chunks so peak memory is
        // O(batch), not O(all_files). The previous whole-tree materialisation here
        // grew to 68 GB and OOM-killed the host
        // (docs/issues/2026-06-19-mcp-server-oom-68gb.md).
        const DEFAULT_FLUSH_BATCH: usize = 256;
        let flush_batch: usize = std::env::var("CODESCOUT_INDEX_FLUSH_BATCH")
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_FLUSH_BATCH);
        tracing::info!(
            chunk_target,
            flush_batch,
            force_reindex = opts.force_reindex,
            "retrieval sync starting"
        );

        // Serialize index passes per project. MUST be acquired before the
        // `chunk_refs` call below: that read establishes the drift baseline, and
        // `stream_index` then mutates it. Two overlapping runs would each diff
        // against a snapshot the other is invalidating.
        //
        // Bound to `_index_lock` (not `_`) so it lives until the end of this
        // function — `let _ = ...` would drop it immediately and release the lock.
        // Guarded by `sync_project_holds_index_lock_for_its_full_duration`.
        let _index_lock = match opts.index_lock_dir.as_deref() {
            Some(dir) => crate::retrieval::index_lock::acquire_in(dir, project_id)?,
            None => crate::retrieval::index_lock::acquire(project_id)?,
        };

        let started = std::time::Instant::now();
        let collection = self.config.collection("code_chunks");
        self.code_store
            .ensure_collection(&collection, self.config.model_dim as u64)
            .await?;

        // Fetch existing chunk refs (id + hash only — bounded) for drift diffing.
        let server = self
            .code_store
            .chunk_refs(&collection, project_id)
            .await
            .unwrap_or_default();

        let (added, deleted) = stream_index(
            root,
            project_id,
            &collection,
            &server,
            &self.embedder,
            self.code_store.as_ref(),
            opts.force_reindex,
            chunk_target,
            flush_batch,
            &opts.ignore_patterns,
        )
        .await?;

        let elapsed_ms = started.elapsed().as_millis();
        tracing::info!(added, deleted, elapsed_ms, "retrieval sync finished");

        // Record the indexed HEAD for external-change freshness detection
        // (checkout/pull/HEAD move). Gated to *project* syncs — library syncs
        // leave record_index_state false so library checkouts aren't polluted.
        // Fail-soft: a sidecar write must never break the sync.
        if opts.record_index_state {
            if let Err(e) = crate::retrieval::index_state::write_index_state(root) {
                tracing::warn!(error = %e, "failed to write index-state sidecar");
            }
        }

        Ok(SyncReport {
            added,
            deleted,
            updated: 0,
            elapsed_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::client::RetrievalClient;
    use crate::retrieval::code_store::CodeVectorStore;
    use crate::retrieval::config::RetrievalConfig;
    use crate::retrieval::drift::ChunkRef;
    use crate::retrieval::embedder::{BatchEmbedder, EmbedOutput, EmbedderHttp, SparseVector};
    use crate::retrieval::payload::CodePayload;
    use crate::retrieval::reranker::RerankerHttp;
    use crate::retrieval::search::Hit;
    use std::sync::{Arc, Mutex};

    #[test]
    fn always_skipped_covers_git_and_codescout_state_only_as_directories() {
        // `.hidden(false)` on the walker is what lets tracked dotfiles in, and it is
        // also what lets `.git/` in — `.gitignore` never lists it, so nothing else
        // stops the walk. This denylist is the only thing that does.
        assert!(is_always_skipped(".git", true));
        assert!(is_always_skipped(".codescout", true));

        // A FILE named `.git` is a worktree pointer, not a state tree. Skipping it by
        // name would be a different, and wrong, decision.
        assert!(!is_always_skipped(".git", false));
        assert!(!is_always_skipped(".codescout", false));

        // Deliberately absent: agent dirs that can hold real project documentation.
        // They belong in per-project `ignore_patterns` — a decision, not a default.
        assert!(!is_always_skipped(".claude", true));
        assert!(!is_always_skipped(".serena", true));
        assert!(!is_always_skipped(".github", true));

        // Whole-name match, not a prefix: a real directory must survive.
        assert!(!is_always_skipped(".gitlab-ci", true));
        assert!(!is_always_skipped("src", true));
    }

    #[test]
    fn chunk_id_normalizes_native_separators() {
        // BUG (docs/issues/2026-07-07-display-audit-scope-gap-non-to-string-sites.md):
        // chunk_id was built with `rel_path.display()`, which renders a PathBuf's
        // internal string VERBATIM. rel_path is OS-derived (strip_prefix of a
        // filesystem-walk path), so on Windows it carries backslashes — persisting
        // native separators as the vector store's primary key, and disagreeing with
        // the `file_path` field of its own payload, which normalizes one line below.
        //
        // to_forward_slash is not cfg(windows)-gated, so a PathBuf built from a
        // literal backslash string reproduces the Windows shape on any host — the
        // same technique util/fs.rs's own tests use.
        let windows_shaped = std::path::PathBuf::from("src\\retrieval\\sync.rs");
        assert_eq!(
            chunk_id("proj", &windows_shaped, "deadbeef"),
            "proj:src/retrieval/sync.rs:deadbeef",
            "the path component of a chunk id must be forward-slash normalized"
        );

        // Already-forward-slash input is untouched (the Linux/macOS path).
        let posix = std::path::PathBuf::from("src/retrieval/sync.rs");
        assert_eq!(
            chunk_id("proj", &posix, "deadbeef"),
            "proj:src/retrieval/sync.rs:deadbeef"
        );
    }

    /// Records every `upsert_chunks` batch size + the refs it upserted, so a test
    /// can assert the indexer flushes in bounded batches (regression guard for the
    /// 68 GB OOM: docs/issues/2026-06-19-mcp-server-oom-68gb.md).
    #[derive(Default)]
    struct RecordingStore {
        upsert_batches: Mutex<Vec<usize>>,
        upserted: Mutex<Vec<ChunkRef>>,
        deleted: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl CodeVectorStore for RecordingStore {
        async fn ensure_collection(&self, _c: &str, _d: u64) -> Result<()> {
            Ok(())
        }
        async fn chunk_refs(&self, _c: &str, _p: &str) -> Result<Vec<ChunkRef>> {
            Ok(self.upserted.lock().unwrap().clone())
        }
        async fn upsert_chunks(
            &self,
            _c: &str,
            chunks: &[(CodePayload, EmbedOutput)],
        ) -> Result<()> {
            self.upsert_batches.lock().unwrap().push(chunks.len());
            let mut u = self.upserted.lock().unwrap();
            for (p, _) in chunks {
                u.push(ChunkRef {
                    chunk_id: p.chunk_id.clone(),
                    content_hash: p.content_hash.clone(),
                });
            }
            Ok(())
        }
        async fn delete_chunks(&self, _c: &str, _p: &str, ids: &[String]) -> Result<()> {
            self.deleted.lock().unwrap().extend(ids.iter().cloned());
            Ok(())
        }
        #[allow(clippy::too_many_arguments)]
        async fn query(
            &self,
            _c: &str,
            _p: &str,
            _dense: &[f32],
            _sparse: &SparseVector,
            _limit: usize,
            _bm25: f32,
            _disable_sparse: bool,
            _excl: &[String],
        ) -> Result<Vec<Hit>> {
            Ok(vec![])
        }
        async fn project_index_stats(&self, _c: &str, _p: &str) -> Result<(usize, usize)> {
            Ok((0, 0))
        }
    }

    /// Deterministic embedder fake: one dense vector per input, no HTTP. Output
    /// length matches `texts` so the zip in `flush_pending` stays aligned.
    struct FakeEmbedder {
        dim: usize,
    }

    #[async_trait::async_trait]
    impl BatchEmbedder for FakeEmbedder {
        async fn embed_batch_dyn(&self, texts: &[String]) -> Result<Vec<EmbedOutput>> {
            Ok(texts
                .iter()
                .map(|_| EmbedOutput {
                    dense: vec![0.1; self.dim],
                    sparse: SparseVector {
                        indices: vec![],
                        values: vec![],
                    },
                })
                .collect())
        }
    }

    fn write_sources(dir: &std::path::Path, n: usize) {
        for i in 0..n {
            std::fs::write(
                dir.join(format!("file_{i}.rs")),
                format!("fn f{i}() {{ let x = {i}; println!(\"{{}}\", x); }}\n"),
            )
            .unwrap();
        }
    }

    #[tokio::test]
    async fn stream_index_flushes_in_bounded_batches() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 10);
        let store = RecordingStore::default();
        let emb = FakeEmbedder { dim: 4 };

        let (added, deleted) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store,
            false,
            1200,
            3,
            &[],
        )
        .await
        .unwrap();

        let batches = store.upsert_batches.lock().unwrap().clone();
        // Pre-fix, the whole-tree sync did ONE upsert of every chunk. Streaming must
        // flush in multiple batches, none larger than flush_batch — the regression
        // guard for the 68 GB OOM.
        assert!(
            batches.len() >= 2,
            "expected multiple bounded flushes, got {batches:?}"
        );
        assert!(
            batches.iter().all(|&n| n <= 3),
            "a flush exceeded flush_batch=3: {batches:?}"
        );
        assert_eq!(batches.iter().sum::<usize>(), added);
        assert!(
            added >= 10,
            "10 files should yield >=1 chunk each; added={added}"
        );
        assert_eq!(deleted, 0);
    }

    #[tokio::test]
    async fn stream_index_incremental_skips_unchanged_and_prunes_stale() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 6);
        let emb = FakeEmbedder { dim: 4 };

        // First pass: empty server -> everything embedded.
        let store1 = RecordingStore::default();
        let (added1, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store1,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        let server: Vec<ChunkRef> = store1.upserted.lock().unwrap().clone();
        assert!(added1 >= 6);

        // Second pass: server already has every chunk -> nothing re-embedded or deleted.
        let store2 = RecordingStore::default();
        let (added2, deleted2) = stream_index(
            dir.path(),
            "p",
            "coll",
            &server,
            &emb,
            &store2,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        assert_eq!(added2, 0, "unchanged tree must not re-embed");
        assert_eq!(deleted2, 0);
        assert!(store2.upsert_batches.lock().unwrap().is_empty());

        // Change one file -> its new chunk upserts, its old chunk id is pruned.
        std::fs::write(
            dir.path().join("file_0.rs"),
            "fn f0() { let changed = 4242; println!(\"{}\", changed); }\n",
        )
        .unwrap();
        let store3 = RecordingStore::default();
        let (added3, deleted3) = stream_index(
            dir.path(),
            "p",
            "coll",
            &server,
            &emb,
            &store3,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        assert!(added3 >= 1, "changed file should re-embed");
        assert!(deleted3 >= 1, "stale chunk id should be pruned");
    }

    #[tokio::test]
    async fn stream_index_force_reembeds_all_present_chunks() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 5);
        let emb = FakeEmbedder { dim: 4 };

        let store1 = RecordingStore::default();
        let (added1, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store1,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        let server: Vec<ChunkRef> = store1.upserted.lock().unwrap().clone();

        // force_reindex re-embeds every present chunk even though the server has them.
        let store2 = RecordingStore::default();
        let (added2, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &server,
            &emb,
            &store2,
            true,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        assert_eq!(added2, added1, "force should re-embed all current chunks");
    }

    #[tokio::test]
    async fn stream_index_excludes_ignored_dirs() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 3); // file_0.rs..file_2.rs at root
        std::fs::create_dir_all(dir.path().join("node_modules")).unwrap();
        std::fs::write(
            dir.path().join("node_modules/dep.js"),
            "function x() { return 1; }\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("svc/.venv")).unwrap();
        std::fs::write(
            dir.path().join("svc/.venv/lib.py"),
            "def y():\n    return 2\n",
        )
        .unwrap();
        let emb = FakeEmbedder { dim: 4 };

        let patterns = vec!["node_modules".to_string(), ".venv".to_string()];
        let store = RecordingStore::default();
        let (added, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store,
            false,
            1200,
            256,
            &patterns,
        )
        .await
        .unwrap();
        let ids: Vec<String> = store
            .upserted
            .lock()
            .unwrap()
            .iter()
            .map(|r| r.chunk_id.clone())
            .collect();
        assert!(
            ids.iter()
                .all(|id| !id.contains("node_modules") && !id.contains(".venv")),
            "ignored dirs must not be indexed: {ids:?}"
        );
        assert!(
            added >= 3,
            "the 3 root .rs files should still index; added={added}"
        );

        // With no patterns, the dep files ARE indexed (more chunks).
        let store2 = RecordingStore::default();
        let (added2, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store2,
            false,
            1200,
            256,
            &[],
        )
        .await
        .unwrap();
        assert!(
            added2 > added,
            "empty patterns must index everything: {added2} vs {added}"
        );
    }

    /// A `CodeVectorStore` whose `ensure_collection` sleeps briefly before
    /// returning. `sync_project` calls `ensure_collection` immediately after
    /// acquiring the index lock (before any real indexing work), so this gives
    /// a controllable window in which the lock is provably still held —
    /// without needing real files, a real embedder, or a real Qdrant.
    struct SlowEnsureStore;

    #[async_trait::async_trait]
    impl CodeVectorStore for SlowEnsureStore {
        async fn ensure_collection(&self, _c: &str, _d: u64) -> Result<()> {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            Ok(())
        }
        async fn chunk_refs(&self, _c: &str, _p: &str) -> Result<Vec<ChunkRef>> {
            Ok(vec![])
        }
        async fn upsert_chunks(
            &self,
            _c: &str,
            _chunks: &[(CodePayload, EmbedOutput)],
        ) -> Result<()> {
            Ok(())
        }
        async fn delete_chunks(&self, _c: &str, _p: &str, _ids: &[String]) -> Result<()> {
            Ok(())
        }
        #[allow(clippy::too_many_arguments)]
        async fn query(
            &self,
            _c: &str,
            _p: &str,
            _dense: &[f32],
            _sparse: &SparseVector,
            _limit: usize,
            _bm25: f32,
            _disable_sparse: bool,
            _excl: &[String],
        ) -> Result<Vec<Hit>> {
            Ok(vec![])
        }
        async fn project_index_stats(&self, _c: &str, _p: &str) -> Result<(usize, usize)> {
            Ok((0, 0))
        }
    }

    fn test_retrieval_client(store: impl CodeVectorStore + 'static) -> RetrievalClient {
        RetrievalClient {
            code_store: Arc::new(store),
            embedder: EmbedderHttp::new("http://unused.invalid", "http://unused.invalid", 3),
            reranker: RerankerHttp::new("http://unused.invalid"),
            config: RetrievalConfig {
                qdrant_url: "http://unused.invalid".into(),
                embedder_url: "http://unused.invalid".into(),
                sparse_embedder_url: "http://unused.invalid".into(),
                reranker_url: "http://unused.invalid".into(),
                model_dim: 3,
                profile: "cpu".into(),
                bm25_boost: 1.0,
                disable_sparse: false,
                collection_prefix: String::new(),
            },
            lite: false,
        }
    }

    /// Regression guard for the index-lock wiring in `sync_project` (the
    /// `let _index_lock = ...acquire_in/acquire(project_id)?;` at the top of the
    /// function). Binding the acquired guard to `_` instead
    /// of `_index_lock` compiles clean and passes every OTHER retrieval test,
    /// but drops the guard immediately — releasing the flock right away
    /// instead of holding it for the sync pass — which is exactly how the
    /// concurrent-index duplication bug this branch fixes would return.
    ///
    /// A single "acquire first, then call sync_project once" test cannot
    /// distinguish `_index_lock` from `_`: if the lock is already held
    /// externally, `sync_project`'s own `acquire(project_id)?` fails
    /// identically either way, since that failure happens at the
    /// `try_lock_exclusive` call itself, before the binding pattern is even
    /// reached. So instead this spawns `sync_project` (slowed down via
    /// `SlowEnsureStore` so it is provably still in flight) and, from the
    /// OUTSIDE, tries to acquire the same lock while it runs: that outside
    /// acquire must fail iff `sync_project`'s guard is still alive at that
    /// moment.
    #[tokio::test]
    async fn sync_project_holds_index_lock_for_its_full_duration() {
        let dir = tempfile::tempdir().unwrap();
        // A dir of its own, not `dir`: the lock must not land inside the tree
        // being indexed. Also the reason `project_id` can be a plain literal —
        // the scratch dir, not the id, is what isolates concurrent runs.
        let lock_dir = tempfile::tempdir().unwrap();
        let project_id = "test-sync-holds-index-lock".to_string();

        let client = test_retrieval_client(SlowEnsureStore);
        let opts = SyncOpts {
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };
        let pid = project_id.clone();
        let root = dir.path().to_path_buf();
        let handle = tokio::spawn(async move { client.sync_project(&pid, &root, opts).await });

        // Give the spawned call time to acquire the lock and enter
        // ensure_collection's 300ms sleep, but stay well inside that window.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let contended = crate::retrieval::index_lock::acquire_in(lock_dir.path(), &project_id);
        assert!(
            contended.is_err(),
            "sync_project's index-lock guard must still be held while the call is in flight"
        );
        let msg = format!("{:#}", contended.unwrap_err());
        assert!(
            msg.contains("already running"),
            "error should surface lock-contention wording, got: {msg}"
        );

        handle
            .await
            .expect("spawned task must not panic")
            .expect("sync_project should still succeed once it completes");
    }
}
