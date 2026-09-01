use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::librarian::indexer;

use super::ToolContext;

#[derive(Deserialize)]
struct Args {
    repo: Option<String>,
    /// When true, the upsert walk ignores cached file hashes and re-processes
    /// every file (re-classification + re-embedding). Plumbed through
    /// `index_repo_sync` as `force_rewalk` (task #31). Default false — files
    /// matching their stored hash are skipped via the early-return path.
    ///
    /// Historically this also issued a destructive pre-walk DELETE that
    /// cascade-removed augmentations on subsequent failure (bug-tracker #7).
    /// That DELETE was removed in commit `d482ca8a`; force is now a safe
    /// hash-cache-bypass with no destructive side-effect.
    force: Option<bool>,
    /// When true, queues every walked file for re-embedding even when its
    /// content hash is unchanged (`force_embed` in `index_repo_sync`). Default
    /// false. Use this after enabling embeddings for the first time, or after
    /// switching embedding models/backends, on a project that was already
    /// indexed — otherwise unchanged content is silently never (re-)embedded,
    /// since `content_unchanged` alone gates the embed queue. Independent of
    /// `force`: `force` alone bypasses the unchanged-ROW skip (metadata is
    /// still re-derived) but does not by itself force re-embedding.
    reembed: Option<bool>,
    /// Scope of the reindex. Defaults to `project` when a current project is
    /// resolved, else `all`. Mirrors the read-tool scope semantics.
    scope: Option<super::scope::Scope>,
}

/// Re-attach augmentation shape from committed sidecars, for artifacts that declare one
/// and currently have no augmentation row.
///
/// This is the half of the cross-machine gap that a reindex can actually close. Bodies,
/// frontmatter and catalog rows already rebuild from disk here; augmentation did not,
/// because it had no on-disk form — so a fresh clone came up with the artifacts present and
/// their `append_entry` / `entry_filter` workflows broken, and `reindex` reported healthy.
///
/// **Attach-only-when-absent, never overwrite.** A live augmentation's `params` are this
/// machine's state and move on independently of the committed shape, so syncing in either
/// direction would eventually clobber real work. Repair is the whole contract: if a row
/// exists, this function does nothing to it.
///
/// `params` are not carried (see `augmentation_sidecar`), so a restored tracker comes back
/// working and empty rather than holding another machine's rows.
fn restore_declared_augmentations(
    catalog: &crate::librarian::catalog::Catalog,
    targets: &[std::path::PathBuf],
) -> (usize, Vec<String>) {
    use crate::librarian::augmentation_sidecar as sidecar;
    use crate::librarian::tools::doctor::{parse_declaration, Declaration};

    let mut restored = 0usize;
    let mut errors: Vec<String> = Vec::new();

    // Same LEFT JOIN as the doctor check, and for the same reason: an artifact that already
    // has a row is never opened from disk.
    let Ok(mut stmt) = catalog.conn.prepare(
        "SELECT a.id, a.abs_path FROM artifact a \
         LEFT JOIN artifact_augmentation g ON g.artifact_id = a.id \
         WHERE g.artifact_id IS NULL AND a.missing_since IS NULL \
         ORDER BY a.abs_path",
    ) else {
        return (
            0,
            vec!["could not prepare augmentation-restore query".to_string()],
        );
    };
    let Ok(rows) = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .and_then(|m| m.collect::<rusqlite::Result<Vec<(String, String)>>>())
    else {
        return (
            0,
            vec!["could not read augmentation-restore candidates".to_string()],
        );
    };

    for (id, abs_path) in rows {
        let path = std::path::Path::new(&abs_path);
        // Component-boundary containment, not a prefix match — `/repo-backup` must not
        // pass as `/repo`. Reindex is scoped, and so is its repair.
        if !targets.iter().any(|t| path.starts_with(t)) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok((Some(fm), _)) = crate::librarian::frontmatter::parse(&content) else {
            continue;
        };
        let Some(raw) = fm.extra.get("expects_augmentation") else {
            continue;
        };
        let Declaration::Declared { sidecar: Some(rel) } = parse_declaration(raw) else {
            // `true` with no sidecar, absent, or unparseable: nothing on disk to attach.
            // The doctor check reports those; this pass has no opinion.
            continue;
        };
        let Some(root) = crate::librarian::current_project::lookup_git_root(path) else {
            continue;
        };
        let sidecar_path = root.join(&rel);
        match sidecar::read(&sidecar_path) {
            Ok(s) => {
                let row = s.to_row(&id);
                match crate::librarian::catalog::augmentation::upsert(catalog, &row) {
                    Ok(()) => restored += 1,
                    Err(e) => errors.push(format!("{abs_path}: attach failed: {e}")),
                }
            }
            // A declared-but-unreadable sidecar is surfaced, never swallowed: it is the
            // one case where the shape was supposed to travel and did not.
            Err(e) => errors.push(format!("{abs_path}: {rel}: {e:#}")),
        }
    }
    (restored, errors)
}

