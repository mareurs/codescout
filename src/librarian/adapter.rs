//! Librarian (markdown artifact registry) integration.
//!
//! Codescout embeds the librarian crate and exposes its tools through the
//! same MCP server, so the agent sees one server with both code-symbol
//! tools and artifact tools. The adapter bridges librarian's sync `Tool`
//! trait (blocking rusqlite + parking_lot) to codescout's async trait
//! via `spawn_blocking`.
//!
//! Builder is fallible and best-effort: when no workspace.toml is
//! discoverable from cwd the librarian tools are simply absent — codescout
//! continues to serve its own tools.

use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;

use crate::librarian::tools::{all_tools as lib_all_tools, ToolContext as LibToolContext};

pub async fn try_build_runtime(
    lsp: Arc<dyn crate::lsp::LspProvider>,
) -> Option<Arc<LibToolContext>> {
    match crate::librarian::build_tool_context(lsp).await {
        Ok(ctx) => Some(Arc::new(ctx)),
        Err(err) => {
            tracing::info!("librarian disabled: {err:#}");
            None
        }
    }
}

pub fn adapters_for(ctx: Arc<LibToolContext>) -> Vec<Arc<dyn crate::tools::Tool>> {
    lib_all_tools()
        .into_iter()
        .map(|t| {
            let adapter: Arc<dyn crate::tools::Tool> = Arc::new(LibrarianAdapter {
                inner: t,
                ctx: Arc::clone(&ctx),
            });
            adapter
        })
        .collect()
}

struct LibrarianAdapter {
    inner: Arc<dyn crate::librarian::tools::Tool>,
    ctx: Arc<LibToolContext>,
}

#[async_trait::async_trait]
impl crate::tools::Tool for LibrarianAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn input_schema(&self) -> Value {
        self.inner.input_schema()
    }

    async fn call(&self, input: Value, ctx: &crate::tools::ToolContext) -> Result<Value> {
        let active_root: Option<std::path::PathBuf> = {
            let inner = ctx.agent.inner.read().await;
            inner.active_project().map(|p| p.root.clone())
        };
        let lib_ctx = self.derive_ctx(active_root.as_deref());
        self.inner.call(&lib_ctx, input).await
    }

    fn is_write(&self, input: &Value) -> bool {
        let action = input.get("action").and_then(Value::as_str);
        match self.inner.name() {
            // CRUD tool — mutating actions only; find/get/graph/state_at are reads.
            "artifact" => matches!(
                action,
                Some("create" | "update" | "move" | "delete" | "link")
            ),
            // Append-only event log: `create` writes, `list` reads.
            "artifact_event" => action == Some("create"),
            // Always attaches/replaces/merges an augmentation row.
            "artifact_augment" => true,
            // gather / list_stale are both read-only — the write-back is
            // artifact(update, commit_refresh=true), classified under "artifact".
            "artifact_refresh" => false,
            // reindex rewrites the catalog; audit_doc_refs emits a tracker unless
            // emit_tracker=false; legibility_scan reconciles the backlog unless
            // write=false; link_scan mutates edges ONLY when write=true (read-
            // default — polarity is the inverse of legibility_scan's, do not
            // copy that arm); context/tracker_design/workspace_state_at/doctor read.
            "librarian" => match action {
                Some("reindex") => true,
                Some("audit_doc_refs") => {
                    input.get("emit_tracker").and_then(Value::as_bool) != Some(false)
                }
                Some("legibility_scan") => {
                    input.get("write").and_then(Value::as_bool) != Some(false)
                }
                Some("link_scan") => input.get("write").and_then(Value::as_bool) == Some(true),
                _ => false,
            },
            _ => false,
        }
    }

    fn relevant_guide_topic(&self) -> Option<&str> {
        Some("librarian")
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        librarian_compact_summary(self.inner.name(), result)
    }
}

