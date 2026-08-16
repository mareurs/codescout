use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ToolContext;
use crate::librarian::catalog::artifact;
use crate::util::fs::to_forward_slash;

#[derive(Deserialize)]
struct Args {
    id: String,
    new_rel_path: String,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        super::RecoverableError::new(format!("move requires 'id' and 'new_rel_path': {e}"))
    })?;

    // Defense-in-depth: new_rel_path must stay within the resolved root. Reject
    // absolute paths and `..` segments so a move can never escape the project
    // even if root resolution is wrong (1a5acfc0).
    if a.new_rel_path.is_empty()
        || std::path::Path::new(&a.new_rel_path).components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(super::RecoverableError::new(format!(
            "new_rel_path '{}' must be a non-empty relative path with no '..' or absolute segments",
            a.new_rel_path
        )));
    }

    let mut cat = ctx.catalog.lock();
    let row = artifact::get(&cat, &a.id)?
        .ok_or_else(|| super::RecoverableError::new(format!("unknown id `{}`", a.id)))?;

    // Fork-on-first-write gate: a worktree session may not move an artifact
    // that belongs to the main checkout — that would rename the shared
    // file/row out from under the main checkout. Merge first, or run from
    // the main checkout.
    if let Some(cp) = ctx.current_project.as_deref() {
        if super::worktree::is_main_checkout_artifact(cp, &row.abs_path) {
            return Err(super::RecoverableError::new(
                "refused from a worktree session: this artifact belongs to the main checkout. \
                 Merge the worktree (librarian action=\"merge_worktree\") or run this from the main checkout.",
            ));
        }
    }

    // Find the managed root that contains this artifact — a workspace
    // `[[roots]]` entry or the active project. `new_rel_path` is interpreted
    // relative to that root. See `super::managed_roots`.
    let roots = super::managed_roots(ctx);
    let root_path = super::containing_root(&roots, &row.abs_path)
        .ok_or_else(|| anyhow::anyhow!("no managed root contains {}", row.abs_path.display()))?;

    let old_full = row.abs_path.clone();
    let new_full = root_path.join(&a.new_rel_path);

    if new_full.exists() {
        return Err(super::RecoverableError::new(format!(
            "destination '{}' already exists — choose a different path or delete it first",
            a.new_rel_path
        )));
    }

    if let Some(parent) = new_full.parent() {
        std::fs::create_dir_all(parent)?;
    }

    std::fs::rename(&old_full, &new_full)?;

    let now = chrono::Utc::now().timestamp_millis();

    // Catalog identity is `id == artifact_id_from_abs(abs_path)` — stated in
    // `doctor.rs` and relied on by `migrate_v6`'s implicit id migration. Keeping
    // the old id while rewriting `abs_path` leaves that invariant broken, and the
    // next reindex's `artifact::upsert` pre-clean (`DELETE FROM artifact WHERE
    // abs_path=? AND id != ?`) deletes the row — cascading its events, links,
    // observations and augmentation away, silently and later.
    //
    // So do what `doctor`'s `reseat_worktree` does for the same situation: seed a
    // row at the path-derived id, then graft the history across and drop the old
    // row. `graft_rows` re-points `artifact_link` on BOTH endpoints, so a
    // `worktree_of` lineage edge survives whether it was the shadow or the main
    // twin that moved.
    // docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md
    let new_id = crate::librarian::ids::artifact_id_from_abs(&new_full);

    // The file's own `id:` now asserts an identity this move just invalidated.
    // Repair it here, in the same call, because nothing downstream can: every
    // write path into a managed artifact refuses one (`edit_markdown` and
    // `edit_file` both guard on the frontmatter id; `artifact(update)`'s `extra`
    // writes custom keys but never `id`), so a later repair pass has no route to
    // the file. BL-23.
    let content = repair_frontmatter_id(&new_full, &new_id)?;

    // Both derived AFTER the repair. A digest taken before it describes a file
    // that no longer exists on disk, which leaves the row looking dirty on every
    // subsequent walk.
    let file_mtime = std::fs::metadata(&new_full)
        .ok()
        .and_then(|m| {
            m.modified().ok().and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_millis() as i64)
            })
        })
        .unwrap_or(now);
    let file_sha256 = crate::librarian::util::sha_of_bytes(content.as_bytes());

    let updated_row = crate::librarian::catalog::artifact::ArtifactRow {
        id: new_id.clone(),
        abs_path: new_full.clone(),
        updated_at: now,
        file_mtime,
        file_sha256,
        ..row.clone()
    };
    artifact::upsert(&cat, &updated_row)?;

    // Two transactions (`upsert` autocommits, `graft_rows` runs its own IMMEDIATE
    // tx) — same shape as `reseat_worktree`. A crash between them leaves both rows
    // present with the history still on the old one: recoverable by re-running,
    // not data loss.
    let grafted = if new_id != a.id {
        Some(crate::librarian::catalog::graft::graft_rows(
            &mut cat, &a.id, &new_id,
        )?)
    } else {
        None
    };

    Ok(json!({
        "id": new_id,
        // The id is derived from the path, so a move mints a new one. Reported
        // explicitly: prose that cites the old id has to be re-pointed, and a
        // caller that assumed stability would otherwise find out via a later
        // `unknown id` error.
        "previous_id": a.id,
        "id_changed": grafted.is_some(),
        "history_grafted": grafted.map(|r| json!({
            "events": r.events_repointed,
            "observations": r.observations_repointed,
            "links": r.links_repointed,
            "event_edges": r.event_edges_repointed,
        })),
        "old_abs_path": to_forward_slash(&old_full),
        "new_abs_path": to_forward_slash(&new_full),
        "moved": true
    }))
}

