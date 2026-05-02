//! `call_graph` — transitive call graph for a symbol.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::symbol::call_edges::cache::EdgeCache;
use crate::tools::symbol::call_edges::resolver::{resolve_one_hop, Direction, Edge, EdgeSource};
use crate::tools::symbol::call_graph::traversal::{EdgeWithDepth, OneHopResolver, TraversalResult};
use crate::tools::{Tool, ToolContext};

/// Cache-checking one-hop resolver.
///
/// Checks the SQLite edge cache before calling LSP/ts. On a miss, calls
/// `resolve_one_hop`, upserts the results, and returns them.
///
/// `positions` maps symbol names to their definition (path, line, col) so
/// that non-seed symbols discovered during BFS traversal can be resolved.
/// The seed position is pre-populated by the caller; subsequent positions are
/// discovered lazily via `workspace_symbols`.
struct CachedResolver {
    conn: Arc<Mutex<rusqlite::Connection>>,
    project_id: String,
    client: Arc<dyn crate::lsp::ops::LspClientOps>,
    lang: String,
    /// sym_name → (definition path, line, col)
    positions: Mutex<HashMap<String, (PathBuf, u32, u32)>>,
}

impl CachedResolver {
    /// Look up the definition location of `symbol`.
    ///
    /// Checks the positions map first; on a miss, queries `workspace_symbols`
    /// to discover it. Returns `None` when the symbol cannot be located.
    async fn lookup_pos(&self, symbol: &str) -> Option<(PathBuf, u32, u32)> {
        // Fast path: already known
        if let Some(pos) = self.positions.lock().unwrap().get(symbol).cloned() {
            return Some(pos);
        }

        // Slow path: ask the LSP
        let ws_syms = self.client.workspace_symbols(symbol).await.ok()?;
        let found = ws_syms
            .into_iter()
            .find(|s| s.name == symbol || s.name_path == symbol)?;
        let pos = (found.file, found.start_line, found.start_col);
        self.positions
            .lock()
            .unwrap()
            .insert(symbol.to_string(), pos.clone());
        Some(pos)
    }
}

#[async_trait]
impl OneHopResolver for CachedResolver {
    async fn one_hop(&self, symbol: &str, direction: Direction) -> anyhow::Result<Vec<Edge>> {
        // Cache hit? Lock conn briefly to query, then drop the lock.
        let cached = {
            let conn = self.conn.lock().unwrap();
            let cache = EdgeCache::new(&conn, &self.project_id);
            match direction {
                Direction::Callers => cache.lookup_callers(symbol),
                Direction::Callees => cache.lookup_callees(symbol),
            }?
        };
        if !cached.is_empty() {
            return Ok(cached);
        }

        // Cache miss: resolve the symbol's definition location
        let (path, line, col) = match self.lookup_pos(symbol).await {
            Some(p) => p,
            None => {
                // Can't locate the symbol — skip rather than error so BFS
                // can continue for the rest of the graph
                return Ok(vec![]);
            }
        };

        let edges = resolve_one_hop(
            self.client.as_ref(),
            symbol,
            &path,
            line,
            col,
            &self.lang,
            direction,
        )
        .await?;
        {
            let conn = self.conn.lock().unwrap();
            let cache = EdgeCache::new(&conn, &self.project_id);
            cache.upsert(&edges)?;
        }
        Ok(edges)
    }
}

