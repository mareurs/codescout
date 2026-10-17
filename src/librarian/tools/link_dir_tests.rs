//! A catalogued path that passes through a symbolic link to a DIRECTORY must not be written,
//! moved, deleted or read as if it were inside the repository.
//!
//! `link_write_tests.rs` covers the LEAF link. These are the parent-directory forms. The indexer
//! can mint such a row two ways, both of which a repository can ship: `force_include` walks a
//! symlinked anchor directory (`WalkBuilder` follows a symlinked ROOT), and a directory swapped
//! for a link after indexing keeps its rows because reclamation asks `Path::exists()`, which
//! follows the link. Fixtures place rows by hand where the walk would not, and the two
//! indexer-born forms get their own tests through the real `index_repo_sync`.
//!
//! The victim always lives in a SEPARATE `TempDir` bound to a local. Unix-only (links).
#![cfg(unix)]

use super::{augment, create, delete, doctor, mv, update, TestToolContextBuilder, ToolContext};
use crate::librarian::catalog::artifact::TestArtifactRowBuilder;
use crate::librarian::catalog::{artifact, augmentation, Catalog};
use crate::librarian::tools::LibrarianRecoverableError;
use crate::librarian::workspace::Root;
use serde_json::json;
use std::path::Path;
use tempfile::TempDir;

const DOC: &str = "---\nkind: spec\nstatus: draft\ntitle: T\n---\n\nbody\n";

fn mk_ctx(root: &Path) -> ToolContext {
    TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_root(Root {
            name: "r".into(),
            path: root.to_path_buf(),
        })
        .build()
}

fn md_rules() -> Vec<crate::librarian::classify::CompiledRule> {
    crate::librarian::classify::load_rules("[[rule]]\nglob = \"**/*.md\"\nkind = \"doc\"\n")
        .unwrap()
}

/// A repository root (a `.git` marks it, as `lookup_git_root` needs) where `link` is a symlink
/// to a directory in a SEPARATE temp dir that holds `x.md`. Returns (outside, root, ctx).
fn repo_with_dir_link(link: &str) -> (TempDir, TempDir, ToolContext) {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("x.md"), DOC).unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join(link)).unwrap();
    let ctx = mk_ctx(root.path());
    (outside, root, ctx)
}

/// Catalog a row at `path` by hand and return its id (the shape a walk would not produce today,
/// but which `force_include` or a swapped directory leaves behind).
fn catalog_row(ctx: &ToolContext, path: &Path) -> String {
    let id = crate::librarian::ids::artifact_id_from_abs(path);
    artifact::upsert(
        &ctx.catalog.lock(),
        &TestArtifactRowBuilder::new(&id)
            .with_abs_path(path)
            .with_kind("doc")
            .with_status("draft")
            .build(),
    )
    .unwrap();
    id
}

fn msg(e: &anyhow::Error) -> String {
    format!("{e:#}")
}

// ---- writers, below a link directory -----------------------------------------------------

#[tokio::test]
async fn doc_update_refuses_a_row_below_a_symlinked_directory_that_leaves_the_repo() {
    let (outside, root, ctx) = repo_with_dir_link("linkdir");
    let id = catalog_row(&ctx, &root.path().join("linkdir/x.md"));

    let err = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("a write below a link to an outside directory must be refused");

    assert!(
        msg(&err).contains("outside the repository root"),
        "{}",
        msg(&err)
    );
    assert_eq!(
        std::fs::read_to_string(outside.path().join("x.md")).unwrap(),
        DOC
    );
}

/// The control: a directory link that stays INSIDE the repo is a legitimate setup (a symlinked
/// tracker directory) and its rows must still be writable.
#[tokio::test]
async fn doc_update_still_writes_below_a_directory_link_that_stays_inside() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::fs::create_dir_all(root.path().join("real")).unwrap();
    std::fs::write(root.path().join("real/x.md"), DOC).unwrap();
    std::os::unix::fs::symlink(root.path().join("real"), root.path().join("alias")).unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &root.path().join("alias/x.md"));

    update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect("an in-root directory link still works");

    let on_disk = std::fs::read_to_string(root.path().join("real/x.md")).unwrap();
    assert!(on_disk.contains("status: fixed"), "{on_disk}");
}

