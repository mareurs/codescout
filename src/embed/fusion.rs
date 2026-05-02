use crate::embed::schema::SearchResult;

#[derive(Debug, Clone)]
pub struct BM25Result {
    pub chunk_id: u64,
    pub score: f32,
    pub rank: usize,
}

/// Fuse vector and BM25 ranked lists via Reciprocal Rank Fusion.
/// Returns chunk_ids in descending RRF score order.
/// k=60 is the canonical constant — set it lower (e.g. 1.0) to amplify rank differences.
pub fn rrf_fuse(vector: &[SearchResult], bm25: &[BM25Result], k: f32) -> Vec<u64> {
    todo!("Task 5")
}

#[cfg(test)]
mod tests {
    use super::*;
}