/// Format BFS results for tool output.
fn format_output(
    symbol: &str,
    by_dir: &HashMap<&str, TraversalResult>,
    render_full: bool,
    auto_promote: bool,
) -> Value {
    let mut out = json!({ "symbol": symbol });

    for (key, res) in by_dir {
        if render_full {
            let edges_json: Vec<Value> = res
                .edges
                .iter()
                .map(|e: &EdgeWithDepth| {
                    json!({
                        "caller": e.edge.caller_sym,
                        "callee": e.edge.callee_sym,
                        "file":   e.edge.file.to_string_lossy(),
                        "line":   e.edge.line + 1,
                        "depth":  e.depth,
                        "source": match e.edge.source {
                            EdgeSource::Lsp => "lsp",
                            EdgeSource::Ts  => "ts",
                        },
                        "paths":  e.paths,
                    })
                })
                .collect();
            out[key] = json!(edges_json);
        } else {
            let mut by_file: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();
            let mut by_depth: std::collections::BTreeMap<u32, usize> =
                std::collections::BTreeMap::new();
            for e in &res.edges {
                *by_file
                    .entry(e.edge.file.to_string_lossy().into_owned())
                    .or_default() += 1;
                *by_depth.entry(e.depth).or_default() += 1;
            }
            out[*key] = json!({
                "count":    res.edges.len(),
                "by_file":  by_file,
                "by_depth": by_depth,
            });
        }
        if res.truncated {
            out[format!("{}_truncated_at_depth", key)] = json!(res.truncated_at_depth);
        }
    }

    if auto_promote {
        out["auto_promoted"] = json!(true);
    }
    let max_d = by_dir
        .values()
        .map(|r| r.max_depth_reached)
        .max()
        .unwrap_or(0);
    out["max_depth_reached"] = json!(max_d);
    out
}

pub struct CallGraph;

#[async_trait::async_trait]
impl Tool for CallGraph {
    fn name(&self) -> &str {
        "call_graph"
    }

    fn description(&self) -> &str {
        "Transitive call graph for a symbol. `direction=callers` for blast radius, \
         `callees` for outbound flow, `both` for both. `max_depth` (default 3) bounds \
         traversal. Edges tagged `source: \"lsp\"` (authoritative) or `\"ts\"` \
         (tree-sitter, best-effort). Use `references` for ALL refs (not call-filtered)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol":       { "type": "string", "description": "Symbol identifier. Plain method: 'MyStruct/method'. Trait impl method: 'impl Trait for Struct/method'." },
                "path":         { "type": "string", "description": "File containing the symbol (required for seed resolution)" },
                "direction":    { "enum": ["callers", "callees", "both"], "default": "callers" },
                "max_depth":    { "type": "integer", "default": 3, "description": "Max BFS depth (capped at 10)" },
                "detail_level": { "type": "string", "enum": ["exploring", "full"], "default": "exploring" }
            },
            "required": ["symbol", "path"]
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use crate::symbol::query::find_unique_symbol_by_name_path;
        use crate::tools::symbol::call_graph::traversal::{bfs, TraversalConfig};
        use crate::tools::symbol::path_helpers::{
            get_lsp_client, require_path_param, resolve_read_path,
        };
        use crate::tools::{require_str_param, RecoverableError};

        let symbol = require_str_param(&input, "symbol")?;
        let rel_path = require_path_param(&input)?;
        let direction_str = input["direction"].as_str().unwrap_or("callers");
        let max_depth = input["max_depth"].as_u64().unwrap_or(3).min(10) as u32;
        let detail_full = input["detail_level"].as_str() == Some("full");

        let directions: Vec<Direction> = match direction_str {
            "callers" => vec![Direction::Callers],
            "callees" => vec![Direction::Callees],
            "both" => vec![Direction::Callers, Direction::Callees],
            other => {
                return Err(RecoverableError::with_hint(
                    format!(
                        "invalid direction '{}'; use callers, callees, or both",
                        other
                    ),
                    "direction must be one of: callers, callees, both",
                )
                .into())
            }
        };

        // Resolve seed path and language
        let seed_path = resolve_read_path(ctx, rel_path).await?;
        let (client, lang) = get_lsp_client(ctx, &seed_path).await?;

        // Find the seed symbol's position in the file
        let doc_symbols = client.document_symbols(&seed_path, &lang).await?;
        let sym_info = find_unique_symbol_by_name_path(&doc_symbols, symbol)?;
        let seed_line = sym_info.start_line;
        let seed_col = sym_info.start_col;

