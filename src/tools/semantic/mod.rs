//! Semantic search and indexing tools.

mod index;
mod semantic_search;

pub use index::{Index, IndexProject, IndexStatus};
pub use semantic_search::SemanticSearch;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod worktree_store_tests;
