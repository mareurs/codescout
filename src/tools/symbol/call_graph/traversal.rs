use std::collections::{HashMap, HashSet, VecDeque};

use futures::future::try_join_all;

use crate::tools::symbol::call_edges::resolver::{Definition, Direction, Edge};

#[derive(Clone)]
pub struct TraversalConfig {
    pub max_depth: u32,
    pub max_edges: usize,
}

pub struct TraversalResult {
    pub edges: Vec<EdgeWithDepth>,
    pub truncated: bool,
    pub truncated_at_depth: Option<u32>,
    pub max_depth_reached: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeWithDepth {
    pub edge: Edge,
    pub depth: u32,
    pub paths: u32,
}

/// A queue key carries the actual declaration once semantic resolution placed it.
#[derive(Clone, Debug)]
pub struct TraversalNode {
    pub symbol: String,
    /// The provider's query position; do not replace it with an AST range start.
    pub definition: Option<Definition>,
    identity: Option<Definition>,
}

impl TraversalNode {
    pub fn named(symbol: &str) -> Self {
        Self {
            symbol: symbol.to_owned(),
            definition: None,
            identity: None,
        }
    }

    pub fn at(symbol: &str, definition: Definition) -> Self {
        Self {
            symbol: symbol.to_owned(),
            identity: Some(definition.clone()),
            definition: Some(definition),
        }
    }

    pub fn with_identity(mut self, identity: Definition) -> Self {
        self.identity = Some(identity);
        self
    }

    fn key(&self) -> (String, Option<Definition>) {
        (self.symbol.clone(), self.identity.clone())
    }
}

#[async_trait::async_trait]
pub trait OneHopResolver: Send + Sync {
    async fn one_hop(&self, symbol: &str, direction: Direction) -> anyhow::Result<Vec<Edge>>;

    async fn seed_node(&self, symbol: &str) -> anyhow::Result<TraversalNode> {
        Ok(TraversalNode::named(symbol))
    }

    async fn one_hop_at(
        &self,
        node: &TraversalNode,
        direction: Direction,
    ) -> anyhow::Result<Vec<Edge>> {
        self.one_hop(&node.symbol, direction).await
    }

