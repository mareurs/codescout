//! `call_graph` — transitive call graph for a symbol.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::tools::symbol::call_edges::cache::EdgeCache;
use crate::tools::symbol::call_edges::resolver::{resolve_one_hop, Direction, Edge, EdgeSource};
use crate::tools::symbol::call_graph::traversal::{EdgeWithDepth, OneHopResolver, TraversalResult};
use crate::tools::{OutputForm, Tool, ToolContext};

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
    /// Project root for the workspace-wide tree-sitter fallback (Phase B).
    root: PathBuf,
    /// sym_name → (definition path, line, col)
    positions: Mutex<HashMap<String, (PathBuf, u32, u32)>>,
    /// Symbols whose definition could not be resolved via LSP or any
    /// tree-sitter fallback. Cached so BFS doesn't re-scan the workspace
    /// for the same unresolvable identifier on every hop.
    not_found: Mutex<std::collections::HashSet<String>>,
}

impl CachedResolver {
    /// Look up the definition location of `symbol`.
    ///
    /// Resolution order:
    /// 1. Positions map (cache from earlier hops or pre-seeded callsite).
    /// 2. LSP `workspace_symbols`.
    /// 3. Tree-sitter scan of files already in `positions` (Phase A —
    ///    `lookup_pos_via_ts_in_seed_files`).
    /// 4. Tree-sitter scan of project source files matching `lang`
    ///    (Phase B — `lookup_pos_via_ts_workspace`), bounded by file count.
    ///
    /// Returns `None` when the symbol cannot be located; the miss is
    /// remembered in `not_found` so BFS doesn't re-scan the workspace for
    /// the same identifier on subsequent hops.
    async fn lookup_pos(&self, symbol: &str) -> Option<(PathBuf, u32, u32)> {
        // Fast path: already known
        if let Some(pos) = self.positions.lock().unwrap().get(symbol).cloned() {
            return Some(pos);
        }

        // Negative cache: already tried and failed
        if self.not_found.lock().unwrap().contains(symbol) {
            return None;
        }

        // Slow path: ask the LSP
        if let Ok(ws_syms) = self.client.workspace_symbols(symbol).await {
            if let Some(found) = ws_syms
                .into_iter()
                .find(|s| s.name == symbol || s.name_path == symbol)
            {
                let pos = (found.file, found.start_line, found.start_col);
                self.positions
                    .lock()
                    .unwrap()
                    .insert(symbol.to_string(), pos.clone());
                return Some(pos);
            }
        }

        // Phase A fallback: tree-sitter scan of the seed file(s) already in
        // `positions`. Rescues depth-≥2 BFS in LSP-down scenarios when the
        // callee shares a file with a known caller.
        if let Some(pos) = self.lookup_pos_via_ts_in_seed_files(symbol) {
            self.positions
                .lock()
                .unwrap()
                .insert(symbol.to_string(), pos.clone());
            return Some(pos);
        }

        // Phase B fallback: bounded tree-sitter walk of the project root.
        // Picks up cross-file definitions when LSP is down. The not_found
        // cache below ensures we pay the walk cost at most once per missing
        // symbol per resolver lifetime.
        if let Some(pos) = self.lookup_pos_via_ts_workspace(symbol) {
            self.positions
                .lock()
                .unwrap()
                .insert(symbol.to_string(), pos.clone());
            return Some(pos);
        }

        self.not_found.lock().unwrap().insert(symbol.to_string());
        None
    }

    /// Tree-sitter same-file fallback: scan each unique file already present
    /// in `positions` for a top-level definition (or impl-method) whose name
    /// matches `symbol`. Returns the first match.
    ///
    /// Scope is intentionally narrow — workspace-wide search is Phase B.
    fn lookup_pos_via_ts_in_seed_files(&self, symbol: &str) -> Option<(PathBuf, u32, u32)> {
        // Collect unique candidate files. In practice all positions share a
        // file (BFS pre-seeds the seed only) but we iterate defensively.
        let files: Vec<PathBuf> = {
            let guard = self.positions.lock().unwrap();
            let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
            let mut out = Vec::new();
            for (_, (p, _, _)) in guard.iter() {
                if seen.insert(p.clone()) {
                    out.push(p.clone());
                }
            }
            out
        };

        for file in files {
            let lang = crate::ast::detect_language(&file)?;
            // Only languages with a tree-sitter grammar.
            if crate::ast::get_ts_language(lang).is_none() {
                continue;
            }
            let source = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let syms =
                match crate::ast::parser::extract_symbols_from_source(&source, Some(lang), &file) {
                    Ok(s) => s,
                    Err(_) => continue,
                };
            if let Some(found) = find_named_def(&syms, symbol) {
                return Some((found.file.clone(), found.start_line, found.start_col));
            }
        }
        None
    }

