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

/// A committed sidecar that is a link to a file INSIDE the repo resolves fine, so the containment
/// check lets it through; only the leaf rule refuses it, and `doc(augment)` must refuse it BEFORE
/// the catalog moves, like the escapes above, not after.
#[tokio::test]
async fn doc_augment_refuses_a_sidecar_that_is_an_in_repo_link_before_the_catalog_moves() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::fs::create_dir_all(root.path().join("docs/augmentations")).unwrap();
    std::fs::write(root.path().join("shared.yaml"), "prompt: OLD\n").unwrap();
    std::os::unix::fs::symlink(
        root.path().join("shared.yaml"),
        root.path().join("docs/augmentations/t.yaml"),
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

    let err = augment::call(&ctx, json!({"id": id, "prompt": "NEW-PROMPT"}))
        .await
        .expect_err("a sidecar that is a link is never written through");

    assert!(
        err.downcast_ref::<LibrarianRecoverableError>().is_some(),
        "{err:?}"
    );
    assert!(msg(&err).contains("symbolic link"), "{}", msg(&err));
    assert!(augmentation::get(&ctx.catalog.lock(), &id)
        .unwrap()
        .is_none());
    assert_eq!(
        std::fs::read_to_string(root.path().join("shared.yaml")).unwrap(),
        "prompt: OLD\n"
    );
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

// ======================= round 3: ONE root for "outside" =====================================
//
// Round 2 used three roots for the same question (the managed root, git root first, in move and
// delete; the spelled git root at the write door; the walk root in reclamation) and they disagree
// exactly where it matters: a monorepo project, a project reached through a link, a sibling
// directory whose name differs only in case or in a LIKE wildcard.

/// A monorepo: `mono/.git`, project `mono/pkg`, `pkg/docs -> ../shared/docs` (INSIDE the repo,
/// OUTSIDE the project). The context is the production shape: `current_project` holds the git
/// root and the project dir, so `managed_roots` lists the git root first.
fn monorepo() -> (TempDir, ToolContext) {
    use crate::librarian::current_project::CurrentProject;
    let mono = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(mono.path().join(".git")).unwrap();
    std::fs::create_dir_all(mono.path().join("pkg")).unwrap();
    std::fs::create_dir_all(mono.path().join("shared/docs")).unwrap();
    std::os::unix::fs::symlink("../shared/docs", mono.path().join("pkg/docs")).unwrap();
    let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_root(Root {
            name: "r".into(),
            path: mono.path().join("pkg"),
        })
        .with_current_project(std::sync::Arc::new(CurrentProject {
            abs_path: mono.path().join("pkg"),
            git_root: mono.path().to_path_buf(),
            main_root: None,
            umbrella: None,
        }))
        .build();
    (mono, ctx)
}

/// `doc(move)` to `pkg/docs/x.md` succeeds (the managed root is the GIT root, and the link
/// stays inside it), and the next reindex of the PROJECT must not then reclaim the row it just
/// minted: reclamation compared against the walk root (`mono/pkg`), under which the same path
/// "escapes". The row, its augmentation and the grafted history were deleted.
#[tokio::test]
async fn a_row_moved_below_an_in_repo_link_survives_a_reindex_of_the_project() {
    let (mono, ctx) = monorepo();
    let v = create::call(
        &ctx,
        json!({"repo": "r", "rel_path": "doc.md", "kind": "spec", "title": "T", "body": "b"}),
    )
    .await
    .unwrap();
    let id = v["id"].as_str().unwrap();
    let moved = mv::call(&ctx, json!({"id": id, "new_rel_path": "pkg/docs/x.md"}))
        .await
        .expect("a link that stays inside the git root is a legitimate destination");
    let new_id = moved["id"].as_str().unwrap().to_string();
    augmentation::upsert(&ctx.catalog.lock(), &aug_row(&new_id)).unwrap();
    assert!(
        mono.path().join("shared/docs/x.md").is_file(),
        "precondition: the file moved"
    );

    let report = index(&ctx, &mono.path().join("pkg"));

    assert!(
        artifact::get(&ctx.catalog.lock(), &new_id)
            .unwrap()
            .is_some(),
        "the reindex must not delete the row `doc(move)` just created"
    );
    assert!(augmentation::get(&ctx.catalog.lock(), &new_id)
        .unwrap()
        .is_some());
    assert_eq!(report.removed, 0);
    assert!(
        report.reclaimed_escaping_ids.is_empty(),
        "{:?}",
        report.reclaimed_escaping_ids
    );
}

/// The same disagreement through `force_include`: the walk skipped (and recorded for
/// reclamation) a file whose path resolves outside the PROJECT but inside the repository.
#[test]
fn force_include_through_a_link_that_stays_inside_the_git_root_is_catalogued_from_a_subproject() {
    let (mono, ctx) = monorepo();
    std::fs::write(mono.path().join("shared/docs/x.md"), DOC).unwrap();
    force_include(&mono.path().join("pkg"), "docs/**");

    index(&ctx, &mono.path().join("pkg"));

    let id = crate::librarian::ids::artifact_id_from_abs(&mono.path().join("pkg/docs/x.md"));
    assert!(artifact::get(&ctx.catalog.lock(), &id).unwrap().is_some());
}

/// An augmentation row (a stand-in for "history that exists nowhere else").
fn aug_row(id: &str) -> augmentation::AugmentationRow {
    augmentation::AugmentationRow {
        artifact_id: id.into(),
        prompt: "p".into(),
        params: r#"{"keep":"me"}"#.into(),
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
    }
}

/// Reclamation selected candidates with `abs_path LIKE '<root>/%'`: no `ESCAPE`, so `_` matched any
/// character, and SQLite's `LIKE` ignores ASCII case. Reindexing `b/my_app` therefore saw the rows of
/// `b/myXapp` and `b/MY_APP` as ITS rows, and round 2's escape test then deleted them (with their
/// augmentation) although the files were on disk.
///
/// The case-only sibling carries NO history on purpose: with history round 2's reclaim merely KEPT
/// it, so a fixture where both siblings had an augmentation let the missing `starts_with` filter
/// survive a mutation.
#[test]
fn a_reindex_never_reclaims_the_rows_of_a_sibling_whose_name_matches_a_like_pattern() {
    let b = tempfile::tempdir().unwrap();
    let ctx = mk_ctx(&b.path().join("my_app"));
    for name in ["my_app", "myXapp", "MY_APP"] {
        std::fs::create_dir_all(b.path().join(name).join("docs")).unwrap();
        std::fs::write(b.path().join(name).join("docs/n.md"), DOC).unwrap();
    }
    let with_history = catalog_row(&ctx, &b.path().join("myXapp").join("docs/n.md"));
    augmentation::upsert(&ctx.catalog.lock(), &aug_row(&with_history)).unwrap();
    let plain = catalog_row(&ctx, &b.path().join("MY_APP").join("docs/n.md"));

    let report = index(&ctx, &b.path().join("my_app"));

    for id in [&with_history, &plain] {
        assert!(
            artifact::get(&ctx.catalog.lock(), id).unwrap().is_some(),
            "a sibling's row must survive a reindex of another root"
        );
    }
    assert!(augmentation::get(&ctx.catalog.lock(), &with_history)
        .unwrap()
        .is_some());
    assert_eq!(report.removed, 0);
    assert_eq!(report.reclaimed_escaping, 0);
    assert!(report.escaping_kept_ids.is_empty());
}

/// A path containing `'` never matched: the prefix was doubled (`''`) for SQL text and then BOUND
/// as a parameter, where the doubling is literal. Its rows could neither be reclaimed when the
/// file vanished nor counted as vectorless.
#[test]
fn a_row_under_a_root_with_an_apostrophe_is_reclaimed_when_its_file_is_deleted() {
    let b = tempfile::tempdir().unwrap();
    let root = b.path().join("o'brien");
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("docs/n.md"), DOC).unwrap();
    let ctx = mk_ctx(&root);
    assert_eq!(index(&ctx, &root).added, 1, "precondition: the row exists");
    std::fs::remove_file(root.join("docs/n.md")).unwrap();

    let report = index(&ctx, &root);

    assert_eq!(
        report.removed, 1,
        "the row of a deleted file must be reclaimed"
    );
}

/// H3: the project root is reached THROUGH a link (`home/proj -> data/proj`) and something above
/// that link is a repository (`home/.git`). The door took `home` for the root, so every write
/// below the project resolved to `data/proj/...` "outside the repository". The door now takes the
/// containing MANAGED root, as move and delete do.
#[tokio::test]
async fn a_project_reached_through_a_link_under_another_repository_is_still_writable() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let data = base.path().join("data");
    std::fs::create_dir_all(home.join(".git")).unwrap();
    std::fs::create_dir_all(data.join("proj/.git")).unwrap();
    std::os::unix::fs::symlink(data.join("proj"), home.join("proj")).unwrap();
    let project = home.join("proj");
    std::fs::write(project.join("x.md"), DOC).unwrap();
    let ctx = mk_ctx(&project);
    let id = catalog_row(&ctx, &project.join("x.md"));

    update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect("a write inside the managed project is not 'outside the repository'");

    assert!(std::fs::read_to_string(data.join("proj/x.md"))
        .unwrap()
        .contains("status: fixed"));
}

/// …and `doc(move)` in that project must not fail AFTER its rename: the id repair writes through
/// the door with a `?`, so a refusal there left the move half done.
#[tokio::test]
async fn doc_move_in_a_project_reached_through_a_link_completes() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let data = base.path().join("data");
    std::fs::create_dir_all(home.join(".git")).unwrap();
    std::fs::create_dir_all(data.join("proj/.git")).unwrap();
    std::os::unix::fs::symlink(data.join("proj"), home.join("proj")).unwrap();
    let project = home.join("proj");
    let ctx = mk_ctx(&project);
    let v = create::call(
        &ctx,
        json!({"repo": "r", "rel_path": "doc.md", "kind": "spec", "title": "T", "body": "b"}),
    )
    .await
    .unwrap();
    let id = v["id"].as_str().unwrap();

    mv::call(&ctx, json!({"id": id, "new_rel_path": "docs/moved.md"}))
        .await
        .expect("the move, including its id repair, must complete");

    assert!(data.join("proj/docs/moved.md").is_file());
}

/// A project with no `.git` anywhere above it had NO containment at the door (86 live rows in
/// the reviewer's catalog): with nothing to compare against, only the leaf rule applied. The
/// managed root is the fallback.
#[tokio::test]
async fn a_project_without_a_git_dir_is_still_confined_at_the_door() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("x.md"), DOC).unwrap();
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("linkdir")).unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &root.path().join("linkdir/x.md"));

    let err = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("no .git does not mean no boundary");

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

/// M1: `R/sub` is a link to ANOTHER repository `S` whose artifact declares an EXISTING sidecar.
/// The preflight resolved the sidecar against `lookup_git_root(artifact)` (= `R/sub`, the link
/// followed to `S`) and passed; the catalog row was then upserted; the door, using another root,
/// refused, and the response said "Nothing was changed". Preflight and door now share one root.
#[tokio::test]
async fn doc_augment_refuses_before_the_upsert_when_the_artifact_lives_behind_a_link_to_another_repo(
) {
    let base = tempfile::tempdir().unwrap();
    let r = base.path().join("R");
    let s = base.path().join("S");
    std::fs::create_dir_all(r.join(".git")).unwrap();
    std::fs::create_dir_all(s.join(".git")).unwrap();
    std::fs::create_dir_all(s.join("docs/aug")).unwrap();
    std::fs::write(s.join("docs/aug/t.yaml"), "prompt: OLD\n").unwrap();
    std::fs::write(
        s.join("t.md"),
        "---\nkind: tracker\nstatus: active\nexpects_augmentation: docs/aug/t.yaml\n---\n\n# T\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(&s, r.join("sub")).unwrap();
    let ctx = mk_ctx(&r);
    let id = catalog_row(&ctx, &r.join("sub/t.md"));

    let err = augment::call(&ctx, json!({"id": id, "prompt": "NEW-PROMPT"}))
        .await
        .expect_err("the sidecar resolves into another repository");

    assert!(
        err.downcast_ref::<LibrarianRecoverableError>().is_some(),
        "{err:?}"
    );
    assert!(
        augmentation::get(&ctx.catalog.lock(), &id)
            .unwrap()
            .is_none(),
        "the refusal must come BEFORE the catalog moves, or its text must say what changed"
    );
    assert_eq!(
        std::fs::read_to_string(s.join("docs/aug/t.yaml")).unwrap(),
        "prompt: OLD\n"
    );
}

/// M2: rows a pre-round-2 `force_include` catalogued through an outward directory link are
/// reclaimed on the first reindex after the upgrade, with augmentation params that exist nowhere
/// else. A row carrying an augmentation or events is now KEPT and reported; one without is
/// reclaimed and reported (`reclaimed_escaping`), so the deletion has a stated reason.
#[test]
fn reclaiming_an_escaping_row_is_reported_and_spares_one_that_carries_history() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("plain.md"), DOC).unwrap();
    std::fs::write(outside.path().join("rich.md"), DOC).unwrap();
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs")).unwrap();
    let ctx = mk_ctx(root.path());
    let plain = catalog_row(&ctx, &root.path().join("docs/plain.md"));
    let rich = catalog_row(&ctx, &root.path().join("docs/rich.md"));
    augmentation::upsert(&ctx.catalog.lock(), &aug_row(&rich)).unwrap();

    let report = index(&ctx, root.path());

    assert!(artifact::get(&ctx.catalog.lock(), &plain)
        .unwrap()
        .is_none());
    assert_eq!(report.reclaimed_escaping, 1);
    assert_eq!(report.reclaimed_escaping_ids, vec![plain.clone()]);
    assert!(
        artifact::get(&ctx.catalog.lock(), &rich).unwrap().is_some(),
        "history that exists nowhere else is not deleted for a policy reason"
    );
    assert!(augmentation::get(&ctx.catalog.lock(), &rich)
        .unwrap()
        .is_some());
    assert_eq!(report.escaping_kept_ids, vec![rich]);
}