    async fn next_node(
        &self,
        edge: &Edge,
        direction: Direction,
    ) -> anyhow::Result<Option<TraversalNode>> {
        let symbol = match direction {
            Direction::Callers => &edge.caller_sym,
            Direction::Callees => &edge.callee_sym,
        };
        Ok(Some(TraversalNode::named(symbol)))
    }
}

pub async fn bfs<R: OneHopResolver>(
    resolver: &R,
    seed_symbol: &str,
    direction: Direction,
    cfg: TraversalConfig,
) -> anyhow::Result<TraversalResult> {
    let seed = resolver.seed_node(seed_symbol).await?;
    let mut visited: HashSet<(String, Option<Definition>)> = HashSet::new();
    let mut current_level: VecDeque<TraversalNode> = VecDeque::new();
    current_level.push_back(seed.clone());
    visited.insert(seed.key());

    let mut all_edges: Vec<EdgeWithDepth> = Vec::new();
    let mut max_depth_reached = 0u32;
    let mut truncated = false;
    let mut truncated_at_depth = None;

    for depth in 1..=cfg.max_depth {
        let nodes: Vec<_> = current_level.drain(..).collect();
        let hop_results = try_join_all(
            nodes
                .iter()
                .map(|node| resolver.one_hop_at(node, direction.clone())),
        )
        .await?;
        let level_raw: Vec<_> = hop_results
            .into_iter()
            .flatten()
            .map(|edge| EdgeWithDepth {
                edge,
                depth,
                paths: 1,
            })
            .collect();
        let level_edges = dedupe_with_paths(level_raw.clone());

        // Preserve full-level caps, and do not resolve neighbors of a discarded level.
        if depth > 1 && !all_edges.is_empty() && all_edges.len() + level_edges.len() > cfg.max_edges
        {
            truncated = true;
            truncated_at_depth = Some(depth);
            break;
        }

        // A requested depth boundary needs no next-hop analysis or uncertainty warning.
        let next = if depth < cfg.max_depth {
            try_join_all(
                level_raw
                    .iter()
                    .map(|hop| resolver.next_node(&hop.edge, direction.clone())),
            )
            .await?
        } else {
            Vec::new()
        };
        all_edges.extend(level_edges);
        max_depth_reached = depth;
        for node in next.into_iter().flatten() {
            if visited.insert(node.key()) {
                current_level.push_back(node);
            }
        }
        if current_level.is_empty() {
            break;
        }
    }

    Ok(TraversalResult {
        edges: all_edges,
        truncated,
        truncated_at_depth,
        max_depth_reached,
    })
}

fn dedupe_with_paths(edges: Vec<EdgeWithDepth>) -> Vec<EdgeWithDepth> {
    // Dedupe on (caller_sym, callee_sym, file, line, col) — sum paths.
    let mut seen: HashMap<(String, String, String, u32, u32), usize> = HashMap::new();
    let mut result: Vec<EdgeWithDepth> = Vec::new();
    for e in edges {
        let key = (
            e.edge.caller_sym.clone(),
            e.edge.callee_sym.clone(),
            e.edge.file.to_string_lossy().into_owned(),
            e.edge.line,
            e.edge.col,
        );
        if let Some(idx) = seen.get(&key) {
            result[*idx].paths += 1;
        } else {
            seen.insert(key, result.len());
            result.push(e);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::tools::symbol::call_edges::resolver::EdgeSource;

    // FakeResolver uses String keys (Direction doesn't impl Hash).
    // Key format: "{sym}:{direction}" where direction is "callers" or "callees".
    struct FakeResolver {
        graph: HashMap<String, Vec<Edge>>,
    }

    impl FakeResolver {
        fn new() -> Self {
            Self {
                graph: HashMap::new(),
            }
        }

        fn add(&mut self, sym: &str, dir: Direction, edges: Vec<Edge>) {
            let key = resolver_key(sym, &dir);
            self.graph.insert(key, edges);
        }
    }

    fn resolver_key(sym: &str, dir: &Direction) -> String {
        match dir {
            Direction::Callers => format!("{}:callers", sym),
            Direction::Callees => format!("{}:callees", sym),
        }
    }

    fn edge(from: &str, to: &str) -> Edge {
        Edge {
            caller_sym: from.into(),
            callee_sym: to.into(),
            file: PathBuf::from("x.rs"),
            line: 0,
            col: 0,
            source: EdgeSource::Lsp,
            target: None,
        }
    }

    #[async_trait::async_trait]
    impl OneHopResolver for FakeResolver {
        async fn one_hop(&self, symbol: &str, direction: Direction) -> anyhow::Result<Vec<Edge>> {
            let key = resolver_key(symbol, &direction);
            Ok(self.graph.get(&key).cloned().unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn bfs_reaches_max_depth_then_stops() {
        // Graph: a calls b, b calls c, c calls d
        let mut r = FakeResolver::new();
        r.add("a", Direction::Callees, vec![edge("a", "b")]);
        r.add("b", Direction::Callees, vec![edge("b", "c")]);
        r.add("c", Direction::Callees, vec![edge("c", "d")]);
        let cfg = TraversalConfig {
            max_depth: 2,
            max_edges: 1000,
        };
        let res = bfs(&r, "a", Direction::Callees, cfg).await.unwrap();
        let callees: Vec<&str> = res
            .edges
            .iter()
            .map(|e| e.edge.callee_sym.as_str())
            .collect();
        assert!(callees.contains(&"b"), "should reach b at depth 1");
        assert!(callees.contains(&"c"), "should reach c at depth 2");
        assert!(!callees.contains(&"d"), "d is depth 3, should be cut off");
        assert_eq!(res.max_depth_reached, 2);
        assert!(!res.truncated);
    }

    #[tokio::test]
    async fn bfs_handles_cycle_without_infinite_loop() {
        // a -> b -> a (cycle)
        let mut r = FakeResolver::new();
        r.add("a", Direction::Callees, vec![edge("a", "b")]);
        r.add("b", Direction::Callees, vec![edge("b", "a")]);
        let cfg = TraversalConfig {
            max_depth: 5,
            max_edges: 1000,
        };
        let res = bfs(&r, "a", Direction::Callees, cfg).await.unwrap();
        // Should terminate, not loop forever; at most 2 unique edges
        assert!(res.edges.len() <= 2);
    }

    #[tokio::test]
    async fn bfs_depth_coherent_cap_preserves_full_levels() {
        // depth-1: 5 edges (a->b1 .. a->b5)
        // depth-2: each b has 10 edges → 50 depth-2 edges total
        // cap=20: depth-1 (5) fits, depth-2 (5+50=55) exceeds → truncate entire depth-2
        let mut r = FakeResolver::new();
        let depth1: Vec<Edge> = (1..=5).map(|i| edge("a", &format!("b{}", i))).collect();
        r.add("a", Direction::Callees, depth1);
        for i in 1..=5 {
            let d2: Vec<Edge> = (1..=10)
                .map(|j| edge(&format!("b{}", i), &format!("c{}_{}", i, j)))
                .collect();
            r.add(&format!("b{}", i), Direction::Callees, d2);
        }
        let cfg = TraversalConfig {
            max_depth: 3,
            max_edges: 20,
        };
        let res = bfs(&r, "a", Direction::Callees, cfg).await.unwrap();
        // All depth-1 edges present
        assert_eq!(
            res.edges.iter().filter(|e| e.depth == 1).count(),
            5,
            "all 5 depth-1 edges should be returned"
        );
        // Truncated at depth 2
        assert!(res.truncated, "should be truncated");
        assert_eq!(res.truncated_at_depth, Some(2));
        // No depth-2 edges
        assert!(
            res.edges.iter().all(|e| e.depth == 1),
            "depth-2 edges should be absent"
        );
    }

    #[tokio::test]
    async fn bfs_dedupes_parallel_paths() {
        // Resolver for "a" returns two identical edges (same caller+callee+file+line+col)
        let mut r = FakeResolver::new();
        let dup_edges = vec![edge("a", "b"), edge("a", "b")];
        r.add("a", Direction::Callees, dup_edges);
        let cfg = TraversalConfig {
            max_depth: 1,
            max_edges: 1000,
        };
        let res = bfs(&r, "a", Direction::Callees, cfg).await.unwrap();
        // After dedup, 1 edge with paths=2
        assert_eq!(res.edges.len(), 1);
        assert_eq!(res.edges[0].paths, 2);
    }

    #[tokio::test]
    async fn bfs_parallelizes_one_hop_within_level() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        struct ConcurrencyTrackingResolver {
            graph: HashMap<String, Vec<Edge>>,
            active: Arc<AtomicUsize>,
            max_active: Arc<AtomicUsize>,
        }

        #[async_trait::async_trait]
        impl OneHopResolver for ConcurrencyTrackingResolver {
            async fn one_hop(
                &self,
                symbol: &str,
                direction: Direction,
            ) -> anyhow::Result<Vec<Edge>> {
                let cur = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                self.max_active.fetch_max(cur, Ordering::SeqCst);
                for _ in 0..10 {
                    tokio::task::yield_now().await;
                }
                let key = resolver_key(symbol, &direction);
                let result = self.graph.get(&key).cloned().unwrap_or_default();
                self.active.fetch_sub(1, Ordering::SeqCst);
                Ok(result)
            }
        }

        // Depth-1: seed "a" expands to 5 leaf children. Depth-2 then fires 5
        // one_hop calls in parallel — that's the level where pre-fix BFS hung.
        let mut graph: HashMap<String, Vec<Edge>> = HashMap::new();
        let depth1: Vec<Edge> = (1..=5).map(|i| edge("a", &format!("b{}", i))).collect();
        graph.insert(resolver_key("a", &Direction::Callees), depth1);

        let r = ConcurrencyTrackingResolver {
            graph,
            active: Arc::new(AtomicUsize::new(0)),
            max_active: Arc::new(AtomicUsize::new(0)),
        };
        let max_active = r.max_active.clone();

        let cfg = TraversalConfig {
            max_depth: 2,
            max_edges: 1000,
        };
        bfs(&r, "a", Direction::Callees, cfg).await.unwrap();

        assert!(
            max_active.load(Ordering::SeqCst) > 1,
            "expected concurrent one_hop calls within a level — got max_active = {}",
            max_active.load(Ordering::SeqCst)
        );
    }
}
