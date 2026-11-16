use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{LibrarianRecoverableError, ToolContext};
use crate::librarian::catalog::dual_write::{TwinWhat, TwinWrite};
use crate::librarian::catalog::link_text::{label_for_rel, splice_relation};
use crate::librarian::catalog::{artifact, links, Catalog};
use crate::librarian::lifecycle::relations::rel_of_label;

#[derive(Deserialize)]
struct Args {
    src_id: String,
    dst_id: String,
    rel: String,
}
/// Argument parse for `link`: pure, no catalog or filesystem access. `call` runs it first, and
/// [`preflight`] runs it alone so the worktree write guard can sit behind it.
fn parse_args(args: Value) -> Result<Args> {
    serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(format!("doc(action=\"link\") requires 'src_id', 'dst_id' and 'rel': {e}"), "e.g. doc(action=\"link\", src_id=\"<16-hex>\", dst_id=\"<16-hex>\", rel=\"supersedes\"). Both ids are 16-hex artifact ids from find/get. Most citations need no manual link - link_scan derives rel=\"cites\" from prose.").into()
    })
}

/// The part of `call` that cannot write: refuses a malformed call with the tool's own error.
/// See `artifact::preflight`.
pub(crate) fn preflight(args: &Value) -> Result<()> {
    parse_args(args.clone()).map(|_| ())
}

/// How `doc(link)` names itself and its text in the error that follows a failed catalog write.
const TWIN_WHAT: TwinWhat = TwinWhat {
    caller: "doc(link)",
    recorded: "link",
    added: "relation line",
};

/// What `call` does about the relation line in the source document.
enum Twin {
    /// A rule leaves the line out, for this reason; the row is still recorded.
    Skip(String),
    /// The source document's new bytes, and the bytes they replace.
    Write(TwinWrite),
}

/// Decide the relation line for `a` in the source document `src_id` without writing anything.
///
/// A rule that leaves the line out is a [`Twin::Skip`] with its reason: a rel that gives no label,
/// a source document that lies in no git repository, a target the grammar cannot name
/// (`grammar_form_of` refuses: outside the key space, a shared key, no git repository), or the
/// identical line already in the document. The source's own key is not checked, so a source
/// outside the key space still gets the line. A document that cannot be read, or whose text cannot
/// take the line, is an error: the writer owns the document, so a line it cannot add is a failed
/// write and records nothing. A source whose file is not in this worktree is `call`'s to skip
/// before it asks here.
fn plan_relation_line(ctx: &ToolContext, cat: &Catalog, src_id: &str, a: &Args) -> Result<Twin> {
    use crate::librarian::catalog::dual_write::document_label;

    let label = match label_for_rel(&a.rel) {
        Ok(label) => label,
        Err(why) => return Ok(Twin::Skip(format!("rel is not a label: {why}"))),
    };
    let row = artifact::get(cat, src_id)?.ok_or_else(|| {
        LibrarianRecoverableError::new(format!("src artifact `{src_id}` not found"))
    })?;
    let path = row.abs_path;
    let Some(root) = crate::librarian::current_project::lookup_git_root(&path) else {
        return Ok(Twin::Skip(
            "target not nameable: the document lies in no git repository".to_string(),
        ));
    };
    let form =
        match crate::librarian::catalog::cite_text::grammar_form_of(&cat.conn, &root, &a.dst_id) {
            Ok(form) => form,
            Err(e) => return Ok(Twin::Skip(format!("target not nameable: {e:#}"))),
        };
    let doc_label = document_label(&path);
    // The read goes through the write door's leaf rule: a link is never written through, so it is
    // not read either.
    let text = crate::librarian::read_door::ReadDoor::new(super::managed_roots(ctx))
        .text_for_write(&path)
        .map_err(|e| {
            lower_error(
                e.into_anyhow(),
                &path,
                &doc_label,
                format!("doc(link): cannot read {doc_label}, so no link was recorded"),
            )
        })?;
    match splice_relation(&text, &label, &form) {
        Ok(None) => Ok(Twin::Skip("already present".to_string())),
        Ok(Some(written)) => Ok(Twin::Write(TwinWrite {
            path,
            original: text,
            written,
        })),
        Err(e) => Err(LibrarianRecoverableError::new(format!(
            "doc(link): the relation line could not be added to {doc_label}, so no link was \
             recorded: {e:#}"
        ))),
    }
}

/// `lead`, then the text of the lower layer's error `e` with the document's path replaced by
/// `label` (the read door and the write door write `path.display()` into their text). A hint the
/// lower layer gave stays a hint, scrubbed the same way.
fn lower_error(
    e: anyhow::Error,
    path: &std::path::Path,
    label: &str,
    lead: String,
) -> anyhow::Error {
    use crate::librarian::catalog::dual_write::without_path;
    match e.downcast_ref::<LibrarianRecoverableError>() {
        Some(lower) => anyhow::Error::new(LibrarianRecoverableError {
            message: format!("{lead}: {}", without_path(&lower.message, path, label)),
            hint: lower.hint.as_deref().map(|h| without_path(h, path, label)),
        }),
        None => LibrarianRecoverableError::new(format!(
            "{lead}: {}",
            without_path(&format!("{e:#}"), path, label)
        )),
    }
}

/// The error for a catalog write that failed after the source document was written: the file is
/// put back and the error says what became of it. With no file written, the error as it is.
fn undo_twin(twin: &Option<TwinWrite>, e: anyhow::Error) -> anyhow::Error {
    match twin {
        Some(file) => file.undo(&TWIN_WHAT, format!("{e:#}")),
        None => e,
    }
}