/// Rewrite a moved file's frontmatter `id:` to the id the move just minted, and
/// return the file's post-repair content.
///
/// **Only an id already present is rewritten.** A file carrying none is not
/// asserting anything false, and `frontmatter::update_in_place` would *insert* a
/// block rather than skip — stamping an `id:` is exactly what subjects a file to
/// the librarian guard, so archiving a prose tracker like
/// `docs/trackers/skill-frictions.md` would quietly make `edit_markdown` refuse
/// the workflow CLAUDE.md documents for it.
///
/// **Best-effort by design.** The rename has already happened by the time this
/// runs, so unparseable frontmatter must not abort the move and strand the
/// catalog mid-update. A failure is logged and the original content returned; the
/// catalog still re-keys correctly, and the file is left exactly as it was.
///
/// BL-23 / `docs/issues/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md`
pub(super) fn repair_frontmatter_id(
    path: &std::path::Path,
    new_id: &str,
) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(path)?;

    let needs_repair = match crate::librarian::frontmatter::parse(&content) {
        Ok((Some(fm), _)) => fm.id.as_deref().is_some_and(|id| id != new_id),
        Ok((None, _)) => false,
        Err(err) => {
            tracing::warn!(
                "move: frontmatter unparseable at {}, leaving its id alone: {err:#}",
                path.display()
            );
            false
        }
    };
    if !needs_repair {
        return Ok(content);
    }

    match crate::librarian::frontmatter::update_in_place(&content, |fm| {
        fm.id = Some(new_id.to_string());
    }) {
        Ok(rewritten) => {
            std::fs::write(path, &rewritten)?;
            Ok(rewritten)
        }
        Err(err) => {
            tracing::warn!(
                "move: could not rewrite frontmatter id at {}: {err:#}",
                path.display()
            );
            Ok(content)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::librarian::{
        catalog::{artifact, artifact::ArtifactRow, Catalog},
        tools::{mv, TestToolContextBuilder, ToolContext},
        workspace::{Root, WorkspaceConfig},
    };

    fn mk_ctx(tmp: &std::path::Path) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();

        let row = ArtifactRow {
            id: "aabbccdd11223344".into(),
            abs_path: tmp.join("docs/trackers/foo.md"),
            kind: "tracker".into(),
            status: "active".into(),
            title: Some("Foo Tracker".into()),
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

        let src = tmp.join("docs/trackers/foo.md");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(
            &src,
            "---\nid: aabbccdd11223344\nkind: tracker\n---\n# Foo\n",
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
    async fn move_renames_file_and_updates_catalog() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(result["old_abs_path"]
            .as_str()
            .unwrap()
            .ends_with("docs/trackers/foo.md"));
        assert!(result["new_abs_path"]
            .as_str()
            .unwrap()
            .ends_with("docs/archive/foo.md"));

        assert!(tmp.path().join("docs/archive/foo.md").exists());
        assert!(!tmp.path().join("docs/trackers/foo.md").exists());

        // The id is derived from the path, so a move mints a new one and reports
        // both. History follows via `graft_rows` — see
        // `move_carries_history_onto_the_new_id_and_survives_a_reindex`.
        assert_eq!(result["previous_id"], "aabbccdd11223344");
        assert_eq!(result["id_changed"], true);

        let cat = ctx.catalog.lock();
        let new_id = result["id"].as_str().unwrap();
        let row = artifact::get(&cat, new_id).unwrap().unwrap();
        assert!(row.abs_path.ends_with("docs/archive/foo.md"));
        assert!(
            artifact::get(&cat, "aabbccdd11223344").unwrap().is_none(),
            "the old id must not linger as a second row"
        );
    }

    /// BL-23 / `docs/issues/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md`.
    ///
    /// A move mints a new id, and the file's own `id:` keeps asserting the old one —
    /// which resolves to nothing. This has to be repaired **here**, in the same call
    /// as the graft, because by the time anyone notices, no write path can reach the
    /// file: `edit_markdown` and `edit_file` both refuse a librarian-managed artifact,
    /// and `artifact(update)`'s `extra` writes custom keys but never `id`.
    ///
    /// The `file_sha256` assertion is the load-bearing one. It fails if the rewrite
    /// happens after the hash is taken — the catalog would then record a digest of a
    /// file that no longer exists on disk, and the next reindex would see the row as
    /// dirty on every walk.
    #[tokio::test]
    async fn move_rewrites_the_frontmatter_id_it_just_invalidated() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["id_changed"], true);
        let new_id = result["id"].as_str().unwrap().to_string();

        let moved = tmp.path().join("docs/archive/foo.md");
        let text = std::fs::read_to_string(&moved).unwrap();
        let (fm, body) = crate::librarian::frontmatter::parse(&text).unwrap();
        let fm = fm.expect("frontmatter must survive the move");

        assert_eq!(
            fm.id.as_deref(),
            Some(new_id.as_str()),
            "the file must assert the id it now has, not the one it was moved away from"
        );
        assert_eq!(
            fm.kind.as_deref(),
            Some("tracker"),
            "rewriting `id` must not disturb the other frontmatter fields"
        );
        assert!(
            body.contains("# Foo"),
            "the body must be byte-untouched, got: {body:?}"
        );

        let cat = ctx.catalog.lock();
        let row = artifact::get(&cat, &new_id).unwrap().unwrap();
        assert_eq!(
            row.file_sha256,
            crate::librarian::util::sha_of_bytes(text.as_bytes()),
            "the recorded sha must describe the file AFTER the frontmatter rewrite — \
             hashing before it leaves the row permanently dirty"
        );
    }

    /// The other half, and the reason this is not simply `update_in_place`.
    ///
    /// `frontmatter::update_in_place` inserts a frontmatter block when none exists,
    /// so applying it unconditionally would stamp an `id:` onto files that never had
    /// one — and a stamped id is exactly what subjects a file to the librarian guard
    /// (BL-33). Archiving `docs/trackers/skill-frictions.md` would silently make it
    /// unreachable by `edit_markdown`, the workflow CLAUDE.md documents for it.
    ///
    /// A file with no `id:` is not asserting anything false. Only a wrong id is repaired.
    #[tokio::test]
    async fn move_does_not_stamp_an_id_onto_a_file_that_never_had_one() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let prose = tmp.path().join("docs/trackers/prose.md");
        std::fs::write(&prose, "---\nkind: tracker\nstatus: active\n---\n# Prose\n").unwrap();
        {
            let cat = ctx.catalog.lock();
            let row = ArtifactRow {
                id: "1111222233334444".into(),
                abs_path: prose.clone(),
                kind: "tracker".into(),
                status: "active".into(),
                title: Some("Prose Tracker".into()),
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
        }

        mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "1111222233334444",
                "new_rel_path": "docs/archive/prose.md"
            }),
        )
        .await
        .unwrap();

        let text = std::fs::read_to_string(tmp.path().join("docs/archive/prose.md")).unwrap();
        let (fm, _) = crate::librarian::frontmatter::parse(&text).unwrap();
        assert!(
            fm.expect("frontmatter block preserved").id.is_none(),
            "a file with no id must not gain one — stamping it would newly subject a \
             prose tracker to the librarian guard. Got: {text:?}"
        );
    }

    /// A move must carry the artifact's history onto the new id, and that id
    /// must survive the next reindex.
    ///
    /// Catalog identity is `id == artifact_id_from_abs(abs_path)` — stated in
    /// `src/librarian/tools/doctor.rs` and relied on by `migrate_v6`. A move that
    /// kept the old id while rewriting `abs_path` leaves that invariant broken,
    /// and the next reindex's `artifact::upsert` pre-clean (`DELETE FROM artifact
    /// WHERE abs_path=? AND id != ?`) deletes the row — cascading its events,
    /// links, observations and augmentation away.
    ///
    /// Measured 2026-08-16 against the live catalog: one reindex following a
    /// 22-tracker archive sweep took the event count from 1845 to 1834 while
    /// reporting `removed: 0`.
    /// docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md
    ///
    /// **The reindex step is the whole test.** Asserting only that history
    /// follows the move passes the moment `graft_rows` is wired up, and would
    /// still pass if the row were left mismatched — the deletion happens later,
    /// on a walk the test never runs.
    #[tokio::test]
    async fn move_carries_history_onto_the_new_id_and_survives_a_reindex() {
        use crate::librarian::catalog::events;

        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let old_id = "aabbccdd11223344";

        // History the artifact accumulated while it was live.
        {
            let cat = ctx.catalog.lock();
            events::insert(
                &cat,
                &events::TestEventRowBuilder::new(old_id, "note").build(),
            )
            .unwrap();
        }

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": old_id,
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        let archived = tmp.path().join("docs/archive/foo.md");
        let new_id = crate::librarian::ids::artifact_id_from_abs(&archived);

        assert_eq!(
            result["id"].as_str(),
            Some(new_id.as_str()),
            "move must report the id the artifact now has, not the one it had"
        );

        {
            let cat = ctx.catalog.lock();
            assert!(
                artifact::get(&cat, old_id).unwrap().is_none(),
                "the old id must not survive — it no longer matches the path it hashes from"
            );
            let row = artifact::get(&cat, &new_id)
                .unwrap()
                .expect("the artifact must live under the path-derived id");
            assert!(row.abs_path.ends_with("docs/archive/foo.md"));
            assert!(
                events::latest_for_artifact(&cat, &new_id)
                    .unwrap()
                    .is_some(),
                "the event history must be grafted onto the new id, not cascade-deleted"
            );
        }

        // The step that matters: a walk over the repo must now hit ON CONFLICT(id)
        // rather than the abs_path pre-clean, and leave the history alone.
        {
            let cat = ctx.catalog.lock();
            let rules = crate::librarian::classify::load_rules(
                "[[rule]]\nglob = \"**/docs/**/*.md\"\nkind = \"tracker\"\n",
            )
            .unwrap();
            crate::librarian::indexer::index_repo_sync(
                &cat,
                &rules,
                tmp.path(),
                &globset::GlobSet::empty(),
                false,
                false,
                false,
            )
            .unwrap();

            assert!(
                artifact::get(&cat, &new_id).unwrap().is_some(),
                "the reindex must not re-key the artifact it just found in place"
            );
            assert!(
                events::latest_for_artifact(&cat, &new_id)
                    .unwrap()
                    .is_some(),
                "the event history must survive the reindex"
            );
        }
    }

    #[tokio::test]
    async fn move_errors_if_destination_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let dst = tmp.path().join("docs/archive/foo.md");
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::write(&dst, "already here").unwrap();

        let err = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn move_errors_on_unknown_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let err = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "deadbeefdeadbeef",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("unknown id"));
    }

    #[tokio::test]
    async fn move_succeeds_for_active_project_absent_from_legacy_roots() {
        // Regression for docs/issues/archive/2026-06-03-artifact-delete-refuses-in-workspace-artifact.md
        // (mv shares delete's guard): under the `[[project]]` model the active project is in
        // `current_project`, not `workspace.roots`. `new_rel_path` must resolve relative to the
        // active project's git_root.
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

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(tmp.path().join("docs/archive/foo.md").exists());
        assert!(!tmp.path().join("docs/trackers/foo.md").exists());
        // The id is derived from the path, so a move mints a new one and reports
        // both. History follows via `graft_rows` — see
        // `move_carries_history_onto_the_new_id_and_survives_a_reindex`.
        assert_eq!(result["previous_id"], "aabbccdd11223344");
        assert_eq!(result["id_changed"], true);

        let cat = ctx.catalog.lock();
        let new_id = result["id"].as_str().unwrap();
        let row = artifact::get(&cat, new_id).unwrap().unwrap();
        assert!(row.abs_path.ends_with("docs/archive/foo.md"));
        assert!(
            artifact::get(&cat, "aabbccdd11223344").unwrap().is_none(),
            "the old id must not linger as a second row"
        );
    }

    #[tokio::test]
    async fn move_resolves_under_nested_project_not_ancestor_root() {
        // 1a5acfc0: active project nested under an ancestor [[roots]] entry.
        // The move must resolve against the nested project, not the ancestor.
        let tmp = tempfile::tempdir().unwrap();
        let ancestor = tmp.path().to_path_buf();
        let child = ancestor.join("child");
        std::fs::create_dir_all(&child).unwrap();
        let mut ctx = mk_ctx(&child); // seeds artifact at child/docs/trackers/foo.md

        // Workspace registers the ANCESTOR as a legacy [[roots]] entry; the
        // active project is the nested child (its own repo), absent from roots.
        ctx.workspace = Arc::new(WorkspaceConfig {
            roots: vec![Root {
                name: "ancestor".into(),
                path: ancestor.clone(),
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        });
        ctx.current_project = Some(Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: child.clone(),
                git_root: child.clone(),
                main_root: None,
                umbrella: None,
            },
        ));

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(
            child.join("docs/archive/foo.md").exists(),
            "move resolved under the nested active project"
        );
        assert!(
            !ancestor.join("docs/archive/foo.md").exists(),
            "move did NOT escape to the ancestor [[roots]] entry"
        );
    }

    #[tokio::test]
    async fn move_rejects_new_rel_path_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let err = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "../escape/foo.md"
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("..") || err.to_string().contains("relative"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn mv_of_main_artifact_from_worktree_is_refused() {
        let ctx = crate::librarian::tools::worktree::test_support::wt_ctx(
            Catalog::open_in_memory().unwrap(),
        );
        let main_id = {
            let c = ctx.catalog.lock();
            crate::librarian::tools::worktree::test_support::seed_main_tracker(&c)
        };

        let err = mv::call(
            &ctx,
            serde_json::json!({"id": main_id, "new_rel_path": "docs/trackers/moved.md"}),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("worktree"),
            "refusal names the worktree overlay: {err}"
        );
    }

    #[tokio::test]
    async fn mv_of_worktree_born_artifact_is_allowed() {
        // Mirror of the delete-side test: an artifact born under the
        // worktree's own root (nested inside main_root) must not be refused.
        let tmp = tempfile::tempdir().unwrap();
        let main_root = tmp.path().to_path_buf();
        let wt_root = main_root.join(".worktrees/feat");
        std::fs::create_dir_all(wt_root.join("docs")).unwrap();
        let file_path = wt_root.join("docs/new.md");
        std::fs::write(
            &file_path,
            "---\nid: mvwtbornmvwtbo1\nkind: tracker\n---\n# New\n",
        )
        .unwrap();

        let id = "mvwtbornmvwtbo1";
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

        let result = mv::call(
            &ctx,
            serde_json::json!({"id": id, "new_rel_path": "docs/moved.md"}),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(wt_root.join("docs/moved.md").exists());
        assert!(
            !file_path.exists(),
            "worktree-born artifact must actually be moved, not refused"
        );
    }
}
