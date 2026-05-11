//! Deterministic test embedder. Produces orthogonal unit vectors derived
//! from a stable hash of the input text. Intentionally returns vectors
//! that are NEAR-ORTHOGONAL between distinct inputs so that any test
//! asserting on semantic similarity ranking will fail — forcing test
//! authors to assert on plumbing (chunk emission, vector storage, factory
//! wiring) rather than retrieval quality.

use crate::embedder::{Embedder, Embedding};
use anyhow::Result;
use std::hash::{Hash, Hasher};

pub struct MockEmbedder {
    dims: usize,
}

impl MockEmbedder {
    pub fn new(dims: usize) -> Self {
        assert!(dims > 0, "MockEmbedder requires dims > 0");
        Self { dims }
    }
}

#[async_trait::async_trait]
impl Embedder for MockEmbedder {
    fn dimensions(&self) -> usize {
        self.dims
    }

    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        Ok(texts.iter().map(|t| vector_for(t, self.dims)).collect())
    }
}

fn vector_for(text: &str, dims: usize) -> Embedding {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    let mut state = h.finish();
    let mut v = Vec::with_capacity(dims);
    for _ in 0..dims {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        // SplitMix64 finalizer to decorrelate output bits from the LCG state,
        // so distinct inputs produce near-orthogonal vectors even at low dims.
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        let bits = (z >> 32) as u32;
        let f = (bits as f32 / u32::MAX as f32) * 2.0 - 1.0;
        v.push(f);
    }
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_is_deterministic() {
        let e = MockEmbedder::new(8);
        let a = e.embed(&["hello"]).await.unwrap();
        let b = e.embed(&["hello"]).await.unwrap();
        assert_eq!(a, b, "same input must produce same vector");
    }

    #[tokio::test]
    async fn mock_vectors_are_unit_norm() {
        let e = MockEmbedder::new(16);
        let v = e.embed(&["anything"]).await.unwrap();
        let n: f32 = v[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((n - 1.0).abs() < 1e-5, "vector must be unit norm, got {n}");
    }

    #[tokio::test]
    async fn mock_distinct_inputs_have_low_similarity() {
        let e = MockEmbedder::new(32);
        let a = &e.embed(&["the quick brown fox"]).await.unwrap()[0];
        let b = &e.embed(&["a completely different sentence"]).await.unwrap()[0];
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        assert!(dot.abs() < 0.5, "distinct inputs must be near-orthogonal, got cos={dot}");
    }
}
