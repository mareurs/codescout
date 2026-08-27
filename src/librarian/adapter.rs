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

    /// Projects `{tool}.{action}` before `call()` consumes `input`. When the call
    /// carries no `action` (e.g. `artifact_augment`, which has no `action` param at
    /// all), falls back to the bare tool name rather than `None` — a tool-only
    /// declaration must still be matchable. `Shape::matches` (Task 5) treats `None`
    /// as "cannot match", so returning `None` here for the no-action case would make
    /// such declarations permanently unmatchable: a silent-absence failure, not the
    /// fail-safe-toward-delivery direction this feature requires.
    fn selector_key(&self, input: &Value) -> Option<String> {
        match input.get("action").and_then(Value::as_str) {
            Some(action) => Some(format!("{}.{}", self.name(), action)),
            None => Some(self.name().to_string()),
        }
    }

    fn relevant_guide_topic(&self, result: &Value) -> Option<&str> {
        // Two guides serve this tool and only one can be delivered per call, so pick by
        // what the call actually touched rather than always sending the bigger one.
        //
        // A response naming a path under `docs/issues/` or `docs/trackers/` is a
        // bug-file or tracker operation, and `tracker-conventions` (frontmatter, the
        // status vocabulary, archive-through-the-catalog) is what the caller is about to
        // need. Nothing fired it before 2026-08-16 — it was authored, pointed at, and
        // never connected. `librarian` remains the default for everything else.
        //
        // Result-based rather than input-based because `call_content` moves `input` into
        // `call()` before the hint is computed, while the result is in scope and already
        // carries `abs_path` on every artifact response. No clone, no plumbing.
        //
        // The ledger deduplicates per topic, so a session doing tracker work can receive
        // both over its lifetime (10.4 KB + 19.9 KB). That is the byte tension BL-25
        // records; the corpus cut is the answer to it, not withholding the guide.
        //
        // See `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`.
        if names_tracker_path(result) {
            return Some("tracker-conventions");
        }
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

/// Whether a librarian response names a path containing `needle`.
///
/// Generalised from the bug-file/tracker-path check below so a section declaration
/// (Task 5) can carry an arbitrary `path~<substring>` predicate. The scanned shapes
/// are unchanged and still deliberately shallow: top-level `abs_path`/`rel_path`,
/// one level into a `find`-style `items` array, and `path` inside a `doctor`-style
/// `violations` array. Each is enumerated explicitly because a missing shape fails
/// as a *wrong guide* rather than an error — see
/// `docs/issues/archive/2026-08-20-doctor-entry-validity-rows-never-route-to-tracker-conventions.md`
/// for the case where `doctor` was the missing shape.
///
/// Separators are normalized before matching. A backslash-spelled Windows path
/// matched nothing against the hardcoded forward-slash needle and silently
/// delivered the *wrong* guide instead of erroring — measured under wine,
/// 2026-08-26, once Git Bash was available there to separate this from the
/// no-POSIX-shell failures it had been hiding behind.
pub fn names_path_containing(result: &Value, needle: &str) -> bool {
    fn hit(v: Option<&Value>, needle: &str) -> bool {
        v.and_then(Value::as_str)
            .is_some_and(|p| p.replace('\\', "/").contains(needle))
    }
    fn any_path_field(obj: &Value, needle: &str) -> bool {
        hit(obj.get("abs_path"), needle) || hit(obj.get("rel_path"), needle)
    }

    if any_path_field(result, needle) {
        return true;
    }
    if result
        .get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| items.iter().any(|i| any_path_field(i, needle)))
    {
        return true;
    }
    // `doctor` is the one action whose rows carry neither `abs_path`/`rel_path` nor an
    // `items` array: it returns `violations: [{check, artifact_id, path, detail}]`
    // (`src/librarian/tools/doctor.rs:466`, field at `:135`). Scanned as its own key
    // rather than folded into `any_path_field`, which would widen the TOP-LEVEL check
    // across every response shape to serve one action; `violations` is unique to
    // `doctor`, so the blast radius is exactly that response.
    result
        .get("violations")
        .and_then(Value::as_array)
        .is_some_and(|rows| rows.iter().any(|row| hit(row.get("path"), needle)))
}