/// `e` with a sentence that says the destination was already changed. A `supersedes` link writes
/// the destination's `status: superseded` before the source document and the row, so an error
/// after that write must not claim less than happened.
fn note_destination_change(e: anyhow::Error, dst_label: &str) -> anyhow::Error {
    let sentence = format!(
        "The destination {dst_label} was already changed to `status: superseded` (the catalog \
         row still says the old status). Repeat the call after fixing the cause: the status \
         write is idempotent."
    );
    crate::librarian::catalog::dual_write::with_sentence(e, &sentence)
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a = parse_args(args)?;
    let now = chrono::Utc::now().timestamp_millis();
    // Any spelling the label reader takes as `Supersedes` is the same fact as the text line it
    // writes, so it moves the destination's status and its row stores the canonical rel.
    let superseding = label_for_rel(&a.rel).is_ok_and(|label| rel_of_label(&label) == "supersedes");

    // The catalog lock is SCOPED here rather than held across the body, because
    // `write_field_to_frontmatter` below acquires it itself and `parking_lot::Mutex` is
    // not reentrant — holding it across that call deadlocks rather than failing. Same
    // shape as `event_create::call`, which takes the lock in narrow blocks for exactly
    // this reason.
    let (src_id, dst_write_id, dst, src_not_here) = {
        let mut cat = ctx.catalog.lock();
        // Before ANY fork: a destination whose file the worktree lacks cannot take the
        // frontmatter write, and forking the source first would leave rows behind.
        if superseding {
            super::worktree::require_shadow_file(&cat, ctx, &a.dst_id)?;
        }
        // A source that exists only in the main checkout is still forked and still gets its row,
        // as before the text twin; only its relation line is skipped, because the worktree has no
        // file to hold it. Asked before the fork, which does not create the file.
        let src_not_here = super::worktree::shadow_file_missing(&cat, ctx, &a.src_id)?;
        let src_id = super::worktree::resolve_write_target(&mut cat, ctx, &a.src_id)?;

        if artifact::get(&cat, &src_id)?.is_none() {
            return Err(LibrarianRecoverableError::new(format!(
                "src artifact `{}` not found",
                src_id
            )));
        }
        // A `supersedes` link WRITES the destination's frontmatter, so the destination goes
        // through the same overlay gate as the source: from a worktree session the write lands
        // on its shadow and never on the main checkout's file. A plain link writes only an
        // edge, whose destination stays the id the caller passed.
        let dst_write_id = if superseding {
            super::worktree::resolve_write_target(&mut cat, ctx, &a.dst_id)?
        } else {
            a.dst_id.clone()
        };
        let dst = artifact::get(&cat, &dst_write_id)?.ok_or_else(|| {
            LibrarianRecoverableError::new(format!("dst artifact `{}` not found", a.dst_id))
        })?;
        (src_id, dst_write_id, dst, src_not_here)
    };

    let previous_status = dst.status.clone();

    // FILE BEFORE CATALOG, and the order is load-bearing. This project declares the file
    // the source of truth and the catalog a derived index, so a failed disk write must
    // leave the catalog untouched rather than recording a transition that never reached
    // the file. Writing the row first and the file second would reproduce the very
    // divergence this fixes, just with a smaller window.
    //
    // The preflight runs the source half of the plan first, read-only, and discards its bytes: an
    // unreadable or symlinked source, or a source that cannot take the line (an unclosed code
    // fence), then fails BEFORE the destination is written, so the call never half-applies on the
    // source's content. The real plan stays after the destination write because the two documents
    // can be one file, and bytes planned before that write would put the old status back.
    let dst_label = if superseding {
        if !src_not_here {
            let cat = ctx.catalog.lock();
            plan_relation_line(ctx, &cat, &src_id, &a)?;
        }
        crate::librarian::catalog::dual_write::document_label(&dst.abs_path)
    } else {
        String::new()
    };
    if superseding {
        crate::librarian::tools::update::write_field_to_frontmatter(
            ctx,
            &dst_write_id,
            "status",
            &json!("superseded"),
        )?;
    }

    let cat = ctx.catalog.lock();
    let twin_result: Result<String> = (|| {
        // The relation line goes into the source document before the row, inside the same lock
        // and the same transaction, and is put back when the commit fails. A rule that does not
        // apply skips the line and keeps the row; a write that is refused or fails is an error,
        // and the transaction drops with no row recorded.
        let tx = cat.conn.unchecked_transaction()?;
        let mut twin_file: Option<TwinWrite> = None;
        let plan = if src_not_here {
            Twin::Skip("source file is not in this worktree".to_string())
        } else {
            plan_relation_line(ctx, &cat, &src_id, &a)?
        };
        let text_twin = match plan {
            Twin::Skip(reason) => format!("skipped: {reason}"),
            Twin::Write(file) => {
                let label = crate::librarian::catalog::dual_write::document_label(&file.path);
                crate::librarian::util::write_catalogued_file(
                    &super::managed_roots(ctx),
                    &file.path,
                    &file.written,
                )
                .map_err(|e| {
                    lower_error(
                        e,
                        &file.path,
                        &label,
                        format!("doc(link): cannot write {label}, so no link was recorded"),
                    )
                })?;
                if let Err(e) = crate::librarian::catalog::dual_write::record_written_file(
                    &tx,
                    &src_id,
                    &file.path.to_string_lossy(),
                    &file.written,
                ) {
                    return Err(file.undo(&TWIN_WHAT, e));
                }
                twin_file = Some(file);
                "written".to_string()
            }
        };

        let row = links::LinkRow {
            src_id: src_id.clone(),
            dst_id: a.dst_id.clone(),
            rel: if superseding {
                "supersedes".to_string()
            } else {
                a.rel.clone()
            },
            created_at: now,
        };
        if let Err(e) = links::insert_with(&tx, &row) {
            return Err(undo_twin(&twin_file, e));
        }
        if let Err(e) = tx.commit() {
            return Err(undo_twin(&twin_file, e.into()));
        }
        Ok(text_twin)
    })();
    // From here the destination's `status: superseded` is on disk, so an error says so.
    let text_twin = match twin_result {
        Ok(text_twin) => text_twin,
        Err(e) if superseding => return Err(note_destination_change(e, &dst_label)),
        Err(e) => return Err(e),
    };

    if superseding {
        let mut dst = dst;
        dst.status = "superseded".into();
        dst.updated_at = now;
        artifact::upsert(&cat, &dst)?;

        let _ = crate::librarian::catalog::events::insert(
            &cat,
            &crate::librarian::catalog::events::EventRow {
                id: ulid::Ulid::new().to_string(),
                artifact_id: src_id.clone(),
                kind: "superseded_by".into(),
                payload: serde_json::json!({"target_artifact_id": a.dst_id}).to_string(),
                anchor_commit: None,
                head_commit: None,
                author: None,
                created_at: now,
            },
        );

        // Deliberately not the bare `json!("ok")` of a no-echo write. The status moved on
        // a SECOND artifact that the caller never named, as a side effect of a different
        // verb — "genuinely new info" by the write-response rule, and the absence of it is
        // why this divergence went unnoticed for the length of a session.
        return Ok(json!({
            "ok": true,
            "superseded": {
                "id": a.dst_id,
                "status": "superseded",
                "previous_status": previous_status,
            },
            "text_twin": text_twin,
        }));
    }

    // A plain link whose line was written has nothing new to tell, so it stays the bare `"ok"`
    // of a no-echo write. A skipped line is new information (the row has no text twin), so the
    // response says why.
    if text_twin == "written" {
        return Ok(json!("ok"));
    }
    Ok(json!({"ok": true, "text_twin": text_twin}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, ArtifactRow, TestArtifactRowBuilder};
    use crate::librarian::catalog::links;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        TestToolContextBuilder::new(cat).build()
    }

    fn mk_row(id: &str) -> ArtifactRow {
        TestArtifactRowBuilder::new(id).build()
    }

    /// A context with a real workspace root, so `create::call` produces artifacts that
    /// exist **on disk**. `mk_ctx` above builds synthetic rows whose `abs_path` names no
    /// file — fine for catalog-only assertions, useless for this one.
    fn mk_ctx_rooted(tmp_root: std::path::PathBuf) -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(crate::librarian::workspace::Root {
                name: "r".into(),
                path: tmp_root,
            })
            .build()
    }

    async fn mk_file_artifact(ctx: &ToolContext, rel_path: &str) -> String {
        crate::librarian::tools::create::call(
            ctx,
            json!({"repo": "r", "rel_path": rel_path, "kind": "spec", "title": "T", "body": "b"}),
        )
        .await
        .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    /// The worktree overlay's promise is that a session in a linked worktree never writes the
    /// shared main checkout. A `supersedes` link writes its DESTINATION's frontmatter, so the
    /// destination goes through the same gate as the source: the write lands on the overlay
    /// shadow at the worktree path, and the main checkout's file is untouched.
    ///
    /// Asserts both halves on disk (main file unchanged, shadow file changed) and the catalog
    /// row the write recorded, because a test that checked only "main is unchanged" would pass
    /// for a link that wrote nowhere. Both files exist: in a real worktree the shadow path holds
    /// git's checkout copy of the main file.
    /// docs/issues/archive/2026-10-17-a-supersedes-link-from-a-worktree-session-writes-the-main-checkouts-destination-file.md
    #[tokio::test]
    async fn supersedes_from_a_worktree_session_writes_the_shadow_and_not_the_main_file() {
        use crate::librarian::current_project::CurrentProject;
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let wt = repo.join(".worktrees/feat");
        let draft = "---\nstatus: draft\n---\nbody\n";
        for root in [&repo, &wt] {
            std::fs::create_dir_all(root.join("docs/specs")).unwrap();
            std::fs::write(root.join("docs/specs/other.md"), draft).unwrap();
        }
        let main_dst_path = repo.join("docs/specs/other.md");
        let shadow_dst_path = wt.join("docs/specs/other.md");
        let src_path = repo.join("docs/trackers/t.md");

        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(Arc::new(CurrentProject {
                abs_path: wt.clone(),
                git_root: wt.clone(),
                main_root: Some(repo.clone()),
                umbrella: None,
            }))
            .build();
        let src = crate::librarian::ids::artifact_id_from_abs(&src_path);
        let dst = crate::librarian::ids::artifact_id_from_abs(&main_dst_path);
        {
            let cat = ctx.catalog.lock();
            for (id, path, kind) in [(&src, &src_path, "tracker"), (&dst, &main_dst_path, "spec")] {
                artifact::upsert(
                    &cat,
                    &TestArtifactRowBuilder::new(id)
                        .with_abs_path(path.to_str().unwrap())
                        .with_kind(kind)
                        .build(),
                )
                .unwrap();
            }
        }
        let main_status_before = artifact::get(&ctx.catalog.lock(), &dst)
            .unwrap()
            .unwrap()
            .status;

        call(
            &ctx,
            json!({"src_id": src, "dst_id": dst, "rel": "supersedes"}),
        )
        .await
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(&main_dst_path).unwrap(),
            draft,
            "a worktree session must not write the main checkout's destination file"
        );
        let shadow = std::fs::read_to_string(&shadow_dst_path).unwrap();
        assert!(
            shadow.contains("status: superseded"),
            "the destination's shadow file must carry the transition, got:\n{shadow}"
        );
        let cat = ctx.catalog.lock();
        let shadow_id = crate::librarian::ids::artifact_id_from_abs(&shadow_dst_path);
        let shadow_row = artifact::get(&cat, &shadow_id)
            .unwrap()
            .expect("the destination was forked into a shadow row");
        assert_eq!(shadow_row.status, "superseded");
        assert_eq!(
            artifact::get(&cat, &dst).unwrap().unwrap().status,
            main_status_before,
            "the main row is merge_worktree's to move, not the link's"
        );
    }
    /// docs/issues/2026-10-24-a-worktree-supersedes-of-a-main-file-the-worktree-lacks-fails-after-forking-shadow-rows.md
    ///
    /// The destination exists in the main checkout only (untracked there, or added after the
    /// worktree's branch point). The write cannot land, so the call must refuse BEFORE the overlay
    /// gate forks anything, name the real cause, and leave the catalog exactly as it was. The
    /// assertions on the row counts are the discriminating half: a call that refuses with the
    /// right words after committing two shadow rows and two lineage links still fails them.
    #[tokio::test]
    async fn supersedes_of_a_main_file_the_worktree_lacks_refuses_before_forking() {
        use crate::librarian::current_project::CurrentProject;
        use std::sync::Arc;
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let wt = repo.join(".worktrees/feat");
        std::fs::create_dir_all(repo.join("docs/specs")).unwrap();
        std::fs::create_dir_all(wt.join("docs/specs")).unwrap();
        let draft = "---\nstatus: draft\n---\nbody\n";
        let main_dst_path = repo.join("docs/specs/other.md");
        std::fs::write(&main_dst_path, draft).unwrap();
        let src_path = repo.join("docs/trackers/t.md");

        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(Arc::new(CurrentProject {
                abs_path: wt.clone(),
                git_root: wt.clone(),
                main_root: Some(repo.clone()),
                umbrella: None,
            }))
            .build();
        let src = crate::librarian::ids::artifact_id_from_abs(&src_path);
        let dst = crate::librarian::ids::artifact_id_from_abs(&main_dst_path);
        {
            let cat = ctx.catalog.lock();
            for (id, path, kind) in [(&src, &src_path, "tracker"), (&dst, &main_dst_path, "spec")] {
                artifact::upsert(
                    &cat,
                    &TestArtifactRowBuilder::new(id)
                        .with_abs_path(path.to_str().unwrap())
                        .with_kind(kind)
                        .build(),
                )
                .unwrap();
            }
        }
        let count = |sql: &str| -> i64 {
            ctx.catalog
                .lock()
                .conn
                .query_row(sql, [], |r| r.get(0))
                .unwrap()
        };
        let rows_before = count("SELECT count(*) FROM artifact");
        let links_before = count("SELECT count(*) FROM artifact_link");

        let err = call(
            &ctx,
            json!({"src_id": src, "dst_id": dst, "rel": "supersedes"}),
        )
        .await
        .expect_err("the destination has no file in the worktree, so the write cannot land");

        let msg = format!("{err:#}");
        assert!(
            msg.contains("main checkout") && msg.contains("other.md"),
            "the refusal must name the real cause and the file, got: {msg}"
        );
        assert!(
            !msg.contains("deleted or moved"),
            "the hint must not blame a deletion that did not happen, got: {msg}"
        );
        assert_eq!(
            count("SELECT count(*) FROM artifact"),
            rows_before,
            "a refused call must not leave shadow rows behind"
        );
        assert_eq!(
            count("SELECT count(*) FROM artifact_link"),
            links_before,
            "a refused call must not leave lineage links behind"
        );
        assert_eq!(
            std::fs::read_to_string(&main_dst_path).unwrap(),
            draft,
            "the main checkout's file is untouched"
        );
    }

    /// Asserts the **file**, not the catalog row.
    ///
    /// `supersedes_transitions_dst_status` above asserts the row, and that is the half
    /// which already worked: it is monotone under exactly the defect this test exists to
    /// catch, passing whether or not the write reaches disk. This project declares the
    /// file the source of truth and the catalog a derived index, so a status that moves
    /// only in the index is a divergence with no reader able to see it — the file says
    /// `active`, every `find` says `superseded`, and `git status` is clean because there
    /// is nothing to commit.
    #[tokio::test]
    async fn supersedes_writes_the_new_status_to_the_dst_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = mk_ctx_rooted(tmp.path().to_path_buf());
        let src = mk_file_artifact(&ctx, "src.md").await;
        let dst = mk_file_artifact(&ctx, "dst.md").await;

        call(
            &ctx,
            json!({"src_id": src, "dst_id": dst, "rel": "supersedes"}),
        )
        .await
        .unwrap();

        let abs = artifact::get(&ctx.catalog.lock(), &dst)
            .unwrap()
            .unwrap()
            .abs_path;
        let on_disk = std::fs::read_to_string(&abs).unwrap();
        assert!(
            on_disk.contains("status: superseded"),
            "dst frontmatter must record the transition the catalog recorded; got:\n{on_disk}"
        );
    }
    /// A `supersedes` link in any spelling the label reader accepts is the same fact: the
    /// destination's frontmatter moves to `superseded` and the row stores the canonical rel.
    // mutation: `superseding` compares `a.rel == "supersedes"` (the exact spelling), or the row keeps
    // the spelling the caller passed.
    #[tokio::test]
    async fn a_supersedes_rel_in_any_spelling_marks_the_destination_superseded() {
        for spelling in ["Supersedes", "SUPERSEDES"] {
            let tmp = tempfile::TempDir::new().unwrap();
            let ctx = mk_ctx_rooted(tmp.path().to_path_buf());
            let src = mk_file_artifact(&ctx, "src.md").await;
            let dst = mk_file_artifact(&ctx, "dst.md").await;

            call(&ctx, json!({"src_id": src, "dst_id": dst, "rel": spelling}))
                .await
                .unwrap();

            let abs = artifact::get(&ctx.catalog.lock(), &dst)
                .unwrap()
                .unwrap()
                .abs_path;
            let on_disk = std::fs::read_to_string(&abs).unwrap();
            assert!(
                on_disk.contains("status: superseded"),
                "{spelling}: dst frontmatter must say superseded; got:\n{on_disk}"
            );
            assert_eq!(
                rel_rows(&ctx, "supersedes"),
                1,
                "{spelling}: the row stores the canonical rel"
            );
            assert_eq!(rel_rows(&ctx, spelling), 0, "{spelling}: not the spelling");
        }
    }

    /// The transition is a side effect of a *different verb*, on a *second* artifact the
    /// caller never named. A bare `"ok"` is why it went unnoticed for a session, so the
    /// response has to name it. This is the documented exception to no-echo writes —
    /// "reserve richer responses only for genuinely new info".
    #[tokio::test]
    async fn supersedes_reports_the_status_transition_it_caused() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = mk_ctx_rooted(tmp.path().to_path_buf());
        let src = mk_file_artifact(&ctx, "src2.md").await;
        let dst = mk_file_artifact(&ctx, "dst2.md").await;

        let v = call(
            &ctx,
            json!({"src_id": src, "dst_id": dst, "rel": "supersedes"}),
        )
        .await
        .unwrap();

        assert_eq!(v["superseded"]["id"], json!(dst), "response: {v}");
        assert_eq!(
            v["superseded"]["status"],
            json!("superseded"),
            "response: {v}"
        );
    }

    /// A synthetic row names no file and no git repository, so the link has no text twin: the row
    /// is still recorded, and the response says why the line was skipped.
    #[tokio::test]
    async fn basic_link_insert() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        artifact::upsert(&cat, &mk_row("b")).unwrap();
        let ctx = mk_ctx(cat);

        let v = call(
            &ctx,
            json!({"src_id": "a", "dst_id": "b", "rel": "implements"}),
        )
        .await
        .unwrap();

        assert_eq!(v["ok"], json!(true), "response: {v}");
        assert!(
            v["text_twin"]
                .as_str()
                .is_some_and(|t| t.starts_with("skipped: target not nameable")),
            "response: {v}"
        );
        let out = links::outgoing(&ctx.catalog.lock(), "a").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rel, "implements");
    }

    /// The catalog half of the transition; its sibling
    /// `supersedes_writes_the_new_status_to_the_dst_file` asserts the disk half. Both are
    /// needed and neither substitutes for the other.
    ///
    /// **The fixture is a real file on purpose — do not revert it to `mk_row`.** Now that
    /// `link` writes frontmatter, a synthetic row whose `abs_path` names nothing on disk
    /// cannot reach the transition at all. That is the contract, not an obstacle: a
    /// status change this tool cannot persist is one it must refuse rather than record.
    #[tokio::test]
    async fn supersedes_transitions_dst_status() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = mk_ctx_rooted(tmp.path().to_path_buf());
        let src = mk_file_artifact(&ctx, "a.md").await;
        let dst = mk_file_artifact(&ctx, "b.md").await;

        call(
            &ctx,
            json!({"src_id": src, "dst_id": dst.clone(), "rel": "supersedes"}),
        )
        .await
        .unwrap();

        let row = artifact::get(&ctx.catalog.lock(), &dst).unwrap().unwrap();
        assert_eq!(row.status, "superseded");
    }

    /// Real-file fixture for the same reason as `supersedes_transitions_dst_status`
    /// above — do not revert it to `mk_row`.
    #[tokio::test]
    async fn link_supersedes_emits_event() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ctx = mk_ctx_rooted(tmp.path().to_path_buf());
        let src = mk_file_artifact(&ctx, "a.md").await;
        let dst = mk_file_artifact(&ctx, "b.md").await;

        call(
            &ctx,
            json!({"src_id": src.clone(), "dst_id": dst, "rel": "supersedes"}),
        )
        .await
        .unwrap();

        // Expect a superseded_by event on the SOURCE artifact.
        let count: i64 = ctx
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id=?1 AND kind='superseded_by'",
                [src.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "supersedes link must emit a superseded_by event");
    }

    #[tokio::test]
    async fn unknown_dst_errors() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);

        let err = call(
            &ctx,
            json!({"src_id": "a", "dst_id": "nonexistent", "rel": "ref"}),
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string().contains("not found"),
            "expected 'not found' error, got: {err}"
        );
    }

    #[tokio::test]
    async fn repeating_link_is_idempotent() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        artifact::upsert(&cat, &mk_row("b")).unwrap();
        let ctx = mk_ctx(cat);

        // First link — ok.
        call(
            &ctx,
            json!({"src_id": "a", "dst_id": "b", "rel": "implements"}),
        )
        .await
        .unwrap();

        // Same link again — must not error.
        let v = call(
            &ctx,
            json!({"src_id": "a", "dst_id": "b", "rel": "implements"}),
        )
        .await
        .unwrap();
        assert_eq!(v["ok"], json!(true), "response: {v}");

        // Only one edge row should exist.
        let count: i64 = ctx
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT count(*) FROM artifact_link WHERE src_id = 'a' AND dst_id = 'b'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn missing_src_errors_clearly() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat);

        let err = call(
            &ctx,
            json!({"src_id": "ghost", "dst_id": "also_ghost", "rel": "implements"}),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "expected 'not found' error, got: {err}"
        );
    }

    // ---- the relation line: the text twin of a manual link ----

    use crate::librarian::lifecycle::relations::relations;
    use tempfile::TempDir;

    const SRC_TEXT: &str = "---\ntitle: S\n---\n\n# S\n\nbody\n";
    const DST_TEXT: &str = "---\nstatus: draft\ntitle: D\n---\n\n# D\n\nbody\n";

    struct Repo {
        _tmp: TempDir,
        ctx: ToolContext,
        root: std::path::PathBuf,
        src: std::path::PathBuf,
    }

    /// A temp git repository holding `docs/src.md` and `docs/dst.md`, with artifact rows `src` and
    /// `dst` whose `abs_path` is each real file, so `grammar_form_of` names `dst` as `[[dst]]`.
    fn repo() -> Repo {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join("docs")).unwrap();
        let ctx = mk_ctx_rooted(root.clone());
        let src = root.join("docs/src.md");
        std::fs::write(&src, SRC_TEXT).unwrap();
        std::fs::write(root.join("docs/dst.md"), DST_TEXT).unwrap();
        for (id, rel) in [("src", "docs/src.md"), ("dst", "docs/dst.md")] {
            artifact::upsert(
                &ctx.catalog.lock(),
                &TestArtifactRowBuilder::new(id)
                    .with_abs_path(root.join(rel))
                    .build(),
            )
            .unwrap();
        }
        Repo {
            _tmp: tmp,
            ctx,
            root,
            src,
        }
    }

    fn link_rows(ctx: &ToolContext) -> i64 {
        ctx.catalog
            .lock()
            .conn
            .query_row("SELECT count(*) FROM artifact_link", [], |r| r.get(0))
            .unwrap()
    }

    fn row_sha(ctx: &ToolContext, id: &str) -> String {
        artifact::get(&ctx.catalog.lock(), id)
            .unwrap()
            .unwrap()
            .file_sha256
    }

    async fn link(ctx: &ToolContext, rel: &str) -> Result<Value> {
        call(ctx, json!({"src_id": "src", "dst_id": "dst", "rel": rel})).await
    }

    /// The error as a caller sees it: the message, the hint and the chain.
    fn shown(err: &anyhow::Error) -> String {
        let hint = err
            .downcast_ref::<LibrarianRecoverableError>()
            .and_then(|e| e.hint.clone())
            .unwrap_or_default();
        format!("{err:#} {hint}")
    }

    #[tokio::test]
    async fn a_link_writes_a_relation_line_and_a_row() {
        let r = repo();
        let sha_before = row_sha(&r.ctx, "src");

        link(&r.ctx, "implements").await.unwrap();

        let text = std::fs::read_to_string(&r.src).unwrap();
        assert_eq!(
            text,
            format!("{SRC_TEXT}\n## Relations\n\n**Implements:** [[dst]]\n"),
            "the line sits under one `## Relations` heading at the end of the body"
        );
        let read = relations(&text);
        assert_eq!(read.len(), 1, "{read:?}");
        assert_eq!(read[0].label, "Implements");
        let out = links::outgoing(&r.ctx.catalog.lock(), "src").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rel, "implements");
        assert_ne!(
            row_sha(&r.ctx, "src"),
            sha_before,
            "the row follows the file"
        );
        assert_eq!(
            row_sha(&r.ctx, "src"),
            crate::librarian::util::sha_of_bytes(text.as_bytes()),
            "the row records the sha of the bytes the call wrote"
        );
    }

    #[tokio::test]
    async fn a_plain_link_still_returns_the_bare_ok_when_the_twin_is_written() {
        let r = repo();
        let v = link(&r.ctx, "see-also").await.unwrap();
        assert_eq!(v, json!("ok"));
        assert!(std::fs::read_to_string(&r.src)
            .unwrap()
            .contains("**See also:** [[dst]]"));
    }

    #[tokio::test]
    async fn a_supersedes_link_writes_the_supersedes_line_and_keeps_the_frontmatter_write() {
        let r = repo();

        let v = link(&r.ctx, "supersedes").await.unwrap();

        assert_eq!(v["ok"], json!(true), "{v}");
        assert_eq!(v["superseded"]["status"], json!("superseded"), "{v}");
        assert_eq!(v["text_twin"], json!("written"), "{v}");
        let src = std::fs::read_to_string(&r.src).unwrap();
        assert!(
            src.ends_with("## Relations\n\n**Supersedes:** [[dst]]\n"),
            "{src}"
        );
        let read = relations(&src);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].label, "Supersedes");
        assert!(crate::librarian::lifecycle::relations::is_backward(
            &read[0].label
        ));
        let dst = std::fs::read_to_string(r.root.join("docs/dst.md")).unwrap();
        assert!(
            dst.contains("status: superseded"),
            "the destination frontmatter write is kept: {dst}"
        );
        let events: i64 = r
            .ctx
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id='src' AND kind='superseded_by'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(events, 1);
        assert_eq!(link_rows(&r.ctx), 1);
    }

    #[tokio::test]
    async fn a_supersedes_link_reports_a_skipped_twin_too() {
        // No git repository: the frontmatter write and the row stay, and the response says why
        // the source has no line.
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx_rooted(tmp.path().to_path_buf());
        let src = mk_file_artifact(&ctx, "s.md").await;
        let dst = mk_file_artifact(&ctx, "d.md").await;
        let v = call(
            &ctx,
            json!({"src_id": src, "dst_id": dst, "rel": "supersedes"}),
        )
        .await
        .unwrap();
        assert_eq!(v["ok"], json!(true), "{v}");
        assert_eq!(
            v["text_twin"],
            json!("skipped: target not nameable: the document lies in no git repository"),
            "{v}"
        );
        assert_eq!(link_rows(&ctx), 1);
    }

    #[tokio::test]
    async fn a_link_to_a_target_outside_the_key_space_keeps_the_row_and_skips_the_line() {
        let r = repo();
        let cfg = r.root.join(".codescout");
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(cfg.join("key-space.toml"), "exclude = [\"contrib/**\"]\n").unwrap();
        std::fs::create_dir_all(r.root.join("contrib")).unwrap();
        std::fs::write(r.root.join("contrib/x.md"), "x\n").unwrap();
        artifact::upsert(
            &r.ctx.catalog.lock(),
            &TestArtifactRowBuilder::new("outside")
                .with_abs_path(r.root.join("contrib/x.md"))
                .build(),
        )
        .unwrap();

        let v = call(
            &r.ctx,
            json!({"src_id": "src", "dst_id": "outside", "rel": "implements"}),
        )
        .await
        .unwrap();

        assert_eq!(v["ok"], json!(true), "{v}");
        let twin = v["text_twin"].as_str().unwrap();
        assert!(
            twin.starts_with("skipped: target not nameable: ") && twin.contains("key-space.toml"),
            "{twin}"
        );
        assert!(
            !twin.contains(&r.root.display().to_string()),
            "no absolute path in the response: {twin}"
        );
        assert_eq!(std::fs::read_to_string(&r.src).unwrap(), SRC_TEXT);
        assert_eq!(link_rows(&r.ctx), 1, "the row stays");
    }

    #[tokio::test]
    async fn a_rel_that_is_not_a_label_keeps_the_row_and_skips_the_line() {
        let r = repo();
        let v = link(&r.ctx, "depends2").await.unwrap();
        assert_eq!(v["ok"], json!(true), "{v}");
        let twin = v["text_twin"].as_str().unwrap();
        assert!(twin.starts_with("skipped: rel is not a label: "), "{twin}");
        assert_eq!(std::fs::read_to_string(&r.src).unwrap(), SRC_TEXT);
        let out = links::outgoing(&r.ctx.catalog.lock(), "src").unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].rel, "depends2");
    }

    #[tokio::test]
    async fn the_same_link_twice_leaves_one_line() {
        let r = repo();
        assert_eq!(link(&r.ctx, "implements").await.unwrap(), json!("ok"));
        let after_first = std::fs::read_to_string(&r.src).unwrap();

        let second = link(&r.ctx, "implements").await.unwrap();

        assert_eq!(
            second,
            json!({"ok": true, "text_twin": "skipped: already present"})
        );
        let text = std::fs::read_to_string(&r.src).unwrap();
        assert_eq!(text, after_first, "the second call writes nothing");
        assert_eq!(text.matches("**Implements:** [[dst]]").count(), 1, "{text}");
        assert_eq!(text.matches("## Relations").count(), 1, "{text}");
        assert_eq!(link_rows(&r.ctx), 1);
    }

    #[tokio::test]
    async fn a_second_relation_joins_the_one_section() {
        let r = repo();
        link(&r.ctx, "implements").await.unwrap();
        link(&r.ctx, "rests_on").await.unwrap();
        let text = std::fs::read_to_string(&r.src).unwrap();
        assert!(
            text.ends_with("**Implements:** [[dst]]\n\n**Rests on:** [[dst]]\n"),
            "{text}"
        );
        assert_eq!(text.matches("## Relations").count(), 1, "{text}");
        assert_eq!(relations(&text).len(), 2);
        assert_eq!(link_rows(&r.ctx), 2);
    }

    #[tokio::test]
    async fn a_failed_commit_restores_the_source_file() {
        let r = repo();
        let sha_before = row_sha(&r.ctx, "src");
        // A deferred foreign-key violation raised by the `artifact_link` INSERT: every statement
        // and the file write succeed, and only the COMMIT fails.
        r.ctx
            .catalog
            .lock()
            .conn
            .execute_batch(
                "CREATE TABLE commit_tripwire(
                     missing TEXT REFERENCES artifact(id) DEFERRABLE INITIALLY DEFERRED
                 );
                 CREATE TRIGGER commit_tripwire_trg AFTER INSERT ON artifact_link
                 BEGIN
                     INSERT INTO commit_tripwire(missing) VALUES ('no-such-artifact');
                 END;",
            )
            .unwrap();

        let err = link(&r.ctx, "implements").await.unwrap_err();

        assert_eq!(
            std::fs::read_to_string(&r.src).unwrap(),
            SRC_TEXT,
            "the file is back to its old bytes"
        );
        assert_eq!(link_rows(&r.ctx), 0, "no row remains");
        assert_eq!(row_sha(&r.ctx, "src"), sha_before, "the row is untouched");
        let msg = shown(&err);
        assert!(msg.contains("doc(link): the catalog write failed"), "{msg}");
        assert!(msg.contains("no link was recorded"), "{msg}");
        assert!(msg.contains("put back as it was"), "{msg}");
        assert!(
            !msg.contains(&r.root.display().to_string()),
            "no absolute path in the error: {msg}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_refused_file_write_is_an_error_and_records_no_row() {
        let r = repo();
        // The source row names a link; the link's target lies in a directory of its own, so the
        // target path is not covered by stripping the link's own directory.
        std::fs::create_dir(r.root.join("sub")).unwrap();
        let real = r.root.join("sub/real.md");
        std::fs::write(&real, SRC_TEXT).unwrap();
        let link_path = r.root.join("docs/linked.md");
        std::os::unix::fs::symlink(&real, &link_path).unwrap();
        artifact::upsert(
            &r.ctx.catalog.lock(),
            &TestArtifactRowBuilder::new("linked")
                .with_abs_path(link_path.clone())
                .build(),
        )
        .unwrap();

        let err = call(
            &r.ctx,
            json!({"src_id": "linked", "dst_id": "dst", "rel": "implements"}),
        )
        .await
        .unwrap_err();

        assert_eq!(link_rows(&r.ctx), 0, "no row remains: {err:#}");
        assert_eq!(
            std::fs::read_to_string(&real).unwrap(),
            SRC_TEXT,
            "nothing was written through the link"
        );
        let msg = shown(&err);
        assert!(msg.contains("no link was recorded"), "{msg}");
        assert!(
            msg.contains("[[linked]]"),
            "the error names the document: {msg}"
        );
        for (what, path) in [
            ("the link path", link_path.display().to_string()),
            ("the link's target path", real.display().to_string()),
            ("the temp-dir root", r.root.display().to_string()),
        ] {
            assert!(!msg.contains(&path), "the error holds {what}: {msg}");
        }
    }

    /// The read passes and the write door fails with an I/O error (the writability probe refuses a
    /// read-only document with `Permission denied`): the error is an error, no row is recorded, the
    /// document keeps its bytes and is named by its label, and the error does not say it may be cut.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_failed_file_write_is_an_error_and_records_no_row() {
        use std::os::unix::fs::PermissionsExt;
        let r = repo();
        std::fs::set_permissions(&r.src, std::fs::Permissions::from_mode(0o444)).unwrap();
        if std::fs::OpenOptions::new().write(true).open(&r.src).is_ok() {
            // Running as a user who ignores file modes (root): the write cannot be made to fail.
            return;
        }

        let err = link(&r.ctx, "implements").await.unwrap_err();

        let msg = shown(&err);
        assert!(msg.contains("doc(link): cannot write `[[src]]`"), "{msg}");
        assert!(msg.contains("no link was recorded"), "{msg}");
        assert!(
            msg.contains("Permission denied"),
            "the error is the I/O error of the door, not a refusal: {msg}"
        );
        assert!(
            !msg.contains("partly written"),
            "a failed write leaves the document unchanged, so the error does not say it may be cut: {msg}"
        );
        assert!(
            !msg.contains(&r.root.display().to_string()),
            "no absolute path in the error: {msg}"
        );
        assert_eq!(link_rows(&r.ctx), 0, "no row remains");
        assert_eq!(std::fs::read_to_string(&r.src).unwrap(), SRC_TEXT);
    }

    #[tokio::test]
    async fn an_unreadable_source_file_is_an_error_and_not_a_skip() {
        let r = repo();
        std::fs::remove_file(&r.src).unwrap();

        let err = link(&r.ctx, "implements").await.unwrap_err();

        assert_eq!(link_rows(&r.ctx), 0, "no row remains");
        let msg = shown(&err);
        assert!(msg.contains("doc(link): cannot read `[[src]]`"), "{msg}");
        assert!(
            msg.contains("not on disk"),
            "the lower layer's remedy hint is kept: {msg}"
        );
        assert!(!msg.contains(&r.root.display().to_string()), "{msg}");
    }

    #[tokio::test]
    async fn a_document_that_cannot_take_the_line_is_an_error_and_records_no_row() {
        // An unclosed code fence swallows the new line: it would not read back.
        let r = repo();
        let text = format!("{SRC_TEXT}\n```\nunclosed\n");
        std::fs::write(&r.src, &text).unwrap();

        let err = link(&r.ctx, "implements").await.unwrap_err();

        let msg = shown(&err);
        assert!(msg.contains("nothing was added"), "{msg}");
        assert!(msg.contains("no link was recorded"), "{msg}");
        assert!(!msg.contains(&r.root.display().to_string()), "{msg}");
        assert_eq!(std::fs::read_to_string(&r.src).unwrap(), text);
        assert_eq!(link_rows(&r.ctx), 0);
    }

    // ---- a supersedes link checks the source can take the line before it writes the destination ----

    async fn supersedes(ctx: &ToolContext, src: &str, dst: &str) -> Result<Value> {
        call(
            ctx,
            json!({"src_id": src, "dst_id": dst, "rel": "supersedes"}),
        )
        .await
    }

    fn dst_text(r: &Repo) -> String {
        std::fs::read_to_string(r.root.join("docs/dst.md")).unwrap()
    }

    #[tokio::test]
    async fn a_supersedes_link_whose_source_cannot_take_the_line_leaves_the_destination_alone() {
        // An unclosed code fence swallows the new line.
        let r = repo();
        let text = format!("{SRC_TEXT}\n```\nunclosed\n");
        std::fs::write(&r.src, &text).unwrap();

        let err = supersedes(&r.ctx, "src", "dst").await.unwrap_err();

        let msg = shown(&err);
        assert!(msg.contains("no link was recorded"), "{msg}");
        assert_eq!(dst_text(&r), DST_TEXT, "the destination is byte-identical");
        assert_eq!(std::fs::read_to_string(&r.src).unwrap(), text);
        assert_eq!(link_rows(&r.ctx), 0);
        let row = artifact::get(&r.ctx.catalog.lock(), "dst")
            .unwrap()
            .unwrap();
        assert_eq!(row.status, "active", "the catalog status did not move");
        assert!(
            !msg.contains("already changed"),
            "nothing was written, so the error claims no destination change: {msg}"
        );
    }

    #[tokio::test]
    async fn a_supersedes_link_whose_source_file_is_missing_leaves_the_destination_alone() {
        let r = repo();
        std::fs::remove_file(&r.src).unwrap();

        let err = supersedes(&r.ctx, "src", "dst").await.unwrap_err();

        assert!(shown(&err).contains("no link was recorded"), "{err:#}");
        assert_eq!(dst_text(&r), DST_TEXT, "the destination is byte-identical");
        assert_eq!(link_rows(&r.ctx), 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_supersedes_link_whose_source_is_a_symlink_leaves_the_destination_alone() {
        let r = repo();
        std::fs::create_dir(r.root.join("sub")).unwrap();
        let real = r.root.join("sub/real.md");
        std::fs::write(&real, SRC_TEXT).unwrap();
        let link_path = r.root.join("docs/linked.md");
        std::os::unix::fs::symlink(&real, &link_path).unwrap();
        artifact::upsert(
            &r.ctx.catalog.lock(),
            &TestArtifactRowBuilder::new("linked")
                .with_abs_path(link_path)
                .build(),
        )
        .unwrap();

        let err = supersedes(&r.ctx, "linked", "dst").await.unwrap_err();

        assert!(shown(&err).contains("no link was recorded"), "{err:#}");
        assert_eq!(dst_text(&r), DST_TEXT, "the destination is byte-identical");
        assert_eq!(std::fs::read_to_string(&real).unwrap(), SRC_TEXT);
        assert_eq!(link_rows(&r.ctx), 0);
    }

    /// The real plan stays AFTER the destination write: here the source and the destination are
    /// one file, so a plan made once before that write would build its bytes from the text without
    /// the new status, and writing them would put `status: draft` back.
    #[tokio::test]
    async fn a_self_supersedes_keeps_both_the_new_status_and_the_line() {
        let r = repo();

        let v = supersedes(&r.ctx, "dst", "dst").await.unwrap();

        assert_eq!(v["text_twin"], json!("written"), "{v}");
        let text = dst_text(&r);
        assert!(text.contains("status: superseded"), "{text}");
        assert!(!text.contains("status: draft"), "{text}");
        assert!(
            text.ends_with("## Relations\n\n**Supersedes:** [[dst]]\n"),
            "{text}"
        );
        assert_eq!(link_rows(&r.ctx), 1);
    }

    /// A write failure is the one the preflight cannot see: the destination is already changed,
    /// and the error says so, by the document's label.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_write_failure_after_the_destination_write_names_the_destination_change() {
        use std::os::unix::fs::PermissionsExt;
        let r = repo();
        std::fs::set_permissions(&r.src, std::fs::Permissions::from_mode(0o444)).unwrap();
        if std::fs::OpenOptions::new().write(true).open(&r.src).is_ok() {
            // Running as a user who ignores file modes (root): the write cannot be made to fail.
            return;
        }

        let err = supersedes(&r.ctx, "src", "dst").await.unwrap_err();

        let msg = shown(&err);
        assert!(msg.contains("doc(link): cannot write `[[src]]`"), "{msg}");
        assert!(
            msg.contains("`[[dst]]` was already changed to `status: superseded`"),
            "the error names the destination's status change: {msg}"
        );
        assert!(
            !msg.contains(&r.root.display().to_string()),
            "no absolute path in the error: {msg}"
        );
        assert!(dst_text(&r).contains("status: superseded"));
        assert_eq!(link_rows(&r.ctx), 0, "no row remains");
    }

    /// The same sentence on every kind of error, and a lower layer's hint stays a hint.
    #[test]
    fn the_destination_note_is_added_to_a_recoverable_error_and_to_a_plain_one() {
        let recoverable = note_destination_change(
            LibrarianRecoverableError::with_hint("cannot write the source", "fix the source"),
            "`[[dst]]`",
        );
        let lower = recoverable
            .downcast_ref::<LibrarianRecoverableError>()
            .expect("stays recoverable");
        assert!(
            lower.message.starts_with("cannot write the source"),
            "{lower}"
        );
        assert!(
            lower
                .message
                .contains("`[[dst]]` was already changed to `status: superseded`"),
            "{lower}"
        );
        assert_eq!(lower.hint.as_deref(), Some("fix the source"));

        let plain = note_destination_change(anyhow::anyhow!("disk full"), "`[[dst]]`");
        let text = format!("{plain:#}");
        assert!(text.starts_with("disk full"), "{text}");
        assert!(
            text.contains("`[[dst]]` was already changed to `status: superseded`"),
            "{text}"
        );
    }

    // ---- a worktree session whose source file exists only in the main checkout ----

    struct Worktree {
        _tmp: TempDir,
        ctx: ToolContext,
        wt: std::path::PathBuf,
        main_src: std::path::PathBuf,
        src: String,
        dst: String,
    }

    /// A main checkout `repo` (a `.git` directory) and its linked worktree `repo/.worktrees/feat`
    /// (a `.git` pointer file), with a session in the worktree. `docs/src.md` exists in the main
    /// checkout only; `docs/dst.md` exists in both. The rows are the main checkout's.
    fn worktree() -> Worktree {
        use crate::librarian::current_project::CurrentProject;
        use std::sync::Arc;
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        let wt = repo.join(".worktrees/feat");
        std::fs::create_dir_all(repo.join(".git/worktrees/feat")).unwrap();
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        std::fs::create_dir_all(wt.join("docs")).unwrap();
        std::fs::write(
            wt.join(".git"),
            format!("gitdir: {}/.git/worktrees/feat\n", repo.display()),
        )
        .unwrap();
        let main_src = repo.join("docs/src.md");
        std::fs::write(&main_src, SRC_TEXT).unwrap();
        for root in [&repo, &wt] {
            std::fs::write(root.join("docs/dst.md"), DST_TEXT).unwrap();
        }
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(Arc::new(CurrentProject {
                abs_path: wt.clone(),
                git_root: wt.clone(),
                main_root: Some(repo.clone()),
                umbrella: None,
            }))
            .build();
        let src = crate::librarian::ids::artifact_id_from_abs(&main_src);
        let dst = crate::librarian::ids::artifact_id_from_abs(&repo.join("docs/dst.md"));
        for (id, rel) in [(&src, "docs/src.md"), (&dst, "docs/dst.md")] {
            artifact::upsert(
                &ctx.catalog.lock(),
                &TestArtifactRowBuilder::new(id)
                    .with_abs_path(repo.join(rel))
                    .build(),
            )
            .unwrap();
        }
        Worktree {
            _tmp: tmp,
            ctx,
            wt,
            main_src,
            src,
            dst,
        }
    }

    const NOT_IN_WORKTREE: &str = "skipped: source file is not in this worktree";

    /// The link rows of one rel; the fork's lineage links (`worktree_of`) are not counted.
    fn rel_rows(ctx: &ToolContext, rel: &str) -> i64 {
        ctx.catalog
            .lock()
            .conn
            .query_row(
                "SELECT count(*) FROM artifact_link WHERE rel = ?1",
                [rel],
                |r| r.get(0),
            )
            .unwrap()
    }

    /// Before the text twin, a plain link from a worktree session to a source that exists only in
    /// the main checkout recorded its row. It still does: the line is skipped with its own reason,
    /// and no file is created at the shadow path. The second call runs with the shadow row already
    /// forked, the path a check made only before the fork would miss.
    #[tokio::test]
    async fn a_plain_link_from_a_worktree_whose_source_file_is_not_there_keeps_its_row() {
        let w = worktree();
        let args = json!({"src_id": w.src, "dst_id": w.dst, "rel": "implements"});

        for round in 0..2 {
            let v = call(&w.ctx, args.clone())
                .await
                .unwrap_or_else(|e| panic!("round {round}: {e:#}"));
            assert_eq!(
                v,
                json!({"ok": true, "text_twin": NOT_IN_WORKTREE}),
                "round {round}"
            );
        }

        assert_eq!(
            rel_rows(&w.ctx, "implements"),
            1,
            "the row is recorded once"
        );
        let shadow_src = w.wt.join("docs/src.md");
        let shadow_id = crate::librarian::ids::artifact_id_from_abs(&shadow_src);
        let out = links::outgoing(&w.ctx.catalog.lock(), &shadow_id).unwrap();
        assert!(
            out.iter()
                .any(|l| l.dst_id == w.dst && l.rel == "implements"),
            "the row goes from the source's shadow, as before: {:?}",
            out.iter().map(|l| (&l.dst_id, &l.rel)).collect::<Vec<_>>()
        );
        assert!(!shadow_src.exists(), "no file is created in the worktree");
        assert_eq!(
            std::fs::read_to_string(&w.main_src).unwrap(),
            SRC_TEXT,
            "the main checkout's file is untouched"
        );
    }

    /// The same source with `supersedes`: the destination's shadow takes the new status and the row
    /// is recorded, as before the text twin; the line is skipped for the same reason. The preflight
    /// must not turn the missing source file into an error after the source was forked.
    #[tokio::test]
    async fn a_supersedes_link_from_a_worktree_whose_source_file_is_not_there_skips_the_line() {
        let w = worktree();

        let v = supersedes(&w.ctx, &w.src, &w.dst).await.unwrap();

        assert_eq!(v["text_twin"], json!(NOT_IN_WORKTREE), "{v}");
        assert_eq!(v["superseded"]["status"], json!("superseded"), "{v}");
        let shadow_dst = std::fs::read_to_string(w.wt.join("docs/dst.md")).unwrap();
        assert!(shadow_dst.contains("status: superseded"), "{shadow_dst}");
        assert_eq!(rel_rows(&w.ctx, "supersedes"), 1);
        assert!(!w.wt.join("docs/src.md").exists());
        assert_eq!(std::fs::read_to_string(&w.main_src).unwrap(), SRC_TEXT);
    }

    /// The control: when the worktree holds the source file, the line is written there (the skip
    /// is for a missing shadow file only, not for every worktree session).
    #[tokio::test]
    async fn a_plain_link_from_a_worktree_that_holds_the_source_file_writes_the_line() {
        let w = worktree();
        let shadow_src = w.wt.join("docs/src.md");
        std::fs::write(&shadow_src, SRC_TEXT).unwrap();

        let v = call(
            &w.ctx,
            json!({"src_id": w.src, "dst_id": w.dst, "rel": "implements"}),
        )
        .await
        .unwrap();

        assert_eq!(v, json!("ok"));
        assert!(std::fs::read_to_string(&shadow_src)
            .unwrap()
            .contains("**Implements:** [[dst]]"));
        assert_eq!(std::fs::read_to_string(&w.main_src).unwrap(), SRC_TEXT);
    }
}
