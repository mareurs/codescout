//! Embedding utilities used by the retrieval stack pipeline.
//!
//! The legacy sqlite-vec index (`embed::index`) with its drift + BM25
//! helpers were removed in L-01 step 8; the in-process RRF fusion
//! (`fusion`) was removed once Qdrant-native RRF (`retrieval::qdrant`)
//! replaced it. What remains is the language-aware AST chunker
//! (`ast_chunker`), payload schemas (`schema`), and the preflight
//! scope-check used by `index(action='build')` (`preflight`). All
//! embedding/HTTP work now lives in the sibling `codescout-embed` crate
//! and the `retrieval::*` modules.

pub mod ast_chunker;
pub mod preflight;
pub mod schema;

pub use codescout_embed::{chunk_markdown, split, split_markdown, RawChunk};
pub use codescout_embed::{chunk_size_for_model, create_embedder, create_embedder_with_config};
pub use codescout_embed::{Embedder, Embedding};

/// Map a file extension to the indexer's language tag, or `None` if the
/// extension is not indexed.
///
/// **Single source of truth** for "which files get embedded", shared by the
/// indexing walk (`crate::retrieval::sync::RetrievalClient::sync_project`) and
/// the preflight scope estimate (`preflight::check_index_scope`). Keeping both
/// on this one function is what guarantees the guard counts exactly the files
/// the indexer will embed — the two previously diverged (the guard walked a
/// different tree and counted every file regardless of extension; see
/// `docs/issues/2026-06-02-preflight-sync-walker-divergence.md`).
///
/// Matching is case-sensitive on the raw extension, mirroring `sync_project`'s
/// historical behaviour (e.g. `README.MD` is *not* treated as markdown here).
pub fn lang_for_ext(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "rs" => "rust",
        "py" => "python",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "go" => "go",
        "java" => "java",
        "kt" => "kotlin",
        "md" | "mdx" => "markdown",
        "sh" | "bash" => "shell",
        "toml" => "toml",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {

    #[test]
    fn unknown_prefix_returns_error() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(super::create_embedder("bogus:model"));
        assert!(result.is_err(), "unknown prefix must error");
    }

    #[test]
    fn chunk_size_for_local_model_uses_table() {
        let sz = super::chunk_size_for_model("local:JinaEmbeddingsV2BaseCode");
        assert!(sz > 0);
    }
}