/// Whether a librarian response names a path under the bug-file or tracker directories.
///
/// Scans `abs_path`/`rel_path` at the top level and one level into a `find`-style `items`
/// array, plus `path` inside a `doctor`-style `violations` array — deliberately shallow. A
/// deep walk over an arbitrarily large response to choose a guide would cost more than the
/// guide saves, and the shallow form covers the shapes that actually carry a path:
/// `get`/`create`/`update`/`move` return one at the top level, `find` returns them per
/// item, `doctor` returns them per violation under a differently-named key.
///
/// Each shape is enumerated explicitly because a missing one fails as a *wrong guide*
/// rather than as an error — see
/// `docs/issues/archive/2026-08-20-doctor-entry-validity-rows-never-route-to-tracker-conventions.md`
/// for the case where `doctor` was the missing shape.
///
/// Separators are normalized before matching. This paragraph used to claim a forward-slash
/// comparison was safe — `rel_path` stored forward-slash, `abs_path` relativized before the
/// hint is computed. Neither guarantee holds on Windows, where a backslash-spelled response
/// path matched nothing and the caller silently got the general `librarian` guide. Measured
/// under wine 2026-08-26, once Git Bash was available there to separate this from the eight
/// no-POSIX-shell failures it had been hiding behind.
fn names_tracker_path(result: &Value) -> bool {
    names_path_containing(result, "docs/issues/") || names_path_containing(result, "docs/trackers/")
}

/// Compact summary shown in place of a buffered librarian response.
///
/// Ordered deliberately, because `truncate_compact` cuts from the **tail**: anything
/// below can be lost, so incompleteness signals lead.
///
/// 1. **Incompleteness.** The `artifact(get)` body cap (`$.overflow.shown_lines`), then
///    any other action's own `$.overflow.hint`. A body large enough to trip the cap also
///    exceeds the inline budget, so without promotion the warning is buffered away and an
///    agent extracts `$.body`, sees ~500 lines and never learns it was truncated — that
///    silent loss caused duplicate sections written from a short-read line count
///    (`docs/issues/archive/2026-07-09-artifact-get-full-true-body-silent-truncation.md`).
///    The generic hint half was found by measurement: `librarian(context)` returned 10 of
///    50 candidates with discovery capped, and the envelope said only "15357 bytes".
///
/// 2. **The answer the action was asked for** — matched titles for `find`, section
///    headings for `get`, and a head-preview of whichever field actually holds the prose.
///    Previously the body-cap warning was the *only* case, so every other response fell
///    through to the generic envelope and the call returned no payload at all: 104
///    `artifact` overflows in the live corpus, each a guaranteed wasted turn. BL-19.
///
/// 3. **The generic shape description**, appended rather than displaced. `format_compact`
///    *replaces* the fallback in `ToolContext::call_content` (`unwrap_or_else`), so
///    returning `Some` here would otherwise drop the top-level key list a `json_path` is
///    aimed with — trading one gap for another.
///
/// Returns `None` when there is nothing to add, leaving the generic describer in place
/// rather than replacing it with something worse.
fn librarian_compact_summary(inner_name: &str, result: &Value) -> Option<String> {
    let is_artifact = inner_name == "artifact";
    let mut lines: Vec<String> = Vec::new();

    // Artifact-shaped messages stay gated on the tool, so another librarian action
    // carrying a similar-looking field is never described as an artifact body.
    if is_artifact {
        if let Some(warning) = body_truncation_warning(result) {
            lines.push(warning);
        }
    }
    if let Some(hint) = overflow_hint(result) {
        lines.push(hint);
    }
    if is_artifact {
        if let Some(matched) = matched_items_summary(result) {
            lines.push(matched);
        } else if let Some(sections) = section_headings_summary(result) {
            lines.push(sections);
        }
    }
    if let Some(preview) = dominant_text_preview(result) {
        lines.push(preview);
    }

    if lines.is_empty() {
        return None;
    }

    if let Some(shape) = crate::tools::format::describe_payload_shape(result) {
        lines.push(shape);
    }
    Some(lines.join("\n  "))
}

