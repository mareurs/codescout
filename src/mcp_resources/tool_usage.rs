//! `doctor://tool-usage` — MCP resource reporting per-tool call counts,
//! error/overflow rates, and prune candidates (tools with low or zero usage).
//!
//! Powers the post-bundle-(a) idea from `docs/trackers/mcp-integration-ideas-2026-04.md`
//! (#7): quantify what the token-diet actually bought us and surface rarely-used
//! tools for the next prompt-surface review.
//!
//! Returns JSON with fields: `window`, `total_calls`, `tools[]`,
//! `prune_candidates[]` (known tools called < LOW_CALL_THRESHOLD times),
//! `unused_tools[]` (registered tools with zero calls in the window).

use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};
use async_trait::async_trait;
use serde::Serialize;

pub const URI: &str = "doctor://tool-usage";

/// Tools called < this many times in the window are flagged as prune
/// candidates for the next prompt-surface review.
const LOW_CALL_THRESHOLD: i64 = 5;

/// Default analysis window when the resource is read.
const DEFAULT_WINDOW: &str = "30d";

/// Source of truth for tool-usage data. Trait-based so tests can supply
/// deterministic stats without touching the real `usage.db`.
#[async_trait]
pub trait UsageSource: Send + Sync {
    /// Return the usage snapshot for the given window (e.g. "30d", "7d", "1h").
    ///
    /// Implementations that depend on external state (e.g. a SQLite database
    /// that may not exist yet) should return an empty snapshot rather than an
    /// error — the resource must always be readable.
    async fn snapshot(&self, window: &str) -> UsageSnapshot;

    /// Return the list of all currently-registered tool names. Used to
    /// detect tools with zero calls in the window.
    async fn registered_tools(&self) -> Vec<String>;
}

/// Usage snapshot as reported by a [`UsageSource`].
#[derive(Debug, Default, Clone)]
pub struct UsageSnapshot {
    pub total_calls: i64,
    pub by_tool: Vec<ToolCallStats>,
}

/// Per-tool call statistics — maps to the shape of [`crate::usage::db::ToolStats`].
#[derive(Debug, Clone)]
pub struct ToolCallStats {
    pub tool: String,
    pub calls: i64,
    pub errors: i64,
    pub overflows: i64,
    pub error_rate_pct: f64,
    pub overflow_rate_pct: f64,
    pub p50_ms: i64,
    pub p99_ms: i64,
}

#[derive(Debug, Serialize)]
struct ReportEntry<'a> {
    name: &'a str,
    calls: i64,
    errors: i64,
    overflows: i64,
    error_rate_pct: f64,
    overflow_rate_pct: f64,
    p50_ms: i64,
    p99_ms: i64,
}

#[derive(Debug, Serialize)]
struct Report<'a> {
    window: &'a str,
    low_call_threshold: i64,
    total_calls: i64,
    tools: Vec<ReportEntry<'a>>,
    /// Known tools whose call count is below `low_call_threshold`.
    prune_candidates: Vec<&'a str>,
    /// Registered tools not present in usage stats at all (zero calls).
    unused_tools: Vec<String>,
}

pub struct ToolUsageProvider<S: UsageSource> {
    source: S,
}

impl<S: UsageSource> ToolUsageProvider<S> {
    pub fn new(source: S) -> Self {
        Self { source }
    }
}

