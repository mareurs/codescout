//! A catalogued path that is a symbolic link must never be written THROUGH.
//!
//! Every librarian writer takes its target from the catalog (`row.abs_path`) or from a
//! frontmatter declaration, never from a caller-supplied path that a write classifier saw, so
//! none of them is covered by `validate_write_path`. `std::fs::write` follows a leaf link, and
//! a catalogued row can be one: the indexer no longer catalogs a link out of the repository (the
//! read door refuses it), but a pulled commit can turn a catalogued file into a link, so a row can
//! point anywhere the repo's author likes (`catalogued_outward_link` builds exactly that state). These tests are the measured form of
//! `docs/issues/2026-10-17-doc-update-writes-through-a-catalogued-symlink-and-rewrites-a-file-outside-the-root.md`
//! and its sibling sites. The `doc(create)` forms live in `create.rs`'s own tests.
//!
//! Every fixture is under `TempDir`s and the "victim" lives in a SEPARATE `TempDir` bound to a
//! local (an inline temporary would be dropped at the end of its statement and leave the link
//! dangling, which is a different world from the one asserted). Unix-only: Windows needs
//! elevation to create links.
#![cfg(unix)]

use super::{create, doctor, event_create, mv, update, TestToolContextBuilder, ToolContext};
use crate::librarian::catalog::{artifact, Catalog};
use crate::librarian::workspace::Root;
use serde_json::json;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const VICTIM: &str = "---\nkind: spec\nstatus: draft\ntitle: T\n---\n\nbody\n";

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

/// A project root holding `outward.md`, catalogued while it was a regular file and THEN replaced
/// by a link to a document in a SEPARATE directory: how such a row exists since the walk stopped
/// cataloguing outward links (the read door refuses them; a pulled commit can still turn a
/// catalogued file into one). Returns the temp dirs (kept alive), the victim path, and the link's
/// artifact id.
fn catalogued_outward_link(victim_text: &str) -> (TempDir, TempDir, ToolContext, PathBuf, String) {
    let outside = tempfile::tempdir().unwrap();
    let victim = outside.path().join("elsewhere.md");
    std::fs::write(&victim, victim_text).unwrap();
    let root = tempfile::tempdir().unwrap();
    let link = root.path().join("outward.md");
    std::fs::write(&link, victim_text).unwrap();
    let ctx = mk_ctx(root.path());
    let ignore = globset::GlobSet::empty();
    let (report, _) = crate::librarian::indexer::index_repo_sync(
        &ctx.catalog.lock(),
        &md_rules(),
        root.path(),
        &ignore,
        false,
        false,
        false,
    )
    .unwrap();
    // The precondition the whole file rests on: the row exists before the swap.
    assert_eq!(report.added, 1, "the walk must catalog the regular file");
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&victim, &link).unwrap();
    let id = crate::librarian::ids::artifact_id_from_abs(&link);
    assert!(artifact::get(&ctx.catalog.lock(), &id).unwrap().is_some());
    (outside, root, ctx, victim, id)
}

fn assert_victim_untouched(victim: &Path, link: &Path, text: &str) {
    assert_eq!(
        std::fs::read_to_string(victim).unwrap(),
        text,
        "the file the link points at, outside the root, must be byte-identical"
    );
    assert!(
        std::fs::symlink_metadata(link)
            .unwrap()
            .file_type()
            .is_symlink(),
        "a refused write must leave the link in place"
    );
}

#[tokio::test]
async fn doc_update_refuses_to_write_through_a_catalogued_link_to_a_file_outside_the_root() {
    let (_outside, root, ctx, victim, id) = catalogued_outward_link(VICTIM);
    let link = root.path().join("outward.md");

    let err = update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("an update through an outward link must be refused");

    let msg = format!("{err:#}");
    assert!(
        msg.contains("symbolic link"),
        "the refusal names why: {msg}"
    );
    assert_victim_untouched(&victim, &link, VICTIM);
    // Refused BEFORE any catalog mutation: the row still says what the file said.
    let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
    assert_ne!(
        row.status, "fixed",
        "the catalog must not move on a refusal"
    );
}

/// The control the refusal must not break: a regular in-root file updates normally. Without
/// it, "refuse every update" passes the test above.
#[tokio::test]
async fn doc_update_still_writes_a_regular_file_in_the_root() {
    let root = tempfile::tempdir().unwrap();
    let ctx = mk_ctx(root.path());
    let v = create::call(
        &ctx,
        json!({"repo": "r", "rel_path": "plain.md", "kind": "spec", "title": "T", "body": "b"}),
    )
    .await
    .unwrap();
    let id = v["id"].as_str().unwrap();
    update::call(&ctx, json!({"id": id, "patch": {"status": "fixed"}}))
        .await
        .unwrap();
    let on_disk = std::fs::read_to_string(root.path().join("plain.md")).unwrap();
    assert!(on_disk.contains("status: fixed"), "{on_disk}");
}