/// The `get` body-cap warning, promoted out of `$.overflow` so it survives buffering.
fn body_truncation_warning(result: &Value) -> Option<String> {
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

/// Any *other* action's incompleteness signal, promoted out of `$.overflow.hint`.
///
/// Each librarian action carries its own: `context` reports omitted candidates and
/// capped discovery, `find` reports more-in-scope. Buffered away, a partial answer reads
/// as a complete one — measured 2026-08-16, a `context` call returned 10 of 50 candidates
/// with discovery capped while its envelope said only `"15357 bytes … 4 keys"`.
///
/// Declines on the `artifact(get)` body cap, which [`body_truncation_warning`] states
/// more loudly and more specifically just above.
fn overflow_hint(result: &Value) -> Option<String> {
    let overflow = result.get("overflow")?.as_object()?;
    if overflow.contains_key("shown_lines") {
        return None;
    }
    let hint = overflow.get("hint")?.as_str()?;
    Some(format!("INCOMPLETE — {hint}"))
}

/// The largest string field, previewed by its head.
///
/// Shape alone is not an answer. `librarian(context)` described itself as
/// `"4 keys: markdown, included_ids, overflow, scope"` while `markdown` *was* the whole
/// answer — named, never shown. Naming the field and showing its opening is usually
/// enough to decide whether the buffer needs reading at all.
///
/// Bounded hard on both ends: below `MIN_LEN` the generic describer already prints the
/// value verbatim, so a preview would repeat it; above `PREVIEW_CHARS` we would be
/// inlining the payload and undoing the buffering that produced this envelope.
fn dominant_text_preview(result: &Value) -> Option<String> {
    /// `describe_payload_shape` prints strings up to 60 bytes verbatim; well clear of it.
    const MIN_LEN: usize = 200;
    const PREVIEW_CHARS: usize = 240;

    let (key, text) = result
        .as_object()?
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k, s)))
        .filter(|(_, s)| s.len() >= MIN_LEN)
        .max_by_key(|(_, s)| s.len())?;

    // Newlines would break the one-line-per-part layout the caller reads this as.
    let head: String = text
        .chars()
        .take(PREVIEW_CHARS)
        .collect::<String>()
        .replace('\n', " ⏎ ");
    Some(format!("$.{key} starts: {head}…"))
}

/// `find` (and any action answering with `items`) summarised by what it matched.
///
/// The generic describer can only report the array's *length* — `items[50]` — which is
/// the one thing the caller did not ask. Titles are the answer; ids are what make the
/// follow-up call possible without a second lookup; status is what a triage query is
/// usually filtering on.
fn matched_items_summary(result: &Value) -> Option<String> {
    /// Eight rows is roughly 800 bytes — informative without crowding out the shape
    /// description below it, and well inside `COMPACT_SUMMARY_MAX_BYTES`.
    const MAX_ITEMS: usize = 8;
    const MAX_TITLE: usize = 72;

    let items = result.get("items")?.as_array()?;
    if items.is_empty() {
        return None;
    }

    let mut out = format!("{} matched:", items.len());
    for item in items.iter().take(MAX_ITEMS) {
        let id = item.get("id").and_then(Value::as_str).unwrap_or("?");
        let status = item.get("status").and_then(Value::as_str).unwrap_or("-");
        let title = item
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("<untitled>");
        out.push_str(&format!(
            "\n    {id} [{status}] {}",
            ellipsize(title, MAX_TITLE)
        ));
    }
    if items.len() > MAX_ITEMS {
        out.push_str(&format!(
            "\n    … +{} more — narrow the filter, or read them from the buffer",
            items.len() - MAX_ITEMS
        ));
    }
    Some(out)
}

/// `get` summarised by its section headings.
///
/// `title` and `status` already survive as short scalars in the generic description;
/// headings do not, and they are what tells the caller which narrower
/// `artifact(get, heading="…")` call to make instead of pulling the whole body out of
/// the buffer. Rendered with their level markers so a heading can be passed straight
/// back as that argument.
fn section_headings_summary(result: &Value) -> Option<String> {
    const MAX_HEADINGS: usize = 14;

    let headings = result.get("preview")?.get("headings")?.as_array()?;
    let rendered: Vec<String> = headings
        .iter()
        .take(MAX_HEADINGS)
        .filter_map(|h| {
            let text = h.get("text").and_then(Value::as_str)?;
            let level = h
                .get("level")
                .and_then(Value::as_u64)
                .unwrap_or(2)
                .clamp(1, 6);
            Some(format!("{} {text}", "#".repeat(level as usize)))
        })
        .collect();
    if rendered.is_empty() {
        return None;
    }

    let mut out = format!("sections: {}", rendered.join(" · "));
    if headings.len() > rendered.len() {
        out.push_str(&format!(" · … +{} more", headings.len() - rendered.len()));
    }
    Some(out)
}