/// The link's target is ITSELF a repository (`docs -> ~/other-repo`). A root found by walking
/// the spelled ancestors and testing `.git` through the link would stop AT the link, call the
/// outside repository "the root", and read everything below it as inside. The root must be the
/// nearest ancestor that is not a link.
#[tokio::test]
async fn doc_update_refuses_a_row_below_a_link_to_another_repository() {
    let (outside, root, ctx) = repo_with_dir_link("linkdir");
    std::fs::create_dir_all(outside.path().join(".git")).unwrap();
    let id = catalog_row(&ctx, &root.path().join("linkdir/x.md"));

    let err = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("a link to another repository is still outside THIS repository");

    assert!(
        msg(&err).contains("outside the repository root"),
        "{}",
        msg(&err)
    );
    assert_eq!(
        std::fs::read_to_string(outside.path().join("x.md")).unwrap(),
        DOC
    );
}

/// `doc(delete, force=true)` unlinks `row.abs_path`; `remove_file` on a path whose PARENT is a
/// link to an outside directory deletes the outside file. (The leaf case unlinks the link
/// itself, which is the supported way to drop an outward-link row.)
#[tokio::test]
async fn doc_delete_force_refuses_a_row_below_a_symlinked_directory_that_leaves_the_repo() {
    let (outside, root, ctx) = repo_with_dir_link("linkdir");
    let id = catalog_row(&ctx, &root.path().join("linkdir/x.md"));

    let err = delete::call(&ctx, json!({"id": id, "force": true}))
        .await
        .expect_err("deleting through a link to an outside directory must be refused");

    assert!(
        msg(&err).contains("outside the repository root"),
        "{}",
        msg(&err)
    );
    assert!(
        outside.path().join("x.md").exists(),
        "the outside file must still exist"
    );
    assert!(artifact::get(&ctx.catalog.lock(), &id).unwrap().is_some());
}

#[tokio::test]
async fn doc_delete_force_still_deletes_a_leaf_link_row_and_leaves_its_target() {
    let outside = tempfile::tempdir().unwrap();
    let victim = outside.path().join("elsewhere.md");
    std::fs::write(&victim, DOC).unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::os::unix::fs::symlink(&victim, root.path().join("outward.md")).unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &root.path().join("outward.md"));

    delete::call(&ctx, json!({"id": id, "force": true}))
        .await
        .expect("dropping an outward-link row is supported");

    assert!(victim.exists(), "unlink removes the link, never its target");
    assert!(std::fs::symlink_metadata(root.path().join("outward.md")).is_err());
}

// ---- move ---------------------------------------------------------------------------------

/// DESTINATION side. `mv` checked only the SPELLING of `new_rel_path`; `create_dir_all` then
/// built `sub/` OUTSIDE the repo, `rename` moved the document there, `moved: true` was returned,
/// and the id repair wrote the moved file (its leaf is a regular file).
#[tokio::test]
async fn doc_move_refuses_a_destination_through_a_symlinked_directory_that_leaves_the_repo() {
    let outside = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("esc")).unwrap();
    let ctx = mk_ctx(root.path());
    let v = create::call(
        &ctx,
        json!({"repo": "r", "rel_path": "doc.md", "kind": "spec", "title": "T", "body": "b"}),
    )
    .await
    .unwrap();
    let id = v["id"].as_str().unwrap();

    let err = mv::call(&ctx, json!({"id": id, "new_rel_path": "esc/sub/moved.md"}))
        .await
        .expect_err("a move out of the repository must be refused");

    assert!(
        msg(&err).contains("outside the repository root"),
        "{}",
        msg(&err)
    );
    assert!(
        !outside.path().join("sub").exists(),
        "nothing may be built outside before the refusal"
    );
    assert!(root.path().join("doc.md").is_file(), "the source stays put");
}