impl LibrarianAdapter {
    /// Build a fresh `LibToolContext` for a single tool call, using the
    /// host's currently-active project to derive `current_project`. The
    /// catalog/workspace/rules/embedding stay shared with the boot-time ctx.
    fn derive_ctx(&self, active: Option<&std::path::Path>) -> Arc<LibToolContext> {
        let current_project = active.and_then(|p| match std::fs::canonicalize(p) {
            Ok(abs_path) => {
                let git_root = crate::librarian::current_project::lookup_git_root(&abs_path)
                    .unwrap_or_else(|| abs_path.clone());
                let umbrella = crate::librarian::current_project::lookup_umbrella(
                    &abs_path,
                    &self.ctx.workspace,
                );
                Some(Arc::new(
                    crate::librarian::current_project::CurrentProject {
                        abs_path,
                        git_root,
                        umbrella,
                    },
                ))
            }
            Err(err) => {
                tracing::warn!("active project path unresolvable: {} ({err})", p.display());
                None
            }
        });

        Arc::new(LibToolContext {
            catalog: Arc::clone(&self.ctx.catalog),
            workspace: Arc::clone(&self.ctx.workspace),
            rules: Arc::clone(&self.ctx.rules),
            embedding: self.ctx.embedding.clone(),
            artifact_store: self.ctx.artifact_store.clone(),
            current_project,
            lsp: Arc::clone(&self.ctx.lsp),
        })
    }
}

/// Compact summary shown in place of a buffered librarian response.
///
/// The load-bearing case is `artifact(get, full=true)`: `get` caps the returned
/// `body` at `SOFT_CAP_LINES` and records the cut in a sibling `overflow` object
/// (`shown_lines` / `total_lines` / `hint`). But any body large enough to trip
/// that cap also exceeds the inline budget, so the whole response is buffered and
/// the generic `"Result stored in …"` summary would drop the `overflow` warning
/// entirely — leaving an agent to extract `$.body`, see ~500 lines, and never
/// learn the body was truncated. That silent loss caused real downstream damage
/// (duplicate sections written from a short-read line count); see
/// `docs/issues/2026-07-07-artifact-get-full-body-silent-truncation.md` and
/// `docs/issues/2026-07-09-artifact-get-full-true-body-silent-truncation.md`.
///
/// Promote the warning into the summary so it survives buffering. `output_id` and
/// `hint` are set independently of the summary, so buffer navigation is unaffected.
fn librarian_compact_summary(inner_name: &str, result: &Value) -> Option<String> {
    // Only the `artifact` tool emits an `overflow` object (from the `get` action).
    if inner_name != "artifact" {
        return None;
    }
    let overflow = result.get("overflow")?.as_object()?;
    let shown = overflow.get("shown_lines")?.as_u64()?;
    let total = overflow.get("total_lines")?.as_u64()?;
    Some(format!(
        "artifact body TRUNCATED — only {shown} of {total} lines are in $.body \
         (soft cap). $.body is NOT the complete body. Read the rest with a narrower \
         selector — artifact(get, id=…, heading=\"<section>\") or start_line=N, \
         end_line=M — or see $.overflow for total_lines and top-level headings."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compact_summary_surfaces_artifact_get_body_truncation() {
        // Mirrors the real bug: get(full=true) capped body + sibling overflow,
        // whole response buffered. The summary must announce the truncation.
        let result = json!({
            "id": "x",
            "body": "…capped body…",
            "overflow": { "shown_lines": 500, "total_lines": 1841, "hint": "…" },
        });
        let summary = librarian_compact_summary("artifact", &result)
            .expect("an overflow object must yield a truncation summary");
        assert!(summary.contains("500"), "names shown lines: {summary}");
        assert!(summary.contains("1841"), "names total lines: {summary}");
        assert!(
            summary.to_uppercase().contains("TRUNCAT"),
            "must flag truncation loudly: {summary}"
        );
    }

    #[test]
    fn compact_summary_none_without_overflow() {
        // Body fit within the cap → no overflow field → generic fallback preserved.
        let result = json!({ "id": "x", "body": "short body" });
        assert!(librarian_compact_summary("artifact", &result).is_none());
    }

    #[test]
    fn compact_summary_none_for_non_artifact_tools() {
        // Defensive: a different librarian tool emitting an overflow-shaped field
        // must not be hijacked into an artifact-body message.
        let result = json!({ "overflow": { "shown_lines": 1, "total_lines": 2 } });
        assert!(librarian_compact_summary("librarian", &result).is_none());
    }
}
