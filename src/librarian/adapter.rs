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

/// Build the librarian runtime, with the environment inputs supplied explicitly so
/// tests can point it at a tempdir workspace/db without `set_var`. See
/// [`crate::librarian::LibrarianEnv`].
///
/// There was an env-reading `try_build_runtime` wrapper beside this; it had zero
/// callers and was deleted 2026-08-16. Every caller already has a `LibrarianEnv`.
pub async fn try_build_runtime_with(
    lsp: Arc<dyn crate::lsp::LspProvider>,
    env: &crate::librarian::LibrarianEnv,
) -> Option<Arc<LibToolContext>> {
    match crate::librarian::build_tool_context_with(lsp, env).await {
        Ok(ctx) => Some(Arc::new(ctx)),
        Err(err) => {
            tracing::info!("librarian disabled: {err:#}");
            None
        }
    }
}

/// Answers the markdown guard's question "is this file only a rendered snapshot?"
/// by asking the catalog whether the path carries an augmentation.
///
/// Catalog identity is `id == artifact_id_from_abs(abs_path)` (stated in
/// `src/librarian/tools/doctor.rs`), so the lookup is a primary-key hit, not a scan.
struct CatalogAugmentationOracle {
    catalog: Arc<parking_lot::Mutex<crate::librarian::catalog::Catalog>>,
}

impl crate::util::librarian_guard::AugmentedArtifactOracle for CatalogAugmentationOracle {
    fn is_augmented(&self, abs_path: &std::path::Path) -> bool {
        // Canonicalize: the catalog stores canonical absolute paths, and a caller's
        // resolved path may still carry a symlinked prefix. A path that cannot be
        // canonicalized (deleted mid-call) simply does not match.
        let Ok(abs) = std::fs::canonicalize(abs_path) else {
            return false;
        };
        let id = crate::librarian::ids::artifact_id_from_abs(&abs);
        // Plain `lock()` is safe here despite `parking_lot::Mutex` being
        // non-reentrant: the guard is only ever called from the core markdown
        // tools (`read_markdown`, `edit_markdown`, `edit_file`), none of which
        // hold the catalog lock — no librarian tool calls the guard.
        let cat = self.catalog.lock();
        matches!(
            crate::librarian::catalog::augmentation::get(&cat, &id),
            Ok(Some(_))
        )
    }
}