#[tokio::test]
async fn doc_move_still_moves_through_a_directory_link_that_stays_inside() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::fs::create_dir_all(root.path().join("docs/real")).unwrap();
    std::os::unix::fs::symlink(root.path().join("docs/real"), root.path().join("alias")).unwrap();
    let ctx = mk_ctx(root.path());
    let v = create::call(
        &ctx,
        json!({"repo": "r", "rel_path": "doc.md", "kind": "spec", "title": "T", "body": "b"}),
    )
    .await
    .unwrap();
    let id = v["id"].as_str().unwrap();

    mv::call(&ctx, json!({"id": id, "new_rel_path": "alias/moved.md"}))
        .await
        .expect("an in-root directory link is a legitimate destination");

    assert!(root.path().join("docs/real/moved.md").is_file());
}

/// SOURCE side: a row whose ancestor links outside would have the OUTSIDE file renamed INTO the
/// repository, removing it from where its owner keeps it.
#[tokio::test]
async fn doc_move_refuses_a_source_below_a_symlinked_directory_that_leaves_the_repo() {
    let (outside, root, ctx) = repo_with_dir_link("linkdir");
    let id = catalog_row(&ctx, &root.path().join("linkdir/x.md"));

    let err = mv::call(&ctx, json!({"id": id, "new_rel_path": "docs/x.md"}))
        .await
        .expect_err("moving a file that lives outside the repository must be refused");

    assert!(
        msg(&err).contains("outside the repository root"),
        "{}",
        msg(&err)
    );
    assert!(outside.path().join("x.md").exists());
    assert!(!root.path().join("docs/x.md").exists());
}

// ---- sidecars -----------------------------------------------------------------------------

/// An artifact declaring `docs/augmentations/t.yaml` where `docs/augmentations` links outside.
/// Returns (outside, repo-root, ctx, id). The outside directory holds an existing `t.yaml`.
fn declared_through_dir_link() -> (TempDir, TempDir, ToolContext, String) {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("t.yaml"), "prompt: OUTSIDE-ORIGINAL\n").unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::fs::create_dir_all(root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs/augmentations")).unwrap();
    let art = root.path().join("t.md");
    std::fs::write(
        &art,
        "---\nkind: tracker\nstatus: active\nexpects_augmentation: docs/augmentations/t.yaml\n---\n\n# T\n",
    )
    .unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &art);
    (outside, root, ctx, id)
}

/// What the agent actually sees. `doc(augment)` wrapped every `write_through` error in a plain
/// `anyhow!`, so a refusal surfaced as FATAL (`isError: true`), AFTER the catalog had already
/// moved, with the remedy "re-run once the path is writable" (wrong for a refusal).
#[tokio::test]
async fn doc_augment_refuses_a_sidecar_dir_link_recoverably_before_the_catalog_moves() {
    let (outside, _root, ctx, id) = declared_through_dir_link();

    let err = augment::call(&ctx, json!({"id": id, "prompt": "NEW-PROMPT"}))
        .await
        .expect_err("the sidecar write goes through a link that leaves the repository");

    assert!(
        err.downcast_ref::<LibrarianRecoverableError>().is_some(),
        "a refusal must stay RECOVERABLE (isError:false), not fatal: {err:?}"
    );
    assert!(
        msg(&err).contains("outside the repository root"),
        "{}",
        msg(&err)
    );
    assert!(
        !msg(&err).contains("re-run once the path is writable"),
        "the remedy for a refusal is not 'make it writable': {}",
        msg(&err)
    );
    assert!(
        augmentation::get(&ctx.catalog.lock(), &id)
            .unwrap()
            .is_none(),
        "the check must run BEFORE the catalog moves"
    );
    assert_eq!(
        std::fs::read_to_string(outside.path().join("t.yaml")).unwrap(),
        "prompt: OUTSIDE-ORIGINAL\n"
    );
}

