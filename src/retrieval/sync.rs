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
    use crate::retrieval::payload::{embed_text, CodePayload};
    if pending.is_empty() {
        return Ok(0);
    }
    // What a chunk looks like to the embedder is `embed_text`'s decision, not this
    // function's. Reading `p.content` inline here is what silently dropped the AST
    // header when its previous consumer was deleted.
    let texts: Vec<String> = pending.iter().map(embed_text).collect();
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
        // The header this produces is now embedded, so it has to be
        // checkout-independent and separator-stable: hand the chunker the same
        // forward-slashed relative path the payload stores, never the absolute one.
        // Every one of the chunker's own 31 call sites already passes a relative
        // path; this lone production caller passed `path`, and nothing noticed
        // because the header it produced was never consumed.
        let rel_display = to_forward_slash(rel_path);
        for c in split_file(&source, lang, Path::new(&rel_display), chunk_target) {
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
                // The chunker's identity line for this chunk, empty for non-AST
                // languages. `embed_text` prepends it; discarding it here is what
                // made it unreachable for four hundred thousand chunks.
                ast_header: c.metadata.unwrap_or_default(),
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
        // `backend` and `sparse` are logged because their being WRONG is silent otherwise.
        // A plain `cargo build --release` omits the `server-stack` feature, so
        // `VectorBackend::resolve()` defaults to sqlite-vec, which sets `lite` -> `dense_only`
        // -> the sparse leg is skipped AND the writes go to `.codescout/embeddings/project.db`
        // instead of Qdrant. Measured 2026-08-07: a `--force` rebuild ran seven minutes
        // hammering the dense embedder with zero sparse requests before anyone noticed, and the
        // only reason it was caught is that someone compared container logs. Use `cargo rb`
        // (aliased to `--features server-stack`) for the hybrid stack; these two fields make the
        // difference visible in line one instead of two hours later.
        tracing::info!(
            chunk_target,
            flush_batch,
            force_reindex = opts.force_reindex,
            backend = if self.lite { "sqlite-vec" } else { "qdrant" },
            sparse = if self.lite || self.config.disable_sparse {
                "SKIPPED"
            } else {
                "on"
            },
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
        self.guard_index_dim(&collection, project_id).await?;
        self.code_store
            .ensure_collection(
                &collection,
                self.effective_model_dim(crate::retrieval::config::DEFAULT_MODEL_DIM),
            )
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
            &*self.embedder,
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
    use crate::retrieval::embedder::{
        BatchEmbedder, CodeEmbedder, EmbedOutput, EmbedderHttp, SparseVector,
    };
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
        /// Reported by `collection_dim`. `None` (the default) means "no index
        /// yet" — a test overrides it to `Some(n)` to exercise `guard_index_dim`
        /// against a pre-existing index at dimension `n`.
        dim: Mutex<Option<u64>>,
        /// The `dim` argument `sync_project` actually passed to
        /// `ensure_collection` — captured so a test can prove the call site uses
        /// `RetrievalClient::effective_model_dim` (review round-2 I5) rather than
        /// the bare `config.model_dim.unwrap_or(DEFAULT_MODEL_DIM)` this sibling
        /// used to use, without needing to inspect a real Qdrant collection.
        ensured_dim: Mutex<Option<u64>>,
    }

    #[async_trait::async_trait]
    impl CodeVectorStore for RecordingStore {
        async fn ensure_collection(&self, _c: &str, d: u64) -> Result<()> {
            *self.ensured_dim.lock().unwrap() = Some(d);
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
                    file_path: p.file_path.clone(),
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
            _paths: &[String],
        ) -> Result<Vec<Hit>> {
            Ok(vec![])
        }
        async fn project_index_stats(&self, _c: &str, _p: &str) -> Result<(usize, usize)> {
            Ok((0, 0))
        }

        async fn project_has_chunks(&self, _c: &str, _p: &str) -> Result<bool> {
            Ok(false)
        }

        async fn collection_dim(&self, _c: &str, _p: &str) -> Result<Option<u64>> {
            Ok(*self.dim.lock().unwrap())
        }
    }

    /// Deterministic embedder fake: one dense vector per input, no HTTP. Output
    /// length matches `texts` so the zip in `flush_pending` stays aligned.
    struct FakeEmbedder {
        dim: usize,
        /// Every text handed to `embed_batch_dyn`, in order.
        ///
        /// This is what lets a test assert on the EMBEDDING INPUT rather than on the
        /// stored payload — and that distinction is the whole bug: the legacy path
        /// stored raw content while embedding `{header}\n{content}`, so inspecting
        /// stored content would have "confirmed" correct behaviour either way.
        seen: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl BatchEmbedder for FakeEmbedder {
        async fn embed_batch_dyn(&self, texts: &[String]) -> Result<Vec<EmbedOutput>> {
            self.seen.lock().unwrap().extend(texts.iter().cloned());
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
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

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

    /// The regression guard the deleted `embed::index` module took with it.
    ///
    /// `embed_text_format_includes_metadata_prefix` asserted the text sent for
    /// embedding is `{metadata}\n{content}` and not just content. It lived inside
    /// the module removed in `66db4c70`, so when the surviving path turned out not
    /// to implement the contract, nothing failed and the header quietly stopped
    /// being embedded — 579,311 chunks' worth.
    ///
    /// This asserts on what the embedder RECEIVED. Asserting on the stored payload
    /// would not have caught the original defect: the legacy path stored raw
    /// content while embedding header+content, so stored content is raw in both
    /// the working and the broken world.
    #[tokio::test]
    async fn stream_index_embeds_the_ast_header_ahead_of_content() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("widget.rs"),
            "fn assemble_widget(n: usize) -> usize {\n    let total = n * 2;\n    total\n}\n",
        )
        .unwrap();
        let store = RecordingStore::default();
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

        let (added, _) = stream_index(
            dir.path(),
            "p",
            "coll",
            &[],
            &emb,
            &store,
            false,
            1200,
            8,
            &[],
        )
        .await
        .unwrap();
        assert!(added > 0, "expected at least one chunk");

        let seen = emb.seen.lock().unwrap().clone();
        let headed: Vec<&String> = seen
            .iter()
            .filter(|t| t.starts_with("widget.rs ::"))
            .collect();
        assert!(
            !headed.is_empty(),
            "no embedded text carried an AST header; got {seen:?}"
        );

        // Header is a PREFIX, not a replacement — the body has to survive it.
        let (header, body) = headed[0]
            .split_once('\n')
            .expect("header line, then content");
        assert!(
            header.contains("assemble_widget"),
            "header should name the symbol, got {header:?}"
        );
        assert!(
            body.contains("let total = n * 2;"),
            "content must survive the prepend, got {body:?}"
        );

        // Checkout-independence: the absolute temp path must not reach the vector.
        let root = dir.path().to_string_lossy().to_string();
        assert!(
            !seen.iter().any(|t| t.contains(&root)),
            "an absolute path leaked into the embedding input"
        );
    }

    #[tokio::test]
    async fn stream_index_incremental_skips_unchanged_and_prunes_stale() {
        let dir = tempfile::tempdir().unwrap();
        write_sources(dir.path(), 6);
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

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
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

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
        let emb = FakeEmbedder {
            dim: 4,
            seen: Mutex::new(Vec::new()),
        };

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
            _paths: &[String],
        ) -> Result<Vec<Hit>> {
            Ok(vec![])
        }
        async fn project_index_stats(&self, _c: &str, _p: &str) -> Result<(usize, usize)> {
            Ok((0, 0))
        }

        async fn project_has_chunks(&self, _c: &str, _p: &str) -> Result<bool> {
            Ok(false)
        }

        async fn collection_dim(&self, _c: &str, _p: &str) -> Result<Option<u64>> {
            Ok(None)
        }
    }

    fn test_retrieval_client(store: impl CodeVectorStore + 'static) -> RetrievalClient {
        RetrievalClient {
            code_store: Arc::new(store),
            embedder: std::sync::Arc::new(EmbedderHttp::new(
                "http://unused.invalid",
                "http://unused.invalid",
                3,
            )),
            reranker: RerankerHttp::new("http://unused.invalid"),
            config: RetrievalConfig {
                qdrant_url: "http://unused.invalid".into(),
                embedder_url: Some("http://unused.invalid".into()),
                sparse_embedder_url: "http://unused.invalid".into(),
                reranker_url: "http://unused.invalid".into(),
                model_dim: Some(3),
                model: "local:AllMiniLML6V2Q".into(),
                api_key: None,
                profile: "cpu".into(),
                bm25_boost: 1.0,
                disable_sparse: false,
                rerank: false,
                collection_prefix: String::new(),
            },
            lite: false,
        }
    }

    /// A `CodeEmbedder` fake standing in for `CodeEmbedderAdapter` (a local
    /// backend that self-describes its dimension) without a real ONNX load.
    /// Every method but `known_dim` is unreachable — the one test using this
    /// never calls embed.
    struct FixedDimEmbedder(usize);

    #[async_trait::async_trait]
    impl BatchEmbedder for FixedDimEmbedder {
        async fn embed_batch_dyn(&self, _texts: &[String]) -> Result<Vec<EmbedOutput>> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }
    }

    #[async_trait::async_trait]
    impl CodeEmbedder for FixedDimEmbedder {
        async fn embed_one(&self, _text: &str) -> Result<EmbedOutput> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }
        async fn embed_dense_one(&self, _text: &str) -> Result<Vec<f32>> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }
        fn known_dim(&self) -> Option<usize> {
            Some(self.0)
        }
    }

    /// Like `test_retrieval_client`, but with an injectable embedder and no
    /// `model_dim` pin — for exercising `RetrievalClient::effective_model_dim`'s
    /// embedder-first priority directly, rather than the pin-or-default shape
    /// `test_retrieval_client` is set up for. Takes an already-`Arc`'d store
    /// (rather than `impl CodeVectorStore + 'static`, as `test_retrieval_client`
    /// does) so a caller can keep a concrete-typed clone to inspect afterward —
    /// `CodeVectorStore` has no `as_any()`/downcast seam to recover one later.
    fn test_retrieval_client_with_embedder(
        store: Arc<dyn CodeVectorStore>,
        embedder: Arc<dyn CodeEmbedder>,
    ) -> RetrievalClient {
        RetrievalClient {
            code_store: store,
            embedder,
            reranker: RerankerHttp::new("http://unused.invalid"),
            config: RetrievalConfig {
                qdrant_url: "http://unused.invalid".into(),
                embedder_url: Some("http://unused.invalid".into()),
                sparse_embedder_url: "http://unused.invalid".into(),
                reranker_url: "http://unused.invalid".into(),
                model_dim: None,
                model: "local:AllMiniLML6V2Q".into(),
                api_key: None,
                profile: "cpu".into(),
                bm25_boost: 1.0,
                disable_sparse: false,
                rerank: false,
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

    /// Call-site mutation target for `guard_index_dim`'s wiring into
    /// `sync_project`. `test_retrieval_client` pins `model_dim: Some(3)`; this
    /// store reports an EXISTING index already baked at a different dim (999).
    /// The project root is an empty tempdir (nothing to walk, nothing to
    /// embed) and every other `RecordingStore` method trivially succeeds — so
    /// absent the `self.guard_index_dim(&collection, project_id).await?;` line
    /// in `sync_project`, this exact setup returns `Ok` with `added: 0`, not
    /// an error. Deleting that line makes `unwrap_err()` below panic.
    #[tokio::test]
    async fn sync_project_fails_fast_on_a_dim_mismatch_before_touching_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        let store = RecordingStore {
            dim: Mutex::new(Some(999)),
            ..Default::default()
        };
        let client = test_retrieval_client(store);
        let opts = SyncOpts {
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };
        let err = client
            .sync_project("dim-mismatch-project", dir.path(), opts)
            .await
            .expect_err("a stored dim of 999 must fail against the configured model_dim of 3");
        // Review round-2 I2: assert the error CLASS + remedy, not just the
        // numbers in its Display — `RecoverableError`'s Display appends the
        // hint, so a version of this test asserting only on `format!("{err:#}")`
        // stays green even if `RecoverableError::with_hint(...)` were replaced
        // wholesale with a bare `anyhow::anyhow!(...)`, which drops the hint AND
        // flips the MCP contract from `isError: false` to `true`.
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "must be RecoverableError (isError: false) so sibling parallel tool calls \
             survive a dimension mismatch; got: {err:#}"
        );
        let msg = format!("{err:#}");
        assert!(
            msg.contains("Delete the code index"),
            "must carry the reindex remedy, got: {msg}"
        );
        assert!(
            msg.contains("999") && msg.contains('3'),
            "error should name both the stored and configured dims, got: {msg}"
        );
    }

    /// Review round-2 I5: the code-collection sibling of the `memories`-collection
    /// bug named in the task 8 brief. `sync_project`'s `ensure_collection` call
    /// used to size a *fresh* collection with
    /// `self.config.model_dim.unwrap_or(DEFAULT_MODEL_DIM)` — 768 — regardless
    /// of the model actually configured. With an unpinned local embedder
    /// reporting 384 (mirroring `local:AllMiniLML6V2Q`), this test proves the
    /// call site now goes through `effective_model_dim` instead: it must pass
    /// 384 to `ensure_collection`, not the 768 compatibility default. Deleting
    /// the `self.effective_model_dim(...)` call from that line (reverting to
    /// the bare `unwrap_or(DEFAULT_MODEL_DIM)`) makes this test's assertion
    /// fail with `ensured_dim == Some(768)`.
    #[tokio::test]
    async fn sync_project_sizes_a_fresh_collection_from_the_unpinned_local_embedder() {
        let dir = tempfile::tempdir().unwrap();
        let lock_dir = tempfile::tempdir().unwrap();
        // Concrete-typed `Arc` kept alongside the trait-object clone handed to
        // the client, so `ensured_dim` can be read back afterward —
        // `CodeVectorStore` has no downcast seam.
        let store = Arc::new(RecordingStore::default());
        let embedder: Arc<dyn CodeEmbedder> = Arc::new(FixedDimEmbedder(384));
        let client = test_retrieval_client_with_embedder(store.clone(), embedder);
        let opts = SyncOpts {
            index_lock_dir: Some(lock_dir.path().to_path_buf()),
            ..SyncOpts::default()
        };
        client
            .sync_project("fresh-local-project", dir.path(), opts)
            .await
            .expect("an empty project tree with an all-Ok store must sync cleanly");
        assert_eq!(
            *store.ensured_dim.lock().unwrap(),
            Some(384),
            "must size the fresh collection from the unpinned local embedder's own \
             dimension (384), not the DEFAULT_MODEL_DIM compatibility constant (768)"
        );
    }
}
