//! Embedding utilities used by the retrieval stack pipeline.
//!
//! The legacy sqlite-vec index (`embed::index`) along with its drift +
//! BM25 helpers were removed in L-01 step 8. What remains is the
//! language-aware AST chunker (`ast_chunker`), payload schemas
//! (`schema`), search-result fusion (`fusion`), and the preflight
//! scope-check used by `index(action='build')` (`preflight`). All
//! embedding/HTTP work now lives in the sibling `codescout-embed` crate
//! and the `retrieval::*` modules.

pub mod ast_chunker;
pub mod fusion;
pub mod preflight;
pub mod schema;

pub use codescout_embed::{chunk_markdown, split, split_markdown, RawChunk};
pub use codescout_embed::{chunk_size_for_model, create_embedder, create_embedder_with_config};
pub use codescout_embed::{Embedder, Embedding};

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