/// The lexical form from the first fix (`../x.yaml`), at the same agent-visible shape.
#[tokio::test]
async fn doc_augment_refuses_a_declared_dotdot_sidecar_recoverably_before_the_catalog_moves() {
    let base = tempfile::tempdir().unwrap();
    let repo = base.path().join("repo");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::write(
        base.path().join("outside.yaml"),
        "prompt: OUTSIDE-ORIGINAL\n",
    )
    .unwrap();
    let art = repo.join("t.md");
    std::fs::write(
        &art,
        "---\nkind: tracker\nstatus: active\nexpects_augmentation: ../outside.yaml\n---\n\n# T\n",
    )
    .unwrap();
    let ctx = mk_ctx(&repo);
    let id = catalog_row(&ctx, &art);

    let err = augment::call(&ctx, json!({"id": id, "prompt": "NEW-PROMPT"}))
        .await
        .expect_err("a declared path that leaves the repository is refused");

    assert!(
        err.downcast_ref::<LibrarianRecoverableError>().is_some(),
        "{err:?}"
    );
    assert!(augmentation::get(&ctx.catalog.lock(), &id)
        .unwrap()
        .is_none());
    assert_eq!(
        std::fs::read_to_string(base.path().join("outside.yaml")).unwrap(),
        "prompt: OUTSIDE-ORIGINAL\n"
    );
}

/// The control: a sidecar directory that is a link whose target is INSIDE the repo works, and
/// so does a plain one.
#[tokio::test]
async fn doc_augment_still_publishes_through_a_sidecar_dir_link_that_stays_inside() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::fs::create_dir_all(root.path().join("shared/aug")).unwrap();
    std::fs::write(root.path().join("shared/aug/t.yaml"), "prompt: OLD\n").unwrap();
    std::fs::create_dir_all(root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(
        root.path().join("shared/aug"),
        root.path().join("docs/augmentations"),
    )
    .unwrap();
    let art = root.path().join("t.md");
    std::fs::write(
        &art,
        "---\nkind: tracker\nstatus: active\nexpects_augmentation: docs/augmentations/t.yaml\n---\n\n# T\n",
    )
    .unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &art);

    augment::call(&ctx, json!({"id": id, "prompt": "NEW-PROMPT"}))
        .await
        .expect("an in-root directory link is a legitimate sidecar location");

    let on_disk = std::fs::read_to_string(root.path().join("shared/aug/t.yaml")).unwrap();
    assert!(on_disk.contains("NEW-PROMPT"), "{on_disk}");
}

/// `doctor fix=export_augmentations` CREATES sidecars through `docs/augmentations`; with that
/// directory a link out of the repo it would create files in the outside directory.
#[tokio::test]
async fn doctor_export_does_not_create_sidecars_through_a_directory_link_out_of_the_repo() {
    let outside = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::fs::create_dir_all(root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs/augmentations")).unwrap();
    let art = root.path().join("t.md");
    std::fs::write(&art, "---\nkind: tracker\nstatus: active\n---\n\n# T\n").unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &art);
    augmentation::upsert(
        &ctx.catalog.lock(),
        &augmentation::AugmentationRow {
            artifact_id: id,
            prompt: "p".into(),
            params: "{}".into(),
            last_refreshed_at: None,
            refresh_count: 0,
            created_at: "2026-08-01T00:00:00Z".into(),
            updated_at: "2026-08-01T00:00:00Z".into(),
            render_template: None,
            params_schema: None,
            append_mode: false,
            history_cap: None,
            entry_collection: None,
            refreshed_at_commit: None,
        },
    )
    .unwrap();
    let root_s = crate::util::fs::RepoPath::from_path(root.path()).into_string();

    let applied = doctor::call(
        &ctx,
        json!({"fix": "export_augmentations", "root": root_s, "confirm": true}),
    )
    .await
    .unwrap();

    let created: Vec<_> = std::fs::read_dir(outside.path()).unwrap().collect();
    assert!(
        created.is_empty(),
        "nothing may be created outside: {applied}"
    );
    assert!(
        applied.to_string().contains("outside the repository root"),
        "{applied}"
    );
}

// ---- the indexer's own rows ---------------------------------------------------------------