    /// Phase B fallback: bounded workspace walk.
    ///
    /// Scans `self.root` for source files matching `self.lang`, parses each
    /// with tree-sitter (`extract_symbols_from_source`), and returns the first
    /// top-level / impl-method definition whose name matches `symbol`.
    ///
    /// Caps file count at [`MAX_WORKSPACE_FILES_SCAN`] so a monorepo with
    /// 100k source files doesn't burn the BFS budget on one missing symbol.
    /// The caller's `not_found` cache ensures this walk runs at most once per
    /// missing symbol per resolver lifetime.
    fn lookup_pos_via_ts_workspace(&self, symbol: &str) -> Option<(PathBuf, u32, u32)> {
        /// Hard cap on files walked before giving up. Set so a typical
        /// project (1k–10k files) completes well under the MCP 60 s ceiling
        /// while a monorepo stops short instead of stalling the call.
        const MAX_WORKSPACE_FILES_SCAN: usize = 5_000;

        let walker = ignore::WalkBuilder::new(&self.root)
            .hidden(true)
            .git_ignore(true)
            .build();

        let mut scanned = 0usize;
        for entry in walker.flatten() {
            if scanned >= MAX_WORKSPACE_FILES_SCAN {
                break;
            }
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let path = entry.path();
            let Some(file_lang) = crate::ast::detect_language(path) else {
                continue;
            };
            if file_lang != self.lang {
                continue;
            }
            if crate::ast::get_ts_language(file_lang).is_none() {
                continue;
            }
            scanned += 1;
            let Ok(source) = std::fs::read_to_string(path) else {
                continue;
            };
            let Ok(syms) =
                crate::ast::parser::extract_symbols_from_source(&source, Some(file_lang), path)
            else {
                continue;
            };
            if let Some(found) = find_named_def(&syms, symbol) {
                return Some((found.file.clone(), found.start_line, found.start_col));
            }
        }
        None
    }
}