#[async_trait]
impl<S: UsageSource + 'static> ResourceProvider for ToolUsageProvider<S> {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        vec![ResourceDescriptor {
            uri: URI.into(),
            name: "tool-usage".into(),
            description: Some(
                "Per-tool call counts, error/overflow rates, and prune candidates.".into(),
            ),
            mime_type: "application/json".into(),
        }]
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        if uri != URI {
            return Err(ResourceError::NotFound(uri.into()));
        }
        let window = DEFAULT_WINDOW;
        let snap = self.source.snapshot(window).await;
        let registered = self.source.registered_tools().await;

        // Prune candidates: tools with < LOW_CALL_THRESHOLD calls (but > 0 —
        // zero-call tools are separately flagged as `unused_tools`).
        let prune_candidates: Vec<&str> = snap
            .by_tool
            .iter()
            .filter(|t| t.calls > 0 && t.calls < LOW_CALL_THRESHOLD)
            .map(|t| t.tool.as_str())
            .collect();

        // Unused: registered names that never appear in the usage stats.
        let seen: std::collections::HashSet<&str> =
            snap.by_tool.iter().map(|t| t.tool.as_str()).collect();
        let mut unused: Vec<String> = registered
            .into_iter()
            .filter(|n| !seen.contains(n.as_str()))
            .collect();
        unused.sort();

        let tools: Vec<ReportEntry<'_>> = snap
            .by_tool
            .iter()
            .map(|t| ReportEntry {
                name: &t.tool,
                calls: t.calls,
                errors: t.errors,
                overflows: t.overflows,
                error_rate_pct: t.error_rate_pct,
                overflow_rate_pct: t.overflow_rate_pct,
                p50_ms: t.p50_ms,
                p99_ms: t.p99_ms,
            })
            .collect();

        let report = Report {
            window,
            low_call_threshold: LOW_CALL_THRESHOLD,
            total_calls: snap.total_calls,
            tools,
            prune_candidates,
            unused_tools: unused,
        };

        let text = serde_json::to_string_pretty(&report)
            .map_err(|e| ResourceError::Other(anyhow::Error::from(e)))?;
        Ok(ResourceBytes::Text(text))
    }
}

/// Adapter that pulls usage stats from [`crate::agent::Agent`] + the registered tools.
pub struct AgentUsageSource {
    agent: crate::agent::Agent,
    tools: Vec<std::sync::Arc<dyn crate::tools::Tool>>,
}

impl AgentUsageSource {
    pub fn new(
        agent: crate::agent::Agent,
        tools: Vec<std::sync::Arc<dyn crate::tools::Tool>>,
    ) -> Self {
        Self { agent, tools }
    }
}

#[async_trait]
impl UsageSource for AgentUsageSource {
    async fn snapshot(&self, window: &str) -> UsageSnapshot {
        let Some(root) = self.agent.project_root().await else {
            return UsageSnapshot::default();
        };
        // The usage DB may not yet exist — return empty rather than error.
        let conn = match crate::usage::db::open_db(&root) {
            Ok(c) => c,
            Err(_) => return UsageSnapshot::default(),
        };
        let stats = match crate::usage::db::query_stats(&conn, window) {
            Ok(s) => s,
            Err(_) => return UsageSnapshot::default(),
        };
        UsageSnapshot {
            total_calls: stats.total_calls,
            by_tool: stats
                .by_tool
                .into_iter()
                .map(|t| ToolCallStats {
                    tool: t.tool,
                    calls: t.calls,
                    errors: t.errors,
                    overflows: t.overflows,
                    error_rate_pct: t.error_rate_pct,
                    overflow_rate_pct: t.overflow_rate_pct,
                    p50_ms: t.p50_ms,
                    p99_ms: t.p99_ms,
                })
                .collect(),
        }
    }

