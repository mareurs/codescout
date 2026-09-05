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
use crate::util::librarian_response::names_path_containing;

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
        // tools (`read_file`, `edit_file`), none of which
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

/// Writes a file's catalog row back into step after a direct frontmatter edit.
///
/// The write-side twin of [`CatalogAugmentationOracle`], and deliberately its neighbour:
/// the guard reads the catalog to decide whether to REFUSE an edit; this writes the
/// catalog after one it allowed. The guard lets a plain catalogued file through on
/// purpose (`a_catalogued_but_unaugmented_file_stays_directly_editable`), and that
/// population is exactly the one whose row could silently contradict its own file.
/// `open-issue-work-queue:BL-48`.
struct CatalogFrontmatterSyncer {
    catalog: Arc<parking_lot::Mutex<crate::librarian::catalog::Catalog>>,
}

impl crate::util::librarian_sync::CatalogFrontmatterSync for CatalogFrontmatterSyncer {
    fn sync_frontmatter(&self, abs_path: &std::path::Path) -> bool {
        // Canonicalize for the same reason the oracle does: the catalog stores canonical
        // absolute paths, and a resolved path may still carry a symlinked prefix.
        let Ok(abs) = std::fs::canonicalize(abs_path) else {
            return false;
        };
        let Ok(content) = std::fs::read_to_string(&abs) else {
            return false;
        };
        // No frontmatter block at all means nothing indexed can have moved.
        let Ok((Some(fm), _)) = crate::librarian::frontmatter::parse(&content) else {
            return false;
        };

        let id = crate::librarian::ids::artifact_id_from_abs(&abs);

        // Plain `lock()` is safe for the reason stated on the oracle: this is only ever
        // reached from the core markdown tools, none of which hold the catalog lock.
        let cat = self.catalog.lock();

        // NEVER create a row. A path the catalog does not know is ordinary markdown, and
        // inventing an artifact for it would turn every `edit_file` on a stray `.md`
        // into a catalog write. `false` here is the normal answer, not a failure.
        let Ok(Some(row)) = crate::librarian::catalog::artifact::get(&cat, &id) else {
            return false;
        };

        let now = chrono::Utc::now().timestamp_millis();
        let file_mtime = std::fs::metadata(&abs)
            .ok()
            .and_then(|m| {
                m.modified().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_millis() as i64)
                })
            })
            .unwrap_or(now);

        // File wins where it speaks, row survives where the file is silent. The
        // conservative direction on purpose: this runs after an edit that may have
        // touched one key, and a frontmatter block that omits `tags` is far more likely
        // to mean "not my business" than "delete the tags". A full re-derivation is
        // `librarian(action="reindex")`'s job, not a per-edit hook's.
        let updated = crate::librarian::catalog::artifact::ArtifactRow {
            id: row.id.clone(),
            abs_path: row.abs_path.clone(),
            kind: fm.kind.clone().unwrap_or(row.kind),
            status: fm.status.clone().unwrap_or(row.status),
            title: fm.title.clone().or(row.title),
            owners: if fm.owners.is_empty() {
                row.owners
            } else {
                fm.owners.clone()
            },
            tags: if fm.tags.is_empty() {
                row.tags
            } else {
                fm.tags.clone()
            },
            topic: fm.topic.clone().or(row.topic),
            time_scope: fm.time_scope.clone().or(row.time_scope),
            source: row.source,
            created_at: row.created_at,
            updated_at: now,
            file_mtime,
            file_sha256: crate::librarian::util::sha_of_bytes(content.as_bytes()),
            confidence: row.confidence,
        };

        crate::librarian::catalog::artifact::upsert_and_mint_slug(&cat, &updated).is_ok()
    }
}