        // Get project root and project_id for the edge cache.
        // project_id must be derived via the shared helper so it matches what
        // invalidate_call_edges uses — they must write and delete with the same key.
        let root = ctx.agent.require_project_root().await?;
        let project_id = ctx.agent.call_edges_project_id().await;

        // Open the DB on a blocking thread — open_db does filesystem I/O
        // (mkdir, sqlite open, PRAGMA/DDL migrations) that must not run on the
        // async executor.
        let conn = {
            let root = root.clone();
            tokio::task::spawn_blocking(move || crate::embed::index::open_db(&root))
                .await
                .map_err(|e| anyhow::anyhow!("spawn_blocking panicked: {e}"))??
        };
        let conn = Arc::new(Mutex::new(conn));

        let cap = if detail_full { 500 } else { 200 };
        let cfg = TraversalConfig {
            max_depth,
            max_edges: cap,
        };

        let mut by_dir: HashMap<&str, TraversalResult> = HashMap::new();

        for direction in &directions {
            let mut positions: HashMap<String, (PathBuf, u32, u32)> = HashMap::new();
            positions.insert(symbol.to_string(), (seed_path.clone(), seed_line, seed_col));

            let resolver = CachedResolver {
                conn: Arc::clone(&conn),
                project_id: project_id.clone(),
                client: Arc::clone(&client),
                lang: lang.clone(),
                positions: Mutex::new(positions),
            };

            let result = bfs(&resolver, symbol, direction.clone(), cfg.clone()).await?;
            let key = match direction {
                Direction::Callers => "callers",
                Direction::Callees => "callees",
            };
            by_dir.insert(key, result);
        }

        let total_edges: usize = by_dir.values().map(|r| r.edges.len()).sum();
        let auto_promote = total_edges <= 30;
        let render_full = detail_full || auto_promote;

        Ok(format_output(symbol, &by_dir, render_full, auto_promote))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        let sym = result.get("symbol")?.as_str()?;
        let mut parts = vec![format!("call_graph for `{}`", sym)];
        for key in &["callers", "callees"] {
            if let Some(v) = result.get(key) {
                if let Some(count) = v.get("count").and_then(|c| c.as_u64()) {
                    let n_files = v
                        .get("by_file")
                        .and_then(|f| f.as_object())
                        .map(|m| m.len())
                        .unwrap_or(0);
                    parts.push(format!("{}: {} across {} files", key, count, n_files));
                } else if let Some(arr) = v.as_array() {
                    parts.push(format!("{}: {}", key, arr.len()));
                }
            }
        }
        Some(parts.join("; "))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresLsp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lsp::{MockLspClient, MockLspProvider};

    // ── helpers ──────────────────────────────────────────────────────────────────

    fn mock_lsp_provider(client: MockLspClient) -> std::sync::Arc<dyn crate::lsp::LspProvider> {
        MockLspProvider::with_client(client)
    }

