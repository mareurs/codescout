//! Data types for the embedding index.

use serde::{Deserialize, Serialize};

/// A ranked result from a semantic search query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Row id of the chunk in the `chunks` table.
    pub id: u64,
    pub file_path: String,
    pub language: String,
    pub content: String,
    pub start_line: usize,
    pub end_line: usize,
    /// Cosine similarity score in [0, 1]; higher is more relevant.
    pub score: f32,
    /// Source identifier: "project" or "lib:<name>"
    pub source: String,
    /// Project identifier this chunk belongs to (e.g. "mcp-server"). Empty string if unset.
    pub project_id: String,
}