/// Wire the catalog into the markdown frontmatter sync, once, at server construction.
pub fn install_catalog_frontmatter_sync(ctx: &LibToolContext) {
    crate::util::librarian_sync::install_catalog_sync(Arc::new(CatalogFrontmatterSyncer {
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

/// `Some("$.body")` when a buffered librarian result is a **scoped** read, `None` to leave
/// the general heuristic in place.
///
/// `default_json_path_hint` picks the largest array within a bounded depth. That is the
/// right rule for the results that usually overflow — lists of records — and the wrong one
/// for a scoped read: `doc(action="get", heading=…)` answers with a `body` **string**,
/// and a string is never a candidate for an array-selecting heuristic. So the hint named
/// `$.preview.headings[*]` — the heading map, which the caller already had and had not
/// asked for — while the section they requested sat at `$.body`. Following it costs a
/// wasted call, which is the exact cost `default_json_path_hint`'s own docs give as the
/// reason it exists: "a hint that cannot work for the result it is attached to … converts a
/// lookup into a failed call."
///
/// **Keyed on `body_meta`, not on `body` alone, and that is the whole discrimination.**
/// `body` appears on a *full* artifact read too, where the largest-array rule is still the
/// better answer because the caller asked for the whole row — an augmented tracker's
/// `$.augmentation.params.<collection>[*]` is worth far more there than a body they already
/// have in full. `body_meta` is emitted only when the server *scoped* the read (`heading`,
/// `headings`, or a line slice), so it is the narrowest available signal for "the caller
/// named a part, and this is that part".
///
/// Scoped to the librarian adapter rather than changed inside `default_json_path_hint`:
/// that heuristic is right for `find`, `graph`, `state_at`, `link_scan` and the rest, and
/// one action's shape is not a reason to move a rule the others depend on.
/// docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
fn scoped_body_hint(val: &Value) -> Option<String> {
    let scoped = val.get("body_meta").is_some_and(Value::is_object);
    (scoped && val.get("body").is_some()).then(|| "$.body".to_string())
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
    fn description_cap(&self) -> usize {
        self.inner.description_cap()
    }

    fn input_schema(&self) -> Value {
        self.inner.input_schema()
    }

    async fn call(&self, input: Value, ctx: &crate::tools::ToolContext) -> Result<Value> {
        // `doc()` bundles read and write actions under one tool name. Only the
        // mutating ones need the same worktree-activation choice edit_file /
        // edit_code / edit_markdown / create_file / approve_write already
        // require — otherwise doc(append_entry) writes silently to a tree
        // edit_file on the same path would refuse. This is the one place that
        // sees BOTH the core ToolContext (`.agent`, which the guard reads) and
        // the tool's `action` argument. See
        // docs/issues/archive/2026-09-03-the-worktree-write-guard-covers-file-writes-and-no-doc-action.md.
        if self.inner.name() == "doc" {
            let action = input.get("action").and_then(Value::as_str).unwrap_or("");
            if is_mutating_doc_action(action) {
                crate::tools::guard_worktree_write(ctx)
                    .await
                    .map_err(bridge_recoverable_error)?;
            }
        }

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
        let lib_ctx = self.derive_ctx(active_root.as_deref(), ctx.progress.clone());
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

    /// Does this call mutate anything the cross-process write guard protects?
    ///
    /// **Reads are the closed set; everything else is a write.** The inverse —
    /// enumerating the writes — is what got this wrong twice. The `artifact` arm
    /// listed five mutating actions while the dispatcher routed three more
    /// (`graft`, `append_entry`, `update_entry`), all unguarded; `append_entry`
    /// exists *because* concurrent sessions race on entry-id allocation. The bug
    /// report that found those three then missed `audit_log`, whose
    /// `prune_before_ms` + `confirm=true` runs `DELETE FROM catalog_audit` and
    /// whose `export=true` appends to a committed shard. Two independent
    /// enumerations of one small set, both short.
    ///
    /// The asymmetry settles the direction. An unlisted **read** is merely
    /// over-serialised — it takes a lock nobody was contending for. An unlisted
    /// **write** races, on a checkout that routinely carries five or more
    /// concurrent sessions. So a new action, and via the final arm a new
    /// librarian *tool*, is guarded on the day it is added; opting out is a
    /// deliberate edit to a read set right here.
    ///
    /// **The exception is a long read, and it is not a softening of the rule.**
    /// `doctor` and `audit_log` scan the whole catalog in their common form.
    /// Guarding them unconditionally would hold the write lock for the length of
    /// a diagnostic and block every other session's writes behind it — trading
    /// this bug for an availability one. They get a read-default arm each,
    /// keyed on their own schema-documented repair opt-in and over-approximated
    /// (`doctor` on `fix` being present at all, so a dry run is guarded too).
    ///
    /// docs/issues/archive/2026-09-02-is-write-omits-five-mutating-actions-so-the-write-guard-never-fires.md
    ///
    /// Task 4 folded the `artifact_event` tool into `doc` (`event_create` /
    /// `event_list`); `event_list` joins the doc read-set below, `event_create`
    /// falls through the `doc` arm's default (write), same as every other
    /// mutating `doc` action. Task 6 folded the standalone `artifact_refresh`
    /// tool into `doc` as `gather` / `list_stale` — both read-only (`gather`
    /// collects context without writing; `list_stale` only lists), so both join
    /// the doc read-set below and the old `"artifact_refresh" => false` arm is
    /// gone with the tool it guarded.
    fn is_write(&self, input: &Value) -> bool {
        let action = input.get("action").and_then(Value::as_str);
        match self.inner.name() {
            // CRUD tool. find/get/graph/state_at/event_list/gather/list_stale are
            // the only reads; create, update, move, delete, link, graft,
            // append_entry, update_entry, event_create and augment all mutate, as
            // does any action added after this line.
            "doc" => !matches!(
                action,
                Some(
                    "find" | "get" | "graph" | "state_at" | "event_list" | "gather" | "list_stale"
                )
            ),
            "librarian" => match action {
                // Unconditional reads.
                Some("context" | "tracker_design" | "workspace_state_at") => false,
                // Conditional arms, and the polarities are NOT uniform — do not
                // copy one onto another. audit_doc_refs and legibility_scan write
                // by default and opt out on an explicit flag; link_scan, doctor
                // and audit_log read by default and opt IN.
                Some("audit_doc_refs") => {
                    input.get("emit_tracker").and_then(Value::as_bool) != Some(false)
                }
                Some("legibility_scan") => {
                    input.get("write").and_then(Value::as_bool) != Some(false)
                }
                Some("link_scan") => input.get("write").and_then(Value::as_bool) == Some(true),
                // Read-default because both are long full-catalog scans — see the
                // doc comment. A write mode added to either that does not route
                // through these keys must update this arm; `fix` and
                // `export`/`confirm` are the schema's own repair opt-ins.
                Some("doctor") => input.get("fix").is_some(),
                Some("audit_log") => {
                    input.get("export").and_then(Value::as_bool) == Some(true)
                        || input.get("confirm").and_then(Value::as_bool) == Some(true)
                }
                // reindex, merge_worktree, and anything added later.
                _ => true,
            },
            // Every librarian tool is wrapped by the blanket map over
            // `lib_all_tools()` above, so this arm catches tools added later. It
            // returns `true` for the reason the arms above default to write: an
            // unclassified tool that mutates is the failure mode, and an
            // unclassified tool that reads costs only serialisation.
            _ => true,
        }
    }

    fn relevant_guide_topic(&self, result: &Value) -> Option<&str> {
        // Three guides serve this tool and only one can be delivered per call.
        //
        // An overflowing result wins first. Unlike `symbols`/`references`/`call_graph`
        // (`Symbols::relevant_guide_topic`), which self-report their OWN truncation as
        // `result["overflow"]` before this is even called, `doc(find|get)` sets neither
        // `overflow` nor `output_id` on its raw pre-buffer value -- those two only exist
        // on the envelope `call_content` builds AFTER this function returns, from the
        // generic byte-threshold path. So `result.get("overflow").is_some() ||
        // result.get("output_id").is_some()` -- the check this adapter shipped with
        // first, copied from `Symbols` -- is dead code here: always false, on every
        // call, overflowing or not. It passed its own unit test only because that test
        // hand-inserted an `output_id` key no real `doc()` call ever produces at this
        // point in the pipeline; the test mutated its input rather than exercising the
        // production path. Caught live: rebuilding and reconnecting, the exact overflow
        // + tracker-path call from the T-33 investigation still routed to
        // `tracker-conventions`, proving the shipped condition never fires.
        //
        // The fix computes the SAME formula `call_content` uses for its independent
        // `PostCtx::overflowing` gate (`src/engines/coordinator.rs`) -- `overflowing` is
        // precomputed there because deciding it needs the serialised JSON, which the
        // coordinator does not hold, and `relevant_guide_topic` does not hold `json`
        // either, only `result`, so it is recomputed here. `emit_guide_sections` gates
        // `"progressive-disclosure"` on `ctx.overflowing` regardless of what this
        // function names, so if this ever drifts from that formula the topic silently
        // stops firing again rather than erroring -- keep the two in lockstep with the
        // one in `src/tools/core/types.rs::exceeds_inline_limit`.
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
        // both over its lifetime — the two largest guides in the corpus, by a wide margin
        // (34.7 KB + 20.1 KB, measured 2026-08-27; the third-largest is 11.7 KB). That is
        // the byte tension BL-25 records; the corpus cut is the answer to it, not
        // withholding the guide.
        //
        // The RELATION is what carries the point and it survives growth; the figures are a
        // dated snapshot on purpose. An undated one here read "10.4 KB" for eleven days
        // while the file grew to 3.4x that — and the same sentence's other figure was
        // accurate to 1%, with nothing to tell a reader which was which. See
        // `claim-decay:DC-1`, which is also why the sibling claim in `server.rs` was
        // deleted outright rather than corrected: there the number did no work.
        //
        // See `docs/issues/archive/2026-08-16-cap-evicted-guidance-lands-in-guides-nothing-triggers.md`.
        let overflowing = crate::tools::exceeds_inline_limit(
            &serde_json::to_string(result).unwrap_or_else(|_| result.to_string()),
        ) || result
            .as_object()
            .and_then(|o| o.get("output_id"))
            .and_then(|v| v.as_str())
            .is_some();
        if overflowing {
            return Some("progressive-disclosure");
        }
        if names_tracker_path(result) {
            return Some("tracker-conventions");
        }
        Some("librarian")
    }

    /// Point a buffered `doc(get)` at its **body**, not at the largest array in the
    /// envelope. Decision extracted to [`scoped_body_hint`] so a test can reach it without
    /// building an adapter — the same shape `format_compact` uses for
    /// `librarian_compact_summary`.
    /// docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
    fn json_path_hint(&self, val: &Value) -> String {
        scoped_body_hint(val).unwrap_or_else(|| crate::tools::default_json_path_hint(val))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        librarian_compact_summary(self.inner.name(), result)
    }
}

impl LibrarianAdapter {
    /// Build a fresh `LibToolContext` for a single tool call, using the
    /// host's currently-active project to derive `current_project`. The
    /// catalog/workspace/rules/embedding stay shared with the boot-time ctx.
    fn derive_ctx(
        &self,
        active: Option<&std::path::Path>,
        progress: Option<Arc<crate::tools::progress::ProgressReporter>>,
    ) -> Arc<LibToolContext> {
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
            // Carried over rather than re-resolved: this rebuilds a per-call context from
            // the long-lived one, and re-reading the environment here would reintroduce
            // the ambient dependency the field exists to remove.
            temp_guard: self.ctx.temp_guard.clone(),
            // Comes from the CORE context, not `self.ctx` — this is the one place the two
            // context types meet, and the progress reporter is per-call state that only
            // the core side receives. `None` when the client sent no progress token.
            progress,
        })
    }
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
/// 1. **Incompleteness.** First any **cut finding array** (`counts.truncated`), named as
///    `name[shown of total]` — see [`finding_truncation_summary`] for why the warning
///    cannot live in the emitting tool's own `hint`. Then the `doc(get)` body cap
///    (`$.overflow.shown_lines`), then any other action's own `$.overflow.hint`. A body large enough to trip the cap also
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
    let is_artifact = inner_name == "doc";
    let mut lines: Vec<String> = Vec::new();

    // Leads, per the ordering rule above: a cut finding array is the strongest
    // incompleteness signal a librarian result carries, because absence from a
    // truncated bucket reads as a clean answer rather than as a missing one.
    // Keyed on shape, not tool, so it is inert for artifact-shaped results.
    if let Some(cut) = finding_truncation_summary(result) {
        lines.push(cut);
    }
    if let Some(elided) = elided_rows_summary(result) {
        lines.push(elided);
    }

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
         selector — doc(get, id=…, heading=\"<section>\") or start_line=N, \
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
/// Declines on the `doc(get)` body cap, which [`body_truncation_warning`] states
/// more loudly and more specifically just above.
fn overflow_hint(result: &Value) -> Option<String> {
    let overflow = result.get("overflow")?.as_object()?;
    if overflow.contains_key("shown_lines") {
        return None;
    }
    let hint = overflow.get("hint")?.as_str()?;
    Some(format!("INCOMPLETE — {hint}"))
}

/// Name every finding array that was **cut**, as `name[shown of total]`.
///
/// `link_scan` caps each finding array at 50, records the true totals in `counts`, and
/// keeps an accurate per-array `counts.truncated` map beside them. All three are correct
/// and none of them reaches the caller: once the payload overflows it is buffered, and
/// the envelope carries only the generic shape line, which renders every array as
/// `dangling[50]` — indistinguishable from fifty findings.
///
/// So a reader who searches a bucket for a token and does not find it has searched
/// **7.8%** of the population with nothing on the surface saying so. Measured 2026-08-30
/// on this repo: `dangling` 50 of 637, `ambiguous` 50 of 551 — the two buckets a reader
/// is most likely to interrogate are the two truncated hardest.
/// `docs/issues/archive/2026-08-30-link-scan-truncation-is-accurate-and-unreachable.md`.
///
/// **Why here and not in `link_scan`'s own `hint`,** which the bug file proposed as the
/// cheapest fix: that `hint` is a key *inside* the payload (`link_scan/mod.rs`), so it is
/// buffered away by the very overflow that creates the defect. Writing the warning there
/// would place it in the one location the caller demonstrably does not read — and a test
/// asserting on it would pass while the bug stayed live, which is the same shape as
/// asserting on `counts`.
///
/// Keyed on the **shape** rather than the tool name, so any librarian action adopting the
/// `counts.truncated` convention is covered without a second edit here. The shape is
/// specific enough not to collide: a `counts.truncated` object of booleans whose keys name
/// sibling arrays.
///
/// Reports only arrays flagged `true`. Naming untruncated ones too would teach the reader
/// that this line means *every* array, and the next genuinely cut bucket would read as
/// already accounted for.
fn finding_truncation_summary(result: &Value) -> Option<String> {
    let counts = result.get("counts")?.as_object()?;
    let truncated = counts.get("truncated")?.as_object()?;

    // The window the caller actually received. Without it a paged response renders
    // `dangling[37 of 637]` at offset 600 — a true-looking understatement of exactly the
    // kind this line exists to prevent, and one that gets MORE wrong the further a caller
    // has paged, i.e. the more work they did to be sure.
    //
    // Absent (any pre-window producer, or a non-link_scan action adopting `counts`) reads
    // as 0, which is what an unpaged response means and keeps that rendering unchanged.
    let offset = counts
        .get("findings_window")
        .and_then(|w| w.get("offset"))
        .and_then(Value::as_u64)
        .unwrap_or(0);

    let mut cut: Vec<String> = Vec::new();
    for (name, was_cut) in truncated {
        if was_cut.as_bool() != Some(true) {
            continue;
        }
        // A flag without its pair is not reportable as `N of M`. Skip rather than
        // abandon the whole summary: one malformed entry must not suppress the
        // warning for its siblings, which would be this defect again.
        let Some(shown) = result.get(name).and_then(Value::as_array).map(Vec::len) else {
            continue;
        };
        let Some(total) = counts.get(name).and_then(Value::as_u64) else {
            continue;
        };
        cut.push(match offset {
            0 => format!("{name}[{shown} of {total}]"),
            n => format!("{name}[{shown} from {n} of {total}]"),
        });
    }

    if cut.is_empty() {
        return None;
    }
    Some(format!(
        "TRUNCATED: {} — absence from a cut list is not evidence.",
        cut.join(", ")
    ))
}

/// The same incompleteness signal under `doctor`'s shape: `summary.shown` against
/// `summary.total`, with no per-array `counts.truncated` map.
///
/// Kept separate from [`finding_truncation_summary`] rather than folded into it, because
/// the two answer different questions. That one reports **which** arrays were cut, from a
/// map that names them; this one reports **how many** rows a single set lost, from a pair
/// of totals. Merging them would need a union shape that fits neither and would make each
/// branch harder to read than the two functions are.
///
/// **Latent, not live, and that is the argument for it.** Measured 2026-08-31 on this
/// repo: `total: 136, shown: 136` — nothing elided, because only 5 `abs_path_outside_
/// managed_roots` rows existed against a window of 10. Both of `doctor`'s `retain` passes
/// are deliberate and both can cut. When one does, the envelope will read `violations[N]`
/// exactly as it does on a complete run, so nothing prompts anyone to look; the bug file
/// records `417 of 1314` on another machine.
///
/// Names the array whose length matches `shown` so the reader knows which set shrank,
/// falling back to `rows` when no array matches — a wrong name would be worse than none,
/// and `shown`/`total` alone still carry the finding.
fn elided_rows_summary(result: &Value) -> Option<String> {
    let summary = result.get("summary")?.as_object()?;
    let total = summary.get("total")?.as_u64()?;
    let shown = summary.get("shown")?.as_u64()?;
    if shown >= total {
        return None;
    }

    let name = result
        .as_object()
        .into_iter()
        .flatten()
        .find(|(_, v)| v.as_array().is_some_and(|a| a.len() as u64 == shown))
        .map_or("rows", |(k, _)| k.as_str());

    Some(format!(
        "TRUNCATED: {name}[{shown} of {total}] — absence from a cut list is not evidence."
    ))
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
/// `doc(get, heading="…")` call to make instead of pulling the whole body out of
/// the buffer. Rendered with their level markers so a heading can be passed straight
/// back as that argument.
///
/// This is one side of a cross-file contract with `stub_preview` (`src/librarian/tools/get.rs`):
/// this function reads `preview.headings` as an array via `.as_array()?` and early-returns
/// `None` the instant that fails. `stub_preview` replaces that array with a string note
/// (`HEADINGS_OMITTED_NOTE`) whenever `doc(get)`'s caller already selected a body — so a
/// body-selected `get` produces no "sections: …" line here, silently, for free. That is the
/// intended effect, not a bug in this function: the caller already picked a section, and a
/// redundant list of every section would answer a question they did not ask.
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

/// `doc()` actions that mutate the catalog or its backing files — the set
/// [`LibrarianAdapter::call`] must run `guard_worktree_write` against before
/// dispatch. Read actions (`find`, `get`, `graph`, `state_at`, `event_list`,
/// `gather`, `list_stale`) resolve the same tree regardless of activation and
/// are exempt.
fn is_mutating_doc_action(action: &str) -> bool {
    matches!(
        action,
        "create"
            | "update"
            | "move"
            | "delete"
            | "graft"
            | "link"
            | "append_entry"
            | "update_entry"
            | "event_create"
            | "augment"
    )
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

    /// A heading-scoped `get` must be pointed at the section it returned, not at the
    /// heading map it did not ask for.
    ///
    /// Born red: before the override the hint came from `default_json_path_hint`, which
    /// selects the largest array within a bounded depth — `preview.headings` (20 objects
    /// on `issue-clusters.md`) beats a `body` string, which is not an array and so can
    /// never win an array-selecting rule.
    ///
    /// **Row 2 is the discriminator and must not be deleted as redundant.** A *full* read
    /// also carries `body`, and there the largest-array rule is still better — an
    /// augmented tracker's params collection is worth more than a body the caller already
    /// has whole. Without that row, a fix keyed on `body` alone (rather than on
    /// `body_meta`) passes, and every full read gets a worse hint. Row 3 pins the same
    /// boundary from the other side: `body_meta` present with no `body` is not a scoped
    /// body read.
    ///
    /// Mutations this kills: dropping the `body_meta` check → row 2 fails; dropping the
    /// `body` check → row 3 fails; returning `Some` unconditionally → rows 2 and 4 fail.
    /// docs/issues/archive/2026-09-01-heading-scoped-get-overflow-hint-points-at-metadata.md
    #[test]
    fn a_scoped_read_is_hinted_at_its_body_and_a_full_read_is_not() {
        for (label, payload, expect) in [
            (
                "heading-scoped get — the reported case",
                json!({
                    "id": "1b5a080fe2efcb6b",
                    "body": "## Index\n\n| id | class |\n",
                    "body_meta": { "heading": "## Index", "bytes": 25531, "line_count": 277 },
                    "preview": { "headings": [{"text": "a"}, {"text": "b"}, {"text": "c"}] },
                    "tags": ["one", "two", "three", "four", "five"],
                }),
                Some("$.body"),
            ),
            (
                "FULL get — body present, no body_meta: keep the array heuristic",
                json!({
                    "id": "x",
                    "body": "# Whole file\n",
                    "augmentation": { "params": { "tasks": [{"id": "T-1"}, {"id": "T-2"}] } },
                }),
                None,
            ),
            (
                "body_meta with no body — not a body read",
                json!({ "id": "x", "body_meta": { "heading": "## A" } }),
                None,
            ),
            (
                "find — neither field",
                json!({ "count": 2, "items": [{"id": "a"}, {"id": "b"}] }),
                None,
            ),
        ] {
            assert_eq!(scoped_body_hint(&payload).as_deref(), expect, "{label}");
        }
    }

    /// The end-to-end shape, at the trait method rather than the extracted decision, so
    /// the wiring is covered too: a scoped payload must not fall through to the array
    /// heuristic. Pinned against `default_json_path_hint` directly rather than against a
    /// hardcoded string, so this stays true if that heuristic's own answer changes.
    #[test]
    fn the_scoped_hint_overrides_what_the_default_heuristic_would_have_said() {
        let payload = json!({
            "body": "## Index\n",
            "body_meta": { "heading": "## Index" },
            "preview": { "headings": [{"text": "a"}, {"text": "b"}] },
        });
        let default = crate::tools::default_json_path_hint(&payload);
        assert_eq!(
            default, "$.preview.headings[*]",
            "precondition: the default must pick the heading map here, or this test is \
             not about the reported defect"
        );
        assert_eq!(
            scoped_body_hint(&payload).unwrap_or(default),
            "$.body",
            "the override must win for a scoped read"
        );
    }

    #[test]
    fn compact_summary_surfaces_artifact_get_body_truncation() {
        // Mirrors the real bug: get(full=true) capped body + sibling overflow,
        // whole response buffered. The summary must announce the truncation.
        let result = json!({
            "id": "x",
            "body": "…capped body…",
            "overflow": { "shown_lines": 500, "total_lines": 1841, "hint": "…" },
        });
        let summary = librarian_compact_summary("doc", &result)
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
        assert!(librarian_compact_summary("doc", &result).is_none());
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

        let summary = librarian_compact_summary("doc", &result)
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

        let summary = librarian_compact_summary("doc", &result)
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

    /// `section_headings_summary` requires `preview.headings` to be an ARRAY. Once a
    /// body-selected read stubs that key to a string, the summary can no longer lead with
    /// the heading map — so the fix in `get.rs` delivers this for free and no second gate
    /// is needed here.
    /// Controller Ruling 2,
    /// docs/issues/archive/2026-09-01-a-scoped-read-is-billed-the-full-heading-map.md
    ///
    /// `body` is padded past `dominant_text_preview`'s 200-byte `MIN_LEN` so
    /// `librarian_compact_summary` has a non-heading signal to report — without it every
    /// contributor (finding-truncation, elided-rows, body-cap, overflow-hint, matched-items,
    /// section-headings, dominant-text) returns `None` for this fixture, `lines` stays empty,
    /// and the function returns `None` regardless of the headings-array-vs-string fix,
    /// panicking `.expect("a summary")` below for a reason unrelated to what this test names.
    /// Verified by first running this test with the short body from the plan, which panicked
    /// at `.expect("a summary")` with no headings-related message at all.
    #[test]
    fn a_body_selected_read_summary_cannot_lead_with_the_heading_map() {
        // Any non-array `headings` value satisfies this fixture's assertion — the literal
        // stub string below is illustrative, not load-bearing; `section_headings_summary`
        // only requires `.as_array()` to fail, and a bare stubbed-marker string does that.
        let result = json!({
            "body": "## Index\n\nthe section the caller asked for, padded past the two-hundred-byte \
                      dominant-text-preview threshold so the compact summary has a non-heading \
                      signal to report instead of returning None for an unrelated reason.",
            "body_meta": { "heading": "## Index", "line_count": 2, "bytes": 40 },
            "preview": {
                "shape": "default",
                "headings": "omitted (body selector present) — call doc(get, id=…) with no body selector for the map",
                "total_headings": 12
            }
        });

        let summary = librarian_compact_summary("doc", &result).expect("a summary");

        assert!(
            !summary.contains("sections:"),
            "a body-selected read must not lead with the heading map, got: {summary}"
        );
    }

    /// The positive twin: an UNSCOPED result still carries an array, and the summary must
    /// still lead with the map. Without this, deleting `section_headings_summary`'s call
    /// site passes the test above while destroying the map for everyone.
    #[test]
    fn an_unscoped_read_summary_still_leads_with_the_heading_map() {
        let result = json!({
            "body": "# T\n\n## Alpha\n\n## Index\n",
            "preview": { "headings": [
                { "level": 2, "text": "Alpha", "line": 3 },
                { "level": 2, "text": "Index", "line": 5 }
            ]}
        });

        let summary = librarian_compact_summary("doc", &result).expect("a summary");

        assert!(
            summary.contains("sections:"),
            "an unscoped read must keep the heading map, got: {summary}"
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

        let summary = librarian_compact_summary("doc", &result)
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
    /// gated on `inner_name == "doc"`.
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
            librarian_compact_summary("doc", &body_capped).expect("body cap still summarises");
        assert_eq!(
            summary.matches("500").count(),
            1,
            "the body-cap case must be announced once, not twice: {summary}"
        );
    }

    /// `link_scan` caps every finding array at 50 and records the real totals in
    /// `counts`, with an accurate per-array `counts.truncated` map beside them. All of
    /// that is correct and none of it reaches the caller: on overflow the payload is
    /// buffered and the envelope shows the generic shape line, which renders every array
    /// as `dangling[50]` — indistinguishable from fifty findings.
    ///
    /// So a reader who searches a bucket for a token and does not find it has searched
    /// **7.8%** of the population with nothing on the surface saying so. Measured
    /// 2026-08-30 on this repo: `dangling` 50 of 637, `ambiguous` 50 of 551.
    ///
    /// Asserting on the SUMMARY STRING is the whole point of this test. The information
    /// already exists in the payload, so an assertion aimed at `result["counts"]` — or at
    /// `link_scan`'s own `hint` key, which is *inside* the buffered payload — passes while
    /// the caller still sees nothing. That is the defect, not a weaker form of it.
    #[test]
    fn compact_summary_names_the_real_total_for_a_truncated_finding_array() {
        let rows = |n: usize| -> Vec<Value> {
            (0..n)
                .map(|i| json!({ "token": format!("R-{i}") }))
                .collect()
        };
        // `ambiguous` is PRESENT and complete, so the truncated flag is the only thing
        // that can exclude it. An absent array would be skipped by the missing-array
        // guard instead, and the exclusion assertion below could never fire — it would
        // pass under the very mutation it exists to catch.
        let result = json!({
            "scope": "project",
            "dangling": rows(50),
            "ambiguous": rows(12),
            "counts": {
                "dangling": 637,
                "ambiguous": 12,
                "truncated": { "dangling": true, "ambiguous": false },
            },
        });

        let summary = librarian_compact_summary("librarian", &result)
            .expect("a truncated finding array is an incompleteness signal and must surface");

        assert!(
            summary.contains("50 of 637"),
            "the shown count is worthless without the total it is a fraction of: {summary}"
        );
        assert!(
            summary.contains("dangling"),
            "the truncated bucket must be named, since only some arrays are cut: {summary}"
        );
        let warning = summary
            .lines()
            .find(|l| l.contains("TRUNCATED"))
            .unwrap_or_else(|| panic!("no truncation line at all: {summary}"));
        assert!(
            !warning.contains("ambiguous"),
            "`ambiguous` is present and NOT flagged truncated, so naming it would teach \
             the reader that this line means 'every array' — and the next genuinely cut \
             bucket would read as already covered: {warning}"
        );
    }

    /// The sibling direction, and the one that keeps the message meaningful: a scan whose
    /// arrays all fit must say nothing here. Without this, a summary that unconditionally
    /// printed the counts would satisfy the test above while carrying no information —
    /// the reader could not tell a complete result from a cut one, which is the same
    /// conflation one level up.
    #[test]
    fn compact_summary_is_silent_when_no_finding_array_was_cut() {
        let result = json!({
            "scope": "project",
            "dangling": [ { "token": "R-1" } ],
            "counts": {
                "dangling": 1,
                "truncated": { "dangling": false, "ambiguous": false },
            },
        });

        assert!(
            librarian_compact_summary("librarian", &result).is_none(),
            "an untruncated scan must leave the slot to the generic describer"
        );
    }

    /// A paged response must say WHERE its window sits, not only how big it is.
    ///
    /// At `offset` 600 of 637 the unpaged rendering reads `dangling[10 of 637]` — true
    /// as arithmetic and false as a report: it implies 627 are unseen when 600 of them
    /// are precisely what the caller has already paged through. The error grows with how
    /// far the reader paged, which is to say with how much work they did to be sure. That
    /// is the worst possible direction for a line whose entire job is to stop a premature
    /// "nothing here" reading.
    ///
    /// The offset-0 rendering is deliberately unchanged and is pinned by
    /// `compact_summary_names_the_real_total_for_a_truncated_finding_array` — so this
    /// test would not catch a regression that dropped the window lookup *and* the old
    /// form together, and that one would.
    ///
    /// Mutation that must kill this: drop the `findings_window` lookup, so `offset`
    /// reads 0 and the unpaged form renders unconditionally.
    #[test]
    fn compact_summary_names_the_offset_when_the_caller_paged() {
        let rows = |n: usize| -> Vec<Value> {
            (0..n)
                .map(|i| json!({ "token": format!("R-{i}") }))
                .collect()
        };
        let result = json!({
            "scope": "project",
            "dangling": rows(10),
            "counts": {
                "dangling": 637,
                "findings_window": { "offset": 600, "limit": 10 },
                "truncated": { "dangling": true },
            },
        });

        let summary = librarian_compact_summary("librarian", &result)
            .expect("a cut array is an incompleteness signal whether or not it was paged");

        assert!(
            summary.contains("dangling[10 from 600 of 637]"),
            "a paged window must state its offset, or 10-of-637 reads as 627 unseen when \
             600 of those were already returned to this caller: {summary}"
        );
    }

    /// `doctor` elides rows too, by a different mechanism and under a different key:
    /// `summary.shown` against `summary.total`, with no `counts.truncated` map. Two
    /// `retain` passes can cut rows — scoped rows belonging to another project, and the
    /// `abs_path_outside_managed_roots` window (default 10) — and both are deliberate.
    ///
    /// It is **latent rather than live** on this corpus: a real run measured
    /// `total: 136, shown: 136`, because only 5 outside-root rows existed and the window
    /// is 10. That is precisely why it needs the guard now. When the window does bite the
    /// envelope will read `violations[N]` exactly as it does today, so there is no moment
    /// at which anyone is prompted to look — the bug file records `417 of 1314` on another
    /// machine.
    #[test]
    fn compact_summary_names_the_real_total_when_rows_were_elided() {
        let result = json!({
            "violations": (0..136).map(|i| json!({ "check": format!("c{i}") })).collect::<Vec<_>>(),
            "summary": { "total": 1314, "shown": 136, "by_check": { "missing_file": 18 } },
            "catalog_health": {},
        });

        let summary = librarian_compact_summary("librarian", &result)
            .expect("an elided row set is an incompleteness signal and must surface");

        assert!(
            summary.contains("136 of 1314"),
            "the shown count is worthless without the total it is a fraction of: {summary}"
        );
        // Scoped to the TRUNCATED line, not the whole summary: the generic shape
        // description appended below it already prints `arrays: violations[136]`, so a
        // whole-summary `contains` passes whatever this line says. That is the same
        // vacuous shape the sibling test's exclusion assertion hit, caught the same way
        // — by a mutation that makes the name lookup miss.
        let warning = summary
            .lines()
            .find(|l| l.contains("TRUNCATED"))
            .unwrap_or_else(|| panic!("no truncation line at all: {summary}"));
        assert!(
            warning.contains("violations["),
            "the SET that shrank must be named — a report carrying several arrays leaves \
             the reader unable to tell which one they are seeing a fraction of: {warning}"
        );
    }

    /// The sibling direction. `shown == total` is the overwhelmingly common case — the
    /// live run that motivated this test was one — so a summary that announced itself
    /// whenever `summary` merely *carried* the two keys would cry wolf on every clean
    /// report, and the one that mattered would read as routine.
    #[test]
    fn compact_summary_is_silent_when_every_row_was_shown() {
        let result = json!({
            "violations": [ { "check": "missing_file" } ],
            "summary": { "total": 136, "shown": 136, "by_check": { "missing_file": 18 } },
            "catalog_health": {},
        });

        assert!(
            librarian_compact_summary("librarian", &result).is_none(),
            "a complete report must leave the slot to the generic describer"
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

        let derived = adapter.derive_ctx(Some(&wt), None);
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

        let derived = adapter.derive_ctx(Some(tmp.path()), None);
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

        let derived = adapter.derive_ctx(Some(&wt), None);
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
    /// `derive_ctx` is the only place the core and librarian `ToolContext` types meet,
    /// so it is the only place a per-call progress reporter can cross. If it stops
    /// carrying one, every librarian tool silently loses the ability to tell its caller
    /// it is alive — and `reindex`'s own progress test would NOT catch that, because it
    /// sets `ctx.progress` directly rather than going through the adapter.
    ///
    /// That gap is the `declared-not-wired` shape the field was added to close, so it
    /// gets a test at the seam rather than only at the consumer.
    /// `docs/issues/2026-09-05-librarian-tools-cannot-emit-progress-so-a-long-reindex-times-the-caller-out.md`
    #[test]
    fn derive_ctx_carries_the_callers_progress_reporter_across_the_boundary() {
        struct NullSink;
        #[async_trait::async_trait]
        impl crate::tools::progress::ProgressSink for NullSink {
            async fn emit_progress(
                &self,
                _step: f64,
                _total: Option<f64>,
                _token: &rmcp::model::NumberOrString,
            ) {
            }
            async fn emit_text(&self, _text: &str) {}
        }

        let tmp = tempfile::TempDir::new().unwrap();
        let adapter = adapter_for_test();

        let reporter = crate::tools::progress::ProgressReporter::with_sink(
            Arc::new(NullSink),
            rmcp::model::NumberOrString::Number(7),
        );
        let derived = adapter.derive_ctx(Some(tmp.path()), Some(reporter));
        assert!(
            derived.progress.is_some(),
            "derive_ctx must carry the caller's progress reporter onto the librarian \
             context — without it every librarian tool is structurally unable to report"
        );

        // The other direction, and it is not symmetry for its own sake: a reporter
        // appearing when the client sent no progress token is an unsolicited
        // notification, which crashed Claude Code 2.x. `None` in must stay `None` out —
        // never synthesized from the request id.
        // docs/issues/archive/2026-06-14-progress-notifications-unsolicited-token.md
        let derived_none = adapter.derive_ctx(Some(tmp.path()), None);
        assert!(
            derived_none.progress.is_none(),
            "a caller that sent no progress token must yield no reporter"
        );
    }

    /// Mirrors `seed_linked_worktree` in `src/tools/core/tests.rs`: makes `root`
    /// look like a checkout with one linked worktree, the way
    /// `list_git_worktrees` reads it.
    fn seed_linked_worktree_for_guard(root: &std::path::Path, name: &str) {
        let wt_root = root.parent().unwrap().join(format!("wt-{name}"));
        std::fs::create_dir_all(&wt_root).unwrap();
        let entry = root.join(".git").join("worktrees").join(name);
        std::fs::create_dir_all(&entry).unwrap();
        std::fs::write(
            entry.join("gitdir"),
            format!("{}/.git\n", wt_root.display()),
        )
        .unwrap();
    }

    /// A core `crate::tools::ToolContext` rooted at `root`, not yet activated
    /// this session — mirrors `rooted_ctx` in `src/tools/core/tests.rs`. Needed
    /// here (not shared with that module) because `LibrarianAdapter::call`
    /// takes the CORE `ToolContext` (the one with `.agent`), not the
    /// librarian's own.
    async fn core_ctx_for_guard(root: &std::path::Path) -> crate::tools::ToolContext {
        std::fs::create_dir_all(root.join(".codescout")).unwrap();
        crate::tools::ToolContext {
            agent: crate::agent::Agent::new(Some(root.to_path_buf()))
                .await
                .unwrap(),
            lsp: crate::lsp::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(
                crate::tools::guide_ledger::GuideLedger::mid_session(),
            )),
            workspace_override: None,
        }
    }

    /// Closes docs/issues/archive/2026-09-03-the-worktree-write-guard-covers-file-writes-and-no-doc-action.md:
    /// a `doc` mutation must be refused exactly like `edit_file` would be, when
    /// worktrees exist and this session never chose one via `activate`.
    #[tokio::test]
    async fn doc_mutation_is_blocked_when_worktrees_exist_and_not_activated() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("main");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        seed_linked_worktree_for_guard(&root, "feat");
        let ctx = core_ctx_for_guard(&root).await;
        let adapter = adapter_for_test();

        let input = json!({
            "action": "create",
            "kind": "bug",
            "title": "x",
            "rel_path": "docs/issues/x.md",
            "body": "# x",
        });
        let result = adapter.call(input, &ctx).await;

        assert!(
            result.is_err(),
            "a doc mutation with worktrees present and no activate() must be refused"
        );
        assert!(
            result.unwrap_err().to_string().contains("Write blocked"),
            "must be the worktree-activation refusal specifically, not some other failure"
        );
    }

    /// The other half: once `activate` has been called this session, the same
    /// mutation must go through (mirrors
    /// `guard_worktree_write_allows_after_explicit_activate` in
    /// `src/tools/core/tests.rs`).
    #[tokio::test]
    async fn doc_mutation_allowed_after_explicit_activate() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("main");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        seed_linked_worktree_for_guard(&root, "feat");
        let ctx = core_ctx_for_guard(&root).await;
        ctx.agent.activate(root.clone(), None).await.unwrap();
        let adapter = adapter_for_test();

        let input = json!({
            "action": "create",
            "kind": "bug",
            "title": "x",
            "rel_path": "docs/issues/x.md",
            "body": "# x",
        });
        let result = adapter.call(input, &ctx).await;

        assert!(
            result.is_ok(),
            "the caller chose this session; the doc mutation must be allowed: {:?}",
            result.err()
        );
    }

    /// A read action (`find`) must never be gated by the worktree-activation
    /// guard — only the ten mutating actions the bug names.
    #[tokio::test]
    async fn doc_read_is_not_blocked_by_worktree_guard() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("main");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        seed_linked_worktree_for_guard(&root, "feat");
        let ctx = core_ctx_for_guard(&root).await;
        let adapter = adapter_for_test();

        let input = json!({"action": "find"});
        let result = adapter.call(input, &ctx).await;

        if let Err(e) = &result {
            assert!(
                !e.to_string().contains("Write blocked"),
                "a read action must never be refused by the worktree-write guard: {e}"
            );
        }
    }

    #[test]
    fn selector_key_projects_tool_and_action() {
        let a = adapter_for_test();
        assert_eq!(
            a.selector_key(&json!({"action": "append_entry", "id": "x"})),
            Some("doc.append_entry".to_string())
        );
        // No action ⇒ the bare tool name. Tool-only shapes (a tool with no `action`
        // param at all) must still be matchable by a declaration keyed on the tool
        // name alone — see the CORRECTION in task-4-brief.md. Falling back to `?`
        // here would make such a declaration unmatchable forever, which is a
        // silent-absence failure, not the fail-safe-toward-delivery direction this
        // feature requires.
        assert_eq!(
            a.selector_key(&json!({"id": "x"})),
            Some(a.name().to_string())
        );
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

    #[test]
    fn overflow_wins_the_guide_slot_even_on_a_tracker_path() {
        // Regression for the gap this session found: `doc(find|get)` buffers into an
        // @tool_* handle exactly like `symbols`/`references`/`call_graph`, but this
        // adapter's topic split never checked for it, so an overflowing `doc` call could
        // never surface `progressive-disclosure` -- only `librarian`/`tracker-conventions`.
        //
        // The payload here is a REAL overflow, not a stand-in: earlier this session a
        // version of this test hand-inserted an `output_id` key to simulate overflow,
        // which is exactly the field `doc()`'s raw pre-buffer result never carries in
        // production (only `call_content`'s post-buffer envelope does) -- so that test
        // passed while the shipped fix was dead code, confirmed live after a rebuild.
        // Padding past `exceeds_inline_limit`'s ~10KB threshold exercises the same
        // condition `call_content` actually evaluates.
        let a = adapter_for_test();
        let padding = "x".repeat(11_000);
        let overflowed_tracker_result = json!({
            "abs_path": "docs/trackers/tool-usage-patterns.md",
            "padding": padding,
        });
        assert!(crate::tools::exceeds_inline_limit(
            &serde_json::to_string(&overflowed_tracker_result).unwrap()
        ));
        assert_eq!(
            a.relevant_guide_topic(&overflowed_tracker_result),
            Some("progressive-disclosure")
        );
    }

    #[test]
    fn non_overflowing_tracker_path_still_gets_tracker_conventions() {
        // Companion to the regression above: the new overflow check must not swallow
        // the existing split for results that did NOT overflow.
        let a = adapter_for_test();
        let plain_tracker_result = json!({"abs_path": "docs/trackers/tool-usage-patterns.md"});
        assert_eq!(
            a.relevant_guide_topic(&plain_tracker_result),
            Some("tracker-conventions")
        );
    }

    /// A doctor result routes AWAY from `librarian`, so the `fix=` repair modes
    /// must be documented on the schema — the surface a doctor caller receives.
    ///
    /// Both halves are asserted together because either alone is reassuring and
    /// wrong. `d94dd53d` moved the six modes into a `serves: librarian.doctor`
    /// guide section and shipped them unreachable: `relevant_guide_topic` picks
    /// the topic from the RESULT's content via `names_tracker_path`, which scans
    /// `path` inside `violations`, and a real scan names tracker files — measured
    /// 2026-08-31, 128 of 138. So the section was never consulted. Restored in
    /// `c7d66f94`.
    ///
    /// **Still true after the same day's fallthrough fix, for a narrower reason.**
    /// `Tool::call_content` now falls through to the declaring topic when the
    /// result-based one ships nothing, so § *doctor repairs* IS reachable — on a
    /// later call. Not on the first tracker-path-naming call of a session, which
    /// is precisely when a caller forms its first `fix=` call; and a served
    /// section rides the response it should have informed, never precedes it
    /// (`docs/issues/2026-08-31-served-guide-sections-arrive-after-the-call-they-inform.md`).
    /// The schema is the only surface that beats both, which is why these modes
    /// stay inline rather than moving back into the guide now that the guide can
    /// reach them.
    ///
    /// The existing gate `every_observed_shape_of_a_declaring_topic_has_a_section`
    /// cannot catch this and is not the place to try: it skips non-declaring
    /// topics, and `tracker-conventions` declares no sections at all, so the
    /// routing destination is invisible to it by construction.
    ///
    /// Mutations that must kill this: move any mode's semantics out of the `fix`
    /// description again; or make `names_tracker_path` ignore `violations[].path`,
    /// which would silently re-point doctor at `librarian` and make the first
    /// assertion false.
    #[test]
    fn doctor_results_route_away_from_librarian_so_fix_modes_stay_in_the_schema() {
        use crate::librarian::tools::Tool as _;

        // Shaped like a real doctor response: violations carrying tracker paths.
        let doctor_result = json!({
            "violations": [{ "check": "missing_file", "path": "docs/trackers/x.md" }]
        });
        assert!(
            names_tracker_path(&doctor_result),
            "this predicate is what sends a doctor result to `tracker-conventions` \
             instead of `librarian`; if it stops firing, librarian.md's doctor \
             sections become reachable and this test's premise is stale"
        );

        let schema = crate::librarian::tools::librarian::Librarian.input_schema();
        let desc = schema["properties"]["fix"]["description"]
            .as_str()
            .expect("fix must carry a description");
        for mode in [
            "prune_missing",
            "reseat_worktree",
            "rehome",
            "repair_frontmatter_id",
            "mint_slugs",
            "export_augmentations",
        ] {
            assert!(
                desc.contains(mode),
                "`{mode}` must be explained in the fix description, not only listed in \
                 the enum — the enum names the modes, the description is what says what \
                 each DOES, and a doctor caller reaches no guide that carries it"
            );
        }
    }

    /// Seed a catalog holding one row for `abs`, at `status`, with `tags`/`title`.
    ///
    /// The id is computed from the CANONICALIZED path because that is what the syncer
    /// does — a tempdir path is a symlink on macOS, and an id derived from the
    /// uncanonicalized form would simply never match, making every assertion below
    /// vacuously "no row found".
    #[cfg(test)]
    fn seed_catalog_for(
        abs: &std::path::Path,
        status: &str,
        tags: Vec<String>,
        title: Option<&str>,
    ) -> (
        String,
        Arc<parking_lot::Mutex<crate::librarian::catalog::Catalog>>,
    ) {
        let id = crate::librarian::ids::artifact_id_from_abs(abs);
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let mut builder = crate::librarian::catalog::artifact::TestArtifactRowBuilder::new(&id)
            .with_abs_path(abs)
            .with_kind("bug")
            .with_status(status)
            .with_tags(tags);
        if let Some(t) = title {
            builder = builder.with_title(t);
        }
        crate::librarian::catalog::artifact::upsert(&cat, &builder.build()).unwrap();
        (id, Arc::new(parking_lot::Mutex::new(cat)))
    }

    /// The whole of BL-48 in one assertion: a file whose frontmatter says `fixed` must
    /// leave a catalog row that also says `fixed`.
    ///
    /// Without the syncer the row keeps `open` indefinitely, and
    /// `doc(find, kind="bug", status=…)` — the triage query CLAUDE.md and the
    /// activation bootstrap both prescribe — reports a value the file contradicts.
    /// `docs/issues/archive/2026-08-29-edit-markdown-frontmatter-desyncs-catalog-status.md`.
    #[test]
    fn sync_frontmatter_writes_the_files_status_onto_its_catalog_row() {
        use crate::util::librarian_sync::CatalogFrontmatterSync;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("2026-08-30-a-bug.md");
        std::fs::write(
            &path,
            "---\nkind: bug\nstatus: fixed\nclosed: 2026-08-30\n---\n\n# A bug\n",
        )
        .unwrap();
        let abs = std::fs::canonicalize(&path).unwrap();

        let (id, catalog) = seed_catalog_for(&abs, "open", vec![], None);
        let syncer = CatalogFrontmatterSyncer {
            catalog: Arc::clone(&catalog),
        };

        assert!(
            syncer.sync_frontmatter(&path),
            "a path the catalog knows must report that a row was found and updated"
        );

        let row = crate::librarian::catalog::artifact::get(&catalog.lock(), &id)
            .unwrap()
            .expect("the row must still exist");
        assert_eq!(
            row.status, "fixed",
            "the row must adopt the file's status — this single assertion IS the bug"
        );
    }

    /// The one way this hook could do real damage: inventing an artifact for ordinary
    /// markdown. `edit_file` runs on plenty of files the catalog has never heard of,
    /// and every one of them reaches this code.
    #[test]
    fn sync_frontmatter_never_creates_a_row_for_an_uncatalogued_file() {
        use crate::util::librarian_sync::CatalogFrontmatterSync;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ordinary-notes.md");
        std::fs::write(&path, "---\nstatus: fixed\n---\n\n# Notes\n").unwrap();
        let abs = std::fs::canonicalize(&path).unwrap();
        let id = crate::librarian::ids::artifact_id_from_abs(&abs);

        // An EMPTY catalog — nothing seeded, so this path is not an artifact.
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let catalog = Arc::new(parking_lot::Mutex::new(cat));
        let syncer = CatalogFrontmatterSyncer {
            catalog: Arc::clone(&catalog),
        };

        assert!(
            !syncer.sync_frontmatter(&path),
            "a path with no row must report false"
        );
        assert!(
            crate::librarian::catalog::artifact::get(&catalog.lock(), &id)
                .unwrap()
                .is_none(),
            "and must NOT have created one — turning every edit_file on a stray .md \
             into a catalog write is the one unrecoverable failure available here"
        );
    }

    /// The conservative direction, asserted so it is a decision rather than an accident:
    /// the file wins where it speaks, the row survives where the file is silent.
    ///
    /// This runs after an edit that may have touched a single key. A frontmatter block
    /// that omits `tags` means "not my business" far more often than "delete the tags",
    /// and a full re-derivation from the file is `reindex`'s job, not a per-edit hook's.
    #[test]
    fn sync_frontmatter_preserves_row_fields_the_file_does_not_mention() {
        use crate::util::librarian_sync::CatalogFrontmatterSync;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("partial.md");
        // Only `kind` and `status` — no tags, no title.
        std::fs::write(&path, "---\nkind: bug\nstatus: fixed\n---\n\n# Partial\n").unwrap();
        let abs = std::fs::canonicalize(&path).unwrap();

        let (id, catalog) = seed_catalog_for(
            &abs,
            "open",
            vec!["alpha".to_string(), "beta".to_string()],
            Some("Kept title"),
        );
        let syncer = CatalogFrontmatterSyncer {
            catalog: Arc::clone(&catalog),
        };
        assert!(syncer.sync_frontmatter(&path));

        let row = crate::librarian::catalog::artifact::get(&catalog.lock(), &id)
            .unwrap()
            .expect("row");
        assert_eq!(row.status, "fixed", "the file spoke about status");
        assert_eq!(
            row.tags,
            vec!["alpha".to_string(), "beta".to_string()],
            "the file said nothing about tags, so the row's must survive — a sync that \
             cleared them would silently destroy catalog state on every status flip"
        );
        assert_eq!(
            row.title.as_deref(),
            Some("Kept title"),
            "same for title: silence is not deletion"
        );
    }
}