/// Recursively search a `SymbolInfo` tree for a definition whose `name` or
/// `name_path` matches `symbol`. Returns the first hit found in DFS order.
fn find_named_def<'a>(
    syms: &'a [crate::lsp::SymbolInfo],
    symbol: &str,
) -> Option<&'a crate::lsp::SymbolInfo> {
    for s in syms {
        if s.name == symbol || s.name_path == symbol {
            return Some(s);
        }
        if let Some(found) = find_named_def(&s.children, symbol) {
            return Some(found);
        }
    }
    None
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

        let edges = match resolve_one_hop(
            self.client.as_ref(),
            symbol,
            &path,
            line,
            col,
            &self.lang,
            direction,
        )
        .await
        {
            Ok(e) => e,
            Err(e) if e.downcast_ref::<crate::tools::RecoverableError>().is_some() => {
                // Non-seed node's resolver hit a recoverable limit (e.g. tree-sitter
                // callee LIMIT-001). Skip this hop so BFS can continue for the rest
                // of the graph, matching the existing lookup_pos behavior.
                vec![]
            }
            Err(e) => return Err(e),
        };
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
        "Transitive call graph for a symbol. `direction`: callers (blast radius), \
         callees (outbound), both. `max_depth=3` default. Edges tagged \
         `source=\"lsp\"` (authoritative) or `\"ts\"` (best-effort). For all refs \
         (not call-filtered) use `references`."
    }

    fn relevant_guide_topic(&self) -> Option<&str> {
        Some("progressive-disclosure")
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
        use crate::fs::{get_lsp_client, require_path_param, resolve_read_path};
        use crate::symbol::query::find_unique_symbol_by_name_path;
        use crate::tools::symbol::call_graph::traversal::{bfs, TraversalConfig};
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
        let seed_path = resolve_read_path(&ctx.agent, rel_path).await?;
        let (client, lang) = get_lsp_client(&ctx.agent, &*ctx.lsp, &seed_path).await?;

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
            tokio::task::spawn_blocking(move || {
                crate::tools::symbol::call_edges::cache::open_db(&root)
            })
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
                root: root.clone(),
                positions: Mutex::new(positions),
                not_found: Mutex::new(std::collections::HashSet::new()),
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
        let mut out = format!("call_graph for `{sym}`");

        for key in &["callers", "callees"] {
            let v = match result.get(*key) {
                Some(v) => v,
                None => continue,
            };

            if let Some(edges) = v.as_array() {
                // Full mode: edges[]. Group by file → ripgrep-style listing.
                use std::collections::BTreeMap;
                let mut by_file: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
                for e in edges {
                    let file = e.get("file").and_then(|f| f.as_str()).unwrap_or("?");
                    by_file.entry(file).or_default().push(e);
                }
                out.push_str(&format!(
                    "\n  {}: {} edges across {} files",
                    key,
                    edges.len(),
                    by_file.len()
                ));
                for (file, file_edges) in &by_file {
                    out.push_str(&format!("\n    {} ({})", file, file_edges.len()));
                    for e in file_edges {
                        let line = e.get("line").and_then(|l| l.as_u64()).unwrap_or(0);
                        let caller = e.get("caller").and_then(|c| c.as_str()).unwrap_or("?");
                        let callee = e.get("callee").and_then(|c| c.as_str()).unwrap_or("?");
                        let depth = e.get("depth").and_then(|d| d.as_u64()).unwrap_or(0);
                        let source = e.get("source").and_then(|s| s.as_str()).unwrap_or("?");
                        out.push_str(&format!(
                            "\n      {line:>5}: {caller} → {callee} (depth={depth}, {source})"
                        ));
                    }
                }
            } else if let Some(obj) = v.as_object() {
                // Compact aggregate: count + by_file + by_depth.
                let count = obj.get("count").and_then(|c| c.as_u64()).unwrap_or(0);
                let by_file = obj.get("by_file").and_then(|f| f.as_object());
                let n_files = by_file.map(|m| m.len()).unwrap_or(0);
                out.push_str(&format!("\n  {key}: {count} across {n_files} files"));
                if let Some(map) = by_file {
                    let mut entries: Vec<_> = map
                        .iter()
                        .filter_map(|(p, c)| c.as_u64().map(|n| (p.as_str(), n)))
                        .collect();
                    entries.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
                    for (path, n) in entries.iter().take(20) {
                        out.push_str(&format!("\n    {path}: {n}"));
                    }
                    if entries.len() > 20 {
                        out.push_str(&format!("\n    … {} more files", entries.len() - 20));
                    }
                }
                if let Some(by_depth) = obj.get("by_depth").and_then(|d| d.as_object()) {
                    let parts: Vec<String> = by_depth
                        .iter()
                        .filter_map(|(d, c)| c.as_u64().map(|n| format!("{d}={n}")))
                        .collect();
                    if !parts.is_empty() {
                        out.push_str(&format!("\n    by_depth: {}", parts.join(" ")));
                    }
                }
            }

            let truncated_key = format!("{key}_truncated_at_depth");
            if let Some(d) = result.get(&truncated_key).and_then(|x| x.as_u64()) {
                out.push_str(&format!("\n    (truncated at depth {d})"));
            }
        }

        if result
            .get("auto_promoted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            out.push_str("\n  (auto-promoted to full detail — small result)");
        }
        if let Some(d) = result.get("max_depth_reached").and_then(|v| v.as_u64()) {
            out.push_str(&format!("\n  max_depth_reached: {d}"));
        }
        Some(out)
    }

    fn output_form(&self) -> OutputForm {
        OutputForm::Text
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
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
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

    /// `format_compact` renders the full-mode (edges array) shape as a
    /// ripgrep-style listing: `file (count)\n  L: caller → callee (depth=D, source)`.
    #[test]
    fn format_compact_renders_full_mode_edges() {
        let result = json!({
            "symbol": "my_fn",
            "callers": [
                { "caller": "a::caller_one", "callee": "my_fn",
                  "file": "src/a.rs", "line": 12, "depth": 1, "source": "lsp", "paths": [] },
                { "caller": "a::caller_two", "callee": "my_fn",
                  "file": "src/a.rs", "line": 30, "depth": 2, "source": "ts",  "paths": [] },
                { "caller": "b::caller_three", "callee": "my_fn",
                  "file": "src/b.rs", "line": 7,  "depth": 1, "source": "lsp", "paths": [] }
            ],
            "max_depth_reached": 2
        });
        let compact = CallGraph.format_compact(&result).unwrap();
        assert!(compact.contains("call_graph for `my_fn`"));
        assert!(compact.contains("callers: 3 edges across 2 files"));
        assert!(compact.contains("src/a.rs (2)"));
        assert!(compact.contains("src/b.rs (1)"));
        assert!(compact.contains("12: a::caller_one → my_fn (depth=1, lsp)"));
        assert!(compact.contains("30: a::caller_two → my_fn (depth=2, ts)"));
        assert!(compact.contains("max_depth_reached: 2"));
    }

    #[test]
    fn call_graph_declares_output_form_text() {
        use crate::tools::OutputForm;
        assert_eq!(CallGraph.output_form(), OutputForm::Text);
    }

    /// LIMIT-001 Phase A: when LSP `workspace_symbols` returns empty (no LSP, or
    /// LSP doesn't know the symbol), `lookup_pos` should fall back to a
    /// tree-sitter scan of the seed's file to find a same-file definition.
    #[tokio::test]
    async fn lookup_pos_falls_back_to_ts_same_file_when_ws_symbols_empty() {
        // Rust fixture with three top-level fns: a, b, c.
        let src = "fn a() { b(); }\nfn b() { c(); }\nfn c() { a(); }\n";
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("cycle.rs");
        std::fs::write(&fixture, src).unwrap();

        // Mock LSP: workspace_symbols returns empty (default).
        let client = MockLspClient::new();
        let client_arc: Arc<dyn crate::lsp::ops::LspClientOps> = Arc::new(client);

        // In-memory sqlite for the edge cache (unused by lookup_pos, but the
        // struct requires a Connection).
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::tools::symbol::call_edges::cache::apply_schema(&conn);

        // Pre-seed `a` only — `b` must be discovered via the TS fallback.
        let mut positions: HashMap<String, (PathBuf, u32, u32)> = HashMap::new();
        positions.insert("a".to_string(), (fixture.clone(), 0, 3));

        let resolver = CachedResolver {
            conn: Arc::new(Mutex::new(conn)),
            project_id: "test".to_string(),
            client: client_arc,
            lang: "rust".to_string(),
            root: dir.path().to_path_buf(),
            positions: Mutex::new(positions),
            not_found: Mutex::new(std::collections::HashSet::new()),
        };

        let pos = resolver.lookup_pos("b").await;
        assert!(
            pos.is_some(),
            "lookup_pos should fall back to TS same-file scan when LSP returns empty"
        );
        let (path, line, _col) = pos.unwrap();
        assert_eq!(path, fixture);
        // `fn b()` is on the second line (0-indexed line 1).
        assert_eq!(line, 1, "expected line 1 for `fn b`");
    }

    /// LIMIT-001 Phase B: when LSP `workspace_symbols` is empty AND the
    /// callee is defined in a sibling file, `lookup_pos` must fall back to a
    /// bounded workspace tree-sitter walk to find it. Closes the cross-file
    /// edge-drop residual called out in
    /// `docs/issues/2026-05-01-call-graph-callees-ts-fallback.md`.
    #[tokio::test]
    async fn lookup_pos_falls_back_to_ts_workspace_when_def_in_sibling_file() {
        // Two-file fixture: `a()` in caller.rs references `b()` defined in
        // sibling.rs. BFS pre-seeds `a`; `b` must be discovered via the
        // workspace walk.
        let dir = tempfile::tempdir().unwrap();
        let caller = dir.path().join("caller.rs");
        let sibling = dir.path().join("sibling.rs");
        std::fs::write(&caller, "fn a() { b(); }\n").unwrap();
        std::fs::write(&sibling, "fn b() { /* lives here */ }\n").unwrap();

        // Mock LSP returns empty for workspace_symbols.
        let client = MockLspClient::new();
        let client_arc: Arc<dyn crate::lsp::ops::LspClientOps> = Arc::new(client);

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::tools::symbol::call_edges::cache::apply_schema(&conn);

        let mut positions: HashMap<String, (PathBuf, u32, u32)> = HashMap::new();
        positions.insert("a".to_string(), (caller.clone(), 0, 3));

        let resolver = CachedResolver {
            conn: Arc::new(Mutex::new(conn)),
            project_id: "test".to_string(),
            client: client_arc,
            lang: "rust".to_string(),
            root: dir.path().to_path_buf(),
            positions: Mutex::new(positions),
            not_found: Mutex::new(std::collections::HashSet::new()),
        };

        let pos = resolver.lookup_pos("b").await;
        assert!(
            pos.is_some(),
            "lookup_pos should walk the workspace and resolve `b` from sibling.rs"
        );
        let (path, line, _col) = pos.unwrap();
        assert_eq!(path, sibling, "expected sibling.rs as the definition file");
        assert_eq!(line, 0, "fn b is on the first line of sibling.rs");

        // Negative-cache invariant: looking up a symbol that exists nowhere
        // must cache the miss so a subsequent call short-circuits without
        // re-walking the workspace.
        let miss = resolver.lookup_pos("nonexistent_symbol_xyz").await;
        assert!(miss.is_none());
        assert!(
            resolver
                .not_found
                .lock()
                .unwrap()
                .contains("nonexistent_symbol_xyz"),
            "missing symbol must be recorded in the not_found cache"
        );
    }
} // mod tests

pub mod traversal;