/// Wire the catalog into the markdown guard, once, at server construction.
pub fn install_augmentation_guard_oracle(ctx: &LibToolContext) {
    crate::util::librarian_guard::install_augmented_oracle(Arc::new(CatalogAugmentationOracle {
        catalog: Arc::clone(&ctx.catalog),
    }));
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
        // Honor the per-request `workspace=` pin the dispatcher stashed in
        // `ctx.workspace_override` — resolve the pinned workspace's focused root
        // (resident-on-demand) exactly as every other pinnable tool does, rather
        // than always reaching for the session-default active project. Without
        // this, a librarian call pinned to a foreign workspace silently scoped to
        // the session project and returned the wrong repo's rows (fails
        // silent-wrong). An unresolvable pin surfaces loudly instead of falling
        // back. See docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md.
        let active_root: Option<std::path::PathBuf> =
            if let Some(pin) = ctx.workspace_override.as_deref() {
                Some(
                    ctx.agent
                        .require_project_root_for(Some(pin))
                        .await
                        .map_err(bridge_recoverable_error)?,
                )
            } else {
                let inner = ctx.agent.inner.read().await;
                inner.active_project().map(|p| p.root.clone())
            };
        let lib_ctx = self.derive_ctx(active_root.as_deref());
        // Best-effort, throttled (24h) catalog GC reconcile — piggybacks on the
        // first librarian call per session/interval rather than the literal
        // `workspace(activate)` call, since the shared catalog handle only
        // exists here (inside the librarian adapter), not on the core
        // `ToolContext` that `ActivateProject`/`Workspace` receive. Uses a
        // non-blocking try_lock and swallows all errors — see
        // `gc::maybe_reconcile` for the full contract.
        crate::librarian::catalog::gc::maybe_reconcile(
            &lib_ctx.catalog,
            chrono::Utc::now().timestamp_millis(),
        );
        self.inner
            .call(&lib_ctx, input)
            .await
            .map_err(bridge_recoverable_error)
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
                let main_root = if crate::librarian::current_project::is_linked_worktree(&git_root)
                {
                    crate::librarian::current_project::worktree_main_root(&git_root)
                        .and_then(|p| std::fs::canonicalize(&p).ok().or(Some(p)))
                } else {
                    None
                };
                let project_local =
                    crate::librarian::current_project::load_project_umbrellas(&abs_path);
                // Umbrella membership is a property of the PROJECT, not the
                // checkout — resolve it against the main root (when present)
                // so worktree sessions keep umbrella scope.
                let umbrella = crate::librarian::current_project::resolve_umbrella(
                    main_root.as_deref().unwrap_or(&abs_path),
                    &project_local,
                    &self.ctx.workspace.umbrellas,
                );
                Some(Arc::new(
                    crate::librarian::current_project::CurrentProject {
                        abs_path,
                        git_root,
                        main_root,
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
/// `docs/issues/archive/2026-07-07-artifact-get-full-body-silent-truncation.md` and
/// `docs/issues/archive/2026-07-09-artifact-get-full-true-body-silent-truncation.md`.
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

/// Bridge a librarian-side `RecoverableError` into the host `RecoverableError`
/// so `route_tool_error`'s exact-type `downcast_ref` matches it (→ `isError:
/// false`, sibling parallel calls not aborted). The two types are distinct
/// (`crate::librarian::tools::RecoverableError` has `{message, hint}`;
/// `crate::tools::RecoverableError` has `{message, guidance, extra}`), and the
/// librarian tools construct the former. Without this bridge every librarian
/// recoverable condition falls through to the fatal branch in `route_tool_error`
/// and hard-fails, aborting sibling parallel calls — exactly what the type
/// exists to prevent. See
/// docs/issues/archive/2026-07-10-librarian-recoverable-error-downcast-never-matches.md.
fn bridge_recoverable_error(e: anyhow::Error) -> anyhow::Error {
    match e.downcast::<crate::librarian::tools::RecoverableError>() {
        Ok(lib) => match lib.hint {
            Some(h) => crate::tools::RecoverableError::with_hint(lib.message, h).into(),
            None => crate::tools::RecoverableError::new(lib.message).into(),
        },
        Err(orig) => orig,
    }
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

    #[test]
    fn bridge_maps_librarian_recoverable_to_host_type() {
        // route_tool_error downcasts to crate::tools::RecoverableError; the
        // librarian type must be bridged to it, or every librarian recoverable
        // error hard-fails (isError: true) and aborts sibling parallel calls.
        let e = crate::librarian::tools::RecoverableError::with_hint(
            "artifact not found",
            "check the id",
        );
        let bridged = bridge_recoverable_error(e);
        let host = bridged
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("must downcast to the host RecoverableError route_tool_error looks for");
        assert_eq!(host.message, "artifact not found");
        assert!(host.guidance.is_some(), "hint must map to guidance");
    }

    #[test]
    fn bridge_passes_through_non_recoverable_errors() {
        let e = anyhow::anyhow!("fatal: database exploded");
        let bridged = bridge_recoverable_error(e);
        assert!(
            bridged
                .downcast_ref::<crate::tools::RecoverableError>()
                .is_none(),
            "genuine failures must stay fatal (isError: true)"
        );
        assert!(bridged.to_string().contains("database exploded"));
    }

    #[test]
    fn derive_ctx_populates_main_root_for_linked_worktree() {
        // derive_ctx is the LIVE per-tool-call path (see its doc comment) —
        // current_project::resolve() only runs once at boot. A regression
        // here silently makes the whole worktree-overlay feature dead code
        // on every real MCP call, since later tasks branch on main_root.
        let tmp = tempfile::TempDir::new().unwrap();
        let main = tmp.path().join("main");
        std::fs::create_dir_all(main.join(".git/worktrees/feat")).unwrap();
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}/.git/worktrees/feat\n", main.display()),
        )
        .unwrap();

        let ctx = Arc::new(
            crate::librarian::tools::TestToolContextBuilder::new(
                crate::librarian::catalog::Catalog::open_in_memory().unwrap(),
            )
            .build(),
        );
        let adapter = LibrarianAdapter {
            inner: lib_all_tools()
                .into_iter()
                .next()
                .expect("at least one librarian tool registered"),
            ctx,
        };

        let derived = adapter.derive_ctx(Some(&wt));
        let cp = derived
            .current_project
            .as_deref()
            .expect("resolvable active path must yield a current_project");
        assert_eq!(
            cp.main_root.as_deref(),
            Some(std::fs::canonicalize(&main).unwrap().as_path()),
            "derive_ctx must populate main_root for a linked worktree, mirroring resolve()"
        );
    }

    #[test]
    fn derive_ctx_main_root_none_for_plain_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".git")).unwrap();

        let ctx = Arc::new(
            crate::librarian::tools::TestToolContextBuilder::new(
                crate::librarian::catalog::Catalog::open_in_memory().unwrap(),
            )
            .build(),
        );
        let adapter = LibrarianAdapter {
            inner: lib_all_tools()
                .into_iter()
                .next()
                .expect("at least one librarian tool registered"),
            ctx,
        };

        let derived = adapter.derive_ctx(Some(tmp.path()));
        let cp = derived
            .current_project
            .as_deref()
            .expect("resolvable active path must yield a current_project");
        assert!(
            cp.main_root.is_none(),
            "plain repo must not get a main_root"
        );
    }

    #[test]
    fn derive_ctx_resolves_umbrella_via_main_root() {
        // Task 2's fix made derive_ctx resolve umbrella membership against
        // `main_root.as_deref().unwrap_or(&abs_path)` for a worktree session
        // rather than the worktree's own abs_path. The umbrella's only member
        // is the MAIN root — if derive_ctx regressed to resolving against
        // `&abs_path` (the worktree checkout), the worktree path is not a
        // member of any umbrella and this assertion fails.
        let tmp = tempfile::TempDir::new().unwrap();
        let main = tmp.path().join("main");
        std::fs::create_dir_all(main.join(".git/worktrees/feat")).unwrap();
        let wt = tmp.path().join("wt");
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}/.git/worktrees/feat\n", main.display()),
        )
        .unwrap();

        let main_canon = std::fs::canonicalize(&main).unwrap();

        let ctx = Arc::new(
            crate::librarian::tools::TestToolContextBuilder::new(
                crate::librarian::catalog::Catalog::open_in_memory().unwrap(),
            )
            .with_umbrellas(vec![crate::librarian::workspace::Umbrella {
                name: "wto-umbrella".into(),
                members: vec![main_canon],
            }])
            .build(),
        );
        let adapter = LibrarianAdapter {
            inner: lib_all_tools()
                .into_iter()
                .next()
                .expect("at least one librarian tool registered"),
            ctx,
        };

        let derived = adapter.derive_ctx(Some(&wt));
        let cp = derived
            .current_project
            .as_deref()
            .expect("resolvable active path must yield a current_project");
        assert_eq!(
            cp.umbrella.as_deref(),
            Some("wto-umbrella"),
            "derive_ctx must resolve umbrella membership against main_root for a worktree session"
        );
    }
}