fn backfill_commits(
    catalog: &crate::librarian::catalog::Catalog,
    repo_path: &std::path::Path,
) -> anyhow::Result<()> {
    use git2::{Repository, Sort};

    let repo = match Repository::open(repo_path) {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("skipping {}: not a git repo ({})", repo_path.display(), e);
            return Ok(());
        }
    };
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::TOPOLOGICAL | Sort::REVERSE)?;
    if let Err(e) = walk.push_head() {
        tracing::debug!(
            "revwalk push_head failed for {}: {}",
            repo_path.display(),
            e
        );
        return Ok(());
    }

    let git_root_str = crate::util::fs::RepoPath::from(repo_path).into_string();
    let rows: anyhow::Result<Vec<_>> = walk
        .enumerate()
        .map(|(order, oid_result)| {
            let oid = oid_result?;
            let commit = repo.find_commit(oid)?;
            Ok(crate::librarian::catalog::commits::CommitRow {
                hash: oid.to_string(),
                git_root: git_root_str.clone(),
                authored_at: Some(commit.time().seconds() * 1000),
                subject: commit.summary().map(String::from),
                topo_order: Some(order as i64),
            })
        })
        .collect();
    crate::librarian::catalog::commits::upsert_many(catalog, &rows?)?;
    Ok(())
}
// Ruling 1/2 (Task 4 brief): `ctx.project_root()` does not exist — `project_root()`
// lives on the *agent* and is async. Duplicated from gather.rs:294 rather than made
// `pub` mid-run. A shard is a per-REPO committed artifact, so export runs once
// against the current project's root, never once per reindex target.
//
// Task 6: `abs_path` is a sub-project inside a repo (e.g. a monorepo member),
// never the export destination — `.codescout/audit/` belongs at the repo
// root so every sub-project's shard lands in the one place `read_shards`
// looks. Use `main_root` (linked worktree → its main checkout) or else
// `git_root`, and drop the `ctx.workspace.roots.first()` fallback entirely:
// guessing an unrelated configured root is worse than skipping export this
// call, which is what `None` now means to every caller.
fn project_root(ctx: &ToolContext) -> Option<std::path::PathBuf> {
    ctx.current_project
        .as_ref()
        .map(|cp| cp.main_root.clone().unwrap_or_else(|| cp.git_root.clone()))
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    use super::scope::Scope;

    let a: Args = serde_json::from_value(args)?;

    let effective_scope = a.scope.unwrap_or_else(|| {
        if ctx.current_project.is_some() {
            Scope::Project
        } else {
            Scope::All
        }
    });

    // targets: abs_root paths to walk.
    let targets: Vec<std::path::PathBuf> = match effective_scope {
        Scope::All => {
            if let Some(ref repo_name) = a.repo {
                let root = ctx
                    .workspace
                    .roots
                    .iter()
                    .find(|r| &r.name == repo_name)
                    .ok_or_else(|| anyhow::anyhow!("unknown repo `{}`", repo_name))?;
                vec![root.path.clone()]
            } else {
                ctx.workspace.roots.iter().map(|r| r.path.clone()).collect()
            }
        }
        Scope::Repo => {
            let cp = ctx.current_project.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=repo requires a resolved current project; cwd is outside all \
                     workspace roots. Pass scope=\"all\" to reindex everything."
                )
            })?;
            vec![cp.git_root.clone()]
        }
        Scope::Project => {
            let cp = ctx.current_project.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=project requires a resolved current project; cwd is outside all \
                     workspace roots. Pass scope=\"all\" to reindex everything."
                )
            })?;
            vec![cp.abs_path.clone()]
        }
        Scope::Umbrella => {
            let cp = ctx.current_project.as_deref().ok_or_else(|| {
                anyhow::anyhow!("scope=umbrella requires a resolved current project")
            })?;
            let umbrella_name = cp.umbrella.as_deref().ok_or_else(|| {
                anyhow::anyhow!(
                    "scope=umbrella but no umbrella declared for {}",
                    cp.abs_path.display(),
                )
            })?;
            let umbrella = ctx
                .workspace
                .umbrellas
                .iter()
                .find(|u| u.name == umbrella_name)
                .ok_or_else(|| {
                    anyhow::anyhow!("umbrella `{umbrella_name}` not found in workspace config")
                })?;
            umbrella.members.clone()
        }
    };

    // Prevention: refuse indexing a temp-dir root into the real shared catalog.
    {
        let cat = ctx.catalog.lock();
        for target in &targets {
            super::temp_write_guard::guard_temp_workspace_write(
                target,
                &cat.conn,
                &ctx.temp_guard,
            )?;
        }
    }

    // NOTE: previously, `force=true` issued
    // `DELETE FROM artifact WHERE abs_path LIKE <root>/%` here, *before* the
    // re-walk. That was destructive: `artifact_augmentation` is declared
    // `ON DELETE CASCADE` (catalog/schema.sql), so the DELETE cascade-wiped
    // augmentations. When the subsequent embedding INSERT failed (e.g.
    // dim mismatch — bug-tracker #6), the DELETE was already committed.
    // Removed 2026-05-17 per bug-tracker #7 (F-9 in
    // docs/trackers/archive/artifact-code-linkage-session-log.md). `force=true`
    // now means "ignore cached file hashes during the upsert walk"; the
    // walk's own deletion logic still removes rows for files no longer
    // on disk (the `removed` count in the response).

    let mut orphan_removed = 0usize;
    if effective_scope == Scope::All && a.repo.is_none() {
        let cat = ctx.catalog.lock();
        let active: Vec<&std::path::Path> = ctx
            .workspace
            .roots
            .iter()
            .map(|r| r.path.as_path())
            .collect();
        // Bound the orphan sweep to THIS workspace's own roots (scope == the
        // walked roots): the catalog is a single machine-global DB, so an
        // unbounded "delete rows not under the active roots" would wipe other
        // workspaces' rows (3ea49090). Within-workspace file deletions are
        // already handled by the per-file walk above; pruning a de-registered
        // root or a renamed repo is the job of an explicit scoped prune
        // (7ca71bf7), not this reindex side-effect.
        orphan_removed =
            crate::librarian::catalog::artifact::delete_orphan_repos(&cat, &active, &active)?;
    }

    let ignore = crate::librarian::workspace::compile_ignore(&ctx.workspace.ignore)?;

    // Workspace root paths for stable, batch-uniform project_id derivation at
    // index time (see artifact_store docs).
    let root_paths: Vec<std::path::PathBuf> =
        ctx.workspace.roots.iter().map(|r| r.path.clone()).collect();

    let mut total_added = 0usize;
    let mut total_updated = 0usize;
    let mut total_removed = 0usize;
    let mut total_unchanged = 0usize;
    let mut all_unknown_ids: Vec<String> = Vec::new();
    let mut backfill_errors: Vec<String> = Vec::new();
    // Reported in the response envelope. Without it, `unchanged: N` renders
    // identically whether N files legitimately needed no work or N files were
    // skipped by mistake — which is exactly how the `reembed` no-op stayed
    // invisible (docs/issues/archive/2026-07-25-reindex-reembed-noop-without-force.md).
    let mut total_embedded = 0usize;
    // Embed failures are COLLECTED, not propagated. A bare `?` on the embed call
    // below escaped the `for abs_root in &targets` loop, so one transport error
    // meant every later target was never walked at all, the succeeded catalog
    // counters were discarded, and `backfill_commits` was skipped — while the
    // catalog writes for earlier targets had already committed. The caller then saw
    // an error and could not tell which half had happened.
    // docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md
    //
    // Same shape as `backfill_errors` above, deliberately: that field exists
    // because F-5 swallowed a failure silently, and the remedy — keep going, report
    // what broke — is identical here.
    let mut embed_errors: Vec<String> = Vec::new();

    let want_embeddings = ctx.embedding.is_some();

    for abs_root in &targets {
        let (report, embed_queue) = {
            let cat = ctx.catalog.lock();
            indexer::index_repo_sync(
                &cat,
                &ctx.rules,
                abs_root,
                &ignore,
                want_embeddings,
                a.force.unwrap_or(false),
                a.reembed.unwrap_or(false),
            )?
        };

        total_added += report.added;
        total_updated += report.updated;
        total_removed += report.removed;
        total_unchanged += report.unchanged;
        all_unknown_ids.extend(report.unknown_ids);

        if let (Some(svc), Some(store)) = (ctx.embedding.as_ref(), ctx.artifact_store.as_ref()) {
            // project_id = the workspace root containing this target — stable
            // and batch-uniform (every file under the target shares it). Empty
            // when outside every registered root → unscoped KNN (the catalog
            // scoped filter still narrows results).
            let project_id = crate::librarian::tools::containing_root(&root_paths, abs_root)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            for (id, title, chunk_text) in &embed_queue {
                match svc.embed_artifact(title.as_deref(), chunk_text).await {
                    Ok(vec) => match store.upsert(&project_id, id, &vec).await {
                        Ok(()) => total_embedded += 1,
                        Err(e) => embed_errors.push(format!("{id}: upsert failed: {e}")),
                    },
                    Err(e) => embed_errors.push(format!("{id}: embed failed: {e}")),
                }
            }
        }

        {
            let cat = ctx.catalog.lock();
            // Derive a git_root for git backfill from the abs_root path.
            if let Err(e) = backfill_commits(&cat, abs_root) {
                // F-5 fix: surface the failure instead of swallowing it. The
                // backfill populates the `commits` table that `state_at(commit=)`
                // depends on; silent failure produced the "commit not indexed"
                // error that misleads callers into running reindex over and over.
                let msg = format!("{}: {}", abs_root.display(), e);
                tracing::warn!("backfill_commits failed for {}", msg);
                backfill_errors.push(msg);
            }
        }
    }

    // Re-attach augmentation shape from committed sidecars. Runs after the walk, because
    // an artifact indexed for the first time in THIS call must be eligible — the fresh-clone
    // case is exactly one where the row and its restore both land in the same reindex.
    let (augmentations_restored, augmentation_restore_errors) = {
        let cat = ctx.catalog.lock();
        restore_declared_augmentations(&cat, &targets)
    };

    // Persist the durable half of the degraded signal. `embed_note` above is an
    // envelope field — gone the moment this call returns — so a later
    // `artifact(action="find")` has no way to know the last refresh was partial.
    // Written unconditionally whenever embeddings were attempted this run
    // (including a clean 0/[] run, which is what clears a stale marker left by
    // an earlier failure) — never when `want_embeddings` is false, since a run
    // with no embedder configured has no evidence about embed health either way.
    // docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md
    if want_embeddings {
        let cat = ctx.catalog.lock();
        let embed_error_sample: Vec<&String> = embed_errors.iter().take(20).collect();
        crate::librarian::catalog::gc::set_meta(
            &cat.conn,
            "last_reindex_embed_error_count",
            &embed_errors.len().to_string(),
        )?;
        crate::librarian::catalog::gc::set_meta(
            &cat.conn,
            "last_reindex_embed_errors_sample",
            &serde_json::to_string(&embed_error_sample)?,
        )?;
    }

    let unknown_count = all_unknown_ids.len();
    const UNKNOWN_SAMPLE: usize = 20;
    let sample: Vec<&String> = all_unknown_ids.iter().take(UNKNOWN_SAMPLE).collect();

    // Fold-in, best effort: an export failure must never fail a reindex. The
    // envelope reports it so a silently-never-exporting machine is visible
    // (a committed replica that quietly stops updating is the IC-13 this
    // whole phase exists to avoid). Ruling 2: export runs once against the
    // CURRENT PROJECT's root, never once per reindex target — a shard is a
    // per-repo committed artifact, and `targets` may span several repos.
    let audit_export = match project_root(ctx) {
        Some(root) => {
            match crate::librarian::catalog::audit::shard::export(&ctx.catalog.lock().conn, &root) {
                Ok(r) => json!({"exported": r.exported, "through_seq": r.through_seq}),
                Err(e) => {
                    tracing::warn!("audit shard export failed: {e}");
                    json!({"error": e.to_string()})
                }
            }
        }
        None => json!({"skipped": "no current project"}),
    };

    Ok(json!({
        "added": total_added,
        "updated": total_updated,
        "removed": total_removed,
        "unchanged": total_unchanged,
        "embedded": total_embedded,
        "embeddings_enabled": want_embeddings,
        "orphans_removed": orphan_removed,
        // Reported even when zero, and distinguishable from "nothing needed restoring" by
        // the note below. The bug this closes was invisible precisely because a reindex
        // that repaired nothing looked identical to one with nothing to repair.
        "augmentations_restored": augmentations_restored,
        "augmentation_restore_error_count": augmentation_restore_errors.len(),
        "augmentation_restore_errors": augmentation_restore_errors.iter().take(20).collect::<Vec<_>>(),
        "unknown_count": unknown_count,
        "unknown_sample": sample,
        "backfill_error_count": backfill_errors.len(),
        "backfill_errors": backfill_errors,
        "embed_error_count": embed_errors.len(),
        // Capped at the same 20 as `unknown_sample`, and for the same reason: one
        // entry per queued artifact could be thousands. The COUNT is exact.
        "embed_errors": embed_errors.iter().take(20).collect::<Vec<_>>(),
        // Name the ambiguous case out loud rather than leaving the caller to
        // infer it from a bare `unchanged: N`.
        // A partial embed is the case worth naming first: the catalog IS refreshed
        // (classification runs before embedding and commits independently), so
        // artifact discovery works — but the un-vectored artifacts are invisible to
        // semantic search, and nothing marks them for retry.
        "embed_note": if !embed_errors.is_empty() {
            format!(
                "DEGRADED: {total_embedded} embedded, {} failed. The catalog is \
                 refreshed and `artifact(action=\"find\")` is accurate, but the failed \
                 artifacts have no vector, so semantic search will not surface them. \
                 Re-run reindex once the embedder is healthy.",
                embed_errors.len()
            )
        } else if want_embeddings && total_embedded == 0 && total_unchanged > 0 {
            format!(
                "0 embedded, {total_unchanged} unchanged — nothing needed a new vector. \
                 If you meant to backfill (embeddings newly enabled, or model/backend \
                 changed), pass reembed=true."
            )
        } else {
            format!("{total_embedded} embedded")
        },
        "unknown_sample_note": if unknown_count > UNKNOWN_SAMPLE {
            format!("showing first {UNKNOWN_SAMPLE} of {unknown_count}; run CLI reindex for full list")
        } else {
            "complete".to_string()
        },
        "audit_export": audit_export,
        "scope": match effective_scope {
            Scope::Project => "project",
            Scope::Repo => "repo",
            Scope::Umbrella => "umbrella",
            Scope::All => "all",
        },
        "targets": targets.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::classify::load_rules;
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::Root;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp_root: std::path::PathBuf, rules_toml: &str) -> ToolContext {
        let rules = load_rules(rules_toml).unwrap();
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: tmp_root,
            })
            .with_rules(rules)
            .build()
    }

    /// The response envelope must be able to express "embedded nothing".
    ///
    /// Before these fields existed, `unchanged: N` rendered identically whether
    /// N files legitimately needed no work or N files were skipped by mistake —
    /// which is how the `reembed` no-op stayed invisible through two apparently
    /// clean reindexes (docs/issues/archive/2026-07-25-reindex-reembed-noop-without-force.md).
    ///
    /// Covers the no-embedder branch only. Asserting a NON-zero `embedded`
    /// needs a mock `EmbeddingService` + artifact store, and
    /// `TestToolContextBuilder` has no `with_embedding` setter today; the
    /// populated path is covered at the layer below by
    /// `indexer::tests::index_repo_sync_force_embed_alone_requeues_without_force_rewalk`,
    /// which asserts on the embed QUEUE rather than the envelope.
    #[tokio::test]
    async fn envelope_reports_embedding_state() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(root.join("docs/specs/a.md"), "# a\nbody\n").unwrap();

        let ctx = mk_ctx(
            root.to_path_buf(),
            "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n",
        );

        let v = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v["added"].as_u64().unwrap(), 1);
        assert!(
            !v["embeddings_enabled"].as_bool().unwrap(),
            "no embedder configured in the test context"
        );
        assert_eq!(v["embedded"].as_u64().unwrap(), 0);
        assert_eq!(
            v["embed_note"].as_str().unwrap(),
            "0 embedded",
            "with embeddings disabled the note must not suggest reembed=true — \
             passing it would change nothing"
        );

        // Second pass: nothing changed, so the row is `unchanged`. The note must
        // still not nag about reembed, because no embedder is configured.
        let v2 = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v2["unchanged"].as_u64().unwrap(), 1);
        assert_eq!(v2["embedded"].as_u64().unwrap(), 0);
        assert_eq!(v2["embed_note"].as_str().unwrap(), "0 embedded");
    }

    /// A failing embedder must not stop the walk — the loop-abort fix.
    ///
    /// TWO roots, so the default `scope="all"` yields two targets. Before the fix the
    /// bare `?` on the embed call escaped `for abs_root in &targets`, so the second
    /// target was never walked at all: its artifacts simply absent from the catalog,
    /// while the caller held a transport error with no way to tell which half had
    /// run. The reported issue diagnosed this as "fails before refreshing the
    /// catalog", which is the opposite of true — `index_repo_sync` commits first —
    /// so the assertions below pin the real contract: catalog work completes for
    /// EVERY target, and the embedding failure is reported rather than fatal.
    ///
    /// `docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md`
    #[tokio::test]
    async fn an_embed_failure_still_walks_every_target_and_reports_it() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;

        struct FailingEmbedder;
        #[async_trait::async_trait]
        impl codescout_embed::Embedder for FailingEmbedder {
            fn dimensions(&self) -> usize {
                4
            }
            async fn embed(&self, _texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
                anyhow::bail!("connection refused")
            }
        }

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("one/docs/specs")).unwrap();
        std::fs::create_dir_all(root.join("two/docs/specs")).unwrap();
        std::fs::write(root.join("one/docs/specs/a.md"), "# A\nbody\n").unwrap();
        std::fs::write(root.join("two/docs/specs/b.md"), "# B\nbody\n").unwrap();

        let rules =
            load_rules("[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n").unwrap();
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "one".into(),
                path: root.join("one"),
            })
            .with_root(Root {
                name: "two".into(),
                path: root.join("two"),
            })
            .with_rules(rules)
            .with_embedding(std::sync::Arc::new(
                crate::librarian::embedding::EmbeddingService::new(std::sync::Arc::new(
                    FailingEmbedder,
                )),
            ))
            .with_artifact_store(std::sync::Arc::new(InMemoryArtifactStore::default()))
            .build();

        let v = call(&ctx, json!({})).await.expect(
            "an embedder outage must not fail the whole reindex — the catalog half \
             succeeds and must be reported",
        );

        assert_eq!(
            v["targets"].as_array().unwrap().len(),
            2,
            "test setup: both roots must resolve as targets, or this proves nothing \
             about the loop the `?` used to escape"
        );
        assert_eq!(
            v["added"].as_u64().unwrap(),
            2,
            "BOTH targets' artifacts must be classified into the catalog — the second \
             is the one the abort used to skip entirely"
        );
        assert_eq!(
            v["embedded"].as_u64().unwrap(),
            0,
            "nothing embeds against a dead embedder"
        );
        assert_eq!(
            v["embed_error_count"].as_u64().unwrap(),
            2,
            "every failure is counted, not just the first"
        );
        let note = v["embed_note"].as_str().unwrap();
        assert!(
            note.contains("DEGRADED"),
            "a partial embed must announce itself, not read as an ordinary success: {note}"
        );
        assert_eq!(
            v["backfill_error_count"].as_u64().unwrap(),
            0,
            "backfill_commits sits AFTER the embed block and the `?` used to skip it, \
             so it must now run for every target"
        );
    }

    /// Step 2 of docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md:
    /// the envelope's `embed_error_count` does not outlive the call — a later
    /// `artifact(action="find")` has no way to know the last refresh was partial.
    /// This pins the durable half: a failed embed run must persist a marker in
    /// `catalog_meta`, the same key-value table `gc.rs` already uses for
    /// `gc_grace_days`.
    #[tokio::test]
    async fn an_embed_failure_persists_a_durable_catalog_meta_marker() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::catalog::gc;

        struct FailingEmbedder;
        #[async_trait::async_trait]
        impl codescout_embed::Embedder for FailingEmbedder {
            fn dimensions(&self) -> usize {
                4
            }
            async fn embed(&self, _texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
                anyhow::bail!("connection refused")
            }
        }

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(root.join("docs/specs/a.md"), "# A\nbody\n").unwrap();
        std::fs::write(root.join("docs/specs/b.md"), "# B\nbody\n").unwrap();

        let rules =
            load_rules("[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n").unwrap();
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: root.to_path_buf(),
            })
            .with_rules(rules)
            .with_embedding(std::sync::Arc::new(
                crate::librarian::embedding::EmbeddingService::new(std::sync::Arc::new(
                    FailingEmbedder,
                )),
            ))
            .with_artifact_store(std::sync::Arc::new(InMemoryArtifactStore::default()))
            .build();

        let v = call(&ctx, json!({})).await.unwrap();
        assert_eq!(
            v["embed_error_count"].as_u64().unwrap(),
            2,
            "test setup sanity"
        );

        let cat = ctx.catalog.lock();
        let count = gc::get_meta(&cat.conn, "last_reindex_embed_error_count").unwrap();
        assert_eq!(
            count.as_deref(),
            Some("2"),
            "the embed failure count must survive past the call, in catalog_meta"
        );
        let sample = gc::get_meta(&cat.conn, "last_reindex_embed_errors_sample")
            .unwrap()
            .expect("sample marker must be written alongside the count");
        let parsed: Vec<String> = serde_json::from_str(&sample).unwrap();
        assert_eq!(
            parsed.len(),
            2,
            "both failures' messages must be sampled: {parsed:?}"
        );
    }

    /// A fixed embedder must clear the marker, not just stop adding to it — a
    /// stuck-true degraded flag would misreport a healthy catalog forever, same
    /// invariant as `sync_project_clears_a_previously_recorded_skip_count_on_a_clean_run`
    /// in `src/retrieval/sync.rs` for the sibling code-index bug.
    #[tokio::test]
    async fn a_clean_reindex_after_a_failure_clears_the_persisted_marker() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::catalog::gc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        struct FlakyEmbedder(Arc<AtomicBool>);
        #[async_trait::async_trait]
        impl codescout_embed::Embedder for FlakyEmbedder {
            fn dimensions(&self) -> usize {
                4
            }
            async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Vec<f32>>> {
                if self.0.load(Ordering::SeqCst) {
                    anyhow::bail!("connection refused")
                } else {
                    Ok(texts.iter().map(|_| vec![0.0f32; 4]).collect())
                }
            }
        }

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(root.join("docs/specs/a.md"), "# A\nbody\n").unwrap();

        let rules =
            load_rules("[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n").unwrap();
        let should_fail = Arc::new(AtomicBool::new(true));
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: root.to_path_buf(),
            })
            .with_rules(rules)
            .with_embedding(std::sync::Arc::new(
                crate::librarian::embedding::EmbeddingService::new(std::sync::Arc::new(
                    FlakyEmbedder(should_fail.clone()),
                )),
            ))
            .with_artifact_store(std::sync::Arc::new(InMemoryArtifactStore::default()))
            .build();

        call(&ctx, json!({})).await.unwrap();
        {
            let cat = ctx.catalog.lock();
            assert_eq!(
                gc::get_meta(&cat.conn, "last_reindex_embed_error_count")
                    .unwrap()
                    .as_deref(),
                Some("1"),
                "marker must be set after the failing run"
            );
        }

        should_fail.store(false, Ordering::SeqCst);
        call(&ctx, json!({})).await.unwrap();

        let cat = ctx.catalog.lock();
        assert_eq!(
            gc::get_meta(&cat.conn, "last_reindex_embed_error_count")
                .unwrap()
                .as_deref(),
            Some("0"),
            "a clean run must reset the marker, not just leave it stuck at the last failure"
        );
    }

    #[tokio::test]
    async fn indexes_two_files_one_unknown() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create 2 fixture .md files
        std::fs::create_dir_all(root.join("docs/specs")).unwrap();
        std::fs::write(
            root.join("docs/specs/auth.md"),
            "---\ntitle: Auth Spec\n---\nbody\n",
        )
        .unwrap();
        std::fs::write(root.join("README.md"), "# README\n").unwrap();

        // Rules match only docs/specs/*.md as "spec"; README.md stays "unknown"
        let rules_toml = "[[rule]]\nglob = \"**/docs/specs/*.md\"\nkind = \"spec\"\n";
        let ctx = mk_ctx(root.to_path_buf(), rules_toml);

        let v = call(&ctx, json!({})).await.unwrap();

        assert_eq!(
            v["added"].as_u64().unwrap(),
            2,
            "should index both .md files"
        );
        assert_eq!(
            v["unknown_count"].as_u64().unwrap(),
            1,
            "README.md should be unknown"
        );
    }

    /// The seeded tracker's path. Spelling is free here — the LOOKUPS normalise.
    ///
    /// The catalog stores `abs_path` in forward-slash form on every platform:
    /// `RepoPath` (src/util/fs.rs) is the write/storage type and guarantees no backslash
    /// byte, which is the invariant `doctor`'s `backslash_in_abs_path` check enforces. So
    /// a lookup must normalise the same way the writer did, and `aug_for` now does.
    ///
    /// Recorded because the obvious fix is the wrong one, and was tried: spelling this
    /// path with NATIVE separators (`join("docs").join("trackers")`) makes the Windows
    /// lookup ask for `C:\...\docs\trackers\t.md` against a stored
    /// `C:/.../docs/trackers/t.md` — further from matching, not closer. Native joins are
    /// right for touching the filesystem and wrong for addressing the catalog.
    fn tracker_path(root: &std::path::Path) -> std::path::PathBuf {
        root.join("docs/trackers/t.md")
    }

    /// Build a repo whose single tracker declares a sidecar, and write that sidecar.
    /// `.git` matters: the declared path is repo-relative and `lookup_git_root` is what
    /// resolves it, so without the marker the restore silently finds nothing.
    fn seed_sidecar_repo(root: &std::path::Path) {
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("docs/trackers")).unwrap();
        std::fs::write(
            tracker_path(root),
            "---\nkind: tracker\nstatus: active\n\
             expects_augmentation: docs/augmentations/t.yaml\n---\n\n# t\n",
        )
        .unwrap();
        crate::librarian::augmentation_sidecar::write(
            &root.join("docs/augmentations/t.yaml"),
            &crate::librarian::augmentation_sidecar::AugmentationSidecar {
                schema_version: 1,
                prompt: "committed prompt".into(),
                entry_collection: Some("rows".into()),
                params_schema: None,
                render_template: None,
                append_mode: false,
                history_cap: None,
            },
        )
        .unwrap();
    }

    /// Look an augmentation up by path, normalising the way the WRITER does.
    ///
    /// `abs_path` is stored via `RepoPath`, i.e. forward-slash form on every platform, and
    /// this compares it as an exact string. Passing `to_string_lossy()` straight through
    /// therefore matched only where the native separator already IS `/` — green on Linux
    /// and macOS, and on Windows it asked for `C:\...\docs\trackers\t.md` against a stored
    /// `C:/.../docs/trackers/t.md`, so the row read as absent: `row must exist`, on three
    /// tests, on all three Windows lanes and wine (CI runs 33433055755 and 33435797552).
    fn aug_for(
        cat: &Catalog,
        abs: &std::path::Path,
    ) -> Option<crate::librarian::catalog::augmentation::AugmentationRow> {
        let id: String = cat
            .conn
            .query_row(
                "SELECT id FROM artifact WHERE abs_path = ?1",
                [crate::util::fs::RepoPath::from(abs).as_str().to_string()],
                |r| r.get(0),
            )
            .ok()?;
        crate::librarian::catalog::augmentation::get(cat, &id)
            .ok()
            .flatten()
    }

    const TRACKER_RULES: &str = "[[rule]]\nglob = \"**/docs/trackers/*.md\"\nkind = \"tracker\"\n";

    #[tokio::test]
    async fn reindex_reattaches_a_declared_sidecar_when_the_row_is_absent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        seed_sidecar_repo(root);
        let ctx = mk_ctx(root.to_path_buf(), TRACKER_RULES);

        let v = call(&ctx, json!({})).await.unwrap();
        assert_eq!(
            v["augmentations_restored"].as_u64().unwrap(),
            1,
            "the declared sidecar must be attached in the same run that indexed the \
             artifact — the fresh-clone case is exactly that one: {v}"
        );

        let cat = ctx.catalog.lock();
        let row = aug_for(&cat, &tracker_path(root)).expect("row must exist");
        assert_eq!(row.prompt, "committed prompt");
        assert_eq!(row.entry_collection.as_deref(), Some("rows"));
        assert_eq!(
            row.params, "{}",
            "params are data and must NOT travel — a restored tracker comes back working \
             and empty, never holding another machine's rows"
        );
    }

    /// The one that matters. `params` are live state that moves on independently of the
    /// committed shape, so a restore that behaved like a sync would silently destroy real
    /// work every time someone reindexed a machine whose tracker had advanced.
    ///
    /// Written as mutate-then-reindex rather than two fixtures, because the dangerous
    /// version of this code is indistinguishable from the correct one until a row that
    /// ALREADY EXISTS meets a sidecar that disagrees with it.
    #[tokio::test]
    async fn reindex_never_overwrites_a_live_augmentation() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        seed_sidecar_repo(root);
        let ctx = mk_ctx(root.to_path_buf(), TRACKER_RULES);
        let art = tracker_path(root);

        call(&ctx, json!({})).await.unwrap();

        // Simulate the tracker being used: params fill up, and the prompt is edited live.
        {
            let cat = ctx.catalog.lock();
            let mut row = aug_for(&cat, &art).unwrap();
            row.prompt = "locally edited prompt".into();
            row.params = r#"{"rows":[{"id":"R-1"}]}"#.into();
            crate::librarian::catalog::augmentation::upsert(&cat, &row).unwrap();
        }

        let v = call(&ctx, json!({})).await.unwrap();
        assert_eq!(
            v["augmentations_restored"].as_u64().unwrap(),
            0,
            "a row that already exists is not a restore candidate: {v}"
        );

        let cat = ctx.catalog.lock();
        let row = aug_for(&cat, &art).unwrap();
        assert_eq!(
            row.prompt, "locally edited prompt",
            "the committed sidecar must not clobber a live prompt"
        );
        assert_eq!(
            row.params, r#"{"rows":[{"id":"R-1"}]}"#,
            "and it must not clobber live params — this is the data-loss case"
        );
    }

    /// The bug, end to end, in one test: export on the machine that HAS the augmentation,
    /// then come up augmented on a catalog that never held it.
    ///
    /// `mk_ctx` builds a fresh in-memory catalog per call, so the second context on the same
    /// root is precisely the fresh-clone case — same files, no rows. Before this change that
    /// second catalog came up with the artifact present, its augmentation absent, and
    /// `reindex` reporting healthy; every documented `append_entry` / `entry_filter` call
    /// against it failed one caller at a time.
    ///
    /// What this adds over the unit tests, stated no more strongly than it can be shown:
    /// it exercises the COMPOSITION — export writes a sidecar, stamps a declaration,
    /// `parse_declaration` accepts that exact spelling, `lookup_git_root` resolves it, and
    /// the attach lands a usable row. Each link is pinned separately elsewhere; this is the
    /// only test that fails if two of them agree with their own unit test and not with each
    /// other. It is deliberately NOT claimed that some specific mutation escapes the unit
    /// suite and dies here — the path helpers are pinned together by
    /// `the_default_sidecar_name_is_injective_over_paths`, so the obvious candidate does
    /// not, and an unverified claim about one's own coverage is the failure this comment
    /// would otherwise be an example of.
    #[tokio::test]
    async fn a_catalog_that_never_held_the_augmentation_comes_up_augmented() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("docs/trackers")).unwrap();
        // No declaration and no sidecar — the state every augmented tracker was in.
        std::fs::write(
            tracker_path(root),
            "---\nkind: tracker\nstatus: active\n---\n\n# t\n",
        )
        .unwrap();

        // --- machine A: has the row, exports it -------------------------------------
        let a = mk_ctx(root.to_path_buf(), TRACKER_RULES);
        call(&a, json!({})).await.unwrap();
        {
            let cat = a.catalog.lock();
            let id: String = cat
                .conn
                .query_row(
                    "SELECT id FROM artifact WHERE abs_path = ?1",
                    [crate::util::fs::RepoPath::from(&tracker_path(root))
                        .as_str()
                        .to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            crate::librarian::catalog::augmentation::upsert(
                &cat,
                &crate::librarian::augmentation_sidecar::AugmentationSidecar {
                    schema_version: 1,
                    prompt: "the only copy of this prompt".into(),
                    entry_collection: Some("rows".into()),
                    params_schema: None,
                    render_template: Some("{{ rows | length }}".into()),
                    append_mode: true,
                    history_cap: Some(9),
                }
                .to_row(&id),
            )
            .unwrap();
        }
        let exported = crate::librarian::tools::doctor::call(
            &a,
            json!({
                "fix": "export_augmentations",
                "root": root.to_string_lossy(),
                "confirm": true,
            }),
        )
        .await
        .unwrap();
        assert_eq!(exported["totals"]["exported"], json!(1), "{exported:#?}");

        // --- machine B: never held the row ------------------------------------------
        let b = mk_ctx(root.to_path_buf(), TRACKER_RULES);
        let v = call(&b, json!({})).await.unwrap();
        assert_eq!(
            v["augmentations_restored"].as_u64().unwrap(),
            1,
            "a catalog that never held this augmentation must come up with it: {v}"
        );

        let cat = b.catalog.lock();
        let row = aug_for(&cat, &tracker_path(root))
            .expect("the augmentation must exist on the second catalog");
        assert_eq!(row.prompt, "the only copy of this prompt");
        assert_eq!(row.entry_collection.as_deref(), Some("rows"));
        assert_eq!(row.render_template.as_deref(), Some("{{ rows | length }}"));
        assert!(row.append_mode);
        assert_eq!(row.history_cap, Some(9));
        assert_eq!(
            row.params, "{}",
            "shape travels, data does not — the restored tracker is working and empty"
        );
    }

    #[tokio::test]
    async fn force_wipes_then_reindexes() {
        // History (kept verbatim — see bug-tracker #7 / F-9):
        //   pre-bug-tracker-#7 → force=true issued a destructive DELETE +
        //     re-INSERT; expected added=1, unchanged=0.
        //   commit d482ca8a → DELETE removed; force=true was a no-op
        //     pending proper plumbing.
        //   task #31 → force_rewalk plumbed through index_repo_sync;
        //     force=true now bypasses the hash-equal early-return, so the
        //     row is re-walked → upsert path → counts as updated (not added).
        //
        // Today's expectation: force=true on an existing-unchanged file →
        // updated=1, added=0, unchanged=0.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        std::fs::write(root.join("docs/a.md"), "# A\n").unwrap();

        let ctx = mk_ctx(
            root.to_path_buf(),
            "[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n",
        );

        // First index
        call(&ctx, json!({})).await.unwrap();

        // Second index without force → unchanged (hash matches)
        let v2 = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v2["unchanged"].as_u64().unwrap(), 1);
        assert_eq!(v2["added"].as_u64().unwrap(), 0);

        // Third index with force=true → re-walks regardless of hash,
        // re-runs the upsert → counts as updated (id pre-existed).
        let v3 = call(&ctx, json!({"force": true})).await.unwrap();
        assert_eq!(v3["updated"].as_u64().unwrap(), 1);
        assert_eq!(v3["added"].as_u64().unwrap(), 0);
        assert_eq!(v3["unchanged"].as_u64().unwrap(), 0);
    }

    fn mk_ctx_with_project(tmp_root: std::path::PathBuf, project_subdir: &str) -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: tmp_root.clone(),
            })
            .with_rules(
                crate::librarian::classify::load_rules(
                    "[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n",
                )
                .unwrap(),
            )
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: tmp_root.join(project_subdir),
                    git_root: tmp_root.clone(),
                    main_root: None,
                    umbrella: None,
                },
            ))
            .build()
    }

    #[tokio::test]
    async fn project_scope_walks_only_subdir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("p1/docs")).unwrap();
        std::fs::create_dir_all(root.join("p2/docs")).unwrap();
        std::fs::write(root.join("p1/docs/a.md"), "# A\n").unwrap();
        std::fs::write(root.join("p2/docs/b.md"), "# B\n").unwrap();

        let ctx = mk_ctx_with_project(root.to_path_buf(), "p1");

        let v = call(&ctx, json!({})).await.unwrap();

        assert_eq!(v["added"].as_u64().unwrap(), 1, "only p1/docs/a.md indexed");
        assert_eq!(v["scope"].as_str().unwrap(), "project");
        let targets = v["targets"].as_array().unwrap();
        assert_eq!(targets.len(), 1);
        let target = targets[0].as_str().unwrap();
        assert!(
            target.ends_with("p1"),
            "target should end with p1, got: {target}"
        );
    }

    // Task 6, required test 5/6: the reindex fold-in export must target the
    // current project's git_root (via `project_root`), never its `abs_path`
    // when the current project is a sub-project inside a larger repo.
    // Mutation this catches: reverting `project_root` to read `cp.abs_path`
    // (or to fall back to `ctx.workspace.roots.first()`) instead of
    // `cp.main_root.unwrap_or(cp.git_root)` — the shard directory would then
    // land at `root/p1/.codescout/audit` instead of `root/.codescout/audit`,
    // and a later reindex of a sibling sub-project would never find it.
    #[tokio::test]
    async fn audit_export_targets_the_repo_root_not_the_sub_projects_abs_path() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("p1/docs")).unwrap();
        std::fs::write(root.join("p1/docs/a.md"), "# A\n").unwrap();

        let ctx = mk_ctx_with_project(root.to_path_buf(), "p1");

        let v = call(&ctx, json!({})).await.unwrap();

        let exported = v["audit_export"]["exported"].as_u64().unwrap_or(0);
        assert!(exported >= 1, "{v}");
        assert!(
            root.join(".codescout").join("audit").is_dir(),
            "the shard must land at the git root"
        );
        assert!(
            !root.join("p1").join(".codescout").join("audit").exists(),
            "export must never use the sub-project's abs_path as its destination"
        );
    }

    #[tokio::test]
    async fn project_scope_force_does_not_nuke_sibling_rows() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("p1/docs")).unwrap();
        std::fs::create_dir_all(root.join("p2/docs")).unwrap();
        std::fs::write(root.join("p1/docs/a.md"), "# A\n").unwrap();
        std::fs::write(root.join("p2/docs/b.md"), "# B\n").unwrap();

        // First, index everything (scope=all)
        let ctx_all = mk_ctx(
            root.to_path_buf(),
            "[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n",
        );
        call(&ctx_all, json!({"scope": "all"})).await.unwrap();
        let total_before: i64 = ctx_all
            .catalog
            .lock()
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(total_before, 2);

        // Reuse same catalog for project-scoped force reindex from p1
        let ctx_p1 = ToolContext {
            lsp: crate::lsp::MockLspProvider::with_client(crate::lsp::MockLspClient::default()),
            catalog: ctx_all.catalog.clone(),
            workspace: ctx_all.workspace.clone(),
            rules: ctx_all.rules.clone(),
            temp_guard: ctx_all.temp_guard.clone(),
            embedding: None,
            artifact_store: None,
            current_project: Some(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: root.join("p1"),
                    git_root: root.to_path_buf(),
                    main_root: None,
                    umbrella: None,
                },
            )),
        };
        call(&ctx_p1, json!({"force": true})).await.unwrap();

        let total_after: i64 = ctx_p1
            .catalog
            .lock()
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            total_after, 2,
            "p2 row must survive a project-scoped force reindex of p1"
        );

        // Forward-slash LIKE pattern — catalog stores abs_paths in forward-slash
        // form (artifact::upsert via to_forward_slash); a native-separator pattern
        // would not match any rows on Windows.
        let p2_pattern = format!("%{}/p2/%", crate::util::fs::RepoPath::from(root));
        let p2_count: i64 = ctx_p1
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact WHERE abs_path LIKE ?1",
                rusqlite::params![p2_pattern],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(p2_count, 1);
    }

    #[tokio::test]
    async fn project_scope_errors_without_current_project() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf(), "");
        let err = call(&ctx, json!({"scope": "project"})).await.unwrap_err();
        assert!(err.to_string().contains("scope=project"));
    }

    #[tokio::test]
    async fn defaults_to_all_when_no_current_project() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::write(root.join("a.md"), "# A\n").unwrap();
        let ctx = mk_ctx(root.to_path_buf(), "");
        let v = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v["scope"].as_str().unwrap(), "all");
        assert_eq!(v["added"].as_u64().unwrap(), 1);
    }

    #[tokio::test]
    async fn reindex_refuses_temp_root_into_real_catalog() {
        // Catalog outside the guard's temp root; workspace root under it. With no current
        // project, reindex defaults to scope=All and walks the workspace roots — so the
        // guard fires on the temp root before any file walk. (No rules / fixtures needed:
        // the refusal happens before classification.)
        //
        // Both dirs are physically under the OS temp dir; `synthetic_temp` explains why the
        // guard's notion of temp is injected rather than inherited.
        let (_scratch, env, inside, outside) =
            crate::librarian::tools::temp_write_guard::synthetic_temp();
        let cat = Catalog::open(&outside.join("catalog.db")).unwrap();
        let ws = inside.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "r".into(),
                path: ws,
            })
            .with_temp_guard(env)
            .build();

        let err = call(&ctx, json!({})).await.expect_err(
            "reindexing a temp root into a real (outside-temp) catalog must be refused",
        );
        assert!(
            err.to_string().contains("temp dir"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn reindex_backfills_commits_table() {
        use std::process::Command;

        let tmp = TempDir::new().unwrap();
        let repo_path = tmp.path().join("r1");
        std::fs::create_dir_all(&repo_path).unwrap();

        let run = |args: &[&str], cwd: &std::path::Path| {
            Command::new("git")
                .args(args)
                .current_dir(cwd)
                .output()
                .unwrap()
        };

        // git init (plain — avoid -b flag for older git compatibility)
        run(&["init", "-q"], &repo_path);
        run(&["config", "user.email", "test@test.com"], &repo_path);
        run(&["config", "user.name", "Test User"], &repo_path);

        // 3 commits
        for i in 1..=3u32 {
            std::fs::write(repo_path.join("f.md"), format!("v{i}")).unwrap();
            run(&["add", "."], &repo_path);
            run(&["commit", "-q", "-m", &format!("c{i}")], &repo_path);
        }

        // Build a ToolContext pointing at this repo as "r1"
        let rules = crate::librarian::classify::load_rules("").unwrap();
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r1".into(),
                path: repo_path.clone(),
            })
            .with_rules(rules)
            .build();

        // Run reindex — this should backfill the commits table
        call(&ctx, json!({})).await.unwrap();

        // Assert 3 rows in commits table for "r1"
        let n: i64 = {
            let cat = ctx.catalog.lock();
            cat.conn
                .query_row(
                    "SELECT COUNT(*) FROM commits WHERE git_root=?1",
                    rusqlite::params![crate::util::fs::RepoPath::from(&repo_path)],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(n, 3, "should have 3 commit rows");

        // newest commit = highest topo_order = 2 (0-indexed: c1=0, c2=1, c3=2)
        let max_order: i64 = {
            let cat = ctx.catalog.lock();
            cat.conn
                .query_row(
                    "SELECT MAX(topo_order) FROM commits WHERE git_root=?1",
                    rusqlite::params![crate::util::fs::RepoPath::from(&repo_path)],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(max_order, 2, "newest commit should have topo_order=2");

        // topo_order is monotonically increasing (all distinct 0,1,2)
        let min_order: i64 = {
            let cat = ctx.catalog.lock();
            cat.conn
                .query_row(
                    "SELECT MIN(topo_order) FROM commits WHERE git_root=?1",
                    rusqlite::params![crate::util::fs::RepoPath::from(&repo_path)],
                    |r| r.get(0),
                )
                .unwrap()
        };
        assert_eq!(min_order, 0, "oldest commit should have topo_order=0");
    }
}