    async fn registered_tools(&self) -> Vec<String> {
        self.tools.iter().map(|t| t.name().to_string()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSource {
        snap: UsageSnapshot,
        registered: Vec<String>,
    }

    #[async_trait]
    impl UsageSource for FakeSource {
        async fn snapshot(&self, _window: &str) -> UsageSnapshot {
            self.snap.clone()
        }
        async fn registered_tools(&self) -> Vec<String> {
            self.registered.clone()
        }
    }

    fn make_stats(tool: &str, calls: i64) -> ToolCallStats {
        ToolCallStats {
            tool: tool.to_string(),
            calls,
            errors: 0,
            overflows: 0,
            error_rate_pct: 0.0,
            overflow_rate_pct: 0.0,
            p50_ms: 10,
            p99_ms: 50,
        }
    }

    #[tokio::test]
    async fn descriptor_shape() {
        let provider = ToolUsageProvider::new(FakeSource {
            snap: UsageSnapshot::default(),
            registered: vec![],
        });
        let descs = provider.descriptors();
        assert_eq!(descs.len(), 1);
        assert_eq!(descs[0].uri, URI);
        assert_eq!(descs[0].mime_type, "application/json");
    }

    #[tokio::test]
    async fn unknown_uri_returns_not_found() {
        let provider = ToolUsageProvider::new(FakeSource {
            snap: UsageSnapshot::default(),
            registered: vec![],
        });
        let err = provider.read("doctor://other").await.unwrap_err();
        assert!(matches!(err, ResourceError::NotFound(_)));
    }

    #[tokio::test]
    async fn empty_snapshot_produces_empty_report() {
        let provider = ToolUsageProvider::new(FakeSource {
            snap: UsageSnapshot::default(),
            registered: vec!["find_symbol".into(), "list_dir".into()],
        });
        let bytes = provider.read(URI).await.unwrap();
        let ResourceBytes::Text(json) = bytes else {
            panic!("expected text bytes")
        };
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["total_calls"], 0);
        assert_eq!(parsed["window"], "30d");
        // Both registered tools are unused.
        assert_eq!(
            parsed["unused_tools"],
            serde_json::json!(["find_symbol", "list_dir"])
        );
        assert_eq!(parsed["prune_candidates"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn prune_candidate_flagged_when_low_usage() {
        let provider = ToolUsageProvider::new(FakeSource {
            snap: UsageSnapshot {
                total_calls: 200,
                by_tool: vec![
                    make_stats("find_symbol", 197), // above threshold
                    make_stats("rename_symbol", 2), // below threshold, but > 0 → prune candidate
                    make_stats("symbol_at", 1),     // below threshold → prune candidate
                ],
            },
            registered: vec![
                "find_symbol".into(),
                "rename_symbol".into(),
                "symbol_at".into(),
                "list_dir".into(), // never called → unused
            ],
        });

        let ResourceBytes::Text(json) = provider.read(URI).await.unwrap() else {
            panic!()
        };
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["total_calls"], 200);
        assert_eq!(
            parsed["prune_candidates"],
            serde_json::json!(["rename_symbol", "symbol_at"])
        );
        assert_eq!(parsed["unused_tools"], serde_json::json!(["list_dir"]));
    }

    #[tokio::test]
    async fn unused_tools_sorted_for_stable_output() {
        let provider = ToolUsageProvider::new(FakeSource {
            snap: UsageSnapshot::default(),
            // Pass in unsorted order — the report must sort for determinism.
            registered: vec!["zeta_tool".into(), "alpha_tool".into(), "beta_tool".into()],
        });
        let ResourceBytes::Text(json) = provider.read(URI).await.unwrap() else {
            panic!()
        };
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed["unused_tools"],
            serde_json::json!(["alpha_tool", "beta_tool", "zeta_tool"])
        );
    }

    #[tokio::test]
    async fn tools_with_exactly_threshold_are_not_prune_candidates() {
        let provider = ToolUsageProvider::new(FakeSource {
            snap: UsageSnapshot {
                total_calls: 5,
                by_tool: vec![make_stats("some_tool", LOW_CALL_THRESHOLD)],
            },
            registered: vec!["some_tool".into()],
        });
        let ResourceBytes::Text(json) = provider.read(URI).await.unwrap() else {
            panic!()
        };
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Exactly LOW_CALL_THRESHOLD → NOT a prune candidate (strict <).
        assert_eq!(parsed["prune_candidates"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn registered_tool_with_calls_is_not_listed_as_unused() {
        let provider = ToolUsageProvider::new(FakeSource {
            snap: UsageSnapshot {
                total_calls: 10,
                by_tool: vec![make_stats("find_symbol", 10)],
            },
            registered: vec!["find_symbol".into()],
        });
        let ResourceBytes::Text(json) = provider.read(URI).await.unwrap() else {
            panic!()
        };
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["unused_tools"], serde_json::json!([]));
        // And no prune candidate either (10 >= threshold).
        assert_eq!(parsed["prune_candidates"], serde_json::json!([]));
    }
}