/// Char-aware truncation — a byte slice would panic mid-codepoint, and artifact titles
/// carry em-dashes and arrows routinely.
fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let kept: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{kept}…")
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
    use crate::tools::Tool as _;
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

    /// BL-19 fix 2. `find` answering "which artifacts matched?" with `items[50]` is the
    /// measured worst case: the envelope names the array's length and nothing in it, so
    /// the call cost a turn and returned no answer. The titles ARE the answer, and the
    /// ids are what makes the next call possible without a second lookup.
    #[test]
    fn compact_summary_lists_what_find_matched() {
        let result = json!({
            "count": 2,
            "items": [
                { "id": "0694a4a9946e10fe", "kind": "bug", "status": "mitigated",
                  "title": "BUG: append_entry writes catalog-only state, so a tracker's committed snapshot silently drifts",
                  "abs_path": "docs/issues/2026-08-16-append-entry.md", "updated_at": 1786902554589u64 },
                { "id": "e04115d9477d280b", "kind": "bug", "status": "open",
                  "title": "BUG: run_command passes the command to sh -c verbatim",
                  "abs_path": "docs/issues/2026-08-16-run-command.md", "updated_at": 1786897911709u64 },
            ],
            "scope": { "applied": "repo" },
            "hints": {},
        });

        let summary = librarian_compact_summary("artifact", &result)
            .expect("a result carrying matched items must summarise them");

        assert!(
            summary.contains("0694a4a9946e10fe") && summary.contains("e04115d9477d280b"),
            "ids make the follow-up call possible without a second lookup: {summary}"
        );
        assert!(
            summary.contains("append_entry writes catalog-only state"),
            "the titles are the answer the caller asked for: {summary}"
        );
        assert!(
            summary.contains("mitigated") && summary.contains("open"),
            "status is what a triage query is usually filtering on: {summary}"
        );
    }

    /// The `get` half. `title` and `status` already survive via the generic shape
    /// describer (they are short scalars), but the section headings do not — and
    /// headings are what tell the caller which narrower `heading=` call to make
    /// instead of pulling the whole body out of the buffer.
    #[test]
    fn compact_summary_lists_section_headings_for_get() {
        let result = json!({
            "id": "60b5323f5e11be66",
            "kind": "bug",
            "status": "open",
            "title": "BUG: repairing a frontmatter id re-serializes the whole block",
            "preview": {
                "shape": "default",
                "headings": [
                    { "level": 1, "text": "BUG: repairing a frontmatter id", "line": 1 },
                    { "level": 2, "text": "Summary", "line": 3 },
                    { "level": 2, "text": "Root cause", "line": 68 },
                    { "level": 3, "text": "The round-trip is the mechanism", "line": 99 },
                ],
                "line_count": 172,
            },
            "body": "…",
        });

        let summary = librarian_compact_summary("artifact", &result)
            .expect("a result carrying section headings must summarise them");

        assert!(
            summary.contains("Summary") && summary.contains("Root cause"),
            "headings name the narrower `heading=` call to make: {summary}"
        );
        assert!(
            summary.contains("##"),
            "carry the level markers so the heading can be passed back verbatim: {summary}"
        );
    }

    /// The sequencing constraint the bug file recorded: keep the truncation warning as
    /// an ADDITIONAL line, never a precondition. It guards a silent-truncation defect
    /// that once cost duplicate sections, and `truncate_compact` cuts from the tail —
    /// so it has to come first or a long summary can bury it.
    #[test]
    fn compact_summary_keeps_the_truncation_warning_first_and_adds_the_answer() {
        let result = json!({
            "id": "x",
            "title": "Some Tracker",
            "overflow": { "shown_lines": 500, "total_lines": 1841, "hint": "…" },
            "preview": {
                "headings": [
                    { "level": 2, "text": "Findings index", "line": 49 },
                    { "level": 2, "text": "History", "line": 378 },
                ],
            },
            "body": "…capped…",
        });

        let summary = librarian_compact_summary("artifact", &result)
            .expect("overflow plus headings must yield both");

        let truncation = summary
            .to_uppercase()
            .find("TRUNCAT")
            .expect("the truncation warning must survive: {summary}");
        let headings = summary
            .find("Findings index")
            .expect("the headings must be added, not replace the warning");
        assert!(
            truncation < headings,
            "truncate_compact cuts from the tail, so the correctness signal goes first: {summary}"
        );
    }

    /// BL-19 fix 3, first half — and a correctness bug found by measuring rather than
    /// reasoning. `librarian(context)` carries its OWN incompleteness signal in
    /// `$.overflow.hint`, and the summary dropped it because the whole function was
    /// gated on `inner_name == "artifact"`.
    ///
    /// Measured live 2026-08-16: a `context` call returned 10 of 50 candidates with
    /// discovery capped, and the envelope said only `"15357 bytes … 4 keys"`. A partial
    /// answer that does not announce itself reads as a complete one.
    #[test]
    fn compact_summary_promotes_an_overflow_hint_from_any_librarian_tool() {
        let result = json!({
            "markdown": "…bundle…",
            "included_ids": ["a", "b"],
            "overflow": {
                "candidates": 50, "included": 10, "omitted": 40, "candidates_capped": true,
                "hint": "40 candidate(s) omitted (token budget) — raise `max_tokens` or narrow `topic`.",
            },
            "scope": {},
        });

        let summary = librarian_compact_summary("librarian", &result)
            .expect("an incompleteness signal must survive buffering, whatever the tool");
        assert!(
            summary.contains("40 candidate(s) omitted"),
            "the hint is the signal; burying it makes a partial answer look whole: {summary}"
        );

        // The artifact body-cap message is louder and more specific; it must not be
        // duplicated by this generic path.
        let body_capped = json!({
            "overflow": { "shown_lines": 500, "total_lines": 1841, "hint": "…" },
        });
        let summary =
            librarian_compact_summary("artifact", &body_capped).expect("body cap still summarises");
        assert_eq!(
            summary.matches("500").count(),
            1,
            "the body-cap case must be announced once, not twice: {summary}"
        );
    }

    /// BL-19 fix 3, second half. Shape alone is not an answer: `librarian(context)`
    /// described itself as `"4 keys: markdown, included_ids, overflow, scope"` while
    /// `markdown` *was* the entire answer. Name the field and show its head.
    ///
    /// The negative assertions are the load-bearing ones — a preview that inlines the
    /// payload has undone the buffering that produced the envelope.
    #[test]
    fn compact_summary_previews_the_dominant_text_field_without_inlining_it() {
        let body = format!(
            "# Frontmatter and the catalog\n\nThe first paragraph is what a caller needs \
             to decide whether to pull the rest.\n\n{}\n\nTAIL_MARKER_NOT_IN_PREVIEW",
            "filler ".repeat(3000)
        );
        let result = json!({ "markdown": body, "included_ids": ["a"], "scope": {} });

        let summary = librarian_compact_summary("librarian", &result)
            .expect("a dominant text field must be previewed");

        assert!(
            summary.contains("markdown"),
            "name the field so a json_path can be aimed at it: {summary}"
        );
        assert!(
            summary.contains("The first paragraph is what a caller needs"),
            "show the head — that is the part that answers the question: {summary}"
        );
        assert!(
            !summary.contains("TAIL_MARKER_NOT_IN_PREVIEW"),
            "the preview must not reach the tail: {summary}"
        );
        assert!(
            summary.len() < 1_200,
            "a preview that inlines the payload defeats buffering; got {} bytes",
            summary.len()
        );

        // A short string is already printed verbatim by the generic describer, so a
        // preview would only repeat it.
        let short = json!({ "markdown": "tiny", "n": 1 });
        assert!(
            librarian_compact_summary("librarian", &short).is_none(),
            "nothing worth previewing means the generic describer keeps the slot"
        );
    }

    /// The guide the librarian delivers depends on what the call touched.
    ///
    /// `tracker-conventions` was authored, pointed at from prose, and wired to nothing
    /// (BL-25). It is the guide a bug-file or tracker operation actually needs, and this
    /// is the discriminator that routes to it — result-based, because `call_content`
    /// moves `input` into `call()` before the hint is computed while the result is still
    /// in scope and already carries `abs_path`.
    ///
    /// See `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`.
    #[test]
    fn tracker_paths_route_to_the_tracker_guide_and_nothing_else_does() {
        // Top-level shapes — what get/create/update/move return.
        assert!(names_tracker_path(&json!({
            "abs_path": "docs/trackers/open-issue-work-queue.md"
        })));
        assert!(names_tracker_path(&json!({
            "rel_path": "docs/issues/2026-08-16-some-bug.md"
        })));

        // `find` returns paths per item, not at the top level — the shape a
        // top-level-only check would silently miss.
        assert!(names_tracker_path(&json!({
            "items": [
                {"abs_path": "src/main.rs"},
                {"abs_path": "docs/trackers/x.md"},
            ]
        })));

        // Everything else keeps the default `librarian` guide.
        assert!(!names_tracker_path(&json!({ "abs_path": "src/main.rs" })));
        assert!(!names_tracker_path(
            &json!({ "items": [{"abs_path": "README.md"}] })
        ));
        assert!(!names_tracker_path(&json!({})));
        // A near-miss that must not match: the directory names have to be path
        // segments, and a bare mention in some other field is not one.
        assert!(!names_tracker_path(&json!({
            "title": "how docs/trackers/ works"
        })));

        // `doctor` returns neither shape above: rows sit under `violations`, and the
        // field is `path`. The entry-validity checks report tracker entries, so this is
        // the caller that most needs `tracker-conventions` and the one that used to be
        // guaranteed not to get it.
        assert!(names_tracker_path(&json!({
            "violations": [
                {"check": "abs_path_must_be_absolute", "path": "src/main.rs"},
                {"check": "entry_cited_from_outside_but_undeclared",
                 "path": "docs/trackers/statement-validity-session-log.md"},
            ]
        })));
        assert!(!names_tracker_path(&json!({
            "violations": [{"check": "snapshot_drift", "path": "src/main.rs"}]
        })));

        // The `path` key is honoured ONLY inside `violations`. Promoting it into
        // `any_path_field` would be the obvious simplification and would widen the
        // top-level check across every librarian response to serve one action; this
        // pins the narrow scoping so that change fails loudly instead of silently
        // re-routing unrelated calls.
        assert!(!names_tracker_path(&json!({
            "path": "docs/trackers/x.md"
        })));
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

    /// Builds the first registered librarian tool (`artifact::Artifact`, per
    /// `all_tools()`'s ordering) wrapped in a `LibrarianAdapter`, for tests that only
    /// need `Tool` methods and not real catalog behaviour. There is no
    /// `LibrarianAdapter::new_for_test` constructor — this mirrors the construction
    /// the `derive_ctx_*` tests above already use.
    fn adapter_for_test() -> LibrarianAdapter {
        let ctx = Arc::new(
            crate::librarian::tools::TestToolContextBuilder::new(
                crate::librarian::catalog::Catalog::open_in_memory().unwrap(),
            )
            .build(),
        );
        LibrarianAdapter {
            inner: lib_all_tools()
                .into_iter()
                .next()
                .expect("at least one librarian tool registered"),
            ctx,
        }
    }

    #[test]
    fn selector_key_projects_tool_and_action() {
        let a = adapter_for_test();
        assert_eq!(
            a.selector_key(&json!({"action": "append_entry", "id": "x"})),
            Some("artifact.append_entry".to_string())
        );
        // No action ⇒ the bare tool name. Tool-only shapes (e.g. artifact_augment,
        // which takes no `action` param at all) must still be matchable by a
        // declaration keyed on the tool name alone — see the CORRECTION in
        // task-4-brief.md. Falling back to `?` here would make such a declaration
        // unmatchable forever, which is a silent-absence failure, not the
        // fail-safe-toward-delivery direction this feature requires.
        assert_eq!(
            a.selector_key(&json!({"id": "x"})),
            Some(a.name().to_string())
        );
    }

    #[test]
    fn names_path_containing_generalises_and_normalises_separators() {
        let v = json!({"abs_path": "docs\\issues\\x.md"});
        assert!(names_path_containing(&v, "docs/issues/"));
        assert!(!names_path_containing(&v, "docs/trackers/"));
        // find-style items and doctor-style violations keep working.
        let items = json!({"items": [{"rel_path": "docs/trackers/t.md"}]});
        assert!(names_path_containing(&items, "docs/trackers/"));
        let viol = json!({"violations": [{"path": "docs/issues/b.md"}]});
        assert!(names_path_containing(&viol, "docs/issues/"));
    }

    #[test]
    fn names_tracker_path_still_agrees_with_the_generalised_form() {
        // The existing trigger must not change behaviour in Phase 1.
        for p in ["docs/issues/x.md", "docs/trackers/y.md", "src/main.rs"] {
            let v = json!({"abs_path": p});
            assert_eq!(
                names_tracker_path(&v),
                names_path_containing(&v, "docs/issues/")
                    || names_path_containing(&v, "docs/trackers/")
            );
        }
    }
}
