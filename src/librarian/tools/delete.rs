use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ToolContext;
use crate::librarian::catalog::artifact;

#[derive(Deserialize)]
struct Args {
    id: String,
    /// Omitted or false = dry run. See the gate in `call`.
    #[serde(default)]
    force: Option<bool>,
}

/// Delete an artifact: remove its file from disk, its catalog row, and its
/// chunk vectors.
///
/// The catalog delete cascades (FK `ON DELETE CASCADE`, with
/// `PRAGMA foreign_keys = ON`) to the artifact's augmentation, links,
/// observations, and events — so no orphaned catalog rows remain (closes
/// metadata-filtering F-6, which noted that `rm` + `reindex` left the
/// catalog-only augmentation behind). The artifact must live under a managed
/// workspace root; out-of-tree paths are refused. A missing file is not fatal —
/// the catalog row is still dropped, so `delete` also repairs a stale entry for
/// an already-removed file.
///
/// **The vectors need an explicit delete, and only on Qdrant.** This comment
/// claimed until 2026-09-04 that "the `artifact_vec` trigger drops its embedding
/// — so no orphaned rows remain". That was true of the sqlite backend and false
/// of the default one: `artifact_chunk`'s cascade plus
/// `artifact_vec_v2_cascade_delete` do handle sqlite, while Qdrant has no
/// foreign keys and kept every point. The claim was correct about the backend
/// the tests exercise, which is why it read as complete.
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)
        .map_err(|e| super::RecoverableError::new(format!("delete requires 'id': {e}")))?;

    // The catalog guard lives in an EXPLICIT BLOCK, and the block is the fix rather
    // than a tidy-up. The vector delete below is `async`, and this guard is a
    // `parking_lot::MutexGuard` -- not `Send`, so holding it across that `.await` does
    // not compile. `drop(cat)` does NOT satisfy that: the binding stays in scope for
    // the generator's state machine even once moved, so only ending the scope works.
    // The `Send` error is the surface symptom; the substance is that
    // `SqliteVecArtifactStore::delete` re-acquires THIS mutex and parking_lot's is not
    // reentrant. Same shape as `mv` (`049c6c97`); see the note on
    // `SqliteVecArtifactStore` for the two ways this enforcement disappears silently.
    let (abs_path, existed) = {
        let cat = ctx.catalog.lock();
        let row = artifact::get(&cat, &a.id)?
            .ok_or_else(|| super::RecoverableError::new(format!("unknown id `{}`", a.id)))?;

        // Fork-on-first-write gate: a worktree session may not delete an artifact
        // that belongs to the main checkout — that would delete the shared row/file
        // out from under the main checkout. Merge first, or run from the main
        // checkout.
        if let Some(cp) = ctx.current_project.as_deref() {
            if super::worktree::is_main_checkout_artifact(cp, &row.abs_path) {
                return Err(super::RecoverableError::new(
                "refused from a worktree session: this artifact belongs to the main checkout. \
                 Merge the worktree (librarian action=\"merge_worktree\") or run this from the main checkout.",
            ));
            }
        }

        // Guard: only delete artifacts under a managed root — a workspace
        // `[[roots]]` entry or the active project. See `super::managed_roots`.
        let abs_path = row.abs_path.clone();
        let roots = super::managed_roots(ctx);
        if super::containing_root(&roots, &abs_path).is_none() {
            return Err(super::RecoverableError::new(format!(
                "artifact '{}' is outside every managed root — refusing to delete {}",
                a.id,
                abs_path.display()
            )));
        }

        // Dry-run gate. The catalog delete cascades to this artifact's augmentation, links,
        // observations and events — and those are CATALOG-ONLY. `reindex` rebuilds the row
        // from the file, but nothing rebuilds an augmentation's params or an event log, and
        // neither is in git. So the FILE is recoverable and the HISTORY is not, which is the
        // asymmetry a caller cannot see from the id alone.
        //
        // Preview first; `force=true` applies. Modelled on `librarian(doctor, fix=…)`, which
        // is a dry run until `confirm=true`. Measured 2026-09-03: `delete` runs ~15 times per
        // 30 days, so the round-trip is cheap — that frequency is why this gate is here and
        // not on `update` (2,555 calls).
        if !a.force.unwrap_or(false) {
            use crate::librarian::catalog::{augmentation, events, links, observations};
            return Ok(json!({
                "dry_run": true,
                "deleted": false,
                "id": a.id,
                "would_delete_abs_path": abs_path.display().to_string(),
                "cascades": {
                    "augmentation": augmentation::get(&cat, &a.id)?.is_some(),
                    "links_out": links::outgoing(&cat, &a.id)?.len(),
                    "links_in": links::incoming(&cat, &a.id)?.len(),
                    "observations": observations::list_for_artifact(&cat, &a.id)?.len(),
                    "has_events": events::latest_for_artifact(&cat, &a.id)?.is_some(),
                },
                "recoverable": "the file is git-tracked and restorable; the augmentation, \
                                events, links and observations are catalog-only and are not",
                "hint": format!("re-run with force=true to apply: doc(action=\"delete\", id=\"{}\", force=true)", a.id),
            }));
        }

        // Remove the file. A missing file is not fatal — still drop the catalog row
        // so a stale entry for an already-deleted file is cleaned up.
        match std::fs::remove_file(&abs_path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "failed to remove {}: {e}",
                    abs_path.display()
                ))
            }
        }

        let existed = artifact::delete(&cat, &a.id)?;

        (abs_path, existed)
    };

    // Drop the artifact's chunk vectors. Only Qdrant needs this: on sqlite the
    // catalog delete above already cascaded through `artifact_chunk` and its
    // `artifact_vec_v2_cascade_delete` trigger. Qdrant has no foreign keys, so
    // the same delete left every point behind — answering KNN, resolving to
    // nothing at hydration, forever.
    //
    // AFTER the catalog delete, not before: if the vector delete fails, the
    // caller gets an error and the artifact is already gone from the catalog,
    // which is recoverable by re-running. The other order can delete vectors
    // for a row that then survives, which reindex would have to notice and
    // does not.
    let vectors_deleted = match ctx.artifact_store.as_ref() {
        Some(store) => {
            store.delete(&a.id).await?;
            true
        }
        // No backend reachable. Reported rather than silently skipped, because
        // "no store to ask" and "asked, nothing there" are different facts and
        // this response is the only place a caller can tell them apart.
        None => false,
    };

    Ok(json!({
        "id": a.id,
        "deleted_abs_path": abs_path.display().to_string(),
        "deleted": existed,
        "vectors_deleted": vectors_deleted,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::librarian::{
        catalog::{
            artifact,
            artifact::ArtifactRow,
            augmentation::{self, AugmentationRow},
            Catalog,
        },
        tools::{delete, TestToolContextBuilder, ToolContext},
        workspace::{Root, WorkspaceConfig},
    };

    const ID: &str = "dddd11112222eeee";

    fn mk_ctx(tmp: &std::path::Path) -> ToolContext {
        mk_ctx_with_store(tmp, None)
    }

    /// `mk_ctx`'s store-aware twin. Parameterised rather than duplicated so the
    /// vector tests cannot drift from the fixture every other test in this module
    /// asserts against -- a second copy would let them disagree silently about what
    /// a "doomed" artifact looks like.
    fn mk_ctx_with_store(
        tmp: &std::path::Path,
        store: Option<Arc<dyn crate::librarian::artifact_store::ArtifactVectorStore>>,
    ) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();

        let row = ArtifactRow {
            id: ID.into(),
            abs_path: tmp.join("docs/trackers/doomed.md"),
            kind: "tracker".into(),
            status: "active".into(),
            title: Some("Doomed Tracker".into()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        };
        artifact::upsert(&cat, &row).unwrap();

        // Attach an augmentation to prove the FK ON DELETE CASCADE drops it.
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: ID.into(),
                prompt: "maintain".into(),
                params: "{}".into(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "0".into(),
                updated_at: "0".into(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();

        let src = tmp.join("docs/trackers/doomed.md");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(
            &src,
            "---\nid: dddd11112222eeee\nkind: tracker\n---\n# Doomed\n",
        )
        .unwrap();

        let builder = TestToolContextBuilder::new(cat).with_root(Root {
            name: "test-repo".into(),
            path: tmp.to_path_buf(),
        });
        match store {
            Some(s) => builder.with_artifact_store(s).build(),
            None => builder.build(),
        }
    }

    #[tokio::test]
    async fn delete_removes_file_catalog_row_and_augmentation() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let file = tmp.path().join("docs/trackers/doomed.md");
        assert!(file.exists());

        let result = delete::call(
            &ctx,
            serde_json::json!({"action": "delete", "id": ID, "force": true}),
        )
        .await
        .unwrap();
        assert_eq!(result["deleted"], true);
        assert!(result["deleted_abs_path"]
            .as_str()
            .unwrap()
            .ends_with("docs/trackers/doomed.md"));

        assert!(!file.exists(), "file should be removed");
        let cat = ctx.catalog.lock();
        assert!(
            artifact::get(&cat, ID).unwrap().is_none(),
            "catalog row should be gone"
        );
        assert!(
            augmentation::get(&cat, ID).unwrap().is_none(),
            "augmentation should cascade-delete"
        );
    }

    /// `delete` drops the artifact's chunk vectors, not only its catalog row.
    ///
    /// The pre-2026-09-04 behaviour was correct on sqlite and wrong on the default
    /// backend, which is why it read as complete for so long: sqlite's
    /// `artifact_chunk` FK cascade plus `artifact_vec_v2_cascade_delete` already took
    /// the vectors, so a test on that path passes either way. Qdrant has no foreign
    /// keys and kept every point — answering KNN, resolving to nothing at hydration.
    /// The fixture here is backend-agnostic on purpose: it asserts the STORE was told,
    /// which is the fact both backends need and only one of them got.
    ///
    /// The surviving second artifact is load-bearing. `delete` fans out over every
    /// collection filtering on `artifact_id`, and a fan-out that dropped the filter
    /// would satisfy an assertion that only checked the target was gone.
    #[tokio::test]
    async fn delete_drops_the_artifacts_chunk_vectors() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        use crate::librarian::artifact_store::ArtifactVectorStore;

        let tmp = tempfile::tempdir().unwrap();
        let store = Arc::new(InMemoryArtifactStore::default());
        // Three chunks: a per-chunk bug removing only the first is invisible against
        // a single-chunk fixture, which behaves identically under both.
        for c in ["c1", "c2", "c3"] {
            store.upsert("p", c, ID, &[1.0, 0.0]).await.unwrap();
        }
        store
            .upsert("p", "c9", "other-art", &[1.0, 0.0])
            .await
            .unwrap();

        let ctx = mk_ctx_with_store(tmp.path(), Some(store.clone()));

        let v = delete::call(&ctx, serde_json::json!({"id": ID, "force": true}))
            .await
            .unwrap();

        assert_eq!(v["deleted"], true);
        assert_eq!(
            v["vectors_deleted"], true,
            "reported so a caller can tell 'no store to ask' from 'asked, nothing there'"
        );
        assert_eq!(
            store.chunks_under(ID),
            0,
            "chunk vectors survived their artifact -- the Qdrant-shaped orphan"
        );
        assert_eq!(
            store.chunks_under("other-art"),
            1,
            "the fan-out dropped its artifact_id filter and took a bystander"
        );
    }

    /// With no vector backend reachable, `delete` still succeeds and says so.
    ///
    /// A lean build or an unreachable Qdrant leaves `ctx.artifact_store` as `None`.
    /// Turning that into a refusal would break `delete` for every offline caller, and
    /// reporting `vectors_deleted: true` would assert work that never happened.
    #[tokio::test]
    async fn delete_without_a_vector_backend_still_deletes_and_reports_it() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path()); // no store attached

        let v = delete::call(&ctx, serde_json::json!({"id": ID, "force": true}))
            .await
            .unwrap();

        assert_eq!(v["deleted"], true);
        assert_eq!(
            v["vectors_deleted"], false,
            "no backend was asked, so the response must not claim vectors were dropped"
        );
    }

    #[tokio::test]
    async fn delete_missing_file_still_drops_catalog_row() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        std::fs::remove_file(tmp.path().join("docs/trackers/doomed.md")).unwrap();

        let result = delete::call(&ctx, serde_json::json!({"id": ID, "force": true}))
            .await
            .unwrap();
        assert_eq!(result["deleted"], true);
        let cat = ctx.catalog.lock();
        assert!(artifact::get(&cat, ID).unwrap().is_none());
    }

    /// The gate, asserted on the EFFECT rather than on the flag.
    ///
    /// A dry run that reported `dry_run: true` and deleted anyway would satisfy a
    /// flag-only assertion, so the load-bearing checks here are that the file is still on
    /// disk and the catalog row still resolves. The flag is asserted too, but it is the
    /// weaker half.
    ///
    /// The preview must also report the augmentation, because that is the one casualty
    /// `reindex` cannot rebuild — the file is git-tracked and restorable; an
    /// augmentation's params are catalog-only and are not.
    #[tokio::test]
    async fn delete_without_force_is_a_dry_run_and_destroys_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let file = tmp.path().join("docs/trackers/doomed.md");
        assert!(file.exists());

        let result = delete::call(&ctx, serde_json::json!({"id": ID}))
            .await
            .unwrap();

        assert_eq!(result["dry_run"], true);
        assert_eq!(result["deleted"], false);
        assert_eq!(
            result["cascades"]["augmentation"], true,
            "this fixture carries an augmentation, and it is the casualty `reindex` cannot \
             rebuild — a preview that omits it hides the only irreversible part"
        );

        assert!(file.exists(), "dry run must not remove the file");
        let cat = ctx.catalog.lock();
        assert!(
            artifact::get(&cat, ID).unwrap().is_some(),
            "dry run must not drop the catalog row"
        );
        assert!(
            augmentation::get(&cat, ID).unwrap().is_some(),
            "dry run must not cascade-delete the augmentation"
        );
    }

    #[tokio::test]
    async fn delete_unknown_id_is_recoverable_error() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let err = delete::call(&ctx, serde_json::json!({"id": "nope"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unknown id"), "got: {err}");
    }

    #[tokio::test]
    async fn delete_succeeds_for_active_project_absent_from_legacy_roots() {
        // Regression for docs/issues/archive/2026-06-03-artifact-delete-refuses-in-workspace-artifact.md:
        // under the `[[project]]` model the active project lives in `current_project`,
        // not in `workspace.roots`. The guard must honor it, else every delete in such
        // a project fails with "outside every workspace root".
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = mk_ctx(tmp.path());
        ctx.workspace = Arc::new(WorkspaceConfig {
            roots: vec![],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        });
        ctx.current_project = Some(Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: tmp.path().to_path_buf(),
                git_root: tmp.path().to_path_buf(),
                main_root: None,
                umbrella: None,
            },
        ));

        let file = tmp.path().join("docs/trackers/doomed.md");
        assert!(file.exists());

        // Before the fix this returned "outside every workspace root".
        let result = delete::call(&ctx, serde_json::json!({"id": ID, "force": true}))
            .await
            .unwrap();
        assert_eq!(result["deleted"], true);
        assert!(!file.exists(), "file should be removed");
        let cat = ctx.catalog.lock();
        assert!(artifact::get(&cat, ID).unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_refuses_artifact_outside_all_managed_roots() {
        // Safety property preserved: with neither a legacy root nor an active
        // project covering the path, delete must refuse and leave the file intact.
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = mk_ctx(tmp.path());
        ctx.workspace = Arc::new(WorkspaceConfig {
            roots: vec![],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        });
        ctx.current_project = None;

        let err = delete::call(&ctx, serde_json::json!({"id": ID}))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("outside every managed root"),
            "got: {err}"
        );
        assert!(
            tmp.path().join("docs/trackers/doomed.md").exists(),
            "refused delete must not remove the file"
        );
    }

    #[tokio::test]
    async fn delete_of_main_artifact_from_worktree_is_refused() {
        let ctx = crate::librarian::tools::worktree::test_support::wt_ctx(
            Catalog::open_in_memory().unwrap(),
        );
        let main_id = {
            let c = ctx.catalog.lock();
            crate::librarian::tools::worktree::test_support::seed_main_tracker(&c)
        };

        let err = delete::call(&ctx, serde_json::json!({"id": main_id}))
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("worktree"),
            "refusal names the worktree overlay: {err}"
        );
    }

    #[tokio::test]
    async fn delete_of_worktree_born_artifact_is_allowed() {
        // /repo/.worktrees/feat is the worktree's OWN root — an artifact born
        // there is not a main-checkout artifact and must not be refused, even
        // though its path also starts with /repo (main_root).
        let tmp = tempfile::tempdir().unwrap();
        let main_root = tmp.path().to_path_buf();
        let wt_root = main_root.join(".worktrees/feat");
        std::fs::create_dir_all(wt_root.join("docs")).unwrap();
        let file_path = wt_root.join("docs/new.md");
        std::fs::write(
            &file_path,
            "---\nid: wtbornwtbornwtb1\nkind: tracker\n---\n# New\n",
        )
        .unwrap();

        let id = "wtbornwtbornwtb1";
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &ArtifactRow {
                id: id.into(),
                abs_path: file_path.clone(),
                kind: "tracker".into(),
                status: "active".into(),
                title: Some("Worktree-born".into()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: 0,
                updated_at: 0,
                file_mtime: 0,
                file_sha256: String::new(),
                confidence: 1.0,
            },
        )
        .unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: wt_root.clone(),
                    git_root: wt_root.clone(),
                    main_root: Some(main_root.clone()),
                    umbrella: None,
                },
            ))
            .build();

        let result = delete::call(&ctx, serde_json::json!({"id": id, "force": true}))
            .await
            .unwrap();
        assert_eq!(result["deleted"], true);
        assert!(
            !file_path.exists(),
            "worktree-born artifact must actually be deleted, not refused"
        );
    }
}