/// LOW 1: a LEAF link inside an outward `force_include` anchor was catalogued (the walk's escape
/// test exempted symlinks), and the row was then stuck: `doc(delete)` is refused because the
/// parent escapes. The test is on the PARENT now, for every kind of leaf.
#[test]
fn force_include_does_not_catalog_a_leaf_link_inside_an_anchor_that_leaves_the_repo() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("real.md"), DOC).unwrap();
    std::os::unix::fs::symlink(outside.path().join("real.md"), outside.path().join("l.md"))
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs/trackers")).unwrap();
    force_include(root.path(), "docs/trackers/**");
    let ctx = mk_ctx(root.path());

    let report = index(&ctx, root.path());

    let id = crate::librarian::ids::artifact_id_from_abs(&root.path().join("docs/trackers/l.md"));
    assert!(artifact::get(&ctx.catalog.lock(), &id).unwrap().is_none());
    assert_eq!(report.added, 0);
}

/// LOW 2: `rename` moves a link without retargeting it, so a RELATIVE link moved to another depth
/// dangles; the id repair's read then failed with ENOENT and left the move half done. Refused
/// up front. The control (an absolute link) is `doc_move_of_a_catalogued_outward_link_...`.
#[tokio::test]
async fn doc_move_refuses_a_relative_leaf_link_that_would_dangle_at_the_new_depth() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(base.path().join("elsewhere.md"), DOC).unwrap();
    std::os::unix::fs::symlink("../elsewhere.md", root.join("outward.md")).unwrap();
    let ctx = mk_ctx(&root);
    let (report, _) = crate::librarian::indexer::index_repo_sync(
        &ctx.catalog.lock(),
        &md_rules(),
        &root,
        &globset::GlobSet::empty(),
        false,
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        report.added, 1,
        "precondition: the relative outward link is catalogued"
    );
    let id = crate::librarian::ids::artifact_id_from_abs(&root.join("outward.md"));

    let err = mv::call(
        &ctx,
        json!({"id": id, "new_rel_path": "docs/deeper/moved.md"}),
    )
    .await
    .expect_err("the link would dangle at the new depth");

    assert!(msg(&err).contains("RELATIVE target"), "{}", msg(&err));
    assert!(
        std::fs::symlink_metadata(root.join("outward.md")).is_ok(),
        "nothing was moved"
    );
}

/// LOW 5: the export's own containment call was load-bearing only where the write door has no
/// root to compare against, which this fixture is (no `.git`). And a row BELOW an outward link
/// directory wrote its sidecar and then failed to stamp the declaration; refused up front.
#[tokio::test]
async fn doctor_export_confines_a_project_with_no_git_dir_and_a_row_below_a_link_directory() {
    let outside = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs/augmentations")).unwrap();
    let art = root.path().join("t.md");
    std::fs::write(&art, "---\nkind: tracker\nstatus: active\n---\n\n# T\n").unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &art);
    augmentation::upsert(&ctx.catalog.lock(), &aug_row(&id)).unwrap();
    let root_s = crate::util::fs::RepoPath::from_path(root.path()).into_string();

    let applied = doctor::call(
        &ctx,
        json!({"fix": "export_augmentations", "root": root_s, "confirm": true}),
    )
    .await
    .unwrap();

    assert!(
        std::fs::read_dir(outside.path()).unwrap().next().is_none(),
        "{applied}"
    );
    assert_eq!(applied["totals"]["exported"], 0, "{applied}");
}

/// The other half: an ordinary `docs/augmentations`, but the ARTIFACT sits below a link directory
/// out of the repository. The sidecar is written inside the repo and the stamp then lands in the
/// OUTSIDE file (no `.git`, so the write door has nothing to compare against), or, with a
/// `.git`, the stamp is refused AFTER the sidecar was written and an orphan is left.
#[tokio::test]
async fn doctor_export_refuses_an_artifact_below_a_link_directory_before_writing_its_sidecar() {
    let linked = tempfile::tempdir().unwrap();
    let outside_art = "---\nkind: tracker\nstatus: active\n---\n\n# U\n";
    std::fs::write(linked.path().join("u.md"), outside_art).unwrap();
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(linked.path(), root.path().join("linkdir")).unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &root.path().join("linkdir/u.md"));
    augmentation::upsert(&ctx.catalog.lock(), &aug_row(&id)).unwrap();
    let root_s = crate::util::fs::RepoPath::from_path(root.path()).into_string();

    let applied = doctor::call(
        &ctx,
        json!({"fix": "export_augmentations", "root": root_s, "confirm": true}),
    )
    .await
    .unwrap();

    assert_eq!(
        std::fs::read_to_string(linked.path().join("u.md")).unwrap(),
        outside_art
    );
    assert!(
        !root.path().join("docs/augmentations").exists(),
        "no orphan sidecar for an artifact that cannot be stamped: {applied}"
    );
    assert_eq!(applied["totals"]["exported"], 0, "{applied}");
}

/// The managed root is registered under its CANONICAL path (`current_project` is canonicalized at
/// the adapter boundary) while a row keeps the SPELLING it was catalogued under
/// (`home/proj/x.md`), so no root contains the spelling. Before falling back to the nearest `.git`
/// (`home`, which would refuse), the door accepts a path whose RESOLUTION lies under a managed
/// root.
#[tokio::test]
async fn a_row_spelled_through_a_link_to_a_canonically_registered_project_is_writable() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let data = base.path().join("data");
    std::fs::create_dir_all(home.join(".git")).unwrap();
    std::fs::create_dir_all(data.join("proj/.git")).unwrap();
    std::os::unix::fs::symlink(data.join("proj"), home.join("proj")).unwrap();
    std::fs::write(data.join("proj/x.md"), DOC).unwrap();
    let canonical = std::fs::canonicalize(data.join("proj")).unwrap();
    let ctx = mk_ctx(&canonical);
    let id = catalog_row(&ctx, &home.join("proj/x.md"));

    update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect("the resolution lies inside the managed project");

    assert!(std::fs::read_to_string(data.join("proj/x.md"))
        .unwrap()
        .contains("status: fixed"));
}

/// The catalog-level writers get their root from `Catalog::write_roots`, armed by the TOOL. A
/// prose-ledger `append_entry` in a project reached through a link under another repository is
/// the H3 shape at that wiring: unarmed, the door falls back to the spelled `home/.git` and
/// refuses the project's own ledger.
#[tokio::test]
async fn append_entry_in_a_project_reached_through_a_link_allocates_an_id() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let data = base.path().join("data");
    std::fs::create_dir_all(home.join(".git")).unwrap();
    std::fs::create_dir_all(data.join("proj/.git")).unwrap();
    std::os::unix::fs::symlink(data.join("proj"), home.join("proj")).unwrap();
    let project = home.join("proj");
    std::fs::write(
        project.join("ledger.md"),
        "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n",
    )
    .unwrap();
    let ctx = mk_ctx(&project);
    let id = catalog_row(&ctx, &project.join("ledger.md"));

    let out = super::append_entry::call(&ctx, json!({"id": id, "id_prefix": "F"}))
        .await
        .expect("the project's own ledger is not 'outside the repository'");

    assert!(out.to_string().contains("F-1"), "{out}");
}

/// The control for the relative-link refusal: moving the link within its OWN directory keeps the
/// relative target valid, so it is allowed (the test `doc_move_refuses_a_relative_leaf_link…` is
/// for a different directory).
#[tokio::test]
async fn doc_move_still_renames_a_relative_leaf_link_within_its_own_directory() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(base.path().join("elsewhere.md"), DOC).unwrap();
    std::os::unix::fs::symlink("../elsewhere.md", root.join("outward.md")).unwrap();
    let ctx = mk_ctx(&root);
    let (report, _) = crate::librarian::indexer::index_repo_sync(
        &ctx.catalog.lock(),
        &md_rules(),
        &root,
        &globset::GlobSet::empty(),
        false,
        false,
        false,
    )
    .unwrap();
    assert_eq!(report.added, 1);
    let id = crate::librarian::ids::artifact_id_from_abs(&root.join("outward.md"));

    mv::call(&ctx, json!({"id": id, "new_rel_path": "renamed.md"}))
        .await
        .expect("a rename inside the same directory leaves the relative target valid");

    assert!(std::fs::read_to_string(root.join("renamed.md")).is_ok());
}

// ---- the arming of the catalog-level writers, in the H3 shape ---------------------------------
//
// A project reached through a link under another repository (`home/proj -> data/proj`, `home/.git`)
// is the input on which the spelled-`.git` root is wrong and the managed roots are right. Each
// tool that reaches a writer without a `ToolContext` must arm `Catalog::write_roots`; these are
// the cases that fail when it does not.

fn linked_project() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let data = base.path().join("data");
    std::fs::create_dir_all(home.join(".git")).unwrap();
    std::fs::create_dir_all(data.join("proj/.git")).unwrap();
    std::os::unix::fs::symlink(data.join("proj"), home.join("proj")).unwrap();
    let project = home.join("proj");
    (base, project, data.join("proj"))
}

#[tokio::test]
async fn doc_augment_publishes_a_sidecar_in_a_project_reached_through_a_link() {
    let (_base, project, real) = linked_project();
    std::fs::create_dir_all(project.join("docs/aug")).unwrap();
    std::fs::write(project.join("docs/aug/t.yaml"), "prompt: OLD\n").unwrap();
    std::fs::write(
        project.join("t.md"),
        "---\nkind: tracker\nstatus: active\nexpects_augmentation: docs/aug/t.yaml\n---\n\n# T\n",
    )
    .unwrap();
    let ctx = mk_ctx(&project);
    let id = catalog_row(&ctx, &project.join("t.md"));

    augment::call(&ctx, json!({"id": id, "prompt": "NEW-PROMPT"}))
        .await
        .expect("the sidecar is inside the managed project");

    let on_disk = std::fs::read_to_string(real.join("docs/aug/t.yaml")).unwrap();
    assert!(on_disk.contains("NEW-PROMPT"), "{on_disk}");
}

#[tokio::test]
async fn a_status_change_event_writes_in_a_project_reached_through_a_link() {
    let (_base, project, real) = linked_project();
    std::fs::write(project.join("x.md"), DOC).unwrap();
    let ctx = mk_ctx(&project);
    let id = catalog_row(&ctx, &project.join("x.md"));

    super::event_create::call(
        &ctx,
        json!({"artifact_id": id, "kind": "status_change", "payload": {"to": "fixed"}}),
    )
    .await
    .expect("the frontmatter writer judges against the managed project");

    assert!(std::fs::read_to_string(real.join("x.md"))
        .unwrap()
        .contains("status: fixed"));
}

