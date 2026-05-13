//! One-shot migration of legacy sqlite-vec memories into Qdrant.
//!
//! Reads the `memories` + `memory_anchors` tables from
//! `.codescout/embeddings.db` (the legacy storage), re-embeds every memory's
//! content with the active HTTP embedder, and upserts via
//! [`crate::memory::semantic_store::SemanticMemoryStore`].
//!
//! Re-embedding (vs copying the stored vector) is deliberate: legacy memories
//! were embedded with whatever local model was active at write-time, and the
//! Qdrant `memories` collection is bound to the current HTTP embedder's
//! dimension. Re-embedding is the only correctness-preserving option.
//!
//! Designed to be deletable: depends only on `rusqlite` + the
//! `SemanticMemoryStore` trait, not on the `embed::index` module that step 8
//! will remove.

pub mod memories;
