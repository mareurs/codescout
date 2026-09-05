use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::scope::{apply_scope, resolve_scope, Scope, ScopeApplied, UmbrellaPolicy};
use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::augmentation;
use crate::librarian::catalog::find::{catalog_summary, count_matching, find, FindOpts};
use crate::librarian::filter::FilterNode;

const MAX_LIMIT: usize = 500;
const MAX_OFFSET: usize = 100_000;
use super::HIDDEN_STATUSES;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    filter: Option<FilterNode>,
    /// Shortcut: equivalent to filter `{kind: {eq: value}}`. Combined with `filter` via AND.
    #[serde(default)]
    kind: Option<String>,
    /// Shortcut: equivalent to filter `{status: {eq: value}}`. Disables archived-hide default.
    #[serde(default)]
    status: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    /// Natural-language query for semantic search. Requires embedding service.
    #[serde(default)]
    semantic: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    /// Include archived/superseded/retired rows. Ignored when the user filter
    /// already constrains `status`.
    /// `retired` covers the in-place redirect pattern (MRV `2-lane-strategy.md`):
    /// the file stays at its original path with `status: retired` and a body
    /// that forwards to the canonical successor, so incoming links keep
    /// resolving while the tracker stops showing in active listings.
    #[serde(default)]
    include_archived: bool,
    /// Filter to augmented (true) or non-augmented (false) artifacts. Omit to return all.
    #[serde(default)]
    augmented: Option<bool>,
    /// Advertised top-level in the shared `artifact` schema for `create`, and honored
    /// here too rather than discarded. `Args` cannot carry `deny_unknown_fields` — the
    /// dispatcher passes `action` through and the shared schema holds sibling actions'
    /// keys — so an advertised param absent from this struct is dropped by serde while
    /// the query still runs, at defaults, and answers with an unfiltered first page.
    /// `call()` lifts this into `filter` and reports the lift.
    /// BUG docs/issues/archive/2026-08-17-find-silently-drops-top-level-rel-path.md
    #[serde(default)]
    rel_path: Option<String>,
}

fn default_limit() -> usize {
    50
}

/// One page row plus the per-HIT data that belongs to it.
///
/// This type exists because the two facts a semantic hit carries — its distance
/// and the chunk that matched — are properties of the HIT, while everything else
/// on the page (overlay status, augmentation) is a property of the ARTIFACT.
/// They used to be carried in two `HashMap<artifact_id, _>` side maps, which is
/// only sound while at most one hit per artifact reaches the page. At
/// `max_per_artifact = 2` a second hit silently overwrote the first and every
/// item for that artifact rendered the last chunk's span — a wrong `matched` on
/// a plausible-looking item, never an error. Pinned by
/// `two_chunks_of_one_artifact_keep_their_own_matched_span`.
///
/// Carrying the data on the record rather than beside it also survives the
/// worktree-overlay `retain` below by construction; a parallel `Vec` would
/// desync there, which is the same defect one refactor later.
///
/// `overlay_ids` and `augmentation_by_id` stay artifact-keyed on purpose: two
/// chunks of one ledger genuinely share its augmentation and its overlay
/// status, so a lookup by id is correct there and a per-hit copy would only
/// duplicate it.
struct PageItem {
    row: crate::librarian::catalog::artifact::ArtifactRow,
    /// `None` on the non-semantic path, which has no distance to report.
    distance: Option<f32>,
    chunk: Option<crate::librarian::catalog::find::ChunkHit>,
}

fn merge_kind_status(
    filter: Option<FilterNode>,
    kind: Option<&str>,
    status: Option<&str>,
) -> Option<FilterNode> {
    let mut parts: Vec<FilterNode> = Vec::new();
    if let Some(k) = kind {
        parts.push(FilterNode::Leaf(
            [("kind".to_string(), json!({"eq": k}))]
                .into_iter()
                .collect(),
        ));
    }
    if let Some(s) = status {
        parts.push(FilterNode::Leaf(
            [("status".to_string(), json!({"eq": s}))]
                .into_iter()
                .collect(),
        ));
    }
    if let Some(f) = filter {
        parts.push(f);
    }
    match parts.len() {
        0 => None,
        1 => parts.into_iter().next(),
        _ => Some(FilterNode::And { and: parts }),
    }
}

fn combine_user_with_archived_hide(
    user: Option<FilterNode>,
    include_archived: bool,
    user_constrains_status: bool,
) -> Option<FilterNode> {
    if include_archived || user_constrains_status {
        return user;
    }
    let hide = FilterNode::Leaf(
        [("status".to_string(), json!({"nin": HIDDEN_STATUSES}))]
            .into_iter()
            .collect(),
    );
    Some(match user {
        Some(u) => FilterNode::And { and: vec![u, hide] },
        None => hide,
    })
}