#[tokio::test]
async fn rekey_prefix_rewrites_a_ledger_in_a_project_reached_through_a_link() {
    let (_base, project, real) = linked_project();
    std::fs::write(
        project.join("ledger.md"),
        "---\nkind: tracker\nentry_prefix: T\n---\n\n## T-1 — a\n\nsee T-1\n",
    )
    .unwrap();
    let ctx = mk_ctx(&project);
    let id = catalog_row(&ctx, &project.join("ledger.md"));

    super::rekey_prefix::call(
        &ctx,
        json!({"id": id, "from": "T", "to": "SRI", "force": true}),
    )
    .await
    .expect("the ledger body rewrite judges against the managed project");

    let body = std::fs::read_to_string(real.join("ledger.md")).unwrap();
    assert!(body.contains("## SRI-1"), "{body}");
}

fn aug_with_collection(id: &str, collection: &str, params: &str) -> augmentation::AugmentationRow {
    let mut a = aug_row(id);
    a.entry_collection = Some(collection.to_string());
    a.params = params.to_string();
    a
}

#[tokio::test]
async fn update_entry_resyncs_a_snapshot_row_in_a_project_reached_through_a_link() {
    let (_base, project, real) = linked_project();
    std::fs::write(
        project.join("queue.md"),
        "---\nkind: tracker\nsnapshot_anchor: '| ID | status |'\n---\n\n# Q\n\n| ID | status |\n| T-1 | open |\n\ntail\n",
    )
    .unwrap();
    let ctx = mk_ctx(&project);
    let id = catalog_row(&ctx, &project.join("queue.md"));
    let mut a = aug_with_collection(&id, "tasks", r#"{"tasks":[{"id":"T-1","status":"open"}]}"#);
    a.render_template = Some(
        "| ID | status |\n{% for t in tasks %}| {{ t.id }} | {{ t.status }} |\n{% endfor %}"
            .to_string(),
    );
    augmentation::upsert(&ctx.catalog.lock(), &a).unwrap();

    super::update_entry::call(
        &ctx,
        json!({"id": id, "entry_collection": "tasks", "entry_id": "T-1",
               "fields": {"status": "done"}}),
    )
    .await
    .expect("the snapshot resync judges against the managed project");

    let text = std::fs::read_to_string(real.join("queue.md")).unwrap();
    assert!(text.contains("| T-1 | done |"), "{text}");
}

#[tokio::test]
async fn append_entry_writes_its_section_in_a_project_reached_through_a_link() {
    let (_base, project, real) = linked_project();
    std::fs::write(
        project.join("ledger.md"),
        "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n## Template for new entries\n\nboilerplate\n",
    )
    .unwrap();
    let ctx = mk_ctx(&project);
    let id = catalog_row(&ctx, &project.join("ledger.md"));
    augmentation::upsert(
        &ctx.catalog.lock(),
        &aug_with_collection(&id, "failures", r#"{"failures":[]}"#),
    )
    .unwrap();

    super::append_entry::call(
        &ctx,
        json!({"id": id, "entry_collection": "failures", "id_prefix": "F", "entry": {},
               "anchor_heading": "## Template for new entries", "title": "first",
               "body": "the prose"}),
    )
    .await
    .expect("the params path's section write judges against the managed project");

    let text = std::fs::read_to_string(real.join("ledger.md")).unwrap();
    assert!(text.contains("F-1 — first"), "{text}");
}

// ======================= round 4 =============================================================

fn cp(path: &Path) -> std::sync::Arc<crate::librarian::current_project::CurrentProject> {
    std::sync::Arc::new(crate::librarian::current_project::CurrentProject {
        abs_path: path.to_path_buf(),
        git_root: path.to_path_buf(),
        main_root: None,
        umbrella: None,
    })
}

/// `U` holds two repositories A and B (a legacy `[[roots]]` entry can hold several), and
/// `A/linkdir -> B/docs`. Returns (u, a, b).
fn umbrella() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let u = tempfile::tempdir().unwrap();
    let a = u.path().join("A");
    let b = u.path().join("B");
    std::fs::create_dir_all(a.join(".git")).unwrap();
    std::fs::create_dir_all(b.join(".git")).unwrap();
    std::fs::create_dir_all(b.join("docs")).unwrap();
    std::fs::write(b.join("docs/x.md"), DOC).unwrap();
    std::os::unix::fs::symlink(b.join("docs"), a.join("linkdir")).unwrap();
    (u, a, b)
}

/// P1: a legacy umbrella root plus an UNRELATED active project. The first managed root that contains
/// the spelling is the umbrella, and judged strictly against it a link from repository A into
/// ANOTHER repository B stays "inside". Round 2 judged against the spelled repository A and refused.
/// The path is now judged against the DEEPER of the confinement root and the spelled repository.
#[tokio::test]
async fn an_umbrella_root_does_not_admit_a_link_into_another_repository() {
    let (u, a, b) = umbrella();
    let c = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(c.path().join(".git")).unwrap();
    let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_root(Root {
            name: "u".into(),
            path: u.path().to_path_buf(),
        })
        .with_current_project(cp(c.path()))
        .build();
    let id = catalog_row(&ctx, &a.join("linkdir/x.md"));

    let err = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("a link from A to ANOTHER repository B is outside A");

    assert!(
        msg(&err).contains("outside the repository root"),
        "{}",
        msg(&err)
    );
    assert_eq!(std::fs::read_to_string(b.join("docs/x.md")).unwrap(), DOC);
}

/// The control of P1: the same layout with A as the active project was already refused.
#[tokio::test]
async fn with_the_owning_repository_active_a_link_into_another_one_is_refused() {
    let (u, a, b) = umbrella();
    let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_root(Root {
            name: "u".into(),
            path: u.path().to_path_buf(),
        })
        .with_current_project(cp(&a))
        .build();
    let id = catalog_row(&ctx, &a.join("linkdir/x.md"));

    update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("refused");

    assert_eq!(std::fs::read_to_string(b.join("docs/x.md")).unwrap(), DOC);
}

/// A repository INSIDE the managed root that links to a place still inside the managed root but
/// outside that repository: judged against the deeper spelled repository, refused; the control is
/// a link that stays inside the nested repository.
#[tokio::test]
async fn a_nested_repository_is_judged_against_itself_not_against_the_umbrella() {
    let (u, a, b) = umbrella();
    std::fs::create_dir_all(a.join("real")).unwrap();
    std::fs::write(a.join("real/y.md"), DOC).unwrap();
    std::os::unix::fs::symlink(a.join("real"), a.join("inner")).unwrap();
    let ctx = mk_ctx(u.path());
    let outside = catalog_row(&ctx, &a.join("linkdir/x.md"));
    let inside = catalog_row(&ctx, &a.join("inner/y.md"));

    update::call(&ctx, json!({"id": outside, "patch": {"status": "fixed"}}))
        .await
        .expect_err("B is another repository");
    update::call(&ctx, json!({"id": inside, "patch": {"status": "fixed"}}))
        .await
        .expect("a link that stays inside its own repository is fine");

    assert_eq!(std::fs::read_to_string(b.join("docs/x.md")).unwrap(), DOC);
}

/// P2: a FOREIGN repository Q (not managed) has `Q/linkdir -> P/docs`, P being the managed
/// project. The resolution lies inside P, so the old "resolution under any managed root" shortcut
/// admitted Q's row and the write landed in P's file under Q's id.
#[tokio::test]
async fn a_foreign_repositorys_row_resolving_into_the_managed_project_is_refused() {
    let p = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(p.path().join(".git")).unwrap();
    std::fs::create_dir_all(p.path().join("docs")).unwrap();
    std::fs::write(p.path().join("docs/x.md"), DOC).unwrap();
    let q = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(q.path().join(".git")).unwrap();
    std::os::unix::fs::symlink(p.path().join("docs"), q.path().join("linkdir")).unwrap();
    let ctx = mk_ctx(p.path());
    let id = catalog_row(&ctx, &q.path().join("linkdir/x.md"));

    update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("Q's row below a link out of Q is outside Q");

    assert_eq!(
        std::fs::read_to_string(p.path().join("docs/x.md")).unwrap(),
        DOC
    );
}

/// P3 (H-1): the managed root is registered CANONICALLY (`data/proj`), rows are spelled through a
/// link to it (`home/proj/...`), no `.git` sits above the link, and INSIDE the project `linkdir`
/// links to an outside directory. No root contains the spelling; the old resolution shortcut could
/// only admit; the spelled walk skips both links and finds no `.git`, so nothing was checked and
/// the write landed OUTSIDE every managed root. An ancestor of the spelling that canonicalizes to
/// a managed root is now the root, strictly.
#[tokio::test]
async fn a_canonically_registered_project_reached_through_a_link_still_refuses_an_outward_dir_link()
{
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let data = base.path().join("data");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(data.join("proj/.git")).unwrap();
    std::os::unix::fs::symlink(data.join("proj"), home.join("proj")).unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("x.md"), DOC).unwrap();
    std::os::unix::fs::symlink(outside.path(), data.join("proj/linkdir")).unwrap();
    let canonical = std::fs::canonicalize(data.join("proj")).unwrap();
    let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_current_project(cp(&canonical))
        .build();
    let id = catalog_row(&ctx, &home.join("proj/linkdir/x.md"));

    let err = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("the resolution leaves every managed root");

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

/// P4: H1 re-derived with `%`, `_` and `'` in the walk root, plus case-only and wildcard siblings.
#[test]
fn a_walk_root_with_like_metacharacters_reclaims_its_own_rows_and_spares_siblings() {
    let b = tempfile::tempdir().unwrap();
    let root = b.path().join("x%_'y");
    let siblings = ["xAB'y", "xA'y", "X%_'Y", "x%_'Y"];
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::write(root.join("docs/n.md"), DOC).unwrap();
    for s in siblings {
        std::fs::create_dir_all(b.path().join(s).join("docs")).unwrap();
        std::fs::write(b.path().join(s).join("docs/n.md"), DOC).unwrap();
    }
    let ctx = mk_ctx(&root);
    let sib: Vec<String> = siblings
        .iter()
        .map(|s| catalog_row(&ctx, &b.path().join(s).join("docs/n.md")))
        .collect();
    assert_eq!(index(&ctx, &root).added, 1);
    std::fs::remove_file(root.join("docs/n.md")).unwrap();

    let report = index(&ctx, &root);

    let survivors = sib
        .iter()
        .filter(|id| artifact::get(&ctx.catalog.lock(), id).unwrap().is_some())
        .count();
    assert_eq!(report.removed, 1);
    assert_eq!(survivors, sib.len());
}

/// L-5: the vectorless count matched case-only siblings (ESCAPE was added, the filter was not).
/// The walked root's row is queued, so it owns chunk rows; the sibling's row owns none and must not
/// be counted as this root's hole.
#[test]
fn the_vectorless_count_ignores_a_case_only_sibling() {
    let b = tempfile::tempdir().unwrap();
    let ctx = mk_ctx(&b.path().join("my_app"));
    for name in ["my_app", "MY_APP"] {
        std::fs::create_dir_all(b.path().join(name).join("docs")).unwrap();
        std::fs::write(b.path().join(name).join("docs/n.md"), DOC).unwrap();
    }
    catalog_row(&ctx, &b.path().join("MY_APP").join("docs/n.md"));

    let (report, _) = crate::librarian::indexer::index_repo_sync(
        &ctx.catalog.lock(),
        &md_rules(),
        &b.path().join("my_app"),
        &globset::GlobSet::empty(),
        true,
        false,
        false,
    )
    .unwrap();

    assert_eq!(report.vectorless, Some(0), "{report:?}");
}

/// P6 / L-1: the armed roots OUTLIVED the call that armed them, so a catalog-level writer reached
/// without arming inherited another call's roots instead of falling back to the spelled repository
/// as the field's doc says.
///
/// The fixture needs an input whose verdict DIFFERS between armed and unarmed. The first version
/// (an umbrella) stopped differing once the door judged against the deeper root, so it went
/// vacuous and a mutant that never cleared the roots survived. Here the project is registered
/// canonically (`data/proj`) and reached through `home/proj`, and `home` holds a `.git`: unarmed,
/// the spelled repository is `home`, and the file resolves to `data/proj/...`, outside it
/// (refused); armed with the project's roots it is inside the project (admitted).
#[tokio::test]
async fn an_unarmed_writer_does_not_inherit_the_previous_calls_roots() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("home");
    let data = base.path().join("data");
    std::fs::create_dir_all(home.join(".git")).unwrap();
    std::fs::create_dir_all(data.join("proj/.git")).unwrap();
    std::fs::create_dir_all(data.join("proj/docs")).unwrap();
    let ledger = "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n";
    std::fs::write(data.join("proj/docs/ledger.md"), ledger).unwrap();
    std::os::unix::fs::symlink(data.join("proj"), home.join("proj")).unwrap();
    let canonical = std::fs::canonicalize(data.join("proj")).unwrap();
    let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
        .with_current_project(cp(&canonical))
        .build();
    let id = catalog_row(&ctx, &home.join("proj/docs/ledger.md"));
    let direct = |ctx: &ToolContext| {
        augmentation::allocate_entry_id(&mut ctx.catalog.lock(), &id, "F", None)
    };
    let unarmed = direct(&ctx).expect_err("unarmed: the spelled repository `home` refuses");
    assert!(
        msg(&unarmed).contains("outside the repository root"),
        "{}",
        msg(&unarmed)
    );

    // A tool that arms, even one that then fails, must not leave its roots behind.
    let _ = super::update_entry::call(
        &ctx,
        json!({"id": "0000000000000000", "entry_collection": "x", "entry_id": "X-1",
               "fields": {"a": 1}}),
    )
    .await;

    let after = direct(&ctx).expect_err("still unarmed: no inherited roots");
    assert!(
        msg(&after).contains("outside the repository root"),
        "{}",
        msg(&after)
    );
    assert_eq!(
        std::fs::read_to_string(data.join("proj/docs/ledger.md")).unwrap(),
        ledger
    );
}

