//! Semantic search and indexing tools.

mod index;
mod semantic_search;

pub use index::{Index, IndexProject, IndexStatus};
pub use semantic_search::SemanticSearch;

#[cfg(test)]
mod tests;
// The fixture's `[embeddings] url` needs the HTTP embed transport; the lean lane has none.
#[cfg(all(test, feature = "remote-embed"))]
mod worktree_store_tests;