    fn test_buf() -> std::sync::Arc<crate::tools::output_buffer::OutputBuffer> {
        std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20))
    }

    async fn ctx_with_lsp(lsp: std::sync::Arc<dyn crate::lsp::LspProvider>) -> ToolContext {
        use std::sync::Arc;
        let agent = crate::agent::Agent::new(None).await.unwrap();
        ToolContext {
            agent,
            lsp,
            output_buffer: test_buf(),
            progress: None,
            peer: None,
            section_coverage: Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        }
    }

    // ── tests ─────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn call_graph_invalid_direction_returns_recoverable_error() {
        let ctx = ctx_with_lsp(mock_lsp_provider(MockLspClient::new())).await;
        let err = CallGraph
            .call(
                json!({ "symbol": "foo", "path": "src/lib.rs", "direction": "sideways" }),
                &ctx,
            )
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid direction"),
            "expected 'invalid direction' in: {msg}"
        );
        assert!(msg.contains("sideways"), "expected bad value in: {msg}");
    }

    /// ≤30 edges → `auto_promoted=true`, full edge list rendered without `detail_level=full`.
    #[test]
    fn format_output_auto_promotes_small_results() {
        use crate::tools::symbol::call_edges::resolver::{Edge, EdgeSource};
        use crate::tools::symbol::call_graph::traversal::{EdgeWithDepth, TraversalResult};
        use std::path::PathBuf;

        let edge = EdgeWithDepth {
            edge: Edge {
                caller_sym: "b".to_string(),
                callee_sym: "a".to_string(),
                file: PathBuf::from("src/lib.rs"),
                line: 5,
                col: 0,
                source: EdgeSource::Lsp,
            },
            depth: 1,
            paths: 1,
        };

        let mut by_dir = HashMap::new();
        by_dir.insert(
            "callers",
            TraversalResult {
                edges: vec![edge],
                truncated: false,
                truncated_at_depth: None,
                max_depth_reached: 1,
            },
        );

        // render_full=true (auto_promote triggers this in call()), auto_promote=true
        let result = format_output("a", &by_dir, true, true);
        assert_eq!(result["auto_promoted"], json!(true));
        let callers = result["callers"].as_array().unwrap();
        assert_eq!(callers.len(), 1);
        assert_eq!(callers[0]["caller"], json!("b"));
        assert_eq!(callers[0]["callee"], json!("a"));
        assert_eq!(callers[0]["line"], json!(6)); // 0-indexed line + 1 for display
        assert_eq!(callers[0]["source"], json!("lsp"));
        assert_eq!(result["max_depth_reached"], json!(1));
    }

    /// >30 edges → compact summary with count/by_file/by_depth, no auto-promote.
    #[test]
    fn format_output_compact_summary_for_large_results() {
        use crate::tools::symbol::call_edges::resolver::{Edge, EdgeSource};
        use crate::tools::symbol::call_graph::traversal::{EdgeWithDepth, TraversalResult};
        use std::path::PathBuf;

        let edges: Vec<EdgeWithDepth> = (0u32..31)
            .map(|i| EdgeWithDepth {
                edge: Edge {
                    caller_sym: format!("caller_{}", i),
                    callee_sym: "target".to_string(),
                    file: PathBuf::from(format!("src/file_{}.rs", i % 3)),
                    line: i,
                    col: 0,
                    source: EdgeSource::Ts,
                },
                depth: 1,
                paths: 1,
            })
            .collect();

        let mut by_dir = HashMap::new();
        by_dir.insert(
            "callers",
            TraversalResult {
                edges,
                truncated: false,
                truncated_at_depth: None,
                max_depth_reached: 1,
            },
        );

        let result = format_output("target", &by_dir, false, false);
        assert!(
            result.get("auto_promoted").is_none(),
            "should not auto-promote"
        );
        let callers = &result["callers"];
        assert_eq!(callers["count"], json!(31));
        let by_file = callers["by_file"].as_object().unwrap();
        assert_eq!(by_file.len(), 3, "expected 3 distinct files");
        let by_depth = callers["by_depth"].as_object().unwrap();
        assert!(by_depth.contains_key("1"), "depth 1 should appear");
    }

    /// `format_compact` produces a readable one-liner with count and file count.
    #[test]
    fn format_compact_renders_count_and_files() {
        let result = json!({
            "symbol": "my_fn",
            "callers": {
                "count": 7,
                "by_file": { "a.rs": 4, "b.rs": 3 },
                "by_depth": { "1": 7 }
            },
            "max_depth_reached": 1
        });
        let compact = CallGraph.format_compact(&result).unwrap();
        assert!(compact.contains("my_fn"), "missing symbol: {compact}");
        assert!(compact.contains("callers"), "missing key: {compact}");
        assert!(compact.contains("7"), "missing count: {compact}");
        assert!(compact.contains("2 files"), "missing file count: {compact}");
    }
} // mod tests

pub mod traversal;
