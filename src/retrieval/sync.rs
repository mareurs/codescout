use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct SyncOpts {
    pub languages: Option<Vec<String>>,
    pub force_reindex: bool,
    /// When true, `sync_project` records the indexed git HEAD to
    /// `.codescout/index-state.json` on success (the freshness sidecar that
    /// external consumers and `index(action="status")` read). Set by *project*
    /// syncs; left false by *library* syncs so library checkouts aren't polluted.
    pub record_index_state: bool,
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
) -> Result<(usize, usize)> {
    use crate::embed::ast_chunker::split_file;
    use crate::retrieval::payload::CodePayload;
    use std::collections::HashSet;

    let server_ids: HashSet<&str> = server.iter().map(|c| c.chunk_id.as_str()).collect();
    let mut local_ids: HashSet<String> = HashSet::new();
    let mut pending: Vec<CodePayload> = Vec::new();
    let mut added = 0usize;

    for entry in ignore::WalkBuilder::new(root)
        .hidden(false) // index tracked dotfiles; gitignore handles exclusions
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
            let chunk_id = format!("{project_id}:{}:{hash}", rel_path.display());
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
                file_path: rel_path.display().to_string(),
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
    use crate::retrieval::code_store::CodeVectorStore;
    use crate::retrieval::drift::ChunkRef;
    use crate::retrieval::embedder::{BatchEmbedder, EmbedOutput, SparseVector};
    use crate::retrieval::payload::CodePayload;
    use crate::retrieval::search::Hit;
    use std::sync::Mutex;

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

        let (added, deleted) =
            stream_index(dir.path(), "p", "coll", &[], &emb, &store, false, 1200, 3)
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
        )
        .await
        .unwrap();
        assert_eq!(added2, added1, "force should re-embed all current chunks");
    }
}
