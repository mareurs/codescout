use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct SyncOpts {
    pub languages: Option<Vec<String>>,
    pub force_reindex: bool,
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

impl crate::retrieval::client::RetrievalClient {
    pub async fn sync_project(
        &self,
        project_id: &str,
        root: &Path,
        opts: SyncOpts,
    ) -> Result<SyncReport> {
        use crate::embed::ast_chunker::split_file;
        use crate::retrieval::drift::{diff_chunks, ChunkRef};
        use crate::retrieval::payload::{payload_to_map, CodePayload};

        // chunk=1200 was the universal sweet spot in the Phase 5.5 chunk×model matrix
        // (see docs/research/2026-05-06-retrieval-stack-benchmark.md). Override with
        // CODESCOUT_CHUNK_TARGET when retuning.
        const STACK_CHUNK_TARGET: usize = 1200;
        let chunk_target: usize = std::env::var("CODESCOUT_CHUNK_TARGET")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(STACK_CHUNK_TARGET);
        tracing::info!(
            chunk_target,
            force_reindex = opts.force_reindex,
            "retrieval sync starting"
        );

        let started = std::time::Instant::now();
        self.qdrant
            .ensure_collection(
                &self.config.collection("code_chunks"),
                self.config.model_dim as u64,
            )
            .await?;

        // 1. Walk files and chunk them — ignore crate respects .gitignore at all levels
        let mut local: Vec<(CodePayload, String)> = Vec::new();
        for entry in ignore::WalkBuilder::new(root)
            .hidden(false) // index tracked dotfiles; gitignore handles exclusions
            .build()
            .filter_map(|e| e.ok())
        {
            let ft = match entry.file_type() {
                Some(ft) => ft,
                None => continue,
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
            let chunks = split_file(&source, lang, path, chunk_target);
            for c in chunks {
                // Skip empty/whitespace-only chunks — embedders reject empty inputs
                if c.content.trim().is_empty() {
                    continue;
                }
                let hash = content_hash(&c.content);
                let chunk_id = format!("{project_id}:{}:{hash}", rel_path.display());
                let p = CodePayload {
                    project_id: project_id.into(),
                    file_path: rel_path.display().to_string(),
                    language: lang.into(),
                    start_line: c.start_line as i64,
                    end_line: c.end_line as i64,
                    ast_kind: String::new(),
                    ast_header: String::new(),
                    content: c.content.clone(),
                    content_hash: hash,
                    last_indexed_commit: String::new(),
                    chunk_id,
                };
                local.push((p, c.content));
            }
        }

        // 2. Fetch existing chunk refs from Qdrant for this project
        let server: Vec<ChunkRef> = self
            .qdrant
            .scroll_chunk_refs(&self.config.collection("code_chunks"), project_id)
            .await
            .unwrap_or_default();
        let local_refs: Vec<ChunkRef> = local
            .iter()
            .map(|(p, _)| ChunkRef {
                chunk_id: p.chunk_id.clone(),
                content_hash: p.content_hash.clone(),
            })
            .collect();
        // With force_reindex, ignore server state for the upsert set — re-embed every
        // local chunk. Delete set is still derived from diff so obsolete chunks are
        // pruned.
        let action = if opts.force_reindex {
            let diff = diff_chunks(&server, &local_refs);
            crate::retrieval::drift::DriftAction {
                to_upsert: local_refs.iter().map(|r| r.chunk_id.clone()).collect(),
                to_delete: diff.to_delete,
            }
        } else {
            diff_chunks(&server, &local_refs)
        };

        // 3. Embed + upsert new/changed chunks
        let upsert_set: std::collections::HashSet<&str> =
            action.to_upsert.iter().map(String::as_str).collect();
        let to_upsert: Vec<&(CodePayload, String)> = local
            .iter()
            .filter(|(p, c)| upsert_set.contains(p.chunk_id.as_str()) && !c.trim().is_empty())
            .collect();
        let texts: Vec<String> = to_upsert.iter().map(|(_, c)| c.clone()).collect();
        let embeds = if !texts.is_empty() {
            self.embedder.embed_batch(&texts).await?
        } else {
            vec![]
        };
        let added = to_upsert.len();
        if !to_upsert.is_empty() {
            let points: Vec<(
                String,
                std::collections::HashMap<String, qdrant_client::qdrant::Value>,
                crate::retrieval::embedder::EmbedOutput,
            )> = to_upsert
                .iter()
                .zip(embeds)
                .map(|((p, _), e)| (p.chunk_id.clone(), payload_to_map(p), e))
                .collect();
            self.qdrant
                .upsert_points(&self.config.collection("code_chunks"), &points)
                .await?;
        }

        // 4. Delete obsolete chunks
        let deleted = action.to_delete.len();
        if !action.to_delete.is_empty() {
            self.qdrant
                .delete_points(&self.config.collection("code_chunks"), &action.to_delete)
                .await?;
        }

        let elapsed_ms = started.elapsed().as_millis();
        tracing::info!(added, deleted, elapsed_ms, "retrieval sync finished");
        Ok(SyncReport {
            added,
            deleted,
            updated: 0,
            elapsed_ms,
        })
    }
}
