use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::{artifact, links};

#[derive(Deserialize)]
struct Args {
    src_id: String,
    dst_id: String,
    rel: String,
}
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(format!("doc(action=\"link\") requires 'src_id', 'dst_id' and 'rel': {e}"), "e.g. doc(action=\"link\", src_id=\"<16-hex>\", dst_id=\"<16-hex>\", rel=\"supersedes\"). Both ids are 16-hex artifact ids from find/get. Most citations need no manual link - link_scan derives rel=\"cites\" from prose.")
    })?;
    let now = chrono::Utc::now().timestamp_millis();

    // The catalog lock is SCOPED here rather than held across the body, because
    // `write_field_to_frontmatter` below acquires it itself and `parking_lot::Mutex` is
    // not reentrant — holding it across that call deadlocks rather than failing. Same
    // shape as `event_create::call`, which takes the lock in narrow blocks for exactly
    // this reason.
    let (src_id, dst) = {
        let mut cat = ctx.catalog.lock();
        let src_id = super::worktree::resolve_write_target(&mut cat, ctx, &a.src_id)?;

        if artifact::get(&cat, &src_id)?.is_none() {
            return Err(RecoverableError::new(format!(
                "src artifact `{}` not found",
                src_id
            )));
        }
        let dst = artifact::get(&cat, &a.dst_id)?.ok_or_else(|| {
            RecoverableError::new(format!("dst artifact `{}` not found", a.dst_id))
        })?;
        (src_id, dst)
    };

    let superseding = a.rel == "supersedes";
    let previous_status = dst.status.clone();

    // FILE BEFORE CATALOG, and the order is load-bearing. This project declares the file
    // the source of truth and the catalog a derived index, so a failed disk write must
    // leave the catalog untouched rather than recording a transition that never reached
    // the file. Writing the row first and the file second would reproduce the very
    // divergence this fixes, just with a smaller window.
    if superseding {
        crate::librarian::tools::update::write_field_to_frontmatter(
            ctx,
            &a.dst_id,
            "status",
            &json!("superseded"),
        )?;
    }

    {
        let cat = ctx.catalog.lock();
        links::insert(
            &cat,
            &links::LinkRow {
                src_id: src_id.clone(),
                dst_id: a.dst_id.clone(),
                rel: a.rel.clone(),
                created_at: now,
            },
        )?;

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
        }
    }

    if superseding {
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
        }));
    }

    Ok(json!("ok"))
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

        assert_eq!(v, json!("ok"));
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
        assert_eq!(v, json!("ok"));

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
}