/// The in-root-target case, decided in the issue: a link whose target is INSIDE the root is
/// refused too, and the target file is updated directly. The row for the link can only exist
/// from before the indexer's duplicate-skip (or after a link was re-pointed), and writing through
/// it would leave the TARGET's own catalog row behind its file (`row_behind_file`).
///
/// Reached the way reality reaches it: index a real file as `alias.md`, then replace it with a
/// link to `real.md`.
#[tokio::test]
async fn doc_update_refuses_a_stale_row_whose_link_now_points_inside_the_root_but_updates_the_target(
) {
    let root = tempfile::tempdir().unwrap();
    let ctx = mk_ctx(root.path());
    let real = root.path().join("real.md");
    let alias = root.path().join("alias.md");
    std::fs::write(&real, VICTIM).unwrap();
    std::fs::write(&alias, "---\nkind: spec\nstatus: draft\n---\n\nold alias\n").unwrap();
    let ignore = globset::GlobSet::empty();
    let (first, _) = crate::librarian::indexer::index_repo_sync(
        &ctx.catalog.lock(),
        &md_rules(),
        root.path(),
        &ignore,
        false,
        false,
        false,
    )
    .unwrap();
    assert_eq!(first.added, 2, "precondition: both files are catalogued");
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink("real.md", &alias).unwrap();
    let alias_id = crate::librarian::ids::artifact_id_from_abs(&alias);
    let real_id = crate::librarian::ids::artifact_id_from_abs(&real);

    let err = update::call(&ctx, json!({"id": alias_id, "patch": {"status": "fixed"}}))
        .await
        .expect_err("a write through an in-root link is refused");
    assert!(format!("{err:#}").contains("symbolic link"));
    assert_eq!(std::fs::read_to_string(&real).unwrap(), VICTIM);

    // The target, addressed directly, is the supported route.
    update::call(&ctx, json!({"id": real_id, "patch": {"status": "fixed"}}))
        .await
        .unwrap();
    assert!(std::fs::read_to_string(&real)
        .unwrap()
        .contains("status: fixed"));
}

#[tokio::test]
async fn a_frontmatter_field_write_refuses_a_catalogued_link_to_a_file_outside_the_root() {
    let (_outside, root, ctx, victim, id) = catalogued_outward_link(VICTIM);
    let link = root.path().join("outward.md");

    let err = update::write_field_to_frontmatter(&ctx, &id, "status", &json!("fixed"))
        .expect_err("the field_patch writer must refuse an outward link");

    assert!(format!("{err:#}").contains("symbolic link"), "{err:#}");
    assert_victim_untouched(&victim, &link, VICTIM);
}

/// Same writer, reached the way an agent reaches it: a `field_patch` event on the artifact.
/// Pins that the event tool does not swallow the refusal and record an event for a change that
/// never happened.
#[tokio::test]
async fn a_field_patch_event_on_a_catalogued_outward_link_fails_and_leaves_the_victim() {
    let (_outside, root, ctx, victim, id) = catalogued_outward_link(VICTIM);
    let link = root.path().join("outward.md");

    let res = event_create::call(
        &ctx,
        json!({"artifact_id": id, "kind": "field_patch",
               "payload": {"field": "status", "to": "fixed"}}),
    )
    .await;

    let err = res.expect_err("the event must not succeed");
    assert!(format!("{err:#}").contains("symbolic link"), "{err:#}");
    assert_victim_untouched(&victim, &link, VICTIM);
    let events: i64 = ctx
        .catalog
        .lock()
        .conn
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        events, 0,
        "no event may record a change that never happened"
    );
}

/// `doc(move)` renames the LINK (rename does not follow) and then repairs the file's frontmatter
/// id by writing the moved path, which follows it. The victim carries a stale 16-hex `id:` so the
/// repair has something to rewrite; the move itself succeeds, the repair is skipped.
#[tokio::test]
async fn doc_move_of_a_catalogued_outward_link_does_not_repair_the_id_through_the_link() {
    let victim_text = "---\nid: 0123456789abcdef\nkind: spec\nstatus: draft\n---\n\nbody\n";
    let (_outside, root, ctx, victim, id) = catalogued_outward_link(victim_text);

    let res = mv::call(&ctx, json!({"id": id, "new_rel_path": "docs/moved.md"})).await;

    // The move itself succeeds: the repair is best-effort and a link is skipped, not failed. Also
    // what makes the skip, not just the write guard, load-bearing (without the skip the guarded
    // write errors and aborts the move after its rename).
    assert!(res.is_ok(), "the move must still succeed: {res:?}");
    // Whatever the move reports, the victim's bytes are what matter.
    assert_eq!(
        std::fs::read_to_string(&victim).unwrap(),
        victim_text,
        "the id repair must not write through the moved link: {res:?}"
    );
    let moved = root.path().join("docs/moved.md");
    assert!(
        std::fs::symlink_metadata(&moved)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        "the link itself was renamed, not replaced by a copy: {res:?}"
    );
}