/// Recursively check whether any leaf in `node` constrains the `status` field.
fn filter_mentions_status(node: &FilterNode) -> bool {
    match node {
        FilterNode::And { and } => and.iter().any(filter_mentions_status),
        FilterNode::Or { or } => or.iter().any(filter_mentions_status),
        FilterNode::Not { not } => filter_mentions_status(not),
        FilterNode::Leaf(map) => map.contains_key("status"),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_hints(
    cat: &crate::librarian::catalog::Catalog,
    base: Option<&FilterNode>,
    applied: &ScopeApplied,
    ws: &crate::librarian::workspace::WorkspaceConfig,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    scope_fallback: bool,
    user_constrains_status: bool,
    include_archived: bool,
    returned_count: usize,
    offset: usize,
    exclude_worktrees: &[String],
    deduped_main_ids: &[String],
    cutoff_ms: i64,
) -> Result<Value> {
    let mut hints = serde_json::Map::new();

    if scope_fallback {
        hints.insert(
            "scope_fallback".into(),
            json!("cwd is outside all workspace roots; defaulted to scope=all"),
        );
    }

    let here = count_for_scope(
        cat,
        base,
        ws,
        current,
        applied.scope,
        exclude_worktrees,
        cutoff_ms,
    )?;
    // Overlay dedup (see `call`) drops main twins whose shadow lives under
    // this worktree session's own root before `items` is built, but `here` is
    // a raw scope COUNT that still counts both the shadow and its main twin.
    // Subtract however many of the deduped main ids are themselves counted in
    // `here` so `more_in_scope` reflects the post-dedup page, not the
    // pre-dedup union. A worktree session's own root is never excluded from
    // scope (see `exclude_worktrees`), so a deduped main id counted here
    // always has its shadow counted here too — the adjustment never
    // over-subtracts.
    let deduped_here = count_deduped_main_ids(
        cat,
        base,
        ws,
        current,
        applied.scope,
        exclude_worktrees,
        deduped_main_ids,
        cutoff_ms,
    )?;
    let here = here.saturating_sub(deduped_here);

    // More matched in THIS scope than were returned on this page? The result
    // set is capped by `limit`/`offset`; without this signal an agent reads the
    // returned page as the complete set (silent-cap bug — see
    // docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md).
    let shown_through = offset.saturating_add(returned_count);
    let more_in_scope = here.saturating_sub(shown_through);
    if more_in_scope > 0 {
        hints.insert("more_in_scope".into(), json!(more_in_scope));
        hints.insert(
            "more_in_scope_hint".into(),
            json!(
                "more artifacts match in this scope than were returned; \
                 raise `limit`, page with `offset`, or narrow the filter"
            ),
        );
    }

    if !matches!(applied.scope, Scope::Repo | Scope::All) && current.is_some() {
        let in_repo = count_for_scope(
            cat,
            base,
            ws,
            current,
            Scope::Repo,
            exclude_worktrees,
            cutoff_ms,
        )?;
        let extra = in_repo.saturating_sub(here);
        if extra > 0 {
            hints.insert("more_in_repo".into(), json!(extra));
        }
    }

    // Two widening hints, measured from DIFFERENT baselines — and the difference
    // is the whole contract.
    //
    // `more_in_umbrella` counts rows a `scope` param can actually fetch, so it is
    // measured from `here` and it earns an `expand` entry. `scope="all"` aliases to
    // umbrella whenever the project has one (`resolve_scope`, UmbrellaPolicy::Require),
    // which makes umbrella the widest reachable scope; excluding Umbrella/All from
    // the guard is what stops the suggestion re-aliasing to the scope already applied
    // — the self-reference fixed in
    // docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md (sub-finding #2).
    //
    // `more_in_workspace` counts what lies BEYOND that ceiling, so it is measured
    // from the umbrella count and it earns NO `expand` entry. No `scope` value
    // reaches those rows: with an umbrella `all` aliases to umbrella, and without one
    // `resolve_scope` refuses `all` outright — so the only honest action is to
    // activate the owning project, which is what its hint says.
    //
    // Measuring it from `here` was the defect: the count then included the whole
    // reachable umbrella delta plus the unreachable remainder, so it advertised 23
    // rows and `scope="all"` delivered 2. The earlier fix gated on `applied.scope`,
    // which held at umbrella scope (where its test lives) and left the repo-scope
    // twin reporting an unreachable total. See
    // docs/issues/archive/2026-08-17-find-more-in-workspace-hint-counts-rows-the-alias-cannot-reach.md.
    if !matches!(applied.scope, Scope::Umbrella | Scope::All)
        && current.and_then(|c| c.umbrella.as_deref()).is_some()
    {
        let in_umbrella = count_for_scope(
            cat,
            base,
            ws,
            current,
            Scope::Umbrella,
            exclude_worktrees,
            cutoff_ms,
        )?;
        let reachable = in_umbrella.saturating_sub(here);
        if reachable > 0 {
            hints.insert("more_in_umbrella".into(), json!(reachable));
        }

        let in_workspace = count_for_scope(
            cat,
            base,
            ws,
            current,
            Scope::All,
            exclude_worktrees,
            cutoff_ms,
        )?;
        let beyond_umbrella = in_workspace.saturating_sub(in_umbrella);
        if beyond_umbrella > 0 {
            hints.insert("more_in_workspace".into(), json!(beyond_umbrella));
            hints.insert(
                "more_in_workspace_hint".into(),
                json!(
                    "these rows sit outside this project's umbrella and no `scope` value \
                     reaches them — activate the owning project to query them"
                ),
            );
        }
    }

    if !user_constrains_status && !include_archived {
        let stripped = base.cloned().map(strip_status_clause);
        let with_archived = count_for_scope(
            cat,
            stripped.as_ref(),
            ws,
            current,
            applied.scope,
            exclude_worktrees,
            cutoff_ms,
        )?;
        let hidden = with_archived.saturating_sub(here);
        if hidden > 0 {
            hints.insert("hidden_archived".into(), json!(hidden));
            hints.insert(
                "include_archived_hint".into(),
                json!("pass include_archived=true to surface archived/superseded rows"),
            );
        }
    }

    // Unindexed-on-disk staleness signal. `find` answers from the catalog,
    // which can legitimately lag disk (no file watcher — see
    // get_guide("librarian") § Gotchas) — but a lagging catalog and an empty
    // filesystem otherwise produce byte-identical, silent `count: 0` answers.
    // See docs/issues/archive/2026-08-17-artifact-find-is-silent-about-files-the-catalog-has-never-seen.md.
    //
    // Project and Repo scope only — the two whose `apply_scope` path prefix
    // (`cp.abs_path`, `cp.git_root` respectively) names a single real
    // directory this walk can anchor on; Umbrella/All span multiple roots.
    // `Scope::Repo` is the query default (`Scope::default()`), so this is the
    // arm that actually fires for a scope-unspecified call — the shape of the
    // bug's own reproduction. Skipped from inside a linked worktree
    // (`main_root.is_some()`): `index_repo_sync` never indexes a worktree root
    // directly, so a worktree-rooted disk count and the (overlay) catalog
    // count are not the same quantity — comparing them would be a category
    // error, not a staleness signal. Cheap because the walk is bounded by
    // this ONE root's own file count, not the workspace's.
    if matches!(applied.scope, Scope::Project | Scope::Repo) {
        if let Some(cp) = current {
            if cp.main_root.is_none() {
                let disk_root = match applied.scope {
                    Scope::Project => &cp.abs_path,
                    _ => &cp.git_root,
                };
                let project_ignore = crate::librarian::workspace::compile_ignore(&ws.ignore)
                    .unwrap_or_else(|_| {
                        globset::GlobSetBuilder::new()
                            .build()
                            .expect("empty globset")
                    });
                let disk_count = count_disk_md(disk_root, &project_ignore);
                let catalog_count = count_for_scope(
                    cat,
                    None,
                    ws,
                    current,
                    applied.scope,
                    exclude_worktrees,
                    cutoff_ms,
                )?;
                let unindexed = disk_count.saturating_sub(catalog_count);
                if unindexed > 0 {
                    hints.insert("unindexed_files".into(), json!(unindexed));
                    hints.insert(
                        "unindexed_hint".into(),
                        json!(format!(
                            "{unindexed} file(s) under this scope are not in the catalog and cannot \
                             match any filter; run librarian(action=\"reindex\") to include them"
                        )),
                    );
                }
            }
        }
    }

    // Read the durable degraded marker `reindex.rs` persists into `catalog_meta`
    // (see docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md
    // step 2). Global, not scope-filtered — the marker is one call's aggregate
    // across every target that call walked, not a per-artifact fact, so it
    // surfaces unconditionally rather than only under Project/Repo scope like
    // `unindexed_files` above.
    if let Some(count_str) =
        crate::librarian::catalog::gc::get_meta(&cat.conn, "last_reindex_embed_error_count")?
    {
        let count: usize = count_str.parse().unwrap_or(0);
        if count > 0 {
            hints.insert("catalog_degraded".into(), json!(count));
            hints.insert(
                "catalog_degraded_hint".into(),
                json!(format!(
                    "the last librarian(action=\"reindex\") left {count} artifact(s) without a \
                     vector; semantic_find will not surface them until reindex succeeds cleanly"
                )),
            );
        }
    }

    let mut expand = Vec::new();
    if hints.contains_key("more_in_repo") {
        expand.push("scope=\"repo\"");
    }
    if hints.contains_key("more_in_umbrella") {
        expand.push("scope=\"all\"");
    }
    // Deliberately no entry for `more_in_workspace`: those rows are beyond the
    // umbrella ceiling and no scope value reaches them, so listing one here would
    // hand back an action that cannot deliver what was counted. It also pushed the
    // identical string twice whenever both hints fired, which read as two distinct
    // remedies for one. `more_in_workspace_hint` carries the action instead.
    if !expand.is_empty() {
        hints.insert("expand".into(), json!(expand));
    }

    Ok(Value::Object(hints))
}

/// How many of `deduped_main_ids` are themselves counted by `count_for_scope`
/// under (`base`, `scope`). Used to correct `more_in_scope`: `find`'s overlay
/// dedup drops these ids from `items` before returning, but the raw scope
/// COUNT (`here`) still counts them — without this adjustment `more_in_scope`
/// overcounts by exactly the number of deduped main twins.
#[allow(clippy::too_many_arguments)]
fn count_deduped_main_ids(
    cat: &crate::librarian::catalog::Catalog,
    base: Option<&FilterNode>,
    ws: &crate::librarian::workspace::WorkspaceConfig,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    scope: Scope,
    exclude_worktrees: &[String],
    deduped_main_ids: &[String],
    cutoff_ms: i64,
) -> Result<usize> {
    if deduped_main_ids.is_empty() {
        return Ok(0);
    }
    let (filter, _) = apply_scope(base.cloned(), scope, ws, current, exclude_worktrees)?;
    let id_values: Vec<Value> = deduped_main_ids.iter().map(|id| json!(id)).collect();
    let id_filter = FilterNode::Leaf(
        [("id".to_string(), json!({"in": id_values}))]
            .into_iter()
            .collect(),
    );
    let combined = match filter {
        Some(f) => FilterNode::And {
            and: vec![f, id_filter],
        },
        None => id_filter,
    };
    count_matching(cat, Some(&combined), cutoff_ms)
}

#[allow(clippy::too_many_arguments)]
fn count_for_scope(
    cat: &crate::librarian::catalog::Catalog,
    base: Option<&FilterNode>,
    ws: &crate::librarian::workspace::WorkspaceConfig,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    scope: Scope,
    exclude_worktrees: &[String],
    cutoff_ms: i64,
) -> Result<usize> {
    if matches!(scope, Scope::Project | Scope::Repo) && current.is_none() {
        return Ok(0);
    }
    if matches!(scope, Scope::Umbrella) && current.and_then(|c| c.umbrella.as_deref()).is_none() {
        return Ok(0);
    }
    let (filter, _) = apply_scope(base.cloned(), scope, ws, current, exclude_worktrees)?;
    count_matching(cat, filter.as_ref(), cutoff_ms)
}

fn strip_status_clause(node: FilterNode) -> FilterNode {
    if let FilterNode::And { and } = node {
        let kept: Vec<FilterNode> = and
            .into_iter()
            .filter(|n| !is_status_nin_clause(n))
            .collect();
        if kept.len() == 1 {
            return kept.into_iter().next().unwrap();
        }
        return FilterNode::And { and: kept };
    }
    node
}

fn is_status_nin_clause(n: &FilterNode) -> bool {
    if let FilterNode::Leaf(map) = n {
        if let Some(ops) = map.get("status").and_then(|v| v.as_object()) {
            return ops.contains_key("nin");
        }
    }
    false
}
/// Extract the first `rel_path` contains/prefix value from a filter tree.
fn rel_path_hint(node: &FilterNode) -> Option<String> {
    match node {
        FilterNode::And { and } => and.iter().find_map(rel_path_hint),
        FilterNode::Or { or } => or.iter().find_map(rel_path_hint),
        FilterNode::Not { not } => rel_path_hint(not),
        FilterNode::Leaf(map) => map
            .get("rel_path")?
            .as_object()?
            .iter()
            .find_map(|(op, v)| {
                if matches!(op.as_str(), "contains" | "prefix") {
                    v.as_str().map(str::to_owned)
                } else {
                    None
                }
            }),
    }
}

/// Walk the current project directory for `.md` files whose repo-relative path
/// contains `hint`. Returns relative paths (relative to the repo root).
fn scan_unindexed_md(
    roots: &[crate::librarian::workspace::Root],
    cp: &crate::librarian::current_project::CurrentProject,
    hint: &str,
    ignore_patterns: &[String],
) -> Vec<String> {
    // Transitional bridge: derive legacy root/subdir from cp.git_root.
    let cp_root: String = cp
        .git_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let Some(root) = roots.iter().find(|r| r.name == cp_root) else {
        return vec![];
    };
    let base = root.path.clone();
    let ignore =
        crate::librarian::workspace::compile_ignore(ignore_patterns).unwrap_or_else(|_| {
            globset::GlobSetBuilder::new()
                .build()
                .expect("empty globset")
        });
    let mut found = Vec::new();
    let walker = ignore::WalkBuilder::new(&base)
        .standard_filters(true)
        .build();
    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = match path.strip_prefix(&root.path) {
            Ok(r) => crate::librarian::util::normalize_rel_path(&r.to_string_lossy()),
            Err(_) => continue,
        };
        if !ignore.is_match(&rel) && rel.contains(hint) {
            found.push(rel);
        }
    }
    found
}

/// Count `.md` files under `abs_root` that a full reindex would produce a
/// catalog row for. `index_repo_sync` gives even an unclassified file
/// `kind: "unknown"` rather than skipping it, so nothing is excluded on
/// classification grounds — only on ignore/gitignore grounds, exactly what
/// this walk replicates. It omits `index_repo_sync`'s `force_include`
/// supplemental scan (locally-tracked-but-gitignored paths); that can only
/// undercount, which can only suppress the staleness hint below, never
/// spuriously fire it.
fn count_disk_md(abs_root: &std::path::Path, ignore: &globset::GlobSet) -> usize {
    let walker = ignore::WalkBuilder::new(abs_root)
        .standard_filters(true)
        .build();
    walker
        .flatten()
        .filter(|entry| {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
                return false;
            }
            let Ok(rel) = path.strip_prefix(abs_root) else {
                return false;
            };
            let rel = crate::librarian::util::normalize_rel_path(&rel.to_string_lossy());
            !ignore.is_match(&rel)
        })
        .count()
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let mut a: Args = serde_json::from_value(args)?;
    // Repair-and-continue: fix the deterministic inverted-leaf filter mistake
    // ({op:{field,value}} -> {field:{op:value}}) in place rather than erroring,
    // so a malformed filter costs zero retry round-trips. The corrections ride
    // back in the response so the agent still learns the canonical shape.
    let mut filter_corrections = a
        .filter
        .as_mut()
        .map(crate::librarian::filter::repair_inverted_leaves)
        .unwrap_or_default();

    // Lift a top-level `rel_path` into the filter, on the same Repair-and-Continue
    // grounds as the inverted-leaf fix above: one unambiguous reading, so repair it and
    // say so. `contains`, not `eq` — the catalog stores absolute paths while responses
    // display the relative form, so `eq` on a path as displayed matches nothing (U-35).
    // Lifting here, before `is_cold_call` and before `rel_path_hint`, is deliberate: the
    // call stops counting as a cold call (it is a filtered query), and it inherits the
    // unindexed-file disk scan that a rel_path filter already triggers on an empty page.
    let mut lift_corrections: Vec<String> = Vec::new();
    if let Some(rp) = a.rel_path.take() {
        let leaf = FilterNode::Leaf(
            [("rel_path".to_string(), json!({"contains": rp.clone()}))]
                .into_iter()
                .collect(),
        );
        a.filter = Some(match a.filter.take() {
            Some(existing) => FilterNode::And {
                and: vec![existing, leaf],
            },
            None => leaf,
        });
        lift_corrections.push(format!(
            "top-level `rel_path` lifted into the filter: {{\"rel_path\": {{\"contains\": \"{rp}\"}}}}"
        ));
    }

    let is_cold_call = a.filter.is_none()
        && a.semantic.is_none()
        && a.kind.is_none()
        && a.status.is_none()
        && a.augmented.is_none();
    let limit = a.limit.min(MAX_LIMIT);
    let offset = a.offset.min(MAX_OFFSET);

    // Grace-period visibility cutoff: rows missing (deleted/moved off-disk) at or
    // before this cutoff are hidden from listing/search (but not from `get`/`doctor`).
    // Computed once so every count/find/summary in this call agrees on one cutoff.
    let cutoff_ms = {
        let cat = ctx.catalog.lock();
        let now_ms = chrono::Utc::now().timestamp_millis();
        crate::librarian::catalog::gc::visibility_cutoff_ms(&cat.conn, now_ms)?
    };

    // Resolve semantic query → embedding vector (if requested and available).
    let semantic_vec: Option<Vec<f32>> = if let Some(ref query) = a.semantic {
        match ctx.embedding.as_ref() {
            Some(svc) => Some(svc.embedder.embed_query(query).await?),
            // Correctable by the caller — drop `semantic` and the same query runs
            // as a plain filter. Recoverable, not a hard failure that aborts
            // sibling calls (omnibus 49ee6a03, F11).
            None => {
                return Err(RecoverableError::with_hint(
                    "semantic search requires an embedding service, which is not configured",
                    "Retry without the `semantic` param (use `filter` with a `contains` op on \
                     title/rel_path for a literal match), or configure an embedder.",
                ))
            }
        }
    } else {
        None
    };

    // Merge kind/status shortcut params into the base filter.
    let status_shortcut_set = a.status.is_some();
    let rel_path_filter_hint = a.filter.as_ref().and_then(rel_path_hint);
    let base_filter = merge_kind_status(a.filter, a.kind.as_deref(), a.status.as_deref());

    // Build augmented pre-filter if requested, then merge with user filter.
    //
    // `augmented=true` with nothing augmented ANYWHERE cannot be expressed as a
    // filter — `compile()` and `eval()` both reject an empty `in` list
    // (filter.rs::eval_and_compile_both_reject_empty_in) — so the rows are forced
    // empty further down instead. Deliberately NOT an early return: a bare
    // `{count: 0, scope: null, hints: {}}` reads identically whether nothing is
    // augmented, the scope resolved to another project, or this session opened a
    // different catalog file. Augmentation lives only in the catalog DB — no on-disk
    // form, not rebuildable by reindex — so that ambiguity is the difference between
    // "nothing to see" and "data loss". Falling through preserves the scope block,
    // the hints, and the catalog counts that tell those apart.
    // docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md
    let mut no_augmentations_anywhere = false;
    let mut augmented_in_catalog: Option<usize> = None;
    let user_filter: Option<FilterNode> = if let Some(want_augmented) = a.augmented {
        let ids = {
            let cat = ctx.catalog.lock();
            augmentation::list_all_ids(&cat)?
        };
        if want_augmented {
            augmented_in_catalog = Some(ids.len());
            if ids.is_empty() {
                no_augmentations_anywhere = true;
                base_filter
            } else {
                let id_values: Vec<Value> = ids.into_iter().map(|id| json!(id)).collect();
                let in_node = FilterNode::Leaf(
                    [("id".to_string(), json!({"in": id_values}))]
                        .into_iter()
                        .collect(),
                );
                Some(match base_filter {
                    Some(f) => FilterNode::And {
                        and: vec![f, in_node],
                    },
                    None => in_node,
                })
            }
        } else if ids.is_empty() {
            // Nothing is augmented → "non-augmented" = everything; base filter unchanged.
            base_filter
        } else {
            let id_values: Vec<Value> = ids.into_iter().map(|id| json!(id)).collect();
            let nin_node = FilterNode::Leaf(
                [("id".to_string(), json!({"nin": id_values}))]
                    .into_iter()
                    .collect(),
            );
            Some(match base_filter {
                Some(f) => FilterNode::And {
                    and: vec![f, nin_node],
                },
                None => nin_node,
            })
        }
    } else {
        base_filter
    };

    let user_constrains_status = status_shortcut_set
        || user_filter
            .as_ref()
            .map(filter_mentions_status)
            .unwrap_or(false);
    let base =
        combine_user_with_archived_hide(user_filter, a.include_archived, user_constrains_status);

    // `Scope::Project` is `find`'s documented default: a search surface answers
    // about the project you are in, and widening is a `scope` param away — the
    // response's `more_in_repo` / `more_in_umbrella` hints name the next rung.
    let (effective_scope, scope_fallback) = resolve_scope(
        a.scope,
        ctx.current_project.as_deref(),
        UmbrellaPolicy::Require,
        Scope::Project,
    )?;

    let current = ctx.current_project.as_deref();
    let exclude_worktrees: Vec<String> = {
        let cat = ctx.catalog.lock();
        let own = current
            .filter(|c| c.main_root.is_some())
            .map(|c| crate::util::fs::RepoPath::from(c.git_root.as_path()).into_string());
        crate::librarian::catalog::worktree::active_roots(&cat)?
            .into_iter()
            .filter(|r| own.as_deref() != Some(r.as_str()))
            .collect()
    };
    let (scoped_filter, applied) = apply_scope(
        base.clone(),
        effective_scope,
        &ctx.workspace,
        current,
        &exclude_worktrees,
    )?;

    // Semantic path runs the async store-backed coordinator (it manages its own
    // catalog locking); the sync `find` below handles the non-semantic case.
    let semantic_page = if let Some(vec) = semantic_vec {
        let store = ctx.artifact_store.as_ref().ok_or_else(|| {
            RecoverableError::new(
                "artifact semantic search backend unavailable — the configured Qdrant is \
                 unreachable. Set `[librarian] vector_backend = \"sqlite-vec\"` (or \
                 CODESCOUT_ARTIFACT_BACKEND=sqlite-vec) for the offline backend.",
            )
        })?;
        // Project scope → the project's OWN collection, named by the same value the
        // write side uses. `reindex` derives its `project_id` from `abs_root`, which
        // for `Scope::Project` IS `current_project.abs_path`, so read and write agree
        // by construction rather than by two lookups happening to land together.
        // Other scopes pass None, which `knn` reads as "every project" and fans out —
        // NOT as "no filter". That distinction is load-bearing; see `knn`'s doc.
        //
        // This used to stamp the containing `[[roots]]` entry, described as
        // "superset-safe for the catalog scoped filter". That reasoning held when one
        // shared collection was narrowed by a `project_id` payload: a broader id still
        // let the catalog filter do the narrowing. Once the id NAMES the collection,
        // a superset id is a different collection that holds none of these vectors, so
        // the safe direction inverted. On a host where no `[[roots]]` entry contained
        // the project, the lookup missed entirely and every project-scoped search
        // silently fanned out, hydrating another project's chunk ids against this
        // catalog — which is what `semantic_find`'s `unresolved` counter reports.
        // docs/issues/2026-09-03-per-project-vector-collections-follow-the-server-cwd-not-the-workspace-param.md
        let project_id = if effective_scope == Scope::Project {
            current.map(|cp| cp.abs_path.to_string_lossy().into_owned())
        } else {
            None
        };
        Some(
            crate::librarian::catalog::find::semantic_find(
                store.as_ref(),
                &ctx.catalog,
                project_id.as_deref(),
                &vec,
                scoped_filter.as_ref(),
                // TWO chunks per artifact. Measured, not chosen: on the artifact
                // suite `cap=2` beats both neighbours (`cap=1` 3/12, `cap=2` 5/12,
                // `cap=3` 6/12 and `cap=∞` 5/12 at limit=10) — uncapping lets one
                // ledger flood the page and crowd out other documents, so "more
                // chunks per artifact" is not monotonically better and this number
                // is load-bearing rather than a legacy default. See BL-72 in
                // docs/trackers/open-issue-work-queue.md for the full sweep.
                //
                // WHY 1 WAS WRONG: in a ledger the preamble/index section is a
                // broad-spectrum attractor — it summarises every entry, so it beats
                // each specific entry on any query about that file. At `1` it always
                // won and the entry the query was actually about never reached the
                // page. Measured 2026-09-04 on the current corpus: of the three
                // cases whose target file DID rank in the top 5, all three matched
                // the preamble and none matched a different entry — the attractor
                // was the whole intra-file loss, not part of it.
                //
                // Raising this was blocked until the side maps below stopped being
                // keyed by artifact id; that is what `PageItem` now fixes.
                2,
                limit,
                offset,
                cutoff_ms,
            )
            .await?,
        )
    } else {
        None
    };

    let (items, hints, catalog_value) = {
        let cat = ctx.catalog.lock();

        // Also on the empty-augmentation path: `total` is what distinguishes a
        // populated catalog that genuinely holds no augmentations from an empty or
        // unexpected one, and it is the reader's cheapest substrate check.
        let catalog_value: Option<serde_json::Value> = if is_cold_call || no_augmentations_anywhere
        {
            let summary = catalog_summary(&cat, scoped_filter.as_ref(), cutoff_ms)?;
            Some(serde_json::json!({
                "total": summary.total,
                "by_kind": summary.by_kind,
                "augmented": summary.augmented,
            }))
        } else {
            None
        };

        // Split the page: `rows` feeds the existing overlay/augmentation pipeline
        // unchanged, while each hit's distance and matched chunk ride ON the row
        // rather than in a side map. Keeping `ArtifactRow` free of a score is
        // deliberate — it is the row type every non-semantic read path shares, and
        // a query-relative number does not belong on a record that outlives the
        // query. `PageItem` is the wrapper that keeps the two apart without
        // keying the second by something that is not unique per hit.
        //
        // This replaces two `HashMap<artifact_id, _>` side maps that were only
        // correct while this caller passed `max_per_artifact = 1`; see `PageItem`.
        let (semantic_items, starvation, cap_suppressed, unresolved) = match semantic_page {
            Some(page) => {
                let starvation = (page.widenings, page.exhausted);
                let cap_suppressed = page.cap_suppressed;
                let unresolved = page.unresolved;
                let items: Vec<PageItem> = page
                    .hits
                    .into_iter()
                    .map(|hit| PageItem {
                        row: hit.row,
                        distance: Some(hit.distance),
                        chunk: hit.chunk,
                    })
                    .collect();
                (Some(items), Some(starvation), cap_suppressed, unresolved)
            }
            None => (None, None, 0usize, 0usize),
        };

        let rows: Vec<PageItem> = if no_augmentations_anywhere {
            // The "match nothing" filter the engine refuses to compile, applied here
            // instead. Everything downstream — scope, hints, catalog — still runs.
            Vec::new()
        } else {
            match semantic_items {
                Some(r) => r,
                None => find(
                    &cat,
                    &FindOpts {
                        filter: scoped_filter,
                        limit,
                        offset,
                    },
                    cutoff_ms,
                )?
                .into_iter()
                // The non-semantic path has no hit-level data at all — no query,
                // so no distance and no matched chunk. Wrapping rather than
                // branching keeps ONE downstream pipeline, so a future field on
                // `PageItem` cannot be wired for one path and forgotten on the
                // other.
                .map(|row| PageItem {
                    row,
                    distance: None,
                    chunk: None,
                })
                .collect(),
            }
        };

        // Overlay dedup: a worktree session sees its shadow INSTEAD of the main
        // twin. `worktree_of` pairs (main_id, shadow_id) whose shadow lives
        // under THIS session's own worktree root identify the main ids to drop;
        // survivors carrying a shadow are flagged `overlay: true` below. Gated
        // on `main_root.is_some()` — a plain (non-worktree) session never runs
        // this query and its find results are unaffected. `shadow_main_pairs`
        // (shared with get.rs) wildcard-escapes the worktree-root LIKE pattern.
        let mut rows = rows;
        let mut overlay_ids: std::collections::HashSet<String> = Default::default();
        let mut deduped_main_ids: Vec<String> = Vec::new();
        if let Some(cp) = current.filter(|c| c.main_root.is_some()) {
            let wt = crate::util::fs::RepoPath::from(cp.git_root.as_path()).into_string();
            let pairs = crate::librarian::tools::worktree::shadow_main_pairs(&cat, &wt)?;
            let shadowed: std::collections::HashSet<&str> =
                pairs.iter().map(|(main_id, _)| main_id.as_str()).collect();
            rows.retain(|it| !shadowed.contains(it.row.id.as_str()));
            overlay_ids = pairs
                .iter()
                .map(|(_, shadow_id)| shadow_id.clone())
                .collect();
            deduped_main_ids = pairs.into_iter().map(|(main_id, _)| main_id).collect();
        }

        // Batched, not per-row: a caller deciding "append_entry(entry_collection=…)
        // (rows) or append_entry(anchor_heading=…) (body sections)" needs this fact
        // without a separate `get` probe per candidate tracker.
        // docs/issues/archive/2026-08-16-adding-one-tracker-entry-makes-the-agent-resolve-identity-and-rendering-by-hand.md
        let row_ids: Vec<String> = rows.iter().map(|it| it.row.id.clone()).collect();
        let augmentation_by_id = augmentation::get_batch(&cat, &row_ids)?;

        let items: Vec<Value> = rows
            .into_iter()
            .map(|it| {
                let PageItem {
                    row: r,
                    distance,
                    chunk,
                } = it;
                let mut item = json!({
                    "id": r.id,
                    "kind": r.kind,
                    "status": r.status,
                    "title": r.title,
                    "abs_path": r.abs_path.display().to_string(),
                    "updated_at": r.updated_at,
                });
                // Artifact-level, so an id lookup is the RIGHT shape here: two
                // chunks of one ledger share its overlay status and augmentation.
                if overlay_ids.contains(&r.id) {
                    item["overlay"] = json!(true);
                }
                if let Some(aug) = augmentation_by_id.get(&r.id) {
                    item["entry_collection"] = json!(aug.entry_collection);
                }
                // Lower is closer, backend-scaled. Present only on the semantic
                // path, and only as a WITHIN-response comparison: it is what lets a
                // reader tell the top hit from the least-bad remainder, which the
                // response could not express at all before.
                if let Some(d) = distance {
                    if d.is_finite() {
                        item["distance"] = json!((d * 1000.0).round() / 1000.0);
                    }
                }
                // The chunk that matched, not the artifact's opening lines. On a
                // ledger those are the same thing only by accident: the preamble
                // is the one part of the file that says nothing about the entry
                // the query actually found.
                if let Some(chunk) = chunk {
                    // Bounded, and it SAYS SO when it cuts. A 2 KB chunk x a
                    // 50-item page is a 100 KB response, so the bound is not
                    // optional — but an unmarked 480-char snippet is
                    // indistinguishable from a chunk that happens to end there,
                    // which is `cluster/capped-result-presented-as-complete`. The
                    // line range travels alongside precisely so the caller can
                    // read the full span with
                    // `doc(action="get", id=…, start_line=…, end_line=…)`.
                    const SNIPPET_CHARS: usize = 480;
                    let snippet = if chunk.content.chars().count() > SNIPPET_CHARS {
                        let mut s: String = chunk.content.chars().take(SNIPPET_CHARS).collect();
                        s.push_str(
                            " … [snippet truncated — read the span with doc(action=\"get\")]",
                        );
                        s
                    } else {
                        chunk.content.clone()
                    };
                    item["matched"] = json!({
                        "start_line": chunk.start_line,
                        "end_line": chunk.end_line,
                        "entry_token": chunk.entry_token,
                        "snippet": snippet,
                    });
                    // "You are holding part 3 of 7." A hit on the fourth chunk of
                    // a long entry was previously indistinguishable from a hit on
                    // a whole short one, and the span cannot carry the difference:
                    // `end_line - start_line` describes the FRAGMENT, never the
                    // entry it came from. Emitted only when both are known — a row
                    // indexed before these columns existed reports neither, and
                    // absence must read as "unknown" rather than "part 1 of 1".
                    if let (Some(part), Some(parts)) = (chunk.entry_part, chunk.entry_parts) {
                        item["matched"]["part"] = json!(part);
                        item["matched"]["of"] = json!(parts);
                    }
                    // The recovery action, addressed by TOKEN rather than by line.
                    //
                    // The line range is the wrong handle for this and has been
                    // measurably wrong: 378 of 3,729 entry-bearing chunks publish a
                    // `start_line` that resolves to the PREVIOUS entry
                    // (docs/issues/2026-09-02-chunk-line-ranges-are-body-relative-
                    // but-published-as-file-lines.md, still open). A token is
                    // resolved by `doc(action="get", heading=…)`'s fuzzy match, so
                    // it survives every edit above it — which is exactly the class
                    // of change that breaks a line number. Cheap, too: no title
                    // text, so this is ~55 chars whatever the entry is called.
                    if let Some(tok) = &chunk.entry_token {
                        item["matched"]["retrieve"] = json!(format!(
                            "doc(action=\"get\", id=\"{}\", heading=\"{tok}\")",
                            item["id"].as_str().unwrap_or_default()
                        ));
                    }
                }
                item
            })
            .collect();

        // Count-based hints stay off the semantic path: `more_in_repo` and friends
        // compare against a total the KNN never computed, so they would mislead.
        // But emptying the whole channel is what left starvation unreportable --
        // the widen-and-retry loop knows it is scraping the barrel and had no way
        // to say so. These two hints are KNN-native and carry no such comparison.
        //
        // BUG docs/issues/archive/2026-08-27-semantic-find-fills-the-page-past-relevance-with-no-score.md
        let hints = if a.semantic.is_some() {
            let mut h = serde_json::Map::new();
            if let Some((widenings, exhausted)) = starvation {
                if widenings > 0 {
                    h.insert("semantic_starved".into(), json!(widenings));
                    h.insert(
                        "semantic_starved_hint".into(),
                        json!(format!(
                            "the filter excluded the nearest matches, so the KNN was widened \
                             {widenings}x to fill this page -- these are the best REMAINING \
                             rows, not necessarily close ones. Compare `distance` across items, \
                             and re-run without the filter before concluding the corpus does \
                             not cover the query."
                        )),
                    );
                }
                if exhausted {
                    h.insert("semantic_exhausted".into(), json!(true));
                    h.insert(
                        "semantic_exhausted_hint".into(),
                        json!(
                            "the KNN cap was reached before the page filled -- this is every \
                               matching ARTIFACT that exists, not a truncated page."
                        ),
                    );
                }
            }
            // Silent at zero, and that silence is the point: on a corpus where
            // every artifact contributed one chunk there is nothing suppressed,
            // and a hint that fired anyway would train readers to ignore it. When
            // it does fire it names a RECOVERY, not just a condition — the whole
            // artifact is one `get` away, and without this the page is a capped
            // result presented as a complete one.
            if cap_suppressed > 0 {
                h.insert("cap_suppressed".into(), json!(cap_suppressed));
                h.insert(
                    "cap_suppressed_hint".into(),
                    json!(format!(
                        "{cap_suppressed} further chunk(s) belonging to artifacts already on \
                         this page were dropped -- this page carries AT MOST TWO chunks per \
                         artifact, so an artifact answering the query in more places than that \
                         is under-represented here. Read the whole thing with \
                         doc(action=\"get\", id=…), or narrow the query if you wanted breadth."
                    )),
                );
            }
            // Also silent at zero, and for a DIFFERENT reason than the cap above:
            // zero here is the healthy steady state, so a hint that fired anyway
            // would be noise on every well-formed query. When it fires it names a
            // store/reader GRAIN MISMATCH, which is not something the caller can
            // fix by rephrasing — it is an operator-facing fact, so the hint says
            // where to look rather than what to retype.
            if unresolved > 0 {
                h.insert("unresolved".into(), json!(unresolved));
                h.insert(
                    "unresolved_hint".into(),
                    json!(format!(
                        "{unresolved} vector(s) the store returned resolved to no chunk row and \
                         were discarded before ranking. Retrying will not change this: the \
                         vectors are stale, or the store holds ids at a grain this reader \
                         cannot resolve. Re-index to refresh them -- \
                         librarian(action=\"reindex\", reembed=true)."
                    )),
                );
            }
            Value::Object(h)
        } else {
            build_hints(
                &cat,
                base.as_ref(),
                &applied,
                &ctx.workspace,
                current,
                scope_fallback,
                user_constrains_status,
                a.include_archived,
                items.len(),
                offset,
                &exclude_worktrees,
                &deduped_main_ids,
                cutoff_ms,
            )?
        };

        (items, hints, catalog_value)
    };

    // When a rel_path filter returns nothing, scan the filesystem for unindexed
    // matching files so the caller gets an actionable error instead of silent empty.
    if items.is_empty() && a.semantic.is_none() {
        if let Some(ref hint) = rel_path_filter_hint {
            if let Some(ref cp) = ctx.current_project {
                let unindexed =
                    scan_unindexed_md(&ctx.workspace.roots, cp, hint, &ctx.workspace.ignore);
                if !unindexed.is_empty() {
                    let sample = unindexed
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(RecoverableError::new(format!(
                        "No indexed artifacts match rel_path ~ {hint:?}. \
                         Found {} unindexed file(s): {sample}. \
                         Run librarian(action=\"reindex\", scope=\"project\") to index them, then retry.",
                        unindexed.len()
                    )));
                }
            }
        }
    }

    let mut response = serde_json::json!({
        "count": items.len(),
        "items": items,
        "scope": applied.to_json(),
        "hints": hints,
    });
    if let Some(cat_val) = catalog_value {
        response["catalog"] = cat_val;
    }
    // `augmented=true` matched nothing, but the catalog DOES hold augmentations —
    // they are simply outside the filter or the scope. Say the count out loud: it is
    // the single fact that separates "these were destroyed" from "you are not looking
    // where they live", and without it a zero here is indistinguishable from loss.
    // On 2026-08-23 this exact shape — count 0 with a populated scope block — was read
    // as repo-wide augmentation loss and filed high-severity; the rows had been present
    // since 2026-07-05 throughout.
    // docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md
    if let Some(total) = augmented_in_catalog {
        if total > 0 && response["count"] == 0 {
            response["hints"]["augmented_present_but_out_of_scope"] = serde_json::json!({
                "augmented_in_catalog": total,
                "note": format!(
                    "This catalog holds {total} augmented artifact(s) — none matched this \
                     query. That is a filter/scope result, NOT evidence that augmentations \
                     were lost. Reconcile the `scope` block above against the project you \
                     meant, and widen with scope=\"repo\"/\"umbrella\" before concluding loss."
                ),
            });
        }
    }
    if no_augmentations_anywhere {
        response["hints"]["augmented_zero_is_catalog_wide"] = serde_json::json!({
            "note": "No artifact in this catalog carries an augmentation, at any scope — \
                     so this zero is catalog-wide, not a limit of the scope you queried.",
            "before_concluding_loss": "Augmentation lives only in the catalog DB and has no \
                     on-disk form, so reindex cannot rebuild it — and a zero here looks the \
                     same whether augmentations were destroyed or this session simply opened \
                     a different catalog file. Read `catalog.total` and the `scope` block \
                     above to tell those apart before reporting data loss.",
        });
    }
    // Two independent repairs can fire, and each needs its own explanation — a lift
    // reported under the inverted-leaf hint would tell the caller to fix a shape they
    // never wrote.
    let inverted_fired = !filter_corrections.is_empty();
    let lift_fired = !lift_corrections.is_empty();
    filter_corrections.extend(lift_corrections);
    if !filter_corrections.is_empty() {
        let mut hint = String::new();
        if inverted_fired {
            hint.push_str("Filter leaf shape is {field: {op: value}}, not {op: {field, value}}. ");
        }
        if lift_fired {
            hint.push_str(
                "`rel_path` is a create-time param; on find it was read as a \
                 filter clause. Pass filter={\"rel_path\": {\"contains\": …}} directly \
                 next time. ",
            );
        }
        hint.push_str("The query ran as corrected; use the canonical form next time.");
        response["corrections"] = serde_json::json!({
            "filter": filter_corrections,
            "hint": hint,
        });
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, ArtifactRow, TestArtifactRowBuilder};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::embedding::EmbeddingService;
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        TestToolContextBuilder::new(cat)
            .with_root(crate::librarian::workspace::Root {
                name: "code-explorer".into(),
                path: "/tmp/code-explorer".into(),
            })
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    main_root: None,
                    umbrella: None,
                },
            ))
            .build()
    }

    fn mk_ctx_with_embedder(cat: Catalog, svc: Arc<EmbeddingService>) -> ToolContext {
        let catalog = Arc::new(parking_lot::Mutex::new(cat));
        ToolContext {
            lsp: crate::lsp::MockLspProvider::with_client(crate::lsp::MockLspClient::default()),
            catalog: Arc::clone(&catalog),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            temp_guard: crate::librarian::tools::TempGuardEnv::from_env(),
            progress: None,
            embedding: Some(svc),
            // Exercise the semantic path against the in-memory artifact_vec via
            // the sqlite-vec backend (no Qdrant daemon in tests).
            artifact_store: Some(Arc::new(
                crate::librarian::artifact_store::SqliteVecArtifactStore::new(Arc::clone(&catalog)),
            )),
            current_project: None,
        }
    }

    fn sample_row(id: &str, title: &str) -> ArtifactRow {
        TestArtifactRowBuilder::new(id)
            .with_abs_path(format!("/test/code-explorer/{id}.md"))
            .with_title(title)
            .with_updated_at(1)
            .build()
    }

    #[tokio::test]
    async fn semantic_without_embedder_is_recoverable() {
        // BUG (omnibus 49ee6a03, F11): find() hard-failed via anyhow::bail! when
        // `semantic=` was passed with no embedding service configured. That is a
        // user-CORRECTABLE input error — the agent can simply retry without the
        // `semantic` param — so it must be a RecoverableError (isError:false at
        // the MCP boundary), not a hard failure that aborts sibling calls.
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat); // no embedder

        let err = call(&ctx, json!({"semantic": "some query"}))
            .await
            .expect_err("semantic without an embedder must error");

        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "must be RecoverableError so the agent can retry without `semantic`; got: {err}"
        );
    }

    #[tokio::test]
    async fn returns_rows_matching_filter() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "alpha")).unwrap();
        artifact::upsert(&cat, &sample_row("b", "beta")).unwrap();

        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"filter": {"kind": {"eq": "spec"}}}))
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(2));
    }

    #[tokio::test]
    async fn repairs_inverted_filter_and_notes_correction() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "or-tools guide")).unwrap();
        artifact::upsert(&cat, &sample_row("b", "unrelated")).unwrap();

        let ctx = mk_ctx(cat);
        // Inverted leaf {op:{field,value}} instead of {field:{op:value}} — the
        // handler repairs it and runs the query rather than erroring (no retry
        // round-trip), and rides a correction note back so the agent learns.
        let v = call(
            &ctx,
            json!({"filter": {"contains": {"field": "title", "value": "or-tools"}}}),
        )
        .await
        .expect("inverted filter should be repaired, not error");
        assert_eq!(v["count"].as_u64(), Some(1), "repaired query matches: {v}");
        assert!(
            v["corrections"]["filter"].is_array(),
            "correction note present: {v}"
        );
    }

    /// BUG docs/issues/archive/2026-08-17-find-silently-drops-top-level-rel-path.md
    ///
    /// `rel_path` is an advertised top-level param of the shared `artifact` schema and
    /// its description is written partly in `find` terms, but `Args` had no such field
    /// and cannot carry `deny_unknown_fields` — the dispatcher passes `action` through,
    /// and adding it once broke every `doc(update)` call. So serde dropped the key
    /// and the call ran at defaults: no filter, `limit: 50`. The reply was an
    /// unfiltered first page whose `count` reads as a match total.
    #[tokio::test]
    async fn lifts_top_level_rel_path_into_a_contains_filter() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("open-issue-work-queue", "the queue")).unwrap();
        artifact::upsert(&cat, &sample_row("tracker-hygiene-log", "hygiene")).unwrap();

        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"rel_path": "open-issue-work-queue"}))
            .await
            .expect("a top-level rel_path must not error");
        assert_eq!(
            v["count"].as_u64(),
            Some(1),
            "rel_path must narrow the query, not be dropped: {v}"
        );
    }

    /// A silent lift is the same defect in a new costume. `find` already teaches the
    /// caller when a filter's *shape* is wrong (see
    /// `repairs_inverted_filter_and_notes_correction`); a reinterpreted param has to
    /// ride back the same way, or the caller still cannot tell what query ran.
    #[tokio::test]
    async fn reports_the_lifted_rel_path_under_corrections() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("open-issue-work-queue", "the queue")).unwrap();

        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"rel_path": "open-issue-work-queue"}))
            .await
            .unwrap();
        assert!(
            v["corrections"]["filter"].is_array(),
            "the lift must be reported, not applied silently: {v}"
        );
    }

    /// `contains`, not `eq`: the catalog stores absolute paths and the relative form in
    /// responses is a display-time transform, so `eq` on a path as displayed matches
    /// nothing (U-35 in docs/trackers/codescout-usage-frictions.md). A lift that chose
    /// `eq` would turn a silent wrong answer into a silent empty one.
    ///
    /// The distractor row is load-bearing. An earlier version seeded one row only, so
    /// `count == 1` held whether or not `rel_path` was honored — it passed before the
    /// fix and proved nothing.
    #[tokio::test]
    async fn lifted_rel_path_uses_contains_so_a_displayed_path_still_matches() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("open-issue-work-queue", "the queue")).unwrap();
        artifact::upsert(&cat, &sample_row("tracker-hygiene-log", "hygiene")).unwrap();

        let ctx = mk_ctx(cat);
        // The caller passes the path as responses display it — no /test/code-explorer
        // prefix — which is exactly the form `eq` against the stored abs_path cannot
        // match.
        let v = call(&ctx, json!({"rel_path": "open-issue-work-queue.md"}))
            .await
            .unwrap();
        assert_eq!(
            v["count"].as_u64(),
            Some(1),
            "a displayed-form path must resolve to exactly its own row: {v}"
        );
    }

    /// An explicit `filter` stays authoritative — the lift ANDs into it rather than
    /// replacing it, so a caller who passes both does not silently lose one.
    #[tokio::test]
    async fn lifted_rel_path_combines_with_an_explicit_filter() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("open-issue-work-queue", "the queue")).unwrap();
        artifact::upsert(&cat, &sample_row("open-issue-archive", "the queue")).unwrap();

        let ctx = mk_ctx(cat);
        let v = call(
            &ctx,
            json!({"rel_path": "open-issue", "filter": {"title": {"contains": "queue"}}}),
        )
        .await
        .unwrap();
        assert_eq!(
            v["count"].as_u64(),
            Some(2),
            "both clauses must apply, not one: {v}"
        );

        let narrowed = call(
            &ctx,
            json!({"rel_path": "work-queue", "filter": {"title": {"contains": "queue"}}}),
        )
        .await
        .unwrap();
        assert_eq!(
            narrowed["count"].as_u64(),
            Some(1),
            "the rel_path clause must actually narrow inside the AND: {narrowed}"
        );
    }

    #[tokio::test]
    async fn defaults_hide_archived_when_filter_does_not_constrain_status() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut active = sample_row("a", "alpha");
        active.status = "active".into();
        let mut archived = sample_row("b", "beta");
        archived.status = "archived".into();
        artifact::upsert(&cat, &active).unwrap();
        artifact::upsert(&cat, &archived).unwrap();

        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"filter": {"kind": {"eq": "spec"}}}))
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(1));
        assert_eq!(v["hints"]["hidden_archived"].as_u64(), Some(1));
    }

    #[tokio::test]
    async fn status_in_filter_disables_archived_default() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut archived = sample_row("a", "alpha");
        archived.status = "archived".into();
        artifact::upsert(&cat, &archived).unwrap();

        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"filter": {"status": {"eq": "archived"}}}))
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(1));
    }

    #[tokio::test]
    async fn scope_all_widens_to_workspace() {
        let make_cat = || {
            let cat = Catalog::open_in_memory().unwrap();
            artifact::upsert(&cat, &sample_row("a", "in-project")).unwrap();
            // Inside the umbrella (member `/test/agents`) but outside the repo —
            // reachable by widening the scope.
            let mut elsewhere = sample_row("b", "elsewhere");
            elsewhere.abs_path = std::path::PathBuf::from("/test/agents/x/y.md");
            artifact::upsert(&cat, &elsewhere).unwrap();
            // Outside the umbrella entirely — in the catalog (it is machine-wide and
            // holds rows for unrelated repos, ghost repos and /tmp) but reachable by
            // NO scope value. Without this row the two hints below are numerically
            // identical and the test cannot tell their baselines apart, which is how
            // more_in_workspace shipped measuring from the wrong one.
            let mut foreign = sample_row("c", "outside-umbrella");
            foreign.abs_path = std::path::PathBuf::from("/other/ghost/c.md");
            artifact::upsert(&cat, &foreign).unwrap();
            cat
        };

        // Without umbrella: more_in_workspace hint must NOT appear — other repos are unrelated.
        let ctx = mk_ctx(make_cat());
        let v_default = call(&ctx, json!({"filter": {"kind": {"eq": "spec"}}}))
            .await
            .unwrap();
        assert_eq!(v_default["count"].as_u64(), Some(1));
        assert!(
            v_default["hints"]["more_in_workspace"].is_null(),
            "no umbrella → more_in_workspace hint must be absent"
        );

        // With umbrella: more_in_workspace hint must appear.
        let ctx_umbrella = TestToolContextBuilder::new(make_cat())
            .with_root(crate::librarian::workspace::Root {
                name: "code-explorer".into(),
                path: "/tmp/code-explorer".into(),
            })
            .with_umbrellas(vec![crate::librarian::workspace::Umbrella {
                name: "main".into(),
                members: vec![
                    std::path::PathBuf::from("/test/code-explorer"),
                    std::path::PathBuf::from("/test/agents"),
                ],
            }])
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    main_root: None,
                    umbrella: Some("main".into()),
                },
            ))
            .build();
        let v_umbrella = call(&ctx_umbrella, json!({"filter": {"kind": {"eq": "spec"}}}))
            .await
            .unwrap();
        assert_eq!(v_umbrella["count"].as_u64(), Some(1));
        // The two hints measure from DIFFERENT baselines, so with one row in each
        // region they must both be 1. Measuring more_in_workspace from `here` — the
        // shipped defect — makes it 2 and this assertion is what catches it.
        assert_eq!(
            v_umbrella["hints"]["more_in_umbrella"].as_u64(),
            Some(1),
            "the in-umbrella row is reachable by widening; got hints: {}",
            v_umbrella["hints"]
        );
        assert_eq!(
            v_umbrella["hints"]["more_in_workspace"].as_u64(),
            Some(1),
            "exactly the out-of-umbrella row lies beyond the reachable ceiling; \
             got hints: {}",
            v_umbrella["hints"]
        );
        // Unreachable rows get no `expand` entry, and the reachable one is offered
        // exactly once — pushing the same string per hint read as two remedies for one.
        let expand: Vec<&str> = v_umbrella["hints"]["expand"]
            .as_array()
            .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
            .unwrap_or_default();
        assert_eq!(
            expand,
            vec!["scope=\"all\""],
            "expand must offer the reachable widening once and nothing for the \
             unreachable surplus"
        );
        assert!(
            !v_umbrella["hints"]["more_in_workspace_hint"].is_null(),
            "an unreachable count must carry the action that does work"
        );

        // The contract the old code broke: following `expand` must deliver the
        // reachable count. count + more_in_umbrella, NOT + more_in_workspace.
        let v_all = call(
            &ctx_umbrella,
            json!({"filter": {"kind": {"eq": "spec"}}, "scope": "all"}),
        )
        .await
        .unwrap();
        assert_eq!(v_all["scope"]["applied"], "umbrella");
        let reachable = v_umbrella["count"].as_u64().unwrap()
            + v_umbrella["hints"]["more_in_umbrella"].as_u64().unwrap();
        assert_eq!(
            v_all["count"].as_u64(),
            Some(reachable),
            "scope=\"all\" must return exactly what the expand hint promised"
        );
        assert_eq!(v_all["count"].as_u64(), Some(2));
    }

    #[tokio::test]
    async fn scope_all_does_not_self_reference_expand_hint() {
        // BUG (docs/issues/archive/2026-07-17-artifact-find-ignores-workspace-pin.md,
        // sub-finding #2): passing scope="all" aliases to umbrella
        // (applied="umbrella"). build_hints counted rows OUTSIDE the umbrella —
        // unreachable, since "all" always re-aliases to umbrella — and emitted
        // expand:["scope=\"all\""], suggesting the exact param already passed.
        // At the broadest reachable scope there is nothing to widen to.
        let cat = Catalog::open_in_memory().unwrap();
        // One row INSIDE the umbrella, one OUTSIDE it (a foreign / ghost repo,
        // mirroring the deleted-repo + /tmp rows the real shared catalog holds).
        let mut inside = sample_row("a", "in-umbrella");
        inside.abs_path = std::path::PathBuf::from("/test/agents/x/y.md");
        artifact::upsert(&cat, &inside).unwrap();
        let mut outside = sample_row("b", "outside-umbrella");
        outside.abs_path = std::path::PathBuf::from("/other/ghost/z.md");
        artifact::upsert(&cat, &outside).unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_umbrellas(vec![crate::librarian::workspace::Umbrella {
                name: "main".into(),
                members: vec![
                    std::path::PathBuf::from("/test/code-explorer"),
                    std::path::PathBuf::from("/test/agents"),
                ],
            }])
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    main_root: None,
                    umbrella: Some("main".into()),
                },
            ))
            .build();

        let v = call(
            &ctx,
            json!({"filter": {"kind": {"eq": "spec"}}, "scope": "all"}),
        )
        .await
        .unwrap();
        // scope="all" aliases to umbrella → only the in-umbrella row is reachable.
        assert_eq!(v["scope"]["applied"], "umbrella");
        assert_eq!(v["count"].as_u64(), Some(1));
        // No widen hint at the broadest reachable scope; in particular no
        // count of the unreachable extra-umbrella row.
        assert!(
            v["hints"]["more_in_workspace"].is_null(),
            "at umbrella scope there is nothing broader to reach; got hints: {}",
            v["hints"]
        );
        // And crucially the expand list must never suggest scope="all" — the
        // very parameter that was passed (the self-referential bug).
        let suggests_all = v["hints"]["expand"]
            .as_array()
            .is_some_and(|e| e.iter().any(|s| s == "scope=\"all\""));
        assert!(
            !suggests_all,
            "expand must not suggest the already-passed scope=\"all\"; got hints: {}",
            v["hints"]
        );
    }

    #[tokio::test]
    async fn clamps_oversized_limit() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "alpha")).unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"limit": 10_000_000})).await.unwrap();
        assert!(v["count"].as_u64().unwrap() <= 500);
    }

    #[tokio::test]
    async fn more_in_scope_signals_capped_page() {
        // Regression for the silent-cap family: a limit-capped page must signal
        // that more match in scope. docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md
        let cat = Catalog::open_in_memory().unwrap();
        for i in 0..3 {
            artifact::upsert(&cat, &sample_row(&format!("id{i}"), &format!("t{i}"))).unwrap();
        }
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"limit": 2})).await.unwrap();
        assert_eq!(v["count"].as_u64(), Some(2), "page size stays items.len()");
        assert_eq!(
            v["hints"]["more_in_scope"].as_u64(),
            Some(1),
            "3 match, 2 returned → 1 more in scope"
        );
    }

    #[tokio::test]
    async fn no_more_in_scope_when_page_holds_everything() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "alpha")).unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({})).await.unwrap();
        assert!(
            v["hints"]["more_in_scope"].is_null(),
            "nothing capped → no more_in_scope signal"
        );
    }

    /// docs/issues/archive/2026-08-17-artifact-find-is-silent-about-files-the-catalog-has-never-seen.md
    ///
    /// A file dropped onto disk without going through `doc(action="create")`
    /// (a `create_file`, `Write`, or peer `git commit`) is invisible to `find` and
    /// nothing in the response says so — the load-bearing half. Real disk I/O:
    /// `git_root` must be a real tempdir since `count_disk_md` walks it for real.
    #[tokio::test]
    async fn unindexed_disk_files_surface_a_staleness_hint_then_clear_after_reindex() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        std::fs::write(root.join("indexed.md"), "# Indexed\n").unwrap();
        std::fs::write(root.join("unindexed.md"), "# Unindexed\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut indexed = sample_row("indexed", "Indexed");
        indexed.abs_path = root.join("indexed.md");
        artifact::upsert(&cat, &indexed).unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: root.clone(),
                    git_root: root.clone(),
                    main_root: None,
                    umbrella: None,
                },
            ))
            .build();

        // The load-bearing assertion: this must fire BEFORE any reindex — a test
        // that only checks post-reindex behaviour would pass today.
        let out = call(&ctx, json!({})).await.unwrap();
        assert_eq!(
            out["hints"]["unindexed_files"].as_u64(),
            Some(1),
            "one file on disk has no catalog row: {:?}",
            out["hints"]
        );
        assert!(
            out["hints"]["unindexed_hint"]
                .as_str()
                .unwrap()
                .contains("reindex"),
            "hint must name the fix: {:?}",
            out["hints"]
        );

        // Reindex for real, then the hint must clear and the new row must return.
        let rules = crate::librarian::classify::default_rules().unwrap();
        let ignore = globset::GlobSetBuilder::new().build().unwrap();
        {
            let cat = ctx.catalog.lock();
            crate::librarian::indexer::index_repo_sync(
                &cat, &rules, &root, &ignore, false, false, false,
            )
            .unwrap();
        }

        let out_after = call(&ctx, json!({})).await.unwrap();
        assert!(
            out_after["hints"].get("unindexed_files").is_none(),
            "reindex must clear the staleness hint: {:?}",
            out_after["hints"]
        );
        assert_eq!(
            out_after["count"].as_u64(),
            Some(2),
            "both files are indexed now"
        );
    }

    /// Step 2 of docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md:
    /// the `catalog_meta` marker `reindex.rs` now persists (see
    /// `librarian::tools::reindex::tests::an_embed_failure_persists_a_durable_catalog_meta_marker`)
    /// is dead weight until something reads it back. `find` is the surface a caller
    /// actually queries after the call that failed has already returned — same shape
    /// as `unindexed_hint` above, sourced from a different signal.
    #[tokio::test]
    async fn catalog_degraded_hint_appears_after_a_persisted_embed_failure_then_clears() {
        use crate::librarian::catalog::gc;

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "A")).unwrap();
        gc::set_meta(&cat.conn, "last_reindex_embed_error_count", "2").unwrap();
        gc::set_meta(
            &cat.conn,
            "last_reindex_embed_errors_sample",
            r#"["a: embed failed: connection refused","b: embed failed: connection refused"]"#,
        )
        .unwrap();

        let ctx = mk_ctx(cat);

        let out = call(&ctx, json!({})).await.unwrap();
        assert_eq!(
            out["hints"]["catalog_degraded"].as_u64(),
            Some(2),
            "the persisted marker must surface on the very next find, not just the \
             call that wrote it: {:?}",
            out["hints"]
        );
        assert!(
            out["hints"]["catalog_degraded_hint"]
                .as_str()
                .unwrap()
                .contains("reindex"),
            "hint must name the fix: {:?}",
            out["hints"]
        );

        {
            let cat = ctx.catalog.lock();
            gc::set_meta(&cat.conn, "last_reindex_embed_error_count", "0").unwrap();
            gc::set_meta(&cat.conn, "last_reindex_embed_errors_sample", "[]").unwrap();
        }

        let out_after = call(&ctx, json!({})).await.unwrap();
        assert!(
            out_after["hints"].get("catalog_degraded").is_none(),
            "a clean reindex clearing the marker must clear the hint too: {:?}",
            out_after["hints"]
        );
    }

    #[tokio::test]
    async fn worktree_find_shadows_main_twin_and_flags_overlay() {
        use crate::librarian::tools::worktree::test_support::{seed_main_tracker, wt_ctx};
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c)
        };
        let shadow_id = {
            let mut c = ctx.catalog.lock();
            crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id).unwrap()
        };
        let out = call(&ctx, serde_json::json!({"scope": "repo"}))
            .await
            .unwrap();
        let ids: Vec<&str> = out["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&shadow_id.as_str()), "shadow visible: {ids:?}");
        assert!(
            !ids.contains(&main_id.as_str()),
            "main twin suppressed: {ids:?}"
        );
        let shadow_item = out["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["id"] == shadow_id.as_str())
            .unwrap();
        assert_eq!(shadow_item["overlay"], true);
        // Canonical Defect-2 regression: `here` (a raw scope COUNT) counts BOTH
        // the shadow and its deduped main twin, but `items.len()` passed to
        // `build_hints` is post-dedup — 1 tracker + 1 fork, scope=repo, default
        // paging must page-complete with NO more_in_scope hint (nothing is left
        // to page to).
        assert!(
            out["hints"].get("more_in_scope").is_none(),
            "spurious more_in_scope after dedup: {:?}",
            out["hints"]
        );
        assert!(
            out["hints"].get("more_in_scope_hint").is_none(),
            "spurious more_in_scope_hint after dedup: {:?}",
            out["hints"]
        );
    }

    #[tokio::test]
    async fn non_worktree_find_does_not_dedup_or_flag_despite_worktree_of_link() {
        // A worktree_of link existing in the catalog must never affect a plain
        // (non-worktree) session's find results — the dedup is gated on
        // `main_root.is_some()`, not merely on whether a matching link exists.
        use crate::librarian::catalog::links::{self, LinkRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("main-1", "Main")).unwrap();
        artifact::upsert(&cat, &sample_row("shadow-1", "Shadow")).unwrap();
        links::insert(
            &cat,
            &LinkRow {
                src_id: "shadow-1".into(),
                dst_id: "main-1".into(),
                rel: "worktree_of".into(),
                created_at: 0,
            },
        )
        .unwrap();

        let ctx = mk_ctx(cat);
        let out = call(
            &ctx,
            json!({"filter": {"id": {"in": ["main-1", "shadow-1"]}}}),
        )
        .await
        .unwrap();
        let items = out["items"].as_array().unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"main-1"), "main not suppressed: {ids:?}");
        assert!(ids.contains(&"shadow-1"), "shadow not suppressed: {ids:?}");
        for item in items {
            assert!(
                item.get("overlay").is_none(),
                "no overlay flag for non-worktree session: {item:?}"
            );
        }
    }

    struct MockEmbedder;

    /// Park a unit vector for `id` on axis `axis` in the sqlite-vec table.
    /// `MockEmbedder` puts an "auth" query on axis 0 and everything else on axis
    /// 1, so `axis` selects whether a fixture is near or far from an auth query.
    ///
    /// **CHUNK-KEYED since Task 8.** `knn` reads `artifact_vec_v2`, whose ids
    /// are chunk ids, and `semantic_find` resolves each back to its artifact
    /// through `artifact_chunk`. Seeding `artifact_vec` — or seeding v2 under an
    /// ARTIFACT id — leaves every candidate unresolvable, and the failure shows
    /// up as an empty page rather than an error, so it reads like a ranking bug.
    fn seed_vec(cat: &Catalog, id: &str, axis: usize) {
        let built = crate::librarian::catalog::chunk::build_chunks(id, "# T\n\nbody\n", 2048, 0);
        let rows = crate::librarian::catalog::chunk::replace_chunks(cat, id, &built).unwrap();
        let mut v = vec![0.0f32; 768];
        v[axis] = 1.0;
        let blob: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
        cat.conn
            .execute(
                "INSERT OR REPLACE INTO artifact_vec_v2 (id, embedding) VALUES (?1, ?2)",
                rusqlite::params![rows[0].chunk_id, blob],
            )
            .unwrap();
    }

    #[async_trait::async_trait]
    impl codescout_embed::Embedder for MockEmbedder {
        fn dimensions(&self) -> usize {
            768
        }
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<codescout_embed::Embedding>> {
            Ok(texts
                .iter()
                .map(|t| {
                    let mut v = vec![0.0f32; 768];
                    if t.contains("auth") {
                        v[0] = 1.0;
                    } else {
                        v[1] = 1.0;
                    }
                    v
                })
                .collect())
        }
    }

    #[tokio::test]
    async fn semantic_search_returns_closest_artifact_first() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("auth-doc", "Authentication Guide")).unwrap();
        artifact::upsert(&cat, &sample_row("deploy-doc", "Deployment Runbook")).unwrap();

        // Was two hand-rolled INSERTs into `artifact_vec`. `seed_vec` owns the
        // chunk-keyed shape now, so there is ONE place to change when the
        // storage grain moves again — this duplication is exactly why the grain
        // change had a second site to fix.
        seed_vec(&cat, "auth-doc", 0);
        seed_vec(&cat, "deploy-doc", 1);

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);

        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 10, "scope": "all"}),
        )
        .await
        .unwrap();

        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), 2, "both artifacts should be returned");
        assert_eq!(items[0]["id"], "auth-doc");
    }

    /// Seed `id` as a three-chunk ledger — a preamble plus `## W-1` and `## W-2`
    /// entries — and park the chunk at `near_ix` on the auth axis while the rest
    /// sit on axis 1. `None` puts **every** chunk on the auth axis, which is how a
    /// test makes the per-artifact cap (rather than distance) the thing that drops
    /// the siblings. Returns the chunk rows in `chunk_ix` order, so a caller names
    /// the one it wants by POSITION and never by a literal id.
    fn seed_entry_chunks(
        cat: &Catalog,
        id: &str,
        near_ix: Option<usize>,
    ) -> Vec<crate::librarian::catalog::chunk::ChunkRow> {
        let body = "# T\n\nintro\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\nbeta\n";
        let built = crate::librarian::catalog::chunk::build_chunks(id, body, 2048, 0);
        let rows = crate::librarian::catalog::chunk::replace_chunks(cat, id, &built).unwrap();
        assert!(
            rows.len() >= 3,
            "fixture needs a preamble plus two entry chunks or the tests below stop \
             discriminating, got {}",
            rows.len()
        );
        for (i, r) in rows.iter().enumerate() {
            let mut v = vec![0.0f32; 768];
            v[if near_ix.is_none_or(|n| n == i) { 0 } else { 1 }] = 1.0;
            let blob: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
            cat.conn
                .execute(
                    "INSERT OR REPLACE INTO artifact_vec_v2 (id, embedding) VALUES (?1, ?2)",
                    rusqlite::params![r.chunk_id, blob],
                )
                .unwrap();
        }
        rows
    }

    /// A hit must name the **entry** that matched, not the artifact's opening
    /// lines. While the store was artifact-keyed the only thing a hit could say
    /// was "somewhere in this file", which on a 9,000-line ledger restates the
    /// retrieval problem rather than answering it.
    ///
    /// LOAD-BEARING: the near chunk is index **2**, not 0. Index 0 is the
    /// preamble — precisely what an artifact-grain hit would report — so a fixture
    /// whose nearest chunk is the preamble cannot tell the two grains apart, and
    /// would pass on a tree where `matched` named line 1 unconditionally.
    #[tokio::test]
    async fn a_semantic_hit_names_the_entry_that_matched_not_the_preamble() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("ledger", "Gate Ledger")).unwrap();
        let rows = seed_entry_chunks(&cat, "ledger", Some(2));
        let expected = rows[2].clone();

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);
        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 3, "scope": "all"}),
        )
        .await
        .unwrap();

        let m = &v["items"][0]["matched"];
        assert_eq!(
            m["entry_token"],
            json!("W-2"),
            "a hit must name the entry it landed in: {v}"
        );
        assert_eq!(m["start_line"], json!(expected.start_line));
        assert_eq!(m["end_line"], json!(expected.end_line));
        assert!(
            m["start_line"].as_u64().unwrap() > 1,
            "line 1 is the preamble — an artifact-grain answer wearing a chunk-grain shape: {m}"
        );
        assert!(
            m["snippet"].as_str().unwrap().contains("beta"),
            "the snippet must be the matched chunk's own text, not the file's head: {m}"
        );
    }

    /// Two chunks of ONE artifact in one page must each carry their own
    /// `matched` span and `distance`.
    ///
    /// This is the case `max_per_artifact = 1` made unreachable, and the reason
    /// raising the cap was blocked: `distance_by_id` and `chunk_by_id` were
    /// `HashMap<artifact_id, _>`, so a second hit for the same artifact
    /// OVERWROTE the first while `rows` kept both. Every item for that artifact
    /// then rendered the LAST chunk's span — a wrong `matched` on a
    /// plausible-looking item, never an error.
    ///
    /// LOAD-BEARING FIXTURE DETAIL: `seed_entry_chunks(.., None)` puts every
    /// chunk on the same axis, so all three are equidistant and the CAP — not
    /// distance — is what decides how many survive. Give one chunk a near axis
    /// instead and the page holds a single hit, the collision cannot occur, and
    /// this test passes against the defect it exists to catch.
    ///
    /// It pins the cap as well as the keying: at `max_per_artifact = 1` the page
    /// holds one item and the first assertion fails. That coupling is deliberate
    /// — the two are only correct together, and a future cap change should have
    /// to come here and read this.
    #[tokio::test]
    async fn two_chunks_of_one_artifact_keep_their_own_matched_span() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("ledger", "Gate Ledger")).unwrap();
        seed_entry_chunks(&cat, "ledger", None);

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);
        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 5, "scope": "all"}),
        )
        .await
        .unwrap();

        let items = v["items"].as_array().expect("items array");
        assert_eq!(
            items.len(),
            2,
            "one artifact should contribute exactly `max_per_artifact` = 2 hits: {v}"
        );
        assert_eq!(items[0]["id"], items[1]["id"], "both hits are one artifact");

        let a = &items[0]["matched"];
        let b = &items[1]["matched"];
        assert!(
            !a.is_null() && !b.is_null(),
            "both hits must carry their matched span: {v}"
        );
        assert_ne!(
            a["start_line"], b["start_line"],
            "two chunks of one artifact rendered the SAME span — the side maps are \
             keyed by artifact id and the second hit overwrote the first: {v}"
        );
        assert_ne!(
            a["entry_token"], b["entry_token"],
            "each hit must name the entry IT landed in, not the last one written: {v}"
        );
    }

    /// A hit on a fragment says so, and hands back a handle that is not a line.
    ///
    /// Both halves matter and they fail differently. Without `part`/`of` a hit on
    /// chunk 4 of 7 is indistinguishable from a hit on a whole short entry — the
    /// span cannot carry it, since `end_line - start_line` describes the fragment.
    /// Without a token-addressed `retrieve` the only recovery offered is the line
    /// range, which is measurably wrong for 378 of 3,729 entry-bearing chunks in
    /// this repo (`7695ad877b44e96a`, open): it resolves to the PREVIOUS entry.
    ///
    /// LOAD-BEARING: `chunk_size` cannot be set from the tool, so the fixture
    /// forces a split by making one entry long rather than by shrinking the
    /// budget. If a future edit makes this body fit in one chunk, the
    /// `parts > 1` assertion fails loudly rather than passing vacuously at
    /// `1 of 1`.
    #[tokio::test]
    async fn a_hit_on_a_fragment_says_which_part_and_offers_a_token_handle() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("ledger", "Gate Ledger")).unwrap();

        let filler = "auth login flow session token exchange and refresh handling\n".repeat(80);
        let body = format!("# T\n\nintro\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\n{filler}");
        let built = crate::librarian::catalog::chunk::build_chunks("ledger", &body, 2048, 0);
        let rows =
            crate::librarian::catalog::chunk::replace_chunks(&cat, "ledger", &built).unwrap();
        let w2_parts = rows
            .iter()
            .filter(|r| r.entry_token.as_deref() == Some("W-2"))
            .count();
        assert!(
            w2_parts > 1,
            "fixture must split W-2 or the part/of assertion is vacuous; got {w2_parts}"
        );
        for r in &rows {
            let mut v = vec![0.0f32; 768];
            v[0] = 1.0;
            let blob: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
            cat.conn
                .execute(
                    "INSERT OR REPLACE INTO artifact_vec_v2 (id, embedding) VALUES (?1, ?2)",
                    rusqlite::params![r.chunk_id, blob],
                )
                .unwrap();
        }

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);
        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 5, "scope": "all"}),
        )
        .await
        .unwrap();

        let m = &v["items"][0]["matched"];
        let tok = m["entry_token"].as_str().expect("a hit names its entry");
        assert_eq!(
            m["of"].as_u64(),
            Some(w2_parts as u64),
            "a fragment must say how many parts its entry has: {v}"
        );
        assert!(
            m["part"]
                .as_u64()
                .is_some_and(|p| p >= 1 && p <= w2_parts as u64),
            "part must be a 1-based index within of: {m}"
        );
        let retrieve = m["retrieve"].as_str().expect("a recovery action");
        assert!(
            retrieve.contains(&format!("heading=\"{tok}\"")),
            "the handle must address the entry by TOKEN — a line number is the thing \
             that decays under edits above it: {retrieve}"
        );
        assert!(
            !retrieve.contains("start_line"),
            "the handle must not fall back to line addressing: {retrieve}"
        );
    }

    /// An artifact answering the query more times than the cap allows appears
    /// only `max_per_artifact` times, and the page has to say so. The dropped
    /// chunks are not absent from the corpus, only from the page, and nothing
    /// else in the response distinguishes those two states — which is
    /// `cluster/capped-result-presented-as-complete`.
    ///
    /// LOAD-BEARING FIXTURE HEADROOM: `seed_entry_chunks` builds exactly three
    /// chunks, so at `max_per_artifact = 2` this fixture has ONE chunk of slack.
    /// Raise the cap to 3 and `cap_suppressed` becomes 0 — the assertions below
    /// would still pass on `items.len()` while testing nothing about suppression
    /// at all, because there would be nothing left to suppress. A cap change must
    /// widen the fixture here, not just retune these two numbers.
    #[tokio::test]
    async fn suppressed_sibling_chunks_are_reported_rather_than_silently_dropped() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("ledger", "Gate Ledger")).unwrap();
        // Every chunk on the auth axis, so all three are candidates and the
        // per-artifact CAP is what removes the surplus rather than distance.
        seed_entry_chunks(&cat, "ledger", None);

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);
        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 5, "scope": "all"}),
        )
        .await
        .unwrap();

        assert_eq!(
            v["items"].as_array().unwrap().len(),
            2,
            "at most `max_per_artifact` = 2 chunks per artifact: {v}"
        );
        assert_eq!(
            v["hints"]["cap_suppressed"],
            json!(1),
            "the third sibling chunk was dropped and the page must say how many: {}",
            v["hints"]
        );
        assert!(
            v["hints"]["cap_suppressed_hint"]
                .as_str()
                .unwrap_or_default()
                .contains("doc(action=\"get\""),
            "the hint must name the recovery action, not only the condition: {}",
            v["hints"]
        );
    }

    /// The control for the hint above — the case where it must stay **silent**.
    /// Without it a `cap_suppressed` that fired unconditionally would look correct
    /// in every other test here, and the field's whole value is that its absence
    /// means something.
    #[tokio::test]
    async fn a_page_with_nothing_suppressed_says_nothing() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("auth-doc", "Authentication Guide")).unwrap();
        artifact::upsert(&cat, &sample_row("deploy-doc", "Deployment Runbook")).unwrap();
        seed_vec(&cat, "auth-doc", 0);
        seed_vec(&cat, "deploy-doc", 1);

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);
        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 10, "scope": "all"}),
        )
        .await
        .unwrap();

        assert_eq!(v["items"].as_array().unwrap().len(), 2);
        assert!(
            v["hints"].get("cap_suppressed").is_none(),
            "single-chunk artifacts suppress nothing, so the field must be absent: {}",
            v["hints"]
        );
    }

    /// The snippet is bounded **and says so when it cuts** — both directions in
    /// one test, because a marker is only informative if its absence is too. A
    /// snippet that always carried the marker satisfies the first assertion and
    /// tells a reader nothing; the second assertion is what makes the first mean
    /// "this one was cut" rather than "snippets have a suffix".
    #[tokio::test]
    async fn a_cut_snippet_says_it_was_cut_and_an_uncut_one_does_not() {
        fn seed_one_chunk(cat: &Catalog, id: &str, body: &str) {
            let built = crate::librarian::catalog::chunk::build_chunks(id, body, 8192, 0);
            let rows = crate::librarian::catalog::chunk::replace_chunks(cat, id, &built).unwrap();
            assert_eq!(
                rows.len(),
                1,
                "this fixture wants exactly one chunk for {id}"
            );
            let mut v = vec![0.0f32; 768];
            v[0] = 1.0;
            let blob: Vec<u8> = v.iter().flat_map(|f| f.to_le_bytes()).collect();
            cat.conn
                .execute(
                    "INSERT OR REPLACE INTO artifact_vec_v2 (id, embedding) VALUES (?1, ?2)",
                    rusqlite::params![rows[0].chunk_id, blob],
                )
                .unwrap();
        }

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("long", "Long Entry")).unwrap();
        artifact::upsert(&cat, &sample_row("short", "Short Entry")).unwrap();
        let long_body = format!("## W-9 — t\n\n{}\n", "padding ".repeat(200));
        assert!(
            long_body.chars().count() > 480,
            "fixture must exceed SNIPPET_CHARS or nothing is cut, got {}",
            long_body.chars().count()
        );
        seed_one_chunk(&cat, "long", &long_body);
        seed_one_chunk(&cat, "short", "## W-8 — t\n\nshort body\n");

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);
        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 10, "scope": "all"}),
        )
        .await
        .unwrap();

        let items = v["items"].as_array().unwrap();
        let snip = |id: &str| -> String {
            items
                .iter()
                .find(|i| i["id"] == id)
                .unwrap_or_else(|| panic!("{id} missing from {v}"))["matched"]["snippet"]
                .as_str()
                .unwrap()
                .to_string()
        };

        let long_snip = snip("long");
        assert!(
            long_snip.contains("snippet truncated"),
            "a cut snippet that does not say it was cut is a capped result presented as \
             complete: {long_snip:?}"
        );
        assert!(
            long_snip.chars().count() < long_body.chars().count(),
            "the bound must actually bind"
        );

        let short_snip = snip("short");
        assert!(
            !short_snip.contains("snippet truncated"),
            "an uncut snippet must not claim it was cut, or the marker means nothing: \
             {short_snip:?}"
        );
        assert!(
            short_snip.contains("short body"),
            "an uncut snippet is the whole chunk: {short_snip:?}"
        );
    }

    /// The distance must reach the caller, per item. Before this, `semantic_find`
    /// widened `k` until it could fill the page and returned rows in KNN order
    /// with the magnitude discarded — ordering survived, magnitude did not — so a
    /// caller could not tell a strong match from the least-bad remainder.
    ///
    /// BUG docs/issues/archive/2026-08-27-semantic-find-fills-the-page-past-relevance-with-no-score.md
    #[tokio::test]
    async fn semantic_items_carry_a_distance_that_ascends_with_rank() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("auth-doc", "Authentication Guide")).unwrap();
        artifact::upsert(&cat, &sample_row("deploy-doc", "Deployment Runbook")).unwrap();
        seed_vec(&cat, "auth-doc", 0);
        seed_vec(&cat, "deploy-doc", 1);

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);
        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 10, "scope": "all"}),
        )
        .await
        .unwrap();

        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        let d0 = items[0]["distance"]
            .as_f64()
            .unwrap_or_else(|| panic!("no distance on the top item: {}", items[0]));
        let d1 = items[1]["distance"]
            .as_f64()
            .unwrap_or_else(|| panic!("no distance on the second item: {}", items[1]));
        assert!(
            d0 < d1,
            "distance must ascend with rank (lower is closer): {d0} then {d1}"
        );
    }

    /// A non-semantic query must NOT grow a `distance` field. It has no query
    /// vector, so any number here would be fabricated.
    #[tokio::test]
    async fn a_plain_filter_query_carries_no_distance() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("auth-doc", "Authentication Guide")).unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"filter": {"id": {"eq": "auth-doc"}}}))
            .await
            .unwrap();
        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert!(
            items[0].get("distance").is_none(),
            "no query vector exists, so no distance may be reported: {}",
            items[0]
        );
    }

    /// A small corpus is EXHAUSTED, not filter-starved, and the two must not be
    /// conflated: `semantic_starved` claims the filter removed the nearest
    /// matches, which is false here and would send a reader hunting a filter that
    /// is not the problem.
    ///
    /// This is also the control for the starvation hint — it is the case where the
    /// hint must stay silent, and without it a hint that fired unconditionally
    /// would look correct in every other test.
    #[tokio::test]
    async fn a_short_page_from_a_small_corpus_reports_exhausted_not_starved() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("auth-doc", "Authentication Guide")).unwrap();
        artifact::upsert(&cat, &sample_row("deploy-doc", "Deployment Runbook")).unwrap();
        seed_vec(&cat, "auth-doc", 0);
        seed_vec(&cat, "deploy-doc", 1);

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);
        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 10, "scope": "all"}),
        )
        .await
        .unwrap();

        assert_eq!(v["items"].as_array().unwrap().len(), 2);
        assert_eq!(
            v["hints"]["semantic_exhausted"], true,
            "a page short of its limit must say the search ran out: {}",
            v["hints"]
        );
        assert!(
            v["hints"].get("semantic_starved").is_none(),
            "a 2-row corpus is not filter starvation: {}",
            v["hints"]
        );
    }

    /// **The load-bearing test.** A filter that excludes the nearest matches must
    /// say so, rather than silently backfilling the page with the least-bad
    /// remainder and presenting it in the same shape as a satisfied query.
    ///
    /// This is the reported symptom: a query whose true best matches are `kind:
    /// bug`, filtered to `kind: tracker`, returned a full page of unrelated
    /// trackers with `hints: {}` — and read as "nothing indexed covers this."
    /// Only the unfiltered control revealed the corpus answered it at #1.
    ///
    /// The fixture needs MORE than the first `k` candidates for the widen path to
    /// be reachable at all. **That floor moved in Task 8** — `k` now starts at
    /// `(target * 5 * max_per_artifact.max(1)).max(200)`, so with `limit: 2` it is
    /// 200, not the 100 it was when this test was written. The seed count went
    /// 150 -> 250 to stay above it. Re-derive this number if either constant
    /// changes: below the floor the store returns fewer rows than `k`,
    /// `store_exhausted` fires on the first pass, and the test goes GREEN while
    /// asserting nothing at all about starvation.
    ///
    /// BUG docs/issues/archive/2026-08-27-semantic-find-fills-the-page-past-relevance-with-no-score.md
    #[tokio::test]
    async fn a_filter_that_excludes_the_nearest_matches_reports_starvation() {
        let cat = Catalog::open_in_memory().unwrap();

        // 250 "note" artifacts sitting ON the auth axis — these are the nearest
        // neighbours of an auth query, and the filter below removes every one.
        // The count is LOAD-BEARING: it must exceed the initial `k`, now 200.
        for i in 0..250 {
            let id = format!("note-{i}");
            let mut row = sample_row(&id, "Auth Note");
            row.kind = "note".into();
            artifact::upsert(&cat, &row).unwrap();
            seed_vec(&cat, &id, 0);
        }
        // Two trackers far away on the other axis — the least-bad remainder.
        for i in 0..2 {
            let id = format!("tracker-{i}");
            let mut row = sample_row(&id, "Unrelated Tracker");
            row.kind = "tracker".into();
            artifact::upsert(&cat, &row).unwrap();
            seed_vec(&cat, &id, 1);
        }

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);
        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "kind": "tracker", "limit": 2, "scope": "all"}),
        )
        .await
        .unwrap();

        // The rows themselves are still the right answer — returning the nearest
        // survivors is defensible. Returning them UNLABELLED was the bug.
        assert_eq!(v["items"].as_array().unwrap().len(), 2);
        let starved = v["hints"]["semantic_starved"].as_u64().unwrap_or_else(|| {
            panic!(
                "a filter that removed all 250 nearest rows must report starvation: {}",
                v["hints"]
            )
        });
        assert!(starved >= 1, "widening count must be reported: {starved}");
        assert!(
            v["hints"]["semantic_starved_hint"]
                .as_str()
                .unwrap_or_default()
                .contains("without the filter"),
            "the hint must name the recovery action, not just the condition: {}",
            v["hints"]
        );
    }

    #[tokio::test]
    async fn augmented_true_returns_only_augmented() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("plain", "Plain")).unwrap();
        artifact::upsert(&cat, &sample_row("aug", "Augmented")).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "aug".to_string(),
                prompt: "p".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"augmented": true})).await.unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "aug");
    }

    #[tokio::test]
    async fn augmented_true_with_no_augmentations_still_reports_scope_and_catalog() {
        // A zero from `augmented=true` used to short-circuit to
        // `{count: 0, items: [], scope: null, hints: {}}` — stripping every diagnostic
        // that separates "nothing is augmented" from "this session resolved another
        // project's scope" or "this session opened a different catalog file", from
        // exactly the one response that needed them. On 2026-08-23 that bare zero was
        // read as repo-wide augmentation loss and filed high-severity with an
        // "Established" root cause; the catalog's own created_at/updated_at columns
        // later showed all 21 rows present throughout.
        // docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("plain", "Plain")).unwrap();
        artifact::upsert(&cat, &sample_row("other", "Other")).unwrap();
        let ctx = mk_ctx(cat);

        let result = call(&ctx, json!({"augmented": true})).await.unwrap();

        assert_eq!(result["count"], 0, "nothing is augmented, so no rows");
        assert!(
            !result["scope"].is_null(),
            "the scope block must survive the empty-augmentation path — it is what \
             tells the caller WHICH world the zero describes: {result}"
        );
        assert_eq!(
            result["catalog"]["total"], 2,
            "catalog counts must ride along, so a populated catalog with no \
             augmentations is distinguishable from an empty or wrong one: {result}"
        );
        assert!(
            !result["hints"]["augmented_zero_is_catalog_wide"].is_null(),
            "the zero must declare itself catalog-wide rather than scope-limited: {result}"
        );
    }

    #[tokio::test]
    async fn augmented_zero_says_how_many_augmentations_the_catalog_holds() {
        // The shape the 2026-08-23 incident actually hit: `augmented=true` matched
        // nothing, the scope block WAS populated, and the zero was still read as
        // repo-wide loss. A zero that does not say "N exist, none here" cannot be
        // told apart from a zero that means "none exist anywhere" — and the rows in
        // question had been present since 2026-07-05.
        // docs/issues/archive/2026-08-23-research-index-tracker-has-no-augmentation.md
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        let cat = Catalog::open_in_memory().unwrap();
        let mut spec = sample_row("aug-spec", "Augmented spec");
        spec.kind = "spec".to_string();
        artifact::upsert(&cat, &spec).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "aug-spec".to_string(),
                prompt: "p".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);

        // Augmented, but excluded by the kind filter — the "not where you looked" case.
        let result = call(&ctx, json!({"augmented": true, "kind": "tracker"}))
            .await
            .unwrap();

        assert_eq!(result["count"], 0, "no augmented *tracker* exists");
        let hint = &result["hints"]["augmented_present_but_out_of_scope"];
        assert_eq!(
            hint["augmented_in_catalog"], 1,
            "the zero must carry the catalog-wide augmentation count, which is what \
             separates 'excluded by this query' from 'destroyed': {result}"
        );
    }

    #[tokio::test]
    async fn result_rows_surface_entry_collection_for_augmented_artifacts() {
        // The bug this pins: nothing in a `find` result says whether a tracker takes
        // append_entry(entry_collection=...) (rows) or append_entry(anchor_heading=...)
        // (body sections), so a caller has to probe with a separate `get` first.
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("plain", "Plain")).unwrap();
        artifact::upsert(&cat, &sample_row("rows", "Row-collection tracker")).unwrap();
        artifact::upsert(&cat, &sample_row("prose", "Body-section tracker")).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "rows".to_string(),
                prompt: "p".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("findings".to_string()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "prose".to_string(),
                prompt: "p".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({})).await.unwrap();
        let items = result["items"].as_array().unwrap();
        let by_id = |id: &str| items.iter().find(|it| it["id"] == id).unwrap();

        assert_eq!(
            by_id("rows")["entry_collection"],
            json!("findings"),
            "{items:?}"
        );
        assert_eq!(
            by_id("prose")["entry_collection"],
            json!(null),
            "augmented with no collection must still report the key, as null: {items:?}"
        );
        assert!(
            by_id("plain").get("entry_collection").is_none(),
            "a non-augmented row must not carry the key at all: {items:?}"
        );
    }

    #[tokio::test]
    async fn augmented_false_returns_only_non_augmented() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("plain", "Plain")).unwrap();
        artifact::upsert(&cat, &sample_row("aug", "Augmented")).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "aug".to_string(),
                prompt: "p".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"augmented": false})).await.unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "plain");
    }

    #[tokio::test]
    async fn kind_shortcut_filters_by_kind() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = Catalog::open_in_memory().unwrap();
        fn row(id: &str, kind: &str) -> ArtifactRow {
            TestArtifactRowBuilder::new(id)
                .with_abs_path(format!("/test/code-explorer/{id}.md"))
                .with_kind(kind)
                .with_title(id)
                .build()
        }
        upsert(&cat, &row("spec-1", "spec")).unwrap();
        upsert(&cat, &row("plan-1", "plan")).unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"kind": "spec"})).await.unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "spec-1");
    }

    #[tokio::test]
    async fn kind_and_filter_combine_with_and() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = Catalog::open_in_memory().unwrap();
        fn row(id: &str, kind: &str, status: &str) -> ArtifactRow {
            TestArtifactRowBuilder::new(id)
                .with_abs_path(format!("/test/code-explorer/{id}.md"))
                .with_kind(kind)
                .with_status(status)
                .with_title(id)
                .build()
        }
        upsert(&cat, &row("spec-active", "spec", "active")).unwrap();
        upsert(&cat, &row("spec-draft", "spec", "draft")).unwrap();
        upsert(&cat, &row("plan-active", "plan", "active")).unwrap();
        let ctx = mk_ctx(cat);
        let result = call(
            &ctx,
            json!({
                "kind": "spec",
                "filter": {"status": {"eq": "active"}},
                "include_archived": true
            }),
        )
        .await
        .unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "spec-active");
    }

    #[tokio::test]
    async fn status_shortcut_filters_by_status() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = Catalog::open_in_memory().unwrap();
        fn row(id: &str, status: &str) -> ArtifactRow {
            TestArtifactRowBuilder::new(id)
                .with_abs_path(format!("/test/code-explorer/{id}.md"))
                .with_status(status)
                .with_title(id)
                .build()
        }
        upsert(&cat, &row("a", "active")).unwrap();
        upsert(&cat, &row("d", "draft")).unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"status": "active", "include_archived": true}))
            .await
            .unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "a");
    }

    #[tokio::test]
    async fn cold_call_returns_catalog_field() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        upsert(
            &cat,
            &ArtifactRow {
                id: "a1".into(),
                abs_path: std::path::PathBuf::from("/test/code-explorer/docs/a1.md"),
                kind: "tracker".into(),
                status: "draft".into(),
                title: None,
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "".into(),
                confidence: 1.0,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, serde_json::json!({})).await.unwrap();
        assert!(
            result["catalog"].is_object(),
            "cold call must include catalog field"
        );
        assert_eq!(result["catalog"]["total"], 1);
        assert_eq!(result["catalog"]["by_kind"]["tracker"], 1);
        assert_eq!(result["catalog"]["augmented"], 0);
    }

    #[tokio::test]
    async fn find_with_kind_filter_omits_catalog_field() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        upsert(
            &cat,
            &ArtifactRow {
                id: "a1".into(),
                abs_path: std::path::PathBuf::from("/test/code-explorer/docs/a1.md"),
                kind: "tracker".into(),
                status: "draft".into(),
                title: None,
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "".into(),
                confidence: 1.0,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, serde_json::json!({"kind": "tracker"}))
            .await
            .unwrap();
        assert!(
            result.get("catalog").is_none() || result["catalog"].is_null(),
            "filtered find must not include catalog field"
        );
    }
    #[tokio::test]
    async fn scope_all_blocked_without_umbrella() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "A")).unwrap();
        let ctx = mk_ctx(cat);
        let err = call(&ctx, json!({"scope": "all"})).await.unwrap_err();
        assert!(
            err.downcast_ref::<crate::librarian::tools::RecoverableError>()
                .is_some(),
            "scope=all without umbrella must be RecoverableError, got: {err}"
        );
        assert!(
            err.to_string().contains("umbrella"),
            "error must mention umbrella"
        );
    }

    #[tokio::test]
    async fn scope_all_allowed_with_umbrella() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "A")).unwrap();
        let ctx = TestToolContextBuilder::new(cat)
            .with_root(crate::librarian::workspace::Root {
                name: "code-explorer".into(),
                path: "/tmp/code-explorer".into(),
            })
            .with_umbrellas(vec![crate::librarian::workspace::Umbrella {
                name: "main".into(),
                members: vec![std::path::PathBuf::from("/test/code-explorer")],
            }])
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    main_root: None,
                    umbrella: Some("main".into()),
                },
            ))
            .build();
        let result = call(&ctx, json!({"scope": "all"})).await.unwrap();
        assert_eq!(result["count"].as_u64(), Some(1));
    }
}