fn index(ctx: &ToolContext, root: &Path) -> crate::librarian::indexer::IndexReport {
    let ignore = globset::GlobSet::empty();
    crate::librarian::indexer::index_repo_sync(
        &ctx.catalog.lock(),
        &md_rules(),
        root,
        &ignore,
        false,
        false,
        false,
    )
    .unwrap()
    .0
}

fn force_include(root: &Path, pattern: &str) {
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    std::fs::write(
        root.join(".codescout/project.toml"),
        format!("[ignored_paths]\nforce_include = [\"{pattern}\"]\n"),
    )
    .unwrap();
}

/// `force_include_candidates` walks `WalkBuilder::new(<anchor>)`, and the walker follows a
/// symlinked ROOT, so `docs/trackers -> <outside>` plus `force_include = ["docs/trackers/**"]`
/// catalogued the outside file as an in-repo artifact. A repository ships both halves.
#[test]
fn force_include_does_not_catalog_files_behind_a_symlinked_anchor_that_leaves_the_repo() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("t.md"), DOC).unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs/trackers")).unwrap();
    force_include(root.path(), "docs/trackers/**");
    let ctx = mk_ctx(root.path());
    // Precondition: the file IS reachable through the link, so a zero is the indexer's doing.
    assert!(root.path().join("docs/trackers/t.md").is_file());

    let report = index(&ctx, root.path());

    let id = crate::librarian::ids::artifact_id_from_abs(&root.path().join("docs/trackers/t.md"));
    assert!(
        artifact::get(&ctx.catalog.lock(), &id).unwrap().is_none(),
        "a file outside the repository must not be catalogued as one inside it"
    );
    assert_eq!(report.added, 0);
}

/// The control: a symlinked tracker directory that resolves INSIDE the repo keeps working.
#[test]
fn force_include_still_catalogs_through_a_symlinked_anchor_that_stays_inside() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("docs")).unwrap();
    std::fs::create_dir_all(root.path().join("real")).unwrap();
    std::fs::write(root.path().join("real/t.md"), DOC).unwrap();
    std::os::unix::fs::symlink(root.path().join("real"), root.path().join("docs/trackers"))
        .unwrap();
    force_include(root.path(), "docs/trackers/**");
    let ctx = mk_ctx(root.path());

    index(&ctx, root.path());

    let via_link =
        crate::librarian::ids::artifact_id_from_abs(&root.path().join("docs/trackers/t.md"));
    assert!(artifact::get(&ctx.catalog.lock(), &via_link)
        .unwrap()
        .is_some());
}

/// A directory swapped for a link AFTER indexing: the walk no longer reaches the row, and
/// reclamation asked `Path::exists()`, which follows the link, so the stale row survived a
/// reindex and `doc(update)` then wrote the outside file.
#[test]
fn a_row_whose_directory_became_a_link_out_of_the_repo_is_reclaimed_by_a_reindex() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("t.md"), DOC).unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("docs")).unwrap();
    std::fs::write(root.path().join("docs/t.md"), DOC).unwrap();
    let ctx = mk_ctx(root.path());
    assert_eq!(
        index(&ctx, root.path()).added,
        1,
        "precondition: the row exists"
    );
    std::fs::remove_dir_all(root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs")).unwrap();
    let id = crate::librarian::ids::artifact_id_from_abs(&root.path().join("docs/t.md"));

    let report = index(&ctx, root.path());

    assert!(
        artifact::get(&ctx.catalog.lock(), &id).unwrap().is_none(),
        "the stale row below the swapped directory must be reclaimed"
    );
    assert_eq!(report.removed, 1);
}

/// The control for the reclamation change: a row below a directory that is a link to a place
/// INSIDE the repo is not reclaimed merely for being reached through a link.
#[test]
fn a_row_below_a_directory_link_that_stays_inside_is_not_reclaimed() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("real")).unwrap();
    std::fs::write(root.path().join("real/t.md"), DOC).unwrap();
    std::os::unix::fs::symlink(root.path().join("real"), root.path().join("alias")).unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &root.path().join("alias/t.md"));

    index(&ctx, root.path());

    assert!(artifact::get(&ctx.catalog.lock(), &id).unwrap().is_some());
}
