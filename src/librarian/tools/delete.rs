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

/// Delete an artifact: remove its file from disk and its catalog row.
///
/// The catalog delete cascades (FK `ON DELETE CASCADE`, with
/// `PRAGMA foreign_keys = ON`) to the artifact's augmentation, links,
/// observations, and events, and the `artifact_vec` trigger drops its
/// embedding — so no orphaned rows remain (closes metadata-filtering F-6,
/// which noted that `rm` + `reindex` left the catalog-only augmentation
/// behind). The artifact must live under a managed workspace root; out-of-tree
/// paths are refused. A missing file is not fatal — the catalog row is still
/// dropped, so `delete` also repairs a stale entry for an already-removed file.
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)
        .map_err(|e| super::RecoverableError::new(format!("delete requires 'id': {e}")))?;

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

    Ok(json!({
        "id": a.id,
        "deleted_abs_path": abs_path.display().to_string(),
        "deleted": existed,
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

        TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "test-repo".into(),
                path: tmp.to_path_buf(),
            })
            .build()
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