/// `doctor fix=repair_frontmatter_id` is the other caller of the same repair writer.
#[tokio::test]
async fn doctor_repair_frontmatter_id_does_not_rewrite_the_file_behind_a_catalogued_link() {
    let victim_text = "---\nid: 0123456789abcdef\nkind: spec\nstatus: draft\n---\n\nbody\n";
    let (_outside, root, ctx, victim, _id) = catalogued_outward_link(victim_text);
    let root_s = crate::util::fs::RepoPath::from_path(root.path()).into_string();

    let applied = doctor::call(
        &ctx,
        json!({"fix": "repair_frontmatter_id", "root": root_s, "confirm": true}),
    )
    .await
    .unwrap();

    assert_eq!(std::fs::read_to_string(&victim).unwrap(), victim_text);
    // Reported as a failure, not as a repair: `repair_frontmatter_id` skips a link quietly (a
    // move's rename has already happened), so the sweep must refuse it before calling that.
    assert_eq!(applied["totals"]["files"], 0, "{applied}");
    assert_eq!(applied["totals"]["failed"], 1, "{applied}");
    let failure = applied["failed"][0]["error"].as_str().unwrap();
    assert!(failure.contains("symbolic link"), "{applied}");
}

/// `doctor fix=export_augmentations` writes a sidecar and then stamps `expects_augmentation:`
/// into the ARTIFACT in place. For a link artifact that stamp would land in the outside file, so
/// the sweep must refuse BEFORE writing the sidecar (else a sidecar nothing declares is left).
#[tokio::test]
async fn doctor_export_augmentations_does_not_stamp_the_file_behind_a_catalogued_link() {
    let victim_text = "---\nkind: tracker\nstatus: draft\n---\n\nbody\n";
    let (_outside, root, ctx, victim, id) = catalogued_outward_link(victim_text);
    crate::librarian::catalog::augmentation::upsert(
        &ctx.catalog.lock(),
        &crate::librarian::catalog::augmentation::AugmentationRow {
            artifact_id: id.clone(),
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

    assert_eq!(std::fs::read_to_string(&victim).unwrap(), victim_text);
    assert!(
        !root.path().join("docs/augmentations").exists(),
        "no sidecar may be written for an artifact the sweep cannot stamp: {applied}"
    );
    assert!(
        applied.to_string().contains("symbolic link"),
        "the refusal is reported, not swallowed: {applied}"
    );
}

/// `doc(link, rel="supersedes")` flips the DESTINATION's status in its file through the same
/// frontmatter writer. Reached the way an agent reaches it, so the call-site's ordering ("file
/// before catalog") is also checked: a refused write must leave no link row and no status change.
#[tokio::test]
async fn doc_link_supersedes_does_not_flip_the_status_of_a_file_behind_a_catalogued_link() {
    let (_outside, root, ctx, victim, dst_id) = catalogued_outward_link(VICTIM);
    let src = create::call(
        &ctx,
        json!({"repo": "r", "rel_path": "newer.md", "kind": "spec", "title": "N", "body": "b"}),
    )
    .await
    .unwrap();
    let src_id = src["id"].as_str().unwrap();

    let err = super::link::call(
        &ctx,
        json!({"src_id": src_id, "dst_id": dst_id, "rel": "supersedes"}),
    )
    .await
    .expect_err("superseding an artifact whose file is a link must be refused");

    assert!(format!("{err:#}").contains("symbolic link"), "{err:#}");
    assert_victim_untouched(&victim, &root.path().join("outward.md"), VICTIM);
    let row = artifact::get(&ctx.catalog.lock(), &dst_id)
        .unwrap()
        .unwrap();
    assert_ne!(row.status, "superseded", "the catalog must not record it");
    let links: i64 = ctx
        .catalog
        .lock()
        .conn
        .query_row("SELECT count(*) FROM artifact_link", [], |r| r.get(0))
        .unwrap();
    assert_eq!(links, 0, "no link row for a transition that never happened");
}
