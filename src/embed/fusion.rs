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
    use std::collections::HashMap;

    let mut scores: HashMap<u64, f32> = HashMap::new();

    // Vector leg: rank = 1-indexed position in the slice
    for (i, sr) in vector.iter().enumerate() {
        let rank = (i + 1) as f32;
        *scores.entry(sr.id).or_insert(0.0) += 1.0 / (k + rank);
    }

    // BM25 leg: rank already stored in BM25Result
    for r in bm25 {
        let rank = r.rank as f32;
        *scores.entry(r.chunk_id).or_insert(0.0) += 1.0 / (k + rank);
    }

    // Sort by RRF score descending, break ties by id for stability
    let mut ranked: Vec<(u64, f32)> = scores.into_iter().collect();
    ranked.sort_by(|(id_a, sa), (id_b, sb)| {
        sb.partial_cmp(sa)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| id_a.cmp(id_b))
    });

    ranked.into_iter().map(|(id, _)| id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::schema::SearchResult;

    fn sr(id: u64, score: f32) -> SearchResult {
        SearchResult {
            id,
            file_path: format!("f{}.rs", id),
            language: "rust".into(),
            content: format!("c{}", id),
            start_line: 0,
            end_line: 1,
            score,
            source: "project".into(),
            project_id: "root".into(),
        }
    }

    fn bm(id: u64, rank: usize) -> BM25Result {
        BM25Result {
            chunk_id: id,
            score: 1.0,
            rank,
        }
    }

    #[test]
    fn rrf_promotes_dual_hit_above_single_leg() {
        // vector: [1(rank1), 2(rank2)]; bm25: [2(rank1), 3(rank2)]
        // id=1: 1/(60+1)            = 0.01639 (vector only)
        // id=2: 1/(60+2)+1/(60+1)   = 0.01613+0.01639 = 0.03252 (both legs)
        // id=3: 1/(60+2)            = 0.01613 (bm25 only)
        // expected order: [2, 1, 3]
        let vector = vec![sr(1, 0.9), sr(2, 0.8)];
        let bm25 = vec![bm(2, 1), bm(3, 2)];
        let fused = rrf_fuse(&vector, &bm25, 60.0);
        assert_eq!(fused[0], 2, "dual hit should rank first");
        assert_eq!(fused[1], 1);
        assert_eq!(fused[2], 3);
    }

    #[test]
    fn rrf_bm25_only_hit_appears_in_output() {
        let vector = vec![sr(1, 0.9), sr(2, 0.8)];
        let bm25 = vec![bm(99, 1), bm(1, 2)];
        let fused = rrf_fuse(&vector, &bm25, 60.0);
        assert!(fused.contains(&99), "BM25-only chunk must appear");
    }

    #[test]
    fn rrf_empty_bm25_preserves_vector_order() {
        let vector = vec![sr(1, 0.9), sr(2, 0.8), sr(3, 0.7)];
        let fused = rrf_fuse(&vector, &[], 60.0);
        assert_eq!(fused, vec![1, 2, 3]);
    }

    #[test]
    fn rrf_empty_vector_preserves_bm25_order() {
        let bm25 = vec![bm(1, 1), bm(2, 2)];
        let fused = rrf_fuse(&[], &bm25, 60.0);
        assert_eq!(fused, vec![1, 2]);
    }
}