/// P7: an ABSOLUTE-target leaf link moves to another directory fine; the outside target is
/// untouched.
#[tokio::test]
async fn doc_move_of_an_absolute_leaf_link_to_another_directory_completes() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("repo");
    std::fs::create_dir_all(root.join(".git")).unwrap();
    let target = base.path().join("elsewhere.md");
    let content = "---\nid: 0123456789abcdef\nkind: spec\nstatus: draft\ntitle: T\n---\n\nbody\n";
    std::fs::write(&target, content).unwrap();
    std::os::unix::fs::symlink(&target, root.join("outward.md")).unwrap();
    let ctx = mk_ctx(&root);
    let id = catalog_row(&ctx, &root.join("outward.md"));

    mv::call(&ctx, json!({"id": id, "new_rel_path": "sub/moved.md"}))
        .await
        .expect("an absolute target still resolves from any directory");

    assert_eq!(std::fs::read_to_string(&target).unwrap(), content);
    assert!(std::fs::symlink_metadata(root.join("sub/moved.md"))
        .unwrap()
        .file_type()
        .is_symlink());
}

/// L-4: `a/x.md -> ../real/x.md` moved to `b/x.md` (same depth): the relative target still
/// resolves to the same file, so the move is allowed. The refusal is for a link that would NOT
/// resolve to the same file.
#[tokio::test]
async fn doc_move_allows_a_relative_link_whose_target_still_resolves_to_the_same_file() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("repo");
    for d in [".git", "real", "a", "b"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(root.join("real/x.md"), DOC).unwrap();
    std::os::unix::fs::symlink("../real/x.md", root.join("a/x.md")).unwrap();
    let ctx = mk_ctx(&root);
    let id = catalog_row(&ctx, &root.join("a/x.md"));

    mv::call(&ctx, json!({"id": id, "new_rel_path": "b/x.md"}))
        .await
        .expect("same depth, same target");

    assert_eq!(std::fs::read_to_string(root.join("b/x.md")).unwrap(), DOC);
}

/// L-4 / M-E: a relative target that does not START with `..` (`real.md`, a sibling of the link)
/// dangles in any other directory, and must be refused; the first version tested the prefix.
#[tokio::test]
async fn doc_move_refuses_a_relative_link_whose_target_would_not_resolve_there() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("repo");
    for d in [".git", "a"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(root.join("a/real.md"), DOC).unwrap();
    std::os::unix::fs::symlink("real.md", root.join("a/x.md")).unwrap();
    let ctx = mk_ctx(&root);
    let id = catalog_row(&ctx, &root.join("a/x.md"));

    let err = mv::call(&ctx, json!({"id": id, "new_rel_path": "b/x.md"}))
        .await
        .expect_err("`real.md` would not exist next to b/x.md");

    assert!(msg(&err).contains("would not resolve"), "{}", msg(&err));
    assert!(
        std::fs::symlink_metadata(root.join("a/x.md")).is_ok(),
        "nothing moved"
    );
}

/// L-3 / M-F: a row below an escaping parent whose FILE is gone is an ordinary vanished row, not
/// an escaping one: it is reclaimed and counted as `removed`, never as `reclaimed_escaping`.
#[test]
fn a_vanished_row_below_an_escaping_parent_is_reclaimed_as_an_ordinary_vanished_row() {
    let outside = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs")).unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &root.path().join("docs/gone.md"));

    let report = index(&ctx, root.path());

    assert!(artifact::get(&ctx.catalog.lock(), &id).unwrap().is_none());
    assert_eq!(report.removed, 1);
    assert_eq!(
        report.reclaimed_escaping, 0,
        "the file is gone: that is not a policy reclaim"
    );
}

/// M-A: a row whose only history is EVENTS is kept as well.
#[tokio::test]
async fn an_escaping_row_whose_only_history_is_an_event_is_kept() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("e.md"), DOC).unwrap();
    let root = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs")).unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &root.path().join("docs/e.md"));
    super::event_create::call(
        &ctx,
        json!({"artifact_id": id, "kind": "note", "payload": {"text": "history"}}),
    )
    .await
    .unwrap();

    let report = index(&ctx, root.path());

    assert!(artifact::get(&ctx.catalog.lock(), &id).unwrap().is_some());
    assert_eq!(report.escaping_kept_ids, vec![id]);
    assert_eq!(report.reclaimed_escaping, 0);
}

/// L-6: `rekey_prefix`'s sidecar write ran AFTER its commit with no preflight, and its refusal
/// hint said "after removing the stale file", which for a refused sidecar points at the file
/// behind the link. The refusal now comes first, with nothing rewritten.
#[tokio::test]
async fn rekey_prefix_refuses_a_ledger_whose_declared_sidecar_leaves_the_repo_before_rewriting() {
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("t.yaml"), "prompt: OUTSIDE\n").unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join(".git")).unwrap();
    std::fs::create_dir_all(root.path().join("docs")).unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("docs/aug")).unwrap();
    let ledger = "---\nkind: tracker\nentry_prefix: T\nexpects_augmentation: docs/aug/t.yaml\n---\n\n## T-1 — a\n\nsee T-1\n";
    std::fs::write(root.path().join("ledger.md"), ledger).unwrap();
    let ctx = mk_ctx(root.path());
    let id = catalog_row(&ctx, &root.path().join("ledger.md"));
    augmentation::upsert(&ctx.catalog.lock(), &aug_row(&id)).unwrap();

    let err = super::rekey_prefix::call(
        &ctx,
        json!({"id": id, "from": "T", "to": "SRI", "force": true}),
    )
    .await
    .expect_err("the sidecar publication would be refused");

    assert!(
        msg(&err).contains("outside the repository root"),
        "{}",
        msg(&err)
    );
    assert!(
        !msg(&err).contains("removing the stale file"),
        "no advice to delete a file behind a link: {}",
        msg(&err)
    );
    assert_eq!(
        std::fs::read_to_string(root.path().join("ledger.md")).unwrap(),
        ledger
    );
    assert_eq!(
        std::fs::read_to_string(outside.path().join("t.yaml")).unwrap(),
        "prompt: OUTSIDE\n"
    );
}

// ======================= round 5: the door's layout table =======================================
//
// The door (`util::door_root`) moved in each of rounds 2-4 because each round's review found a
// layout the last rule answered wrongly. This table is the permanent statement of every answer,
// so a fifth change that breaks one is red here rather than in a review. Each row names a layout
// and its expected verdict; "ADMIT" means the write door accepts the path, "REFUSE" that it is
// outside. The same layouts also run through the DIRECTORY door that `doc(move)` (source) and
// `doc(delete)` use, and the two must agree: one decision, three verbs.
mod door_table {
    use super::*;
    use std::path::PathBuf;

