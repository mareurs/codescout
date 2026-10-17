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