    fn git(p: &Path) {
        std::fs::create_dir_all(p.join(".git")).unwrap();
    }
    fn ln(target: impl AsRef<Path>, link: &Path) {
        std::fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, link).unwrap();
    }
    fn file(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, DOC).unwrap();
    }
    fn canon(p: &Path) -> PathBuf {
        std::fs::canonicalize(p).unwrap()
    }

    /// (name, expected admit, file-door admit, directory-door admit on the parent)
    struct Rows(Vec<(String, bool, bool, bool)>);

    impl Rows {
        fn row(&mut self, name: &str, expect_admit: bool, roots: &[PathBuf], p: &Path) {
            let file =
                crate::librarian::util::refuse_catalogued_path_outside_repo(roots, p, "write")
                    .is_ok();
            let dir = crate::librarian::util::refuse_catalogued_dir_outside_repo(
                roots,
                p.parent().unwrap(),
                "delete from",
            )
            .is_ok();
            self.0.push((name.to_string(), expect_admit, file, dir));
        }
    }

    #[test]
    fn door_layout_table() {
        let mut t = Rows(Vec::new());

        // ---- A: umbrella U{A,B}; C an unrelated active project.
        let ut = tempfile::tempdir().unwrap();
        let u = canon(ut.path());
        let (a, b) = (u.join("A"), u.join("B"));
        git(&a);
        git(&b);
        file(&b.join("docs/x.md"));
        file(&a.join("real/y.md"));
        file(&a.join("docs/z.md"));
        ln(b.join("docs"), &a.join("linkdir"));
        ln(&u, &a.join("up"));
        ln("..", &a.join("up2"));
        ln(a.join("real"), &a.join("inlink"));
        let c = tempfile::tempdir().unwrap();
        git(c.path());
        let roots_c = vec![c.path().to_path_buf(), u.clone()];
        let roots_a = vec![a.clone(), u.clone()];
        t.row(
            "A1 umbrella, C active, A/linkdir -> B/docs",
            false,
            &roots_c,
            &a.join("linkdir/x.md"),
        );
        t.row(
            "A2 A active, A/up -> U (absolute), row A/up/B/docs/x.md",
            false,
            &roots_a,
            &a.join("up/B/docs/x.md"),
        );
        t.row(
            "A3 C active, same row as A2",
            false,
            &roots_c,
            &a.join("up/B/docs/x.md"),
        );
        t.row(
            "A4 A active, A/up2 -> .. (relative), row A/up2/B/docs/x.md",
            false,
            &roots_a,
            &a.join("up2/B/docs/x.md"),
        );
        t.row(
            "A5 control: A/inlink -> A/real",
            true,
            &roots_a,
            &a.join("inlink/y.md"),
        );
        t.row(
            "A6 control: plain row, C active",
            true,
            &roots_c,
            &a.join("docs/z.md"),
        );

        // ---- B: no umbrella; W holds sibling repos P (active) and S; P links up and to `/`.
        let wt0 = tempfile::tempdir().unwrap();
        let w = canon(wt0.path());
        let (pp, s) = (w.join("P"), w.join("S"));
        git(&pp);
        git(&s);
        file(&s.join("docs/x.md"));
        ln(&w, &pp.join("up"));
        ln("/", &pp.join("rootlink"));
        let roots_p = vec![pp.clone()];
        t.row(
            "B1 P active, P/up -> W, row P/up/S/docs/x.md (S a sibling repo)",
            false,
            &roots_p,
            &pp.join("up/S/docs/x.md"),
        );
        let via_root = pp
            .join("rootlink")
            .join(s.strip_prefix("/").unwrap())
            .join("docs/x.md");
        t.row(
            "B2 P active, P/rootlink -> /, row reaching sibling repo S",
            false,
            &roots_p,
            &via_root,
        );

        // ---- C: project registered canonically (data/proj), rows spelled through home/proj.
        let bt = tempfile::tempdir().unwrap();
        let base = canon(bt.path());
        let (home, data) = (base.join("home"), base.join("data"));
        std::fs::create_dir_all(&home).unwrap();
        git(&data.join("proj"));
        git(&data.join("other"));
        file(&data.join("other/x.md"));
        file(&data.join("proj/docs/x.md"));
        file(&data.join("proj/real/y.md"));
        ln(data.join("proj"), &home.join("proj"));
        let outside = tempfile::tempdir().unwrap();
        file(&outside.path().join("x.md"));
        ln(outside.path(), &data.join("proj/linkdir"));
        ln(data.join("proj/real"), &data.join("proj/inlink"));
        ln(&data, &data.join("proj/up"));
        let m2 = base.join("m2");
        std::fs::create_dir_all(&m2).unwrap();
        file(&m2.join("x.md"));
        ln(&m2, &data.join("proj/tom2"));
        // Two ancestors of one spelling resolving to the same root, and a nested repository
        // whose link goes back into the project: the shallowest ancestor is the anchor.
        ln(data.join("proj"), &data.join("proj/self"));
        git(&data.join("proj/sub"));
        file(&data.join("proj/sub/docs/y.md"));
        ln(data.join("proj/docs"), &data.join("proj/sub/linkdir"));
        let cproj = data.join("proj");
        let roots_cp = vec![cproj.clone()];
        let roots_cp_m2 = vec![cproj.clone(), m2.clone()];
        let roots_m2_first = vec![m2.clone(), cproj.clone()];
        t.row(
            "C1 canonical P, plain row via home/proj",
            true,
            &roots_cp,
            &home.join("proj/docs/x.md"),
        );
        t.row(
            "C2 canonical P, outward linkdir via home/proj",
            false,
            &roots_cp,
            &home.join("proj/linkdir/x.md"),
        );
        t.row(
            "C3 canonical P, in-repo link via home/proj",
            true,
            &roots_cp,
            &home.join("proj/inlink/y.md"),
        );
        t.row(
            "C4 canonical P, P/up -> data, row via home/proj/up/other (sibling repo)",
            false,
            &roots_cp,
            &home.join("proj/up/other/x.md"),
        );
        t.row(
            "C5 canonical P then m2, P/tom2 -> m2, spelled home/proj/tom2",
            false,
            &roots_cp_m2,
            &home.join("proj/tom2/x.md"),
        );
        t.row(
            "C5b same layout, spelled data/proj/tom2",
            false,
            &roots_cp_m2,
            &cproj.join("tom2/x.md"),
        );
        t.row(
            "C5c m2 listed FIRST, spelled home/proj/tom2: the root is m2",
            true,
            &roots_m2_first,
            &home.join("proj/tom2/x.md"),
        );
        t.row(
            "C5d m2 listed FIRST, spelled data/proj/tom2: same verdict as C5c",
            true,
            &roots_m2_first,
            &cproj.join("tom2/x.md"),
        );
        t.row("C8 two resolving ancestors (home/proj, home/proj/self): shallowest anchors, the nested repo is behind a link", true, &roots_cp, &home.join("proj/self/sub/linkdir/x.md"));
        t.row(
            "C9 same nested repo reached without the extra link: its link back into P is refused",
            false,
            &roots_cp,
            &home.join("proj/sub/linkdir/x.md"),
        );
        t.row(
            "C9b same, canonical spelling",
            false,
            &roots_cp,
            &cproj.join("sub/linkdir/x.md"),
        );
        git(&home);
        t.row(
            "C6 home/.git above the link, plain row via home/proj",
            true,
            &roots_cp,
            &home.join("proj/docs/x.md"),
        );
        t.row(
            "C7 home/.git above the link, outward linkdir",
            false,
            &roots_cp,
            &home.join("proj/linkdir/x.md"),
        );

        // ---- D: a home directory under git, a project without .git below it.
        let ht = tempfile::tempdir().unwrap();
        let h = canon(ht.path());
        git(&h);
        let dp = h.join("work/p");
        file(&dp.join("docs/x.md"));
        file(&h.join("other/x.md"));
        ln(h.join("other"), &dp.join("tohome"));
        let out2 = tempfile::tempdir().unwrap();
        file(&out2.path().join("x.md"));
        ln(out2.path(), &dp.join("linkdir"));
        let roots_d = vec![h.clone(), dp.clone()];
        let roots_d_only = vec![dp.clone()];
        t.row(
            "D1 home/.git, p without .git, plain",
            true,
            &roots_d,
            &dp.join("docs/x.md"),
        );
        t.row(
            "D2 home is the first root, p/tohome -> home/other (inside home)",
            true,
            &roots_d,
            &dp.join("tohome/x.md"),
        );
        t.row(
            "D2b only p is managed, same link (outside p)",
            false,
            &roots_d_only,
            &dp.join("tohome/x.md"),
        );
        t.row(
            "D3 home/.git, p/linkdir -> outside",
            false,
            &roots_d,
            &dp.join("linkdir/x.md"),
        );

        // ---- E: the managed root is itself a link.
        let et = tempfile::tempdir().unwrap();
        let eb = canon(et.path());
        git(&eb.join("data2/proj"));
        file(&eb.join("data2/proj/docs/x.md"));
        ln(eb.join("data2/proj"), &eb.join("l/proj"));
        let out3 = tempfile::tempdir().unwrap();
        file(&out3.path().join("x.md"));
        ln(out3.path(), &eb.join("data2/proj/linkdir"));
        let roots_e = vec![eb.join("l/proj")];
        t.row(
            "E1 root is a link, row spelled through it",
            true,
            &roots_e,
            &eb.join("l/proj/docs/x.md"),
        );
        t.row(
            "E2 root is a link, row canonical",
            true,
            &roots_e,
            &eb.join("data2/proj/docs/x.md"),
        );
        t.row(
            "E3 root is a link, outward link, spelled through it",
            false,
            &roots_e,
            &eb.join("l/proj/linkdir/x.md"),
        );
        t.row(
            "E4 root is a link, outward link, canonical",
            false,
            &roots_e,
            &eb.join("data2/proj/linkdir/x.md"),
        );

        // ---- F: nested repository, submodule (a .git FILE), linked-worktree root.
        let ft = tempfile::tempdir().unwrap();
        let fp = canon(ft.path());
        git(&fp);
        file(&fp.join("docs/x.md"));
        git(&fp.join("sub"));
        file(&fp.join("sub/docs/y.md"));
        ln(fp.join("docs"), &fp.join("sub/linkdir"));
        std::fs::create_dir_all(fp.join("sm")).unwrap();
        std::fs::write(fp.join("sm/.git"), "gitdir: ../.git/modules/sm\n").unwrap();
        file(&fp.join("sm/docs/y.md"));
        ln(fp.join("docs"), &fp.join("sm/linkdir"));
        ln(fp.join("sub/docs"), &fp.join("tosub"));
        let roots_f = vec![fp.clone()];
        t.row(
            "F1 nested repo, plain",
            true,
            &roots_f,
            &fp.join("sub/docs/y.md"),
        );
        t.row("F2 nested repo linking into its superproject (inside the managed root): refused by choice", false, &roots_f, &fp.join("sub/linkdir/x.md"));
        t.row(
            "F3 submodule (.git file), plain",
            true,
            &roots_f,
            &fp.join("sm/docs/y.md"),
        );
        t.row(
            "F4 submodule linking into its superproject: refused by choice",
            false,
            &roots_f,
            &fp.join("sm/linkdir/x.md"),
        );
        t.row(
            "F5 superproject link into the nested repo's docs",
            true,
            &roots_f,
            &fp.join("tosub/y.md"),
        );
        let wtt = tempfile::tempdir().unwrap();
        let wt = canon(wtt.path());
        std::fs::write(wt.join(".git"), "gitdir: /elsewhere/.git/worktrees/x\n").unwrap();
        file(&wt.join("real/y.md"));
        ln(wt.join("real"), &wt.join("inlink"));
        let out4 = tempfile::tempdir().unwrap();
        file(&out4.path().join("x.md"));
        ln(out4.path(), &wt.join("linkdir"));
        let roots_wt = vec![wt.clone()];
        t.row(
            "F6 linked-worktree root (.git file), in-repo link",
            true,
            &roots_wt,
            &wt.join("inlink/y.md"),
        );
        t.row(
            "F7 linked-worktree root, outward link",
            false,
            &roots_wt,
            &wt.join("linkdir/x.md"),
        );

        // ---- G: project without .git whose link goes up to a directory holding a repository.
        let gt = tempfile::tempdir().unwrap();
        let g = canon(gt.path());
        let gp = g.join("P");
        file(&gp.join("docs/x.md"));
        git(&g.join("S2"));
        file(&g.join("S2/x.md"));
        ln(&g, &gp.join("up"));
        let out5 = tempfile::tempdir().unwrap();
        file(&out5.path().join("x.md"));
        ln(out5.path(), &gp.join("linkdir"));
        let roots_g = vec![gp.clone()];
        t.row("G1 no .git, plain", true, &roots_g, &gp.join("docs/x.md"));
        t.row(
            "G2 no .git, outward link",
            false,
            &roots_g,
            &gp.join("linkdir/x.md"),
        );
        t.row(
            "G3 no .git, P/up -> parent, row P/up/S2/x.md (S2 a repo)",
            false,
            &roots_g,
            &gp.join("up/S2/x.md"),
        );

        // ---- H: a FOREIGN repository Q (not managed) linking into the managed project P.
        let hqt = tempfile::tempdir().unwrap();
        let hq = canon(hqt.path());
        let (hp, q) = (hq.join("P"), hq.join("Q"));
        git(&hp);
        git(&q);
        file(&hp.join("docs/x.md"));
        ln(&hp, &q.join("linkP"));
        ln(hp.join("docs"), &q.join("linkdocs"));
        let roots_h = vec![hp.clone()];
        t.row(
            "H1 foreign Q/linkdocs -> P/docs (a subdirectory of the managed root)",
            false,
            &roots_h,
            &q.join("linkdocs/x.md"),
        );
        // Decided, not derived: a foreign link to the managed root ITSELF is the same input as
        // `home/proj -> data/proj` (C1), so one cannot be refused without the other.
        t.row(
            "H2 foreign Q/linkP -> P (the managed root itself): admitted, ambiguous with C1",
            true,
            &roots_h,
            &q.join("linkP/docs/x.md"),
        );

        // ---- J: a foreign repository linking up to a directory holding another repository.
        let jt = tempfile::tempdir().unwrap();
        let j = canon(jt.path());
        let (jq, js) = (j.join("Q"), j.join("S"));
        git(&jq);
        git(&js);
        file(&js.join("docs/x.md"));
        file(&jq.join("docs/x.md"));
        ln("..", &jq.join("up"));
        let unrelated = tempfile::tempdir().unwrap();
        git(unrelated.path());
        let roots_j = vec![unrelated.path().to_path_buf()];
        t.row(
            "J1 foreign Q/up -> .., row Q/up/S/docs/x.md (S a sibling repo)",
            false,
            &roots_j,
            &jq.join("up/S/docs/x.md"),
        );
        t.row(
            "J2 foreign Q, plain row",
            true,
            &roots_j,
            &jq.join("docs/x.md"),
        );

        // ---- K: a managed root whose directory does not exist (a deleted project's rows).
        let kt = tempfile::tempdir().unwrap();
        let ghost = canon(kt.path()).join("ghost");
        let roots_k = vec![ghost.clone()];
        t.row(
            "K1 root directory gone, row below it",
            true,
            &roots_k,
            &ghost.join("docs/x.md"),
        );
        t.row(
            "K2 root directory gone, an unresolvable `..` below it",
            false,
            &roots_k,
            &ghost.join("sub/../../x.md"),
        );

        let mut bad = Vec::new();
        for (name, expect, file, dir) in &t.0 {
            if file != expect || dir != file {
                bad.push(format!(
                    "{name}: expected {} file-door {} dir-door {}",
                    if *expect { "ADMIT" } else { "REFUSE" },
                    if *file { "ADMIT" } else { "REFUSE" },
                    if *dir { "ADMIT" } else { "REFUSE" },
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "{} of {} rows wrong:\n{}",
            bad.len(),
            t.0.len(),
            bad.join("\n")
        );
    }

    /// B1 end to end: the same layout through the three verbs. `doc(update)` and `doc(delete)`
    /// refuse, and the sibling repository's file is untouched.
    #[tokio::test]
    async fn a_link_to_the_parent_cannot_reach_a_sibling_repository_through_any_verb() {
        let wt0 = tempfile::tempdir().unwrap();
        let w = canon(wt0.path());
        let (pp, s) = (w.join("P"), w.join("S"));
        git(&pp);
        git(&s);
        file(&s.join("docs/x.md"));
        ln("..", &pp.join("up"));
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(cp(&pp))
            .build();
        let id = catalog_row(&ctx, &pp.join("up/S/docs/x.md"));

        let upd = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}})).await;
        let del = delete::call(&ctx, json!({"id": id, "force": true})).await;
        let mvd = mv::call(&ctx, json!({"id": id, "new_rel_path": "moved.md"})).await;

        assert!(upd.is_err(), "update must refuse: {upd:?}");
        assert!(del.is_err(), "delete must refuse: {del:?}");
        assert!(mvd.is_err(), "move must refuse: {mvd:?}");
        assert_eq!(std::fs::read_to_string(s.join("docs/x.md")).unwrap(), DOC);
        assert!(!pp.join("moved.md").exists());
    }

    /// M-1: a destination below a nested repository's link back into the superproject used to pass
    /// `doc(move)`'s check (judged against the managed root: inside), the rename ran, and the id
    /// repair was then refused by the deeper door: file moved, catalog row not, and an error
    /// saying "Nothing was changed". Now the destination is judged by the SAME door, up front.
    #[tokio::test]
    async fn doc_move_into_a_nested_repositorys_link_back_to_the_superproject_moves_nothing() {
        let base = tempfile::tempdir().unwrap();
        let root = canon(base.path()).join("repo");
        git(&root);
        std::fs::create_dir_all(root.join("docs")).unwrap();
        git(&root.join("sub"));
        ln(root.join("docs"), &root.join("sub/linkdir"));
        let old = root.join("a.md");
        let old_id = crate::librarian::ids::artifact_id_from_abs(&old);
        std::fs::write(
            &old,
            format!("---\nid: {old_id}\nkind: spec\nstatus: draft\ntitle: T\n---\n\nbody\n"),
        )
        .unwrap();
        let ctx = mk_ctx(&root);
        let id = catalog_row(&ctx, &old);

        let err = mv::call(&ctx, json!({"id": id, "new_rel_path": "sub/linkdir/a.md"}))
            .await
            .expect_err("the door refuses the destination");

        assert!(
            msg(&err).contains("outside the repository root"),
            "{}",
            msg(&err)
        );
        assert!(old.is_file(), "the file stays where it was");
        assert!(
            !root.join("docs/a.md").exists(),
            "nothing landed in the superproject"
        );
        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(row.abs_path, old, "the row still names the old path");
    }

    /// M-2: a link whose target does not exist cannot be moved: the id repair reads through it
    /// after the rename and fails with ENOENT, leaving the link moved and the row not.
    #[tokio::test]
    async fn doc_move_refuses_a_dangling_link_up_front_whatever_its_target_spelling() {
        for (label, target_abs) in [("relative", false), ("absolute", true)] {
            let base = tempfile::tempdir().unwrap();
            let root = canon(base.path()).join("repo");
            for d in [".git", "real", "a", "b"] {
                std::fs::create_dir_all(root.join(d)).unwrap();
            }
            let target: PathBuf = if target_abs {
                root.join("real/gone.md")
            } else {
                PathBuf::from("../real/gone.md")
            };
            ln(&target, &root.join("a/x.md"));
            let ctx = mk_ctx(&root);
            let id = catalog_row(&ctx, &root.join("a/x.md"));

            let err = mv::call(&ctx, json!({"id": id, "new_rel_path": "b/x.md"}))
                .await
                .expect_err("a dangling link is not moved");

            assert!(msg(&err).contains("dangling"), "{label}: {}", msg(&err));
            assert!(
                std::fs::symlink_metadata(root.join("a/x.md")).is_ok(),
                "{label}: still in place"
            );
            assert!(
                std::fs::symlink_metadata(root.join("b/x.md")).is_err(),
                "{label}: nothing moved"
            );
            let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
            assert_eq!(row.abs_path, root.join("a/x.md"), "{label}: row unchanged");
        }
    }

    /// M5 (a surviving mutant): a `./` segment must not move the landing. `./real.md` next to the
    /// link names `a/real.md`; from `b/` it would name `b/real.md`, which does not exist.
    #[tokio::test]
    async fn doc_move_refuses_a_dot_slash_relative_target_that_would_not_resolve_there() {
        let base = tempfile::tempdir().unwrap();
        let root = canon(base.path()).join("repo");
        for d in [".git", "a", "b"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        std::fs::write(root.join("a/real.md"), DOC).unwrap();
        ln("./real.md", &root.join("a/x.md"));
        let ctx = mk_ctx(&root);
        let id = catalog_row(&ctx, &root.join("a/x.md"));

        let err = mv::call(&ctx, json!({"id": id, "new_rel_path": "b/x.md"}))
            .await
            .expect_err("`./real.md` would not resolve from b/");

        assert!(msg(&err).contains("would not resolve"), "{}", msg(&err));
        assert!(std::fs::symlink_metadata(root.join("b/x.md")).is_err());
    }

    /// L-2: arms are independent. The first version of the guard cleared whatever was armed, so an
    /// out-of-order drop took a LATER arm's roots with it, and the previous call's roots could not
    /// be restored by a nested one.
    #[test]
    fn overlapping_arms_neither_clear_nor_resurrect_each_others_roots() {
        let cat = Catalog::open_in_memory().unwrap();
        assert!(cat.write_roots().is_empty(), "nothing armed: empty");
        let r1 = vec![PathBuf::from("/r1")];
        let r2 = vec![PathBuf::from("/r2")];

        // Out of order: dropping the OLDER arm leaves the newer arm's roots in force.
        let a = cat.set_write_roots(r1.clone());
        let b = cat.set_write_roots(r2.clone());
        assert_eq!(
            cat.write_roots(),
            r2,
            "the newest arm's roots are in force, not a union"
        );
        drop(a);
        assert_eq!(
            cat.write_roots(),
            r2,
            "the older arm's drop must not clear the newer arm"
        );
        drop(b);
        assert!(
            cat.write_roots().is_empty(),
            "the last drop leaves nothing armed"
        );

        // Nested: dropping the newer arm restores the older arm's roots.
        let a = cat.set_write_roots(r1.clone());
        let b = cat.set_write_roots(r2);
        drop(b);
        assert_eq!(
            cat.write_roots(),
            r1,
            "the inner drop restores the outer arm's roots"
        );
        drop(a);
        assert!(cat.write_roots().is_empty());
    }

    /// L-3: a rekey PREVIEW must agree with the apply about a declared sidecar that is a dangling
    /// in-repo link: `write_through` skips a path that is not a file, so neither refuses.
    #[tokio::test]
    async fn rekey_prefix_preview_and_apply_both_pass_a_dangling_sidecar_link() {
        for (label, declare, mk_link) in [
            ("dangling in-repo link sidecar", true, true),
            ("no sidecar declared", false, false),
            ("declared, absent, no link", true, false),
        ] {
            let root = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(root.path().join(".git")).unwrap();
            std::fs::create_dir_all(root.path().join("docs/augmentations")).unwrap();
            if mk_link {
                ln("gone.yaml", &root.path().join("docs/augmentations/t.yaml"));
            }
            let decl = if declare {
                "expects_augmentation: docs/augmentations/t.yaml\n"
            } else {
                ""
            };
            let ledger = format!(
                "---\nkind: tracker\nentry_prefix: T\n{decl}---\n\n## T-1 — a\n\nsee T-1\n"
            );
            std::fs::write(root.path().join("ledger.md"), &ledger).unwrap();
            let ctx = mk_ctx(root.path());
            let id = catalog_row(&ctx, &root.path().join("ledger.md"));
            augmentation::upsert(&ctx.catalog.lock(), &aug_row(&id)).unwrap();

            let preview =
                super::super::rekey_prefix::call(&ctx, json!({"id": id, "from": "T", "to": "SRI"}))
                    .await;
            assert!(
                preview.is_ok(),
                "{label}: preview refused: {:?}",
                preview.err().map(|e| msg(&e))
            );
            let apply = super::super::rekey_prefix::call(
                &ctx,
                json!({"id": id, "from": "T", "to": "SRI", "force": true}),
            )
            .await;
            assert!(
                apply.is_ok(),
                "{label}: apply refused: {:?}",
                apply.err().map(|e| msg(&e))
            );
        }
    }

    fn seed(ctx: &ToolContext, path: &str) -> String {
        catalog_row(ctx, Path::new(path))
    }

    /// M-4: `merge_worktree`'s row selection used `LIKE`, which ignores ASCII case, so the
    /// registration `.worktrees/feat_1` also selected the rows of `.worktrees/FEAT_1`, and
    /// `abandon=true` deleted them with their cascade (and the dry run counted them).
    #[tokio::test]
    async fn merge_worktree_abandon_spares_the_rows_of_a_case_only_sibling_worktree() {
        let ctx = mk_ctx(Path::new("/nonexistent-r5/main"));
        let (own, sib_case, sib_wild) = {
            let cat = ctx.catalog.lock();
            crate::librarian::catalog::worktree::upsert_active(
                &cat,
                "/nonexistent-r5/.worktrees/feat_1",
                "/nonexistent-r5/main",
                None,
                1,
            )
            .unwrap();
            drop(cat);
            (
                seed(&ctx, "/nonexistent-r5/.worktrees/feat_1/docs/a.md"),
                seed(&ctx, "/nonexistent-r5/.worktrees/FEAT_1/docs/a.md"),
                seed(&ctx, "/nonexistent-r5/.worktrees/featX1/docs/a.md"),
            )
        };
        let root = "/nonexistent-r5/.worktrees/feat_1";

        let dry = super::super::merge_worktree::call(
            &ctx,
            json!({"root": root, "abandon": true, "dry_run": true}),
        )
        .await
        .unwrap();
        assert_eq!(
            dry["abandoned"], 1,
            "the dry run counts only this worktree's rows: {dry}"
        );
        let done = super::super::merge_worktree::call(&ctx, json!({"root": root, "abandon": true}))
            .await
            .unwrap();

        assert_eq!(done["abandoned"], 1);
        let cat = ctx.catalog.lock();
        assert!(
            artifact::get(&cat, &own).unwrap().is_none(),
            "its own row is gone"
        );
        assert!(
            artifact::get(&cat, &sib_case).unwrap().is_some(),
            "FEAT_1 survives"
        );
        assert!(
            artifact::get(&cat, &sib_wild).unwrap().is_some(),
            "featX1 survives"
        );
    }

    /// The non-abandon path selects with the same predicate: a merge of `feat_1` must not reseat
    /// the rows of `FEAT_1` onto the main checkout.
    #[tokio::test]
    async fn merge_worktree_merge_does_not_reseat_the_rows_of_a_case_only_sibling_worktree() {
        let ctx = mk_ctx(Path::new("/nonexistent-r5b/main"));
        let sib = {
            let cat = ctx.catalog.lock();
            crate::librarian::catalog::worktree::upsert_active(
                &cat,
                "/nonexistent-r5b/.worktrees/feat_1",
                "/nonexistent-r5b/main",
                None,
                1,
            )
            .unwrap();
            drop(cat);
            seed(&ctx, "/nonexistent-r5b/.worktrees/feat_1/docs/a.md");
            seed(&ctx, "/nonexistent-r5b/.worktrees/FEAT_1/docs/a.md")
        };

        let out = super::super::merge_worktree::call(
            &ctx,
            json!({"root": "/nonexistent-r5b/.worktrees/feat_1"}),
        )
        .await
        .unwrap();

        let touched = out.to_string();
        assert!(
            !touched.contains(&sib),
            "the sibling worktree's row must not be selected: {touched}"
        );
        let row = artifact::get(&ctx.catalog.lock(), &sib)
            .unwrap()
            .expect("the sibling's row survives the merge of feat_1");
        assert_eq!(
            row.abs_path,
            Path::new("/nonexistent-r5b/.worktrees/FEAT_1/docs/a.md"),
            "and keeps its path (it was not reseated onto the main checkout)"
        );
    }

    // ------------------------------- round 6 -----------------------------------------------

    /// Round 6 rows: `..` spellings, odd separators, `.git` markers, root order, raw-byte roots, an
    /// anchor reached through a link. Same shape as the main table: verdict per row, file door and
    /// directory door must agree.
    #[test]
    fn door_layout_table_round6() {
        use std::os::unix::ffi::OsStrExt;
        let mut t = Rows(Vec::new());

        // ---- X1: a `..` in the spelling. W{P, Q} both repositories, P managed.
        let wt0 = tempfile::tempdir().unwrap();
        let w = canon(wt0.path());
        let (p, q) = (w.join("P"), w.join("Q"));
        git(&p);
        git(&q);
        file(&q.join("docs/x.md"));
        file(&p.join("docs/y.md"));
        file(&w.join("R/x.md"));
        ln(q.join("docs"), &p.join("linkq"));
        let rp = vec![p.clone()];
        t.row(
            "X1 roots [P], `P/../Q/docs/x.md`: the spelling claims P, the file is Q's",
            false,
            &rp,
            &p.join("../Q/docs/x.md"),
        );
        t.row(
            "X1b roots [P], `P/docs/../../Q/docs/x.md`",
            false,
            &rp,
            &p.join("docs/../../Q/docs/x.md"),
        );
        t.row(
            "X1c roots [P], the canonical Q row (a foreign repository's own plain row, as J2)",
            true,
            &rp,
            &q.join("docs/x.md"),
        );
        t.row("X1d no roots, `P/../Q/docs/x.md`: not normalised, judged against the outermost repository on the spelling", false, &[], &p.join("../Q/docs/x.md"));
        t.row(
            "X1e roots [P], `P/../R/x.md` (R holds no .git)",
            false,
            &rp,
            &p.join("../R/x.md"),
        );
        t.row(
            "X1f roots [P], `P/./docs/y.md`: a `.` is dropped lexically, a plain in-root path",
            true,
            &rp,
            &p.join("./docs/y.md"),
        );
        t.row(
            "X1g roots [P], `P//docs/y.md`: a doubled separator is dropped lexically",
            true,
            &rp,
            Path::new(&format!("{}//docs/y.md", p.display())),
        );
        let rpw = vec![p.clone(), w.clone()];
        t.row(
            "X1h roots [P,W], `P/linkq/x.md` (link into Q)",
            false,
            &rpw,
            &p.join("linkq/x.md"),
        );
        t.row(
            "X1i roots [P,W], `P/../Q/docs/x.md`: refused like the link form",
            false,
            &rpw,
            &p.join("../Q/docs/x.md"),
        );

        // ---- X3: `.git` markers that are links. One uniform rule: any `.git` entry counts.
        let elsewhere = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(p.join("subl")).unwrap();
        ln(elsewhere.path(), &p.join("subl/.git"));
        ln(p.join("docs"), &p.join("subl/linkdir"));
        std::fs::create_dir_all(p.join("subd")).unwrap();
        ln("/nonexistent-leaf6", &p.join("subd/.git"));
        ln(p.join("docs"), &p.join("subd/linkdir"));
        t.row(
            "X3 nested dir whose `.git` is a link to a directory, its link back into P",
            false,
            &rp,
            &p.join("subl/linkdir/y.md"),
        );
        t.row(
            "X3b same with a DANGLING `.git` link: the marker counts, as in X3",
            false,
            &rp,
            &p.join("subd/linkdir/y.md"),
        );
        std::fs::create_dir_all(w.join("WT/docs")).unwrap();
        std::fs::write(
            w.join("WT/.git"),
            format!("gitdir: {}/.git/worktrees/wt\n", p.display()),
        )
        .unwrap();
        file(&w.join("WT/docs/x.md"));
        ln(w.join("WT"), &p.join("towt"));
        t.row("X3c P/towt -> a linked worktree elsewhere (its `.git` is a file): behind a link, refused", false, &rp, &p.join("towt/docs/x.md"));

        // ---- X6: verdicts depend on the ORDER of the managed roots (known, not changed).
        let u = w.join("U");
        git(&u);
        file(&u.join("other/x.md"));
        file(&u.join("N/docs/x.md"));
        ln(u.join("other"), &u.join("N/l"));
        let n = u.join("N");
        t.row("X6 roots [U,N], `U/N/l -> U/other`: U is the confinement root, the link stays inside it", true, &[u.clone(), n.clone()], &n.join("l/x.md"));
        t.row(
            "X6b roots [N,U], same row: N is the confinement root, the link leaves it",
            false,
            &[n.clone(), u.clone()],
            &n.join("l/x.md"),
        );
        t.row(
            "X6c roots [N] only, same row",
            false,
            std::slice::from_ref(&n),
            &n.join("l/x.md"),
        );

        // ---- X8: roots that are not UTF-8 compare as bytes.
        let ff = w.join(std::ffi::OsStr::from_bytes(b"n\xff"));
        let fe = w.join(std::ffi::OsStr::from_bytes(b"n\xfe"));
        git(&ff);
        git(&fe);
        file(&fe.join("x.md"));
        t.row(
            "X8 roots [n\\xff, n\\xfe], row in n\\xfe: not contained by the OTHER root",
            true,
            &[ff.clone(), fe.clone()],
            &fe.join("x.md"),
        );

        // ---- Y1: a foreign anchor reached through a link.
        let yt = tempfile::tempdir().unwrap();
        let y = canon(yt.path());
        let (yx, yr) = (y.join("X"), y.join("R"));
        std::fs::create_dir_all(&yx).unwrap();
        git(&yr);
        file(&yr.join("docs/x.md"));
        ln(&yr, &yx.join("link"));
        let unrelated = tempfile::tempdir().unwrap();
        git(unrelated.path());
        t.row("Y1a roots [unrelated], X (no repo) with X/link -> repo R: no repository is claimed, the leaf rule only", true, &[unrelated.path().to_path_buf()], &yx.join("link/docs/x.md"));
        t.row(
            "Y1b roots [X], same row: X is managed and the link leaves it",
            false,
            std::slice::from_ref(&yx),
            &yx.join("link/docs/x.md"),
        );

        let mut bad = Vec::new();
        for (name, expect, file, dir) in &t.0 {
            if file != expect || dir != file {
                bad.push(format!(
                    "{name}: expected {} file-door {} dir-door {}",
                    if *expect { "ADMIT" } else { "REFUSE" },
                    if *file { "ADMIT" } else { "REFUSE" },
                    if *dir { "ADMIT" } else { "REFUSE" },
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "{} of {} rows wrong:\n{}",
            bad.len(),
            t.0.len(),
            bad.join("\n")
        );
    }

    /// X1 through the tools: with P active, a row spelled `P/../Q/docs/x.md` is refused by every
    /// verb, and Q's file is neither written, moved nor deleted.
    #[tokio::test]
    async fn a_dotdot_spelling_cannot_reach_a_sibling_repository_through_any_verb() {
        let t = tempfile::tempdir().unwrap();
        let w = canon(t.path());
        let (p, q) = (w.join("P"), w.join("Q"));
        git(&p);
        git(&q);
        file(&q.join("docs/x.md"));
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(cp(&p))
            .build();
        let id = catalog_row(&ctx, &p.join("../Q/docs/x.md"));

        let upd = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}})).await;
        let mvd = mv::call(&ctx, json!({"id": id, "new_rel_path": "stolen.md"})).await;
        let del = delete::call(&ctx, json!({"id": id, "force": true})).await;

        assert!(upd.is_err(), "update: {upd:?}");
        assert!(mvd.is_err(), "move: {mvd:?}");
        assert!(del.is_err(), "delete: {del:?}");
        assert_eq!(std::fs::read_to_string(q.join("docs/x.md")).unwrap(), DOC);
        assert!(!p.join("stolen.md").exists());
    }

    /// MED-2, layout C1 (root `data/proj`, row spelled `home/proj/docs/x.md`): every verb ADMITS.
    /// `doc(move)` and `doc(delete)` gated on the LEXICAL containing root before the door, so the
    /// same row that `doc(update)` wrote was a plain "no managed root contains ..." for move and
    /// "outside every managed root" for delete.
    #[tokio::test]
    async fn every_verb_admits_a_row_spelled_through_a_link_to_the_managed_project() {
        let bt = tempfile::tempdir().unwrap();
        let base = canon(bt.path());
        let (home, data) = (base.join("home"), base.join("data"));
        std::fs::create_dir_all(&home).unwrap();
        git(&data.join("proj"));
        for f in ["u", "m", "d"] {
            file(&data.join(format!("proj/docs/{f}.md")));
        }
        ln(data.join("proj"), &home.join("proj"));
        let ctx = mk_ctx(&data.join("proj"));
        let row = |f: &str| catalog_row(&ctx, &home.join(format!("proj/docs/{f}.md")));
        let (u, m, d) = (row("u"), row("m"), row("d"));

        let upd = update::call(&ctx, json!({"id": u, "patch": {"status": "fixed"}})).await;
        let mvd = mv::call(&ctx, json!({"id": m, "new_rel_path": "docs/moved.md"})).await;
        let del = delete::call(&ctx, json!({"id": d, "force": true})).await;
        let crt = create::call(
            &ctx,
            json!({"repo": "r", "rel_path": "docs/new.md", "kind": "spec", "title": "T", "body": "b"}),
        )
        .await;

        assert!(upd.is_ok(), "update: {:?}", upd.err().map(|e| msg(&e)));
        assert!(mvd.is_ok(), "move: {:?}", mvd.err().map(|e| msg(&e)));
        assert!(del.is_ok(), "delete: {:?}", del.err().map(|e| msg(&e)));
        assert!(crt.is_ok(), "create: {:?}", crt.err().map(|e| msg(&e)));
        assert!(data.join("proj/docs/moved.md").is_file());
        assert!(!data.join("proj/docs/d.md").exists());
        assert!(data.join("proj/docs/new.md").is_file());
    }

    /// MED-2, layout F2 (managed root with a nested repository whose `linkdir` points back into the
    /// superproject): every verb REFUSES, create included. `doc(create)` judged against the project
    /// root alone, so a file created at `sub/linkdir/new.md` landed in the superproject and the new
    /// row was then refused by update, move and delete.
    #[tokio::test]
    async fn every_verb_refuses_a_path_below_a_nested_repositorys_link_into_its_superproject() {
        let ft = tempfile::tempdir().unwrap();
        let fp = canon(ft.path());
        git(&fp);
        for f in ["docs/u.md", "docs/d.md", "plain.md"] {
            file(&fp.join(f));
        }
        git(&fp.join("sub"));
        ln(fp.join("docs"), &fp.join("sub/linkdir"));
        let ctx = mk_ctx(&fp);
        let u = catalog_row(&ctx, &fp.join("sub/linkdir/u.md"));
        let d = catalog_row(&ctx, &fp.join("sub/linkdir/d.md"));
        let plain = catalog_row(&ctx, &fp.join("plain.md"));

        let upd = update::call(&ctx, json!({"id": u, "patch": {"status": "fixed"}})).await;
        let mvd = mv::call(
            &ctx,
            json!({"id": plain, "new_rel_path": "sub/linkdir/moved.md"}),
        )
        .await;
        let del = delete::call(&ctx, json!({"id": d, "force": true})).await;
        let crt = create::call(
            &ctx,
            json!({"repo": "r", "rel_path": "sub/linkdir/new.md", "kind": "spec", "title": "T", "body": "b"}),
        )
        .await;

        for (verb, r) in [
            ("update", upd.map(|_| ())),
            ("move", mvd.map(|_| ())),
            ("delete", del.map(|_| ())),
            ("create", crt.map(|_| ())),
        ] {
            let e = r.expect_err(verb);
            assert!(
                msg(&e).contains("outside the repository root"),
                "{verb}: {}",
                msg(&e)
            );
        }
        assert_eq!(std::fs::read_to_string(fp.join("docs/u.md")).unwrap(), DOC);
        assert!(fp.join("docs/d.md").exists());
        assert!(fp.join("plain.md").exists());
        assert!(!fp.join("docs/moved.md").exists());
        assert!(
            !fp.join("docs/new.md").exists(),
            "nothing was created in the superproject"
        );
    }

    // ---- MED-3: a move never renames a file whose id repair cannot proceed ------------------

    /// Moves `a/x.md` to `b/x.md` in a fresh repository whose `a/x.md` was made by `make`, and
    /// returns (root, ctx, id, result).
    async fn move_a_to_b(
        make: impl FnOnce(&Path),
    ) -> (
        TempDir,
        PathBuf,
        ToolContext,
        String,
        anyhow::Result<serde_json::Value>,
    ) {
        let base = tempfile::tempdir().unwrap();
        let root = canon(base.path()).join("repo");
        for d in [".git", "a", "b"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        make(&root);
        let ctx = mk_ctx(&root);
        let id = catalog_row(&ctx, &root.join("a/x.md"));
        let r = mv::call(&ctx, json!({"id": id, "new_rel_path": "b/x.md"})).await;
        (base, root, ctx, id, r)
    }

    fn assert_nothing_moved(root: &Path, ctx: &ToolContext, id: &str, what: &str) {
        assert!(
            std::fs::symlink_metadata(root.join("a/x.md")).is_ok(),
            "{what}: source stays"
        );
        assert!(
            std::fs::symlink_metadata(root.join("b/x.md")).is_err(),
            "{what}: nothing renamed"
        );
        let row = artifact::get(&ctx.catalog.lock(), id).unwrap().unwrap();
        assert_eq!(row.abs_path, root.join("a/x.md"), "{what}: row unchanged");
    }

    #[tokio::test]
    async fn doc_move_refuses_up_front_a_file_it_cannot_read() {
        use std::os::unix::fs::PermissionsExt;
        let (_b, root, ctx, id, r) = move_a_to_b(|root| {
            std::fs::write(root.join("a/x.md"), DOC).unwrap();
            std::fs::set_permissions(root.join("a/x.md"), std::fs::Permissions::from_mode(0o000))
                .unwrap();
        })
        .await;
        let readable = std::fs::read(root.join("a/x.md")).is_ok();
        let _ =
            std::fs::set_permissions(root.join("a/x.md"), std::fs::Permissions::from_mode(0o644));
        if readable {
            eprintln!("SKIPPED: running as root, a mode-000 file is still readable");
            return;
        }
        let e = r.expect_err("an unreadable file is not renamed");
        assert!(msg(&e).contains("cannot read"), "{}", msg(&e));
        assert_nothing_moved(&root, &ctx, &id, "mode 000");
    }

    #[tokio::test]
    async fn doc_move_refuses_up_front_a_file_that_is_not_utf8() {
        let (_b, root, ctx, id, r) = move_a_to_b(|root| {
            std::fs::write(
                root.join("a/x.md"),
                b"---\nkind: spec\n---\n\xff\xfe body\n",
            )
            .unwrap();
        })
        .await;
        let e = r.expect_err("invalid UTF-8 is not renamed");
        assert!(msg(&e).contains("cannot read"), "{}", msg(&e));
        assert_nothing_moved(&root, &ctx, &id, "invalid utf-8");
    }

    #[tokio::test]
    async fn doc_move_refuses_up_front_a_leaf_link_to_a_directory() {
        let (_b, root, ctx, id, r) = move_a_to_b(|root| {
            std::fs::create_dir_all(root.join("realdir")).unwrap();
            ln(root.join("realdir"), &root.join("a/x.md"));
        })
        .await;
        let e = r.expect_err("a link to a directory is not renamed");
        assert!(msg(&e).contains("cannot read"), "{}", msg(&e));
        assert_nothing_moved(&root, &ctx, &id, "link to a directory");
    }

    #[tokio::test]
    async fn doc_move_refuses_up_front_a_row_that_is_a_directory() {
        let (_b, root, ctx, id, r) = move_a_to_b(|root| {
            std::fs::create_dir_all(root.join("a/x.md")).unwrap();
        })
        .await;
        let e = r.expect_err("a directory is not renamed");
        assert!(msg(&e).contains("cannot read"), "{}", msg(&e));
        assert_nothing_moved(&root, &ctx, &id, "row is a directory");
    }

    /// The id repair's WRITE can still fail after the rename (here a read-only file, which the
    /// rename does not care about). The repair is best-effort, as its doc always said: the move
    /// completes, the catalog is re-keyed with the file's real content, and the response says the
    /// repair failed. It used to return an error with the file moved and the row not.
    #[tokio::test]
    async fn doc_move_completes_and_reports_when_only_the_id_repair_write_fails() {
        use std::os::unix::fs::PermissionsExt;
        let base = tempfile::tempdir().unwrap();
        let root = canon(base.path()).join("repo");
        for d in [".git", "a", "b"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
        }
        let old = root.join("a/x.md");
        let old_id = crate::librarian::ids::artifact_id_from_abs(&old);
        std::fs::write(
            &old,
            format!("---\nid: {old_id}\nkind: spec\nstatus: draft\ntitle: T\n---\n\nbody\n"),
        )
        .unwrap();
        std::fs::set_permissions(&old, std::fs::Permissions::from_mode(0o444)).unwrap();
        let writable = std::fs::OpenOptions::new().append(true).open(&old).is_ok();
        if writable {
            eprintln!("SKIPPED: running as root, a read-only file is still writable");
            return;
        }
        let ctx = mk_ctx(&root);
        let id = catalog_row(&ctx, &old);

        let v = mv::call(&ctx, json!({"id": id, "new_rel_path": "b/x.md"}))
            .await
            .expect("the move completes");

        assert!(
            v["id_repair_error"].is_string(),
            "the failure is reported: {v}"
        );
        assert!(root.join("b/x.md").is_file());
        let new_id = crate::librarian::ids::artifact_id_from_abs(&root.join("b/x.md"));
        assert!(
            artifact::get(&ctx.catalog.lock(), &new_id)
                .unwrap()
                .is_some(),
            "re-keyed"
        );
        let on_disk = std::fs::read_to_string(root.join("b/x.md")).unwrap();
        assert!(
            on_disk.contains(&old_id),
            "the file kept its (stale) id: {on_disk}"
        );
    }

    // ------------------------------- round 7 -----------------------------------------------

    /// A `..` in a root or a spelling must not disable the refinement. The first version of the
    /// `..` clause rejected every candidate below the anchor that carried one, so with a SHALLOWER
    /// anchor (an enclosing managed root listed earlier, or the outermost repository when nothing
    /// is managed) the judgment fell back to the anchor, which admits a write into a sibling nested
    /// repository (F2 class). A `..` that directly follows a plain name is an exact pop; only one
    /// that would climb above the anchor, or follow a link, is refused.
    #[test]
    fn door_layout_table_round7() {
        let mut t = Rows(Vec::new());

        // ---- Z: W is a repository holding nested repositories P and Q; P/lq -> Q/docs.
        let wt = tempfile::tempdir().unwrap();
        let w = canon(wt.path());
        let (p, q) = (w.join("P"), w.join("Q"));
        git(&w);
        git(&p);
        git(&q);
        file(&q.join("docs/x.md"));
        std::fs::create_dir_all(w.join("Z")).unwrap();
        ln(q.join("docs"), &p.join("lq"));
        let dd = w.join("Z/../P");
        t.row(
            "Z1 roots [W], canonical `W/P/lq/x.md` (nested P linking into nested Q)",
            false,
            std::slice::from_ref(&w),
            &p.join("lq/x.md"),
        );
        t.row(
            "Z2 roots [W], the same row spelled `W/Z/../P/lq/x.md`",
            false,
            std::slice::from_ref(&w),
            &dd.join("lq/x.md"),
        );
        t.row(
            "Z3 roots [W, W/Z/../P], same row",
            false,
            &[w.clone(), dd.clone()],
            &dd.join("lq/x.md"),
        );
        t.row("Z4 no roots, same row", false, &[], &dd.join("lq/x.md"));

        // ---- Z6: a `..` that follows a LINK is not an exact pop.
        let lt = tempfile::tempdir().unwrap();
        let l = canon(lt.path());
        let (lp, ls) = (l.join("P"), l.join("S"));
        git(&lp);
        git(&ls);
        file(&ls.join("x.md"));
        ln(&ls, &lp.join("lnk"));
        t.row(
            "Z6 roots [P], `P/lnk/../y.md` with lnk -> S (the `..` climbs out of S, not out of P)",
            false,
            &[lp.clone()],
            &lp.join("lnk/../y.md"),
        );

        // ---- Z7/Z8: a directory NAME containing `..` is a plain name.
        let nt = tempfile::tempdir().unwrap();
        let n = canon(nt.path()).join("P");
        git(&n);
        file(&n.join("docs/y.md"));
        git(&n.join("a..b"));
        file(&n.join("a..b/docs/y.md"));
        ln(n.join("docs"), &n.join("a..b/linkdir"));
        t.row(
            "Z7 roots [P], plain row below `P/a..b`",
            true,
            &[n.clone()],
            &n.join("a..b/docs/y.md"),
        );
        t.row("Z8 roots [P], nested repo `P/a..b` linking back into P: its name is a plain name, so it is the refinement and the link is refused", false, &[n.clone()], &n.join("a..b/linkdir/y.md"));

        let mut bad = Vec::new();
        for (name, expect, file, dir) in &t.0 {
            if file != expect || dir != file {
                bad.push(format!(
                    "{name}: expected {} file-door {} dir-door {}",
                    if *expect { "ADMIT" } else { "REFUSE" },
                    if *file { "ADMIT" } else { "REFUSE" },
                    if *dir { "ADMIT" } else { "REFUSE" },
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "{} of {} rows wrong:\n{}",
            bad.len(),
            t.0.len(),
            bad.join("\n")
        );
    }

    /// The production shape of Z2: active project W plus a `[[roots]]` entry `p` whose path is
    /// spelled `W/Z/../P`. `doc(create, repo=p, rel_path=lq/new.md)` landed in `W/Q/docs/new.md`
    /// and `doc(update)` of the row `W/Z/../P/lq/x.md` wrote Q's file.
    #[tokio::test]
    async fn a_dotdot_in_a_workspace_root_cannot_reach_a_sibling_nested_repository() {
        let wt = tempfile::tempdir().unwrap();
        let w = canon(wt.path());
        let (p, q) = (w.join("P"), w.join("Q"));
        git(&w);
        git(&p);
        git(&q);
        file(&q.join("docs/x.md"));
        std::fs::create_dir_all(w.join("Z")).unwrap();
        ln(q.join("docs"), &p.join("lq"));
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "p".into(),
                path: w.join("Z/../P"),
            })
            .with_current_project(cp(&w))
            .build();
        let id = catalog_row(&ctx, &w.join("Z/../P/lq/x.md"));

        let crt = create::call(
            &ctx,
            json!({"repo": "p", "rel_path": "lq/new.md", "kind": "spec", "title": "T", "body": "b"}),
        )
        .await;
        let upd = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}})).await;

        assert!(crt.is_err(), "create: {crt:?}");
        assert!(upd.is_err(), "update: {upd:?}");
        assert!(!q.join("docs/new.md").exists(), "nothing was created in Q");
        assert_eq!(std::fs::read_to_string(q.join("docs/x.md")).unwrap(), DOC);
    }

    /// LOW-1: the repair of a LINK is skipped (a write would follow it to the target), and that
    /// skip is reported in `id_repair_error`, not as null, which claims "repaired or not needed".
    #[tokio::test]
    async fn doc_move_reports_a_skipped_id_repair_of_a_leaf_link() {
        let base = tempfile::tempdir().unwrap();
        let root = canon(base.path()).join("repo");
        git(&root);
        let target = canon(base.path()).join("elsewhere.md");
        let old_id = crate::librarian::ids::artifact_id_from_abs(&root.join("outward.md"));
        let content =
            format!("---\nid: {old_id}\nkind: spec\nstatus: draft\ntitle: T\n---\n\nbody\n");
        std::fs::write(&target, &content).unwrap();
        ln(&target, &root.join("outward.md"));
        let ctx = mk_ctx(&root);
        let id = catalog_row(&ctx, &root.join("outward.md"));

        let v = mv::call(&ctx, json!({"id": id, "new_rel_path": "sub/moved.md"}))
            .await
            .expect("the move itself completes");

        let said = v["id_repair_error"]
            .as_str()
            .expect("the skipped repair is reported");
        assert!(said.contains("symbolic link"), "{said}");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            content,
            "the target is untouched"
        );
    }

    /// LOW-4: the monorepo-subproject half of the create change. `docs` is a link from the
    /// subproject `mono/pkg` to `mono/shared/docs`, INSIDE the git root. Judged against the project
    /// root alone `doc(create)` refused `docs/new.md`; judged by the door it is ADMITTED, which is
    /// right: `doc(update)`, `doc(move)` and `doc(delete)` already admit rows there (the managed
    /// root is the git root, round 3 H2) and the file lands inside the repository.
    #[tokio::test]
    async fn create_in_a_monorepo_subproject_admits_a_link_that_stays_inside_the_git_root() {
        let (mono, ctx) = monorepo();

        let v = create::call(
            &ctx,
            json!({"repo": "r", "rel_path": "docs/new.md", "kind": "spec", "title": "T", "body": "b"}),
        )
        .await
        .expect("a link that stays inside the git root is a legitimate destination");

        assert!(v["id"].is_string(), "{v}");
        assert!(mono.path().join("shared/docs/new.md").is_file());
    }

    /// LOW-5: a row no managed root contains is a recoverable refusal for `doc(move)` too, with
    /// the wording `doc(delete)` uses (it was a fatal `anyhow!`).
    #[tokio::test]
    async fn doc_move_of_a_row_outside_every_managed_root_is_a_recoverable_refusal() {
        let base = tempfile::tempdir().unwrap();
        let root = canon(base.path()).join("repo");
        git(&root);
        let ctx = mk_ctx(&root);
        let id = catalog_row(&ctx, Path::new("/nonexistent-r7/elsewhere/x.md"));

        let e = mv::call(&ctx, json!({"id": id, "new_rel_path": "moved.md"}))
            .await
            .expect_err("outside every managed root");

        assert!(
            e.downcast_ref::<LibrarianRecoverableError>().is_some(),
            "{}",
            msg(&e)
        );
        assert!(
            msg(&e).contains("outside every managed root"),
            "{}",
            msg(&e)
        );
    }
}
