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
    let mut cat = ctx.catalog.lock();
    let src_id = super::worktree::resolve_write_target(&mut cat, ctx, &a.src_id)?;

    if artifact::get(&cat, &src_id)?.is_none() {
        return Err(RecoverableError::new(format!(
            "src artifact `{}` not found",
            src_id
        )));
    }
    let dst = artifact::get(&cat, &a.dst_id)?
        .ok_or_else(|| RecoverableError::new(format!("dst artifact `{}` not found", a.dst_id)))?;

    links::insert(
        &cat,
        &links::LinkRow {
            src_id: src_id.clone(),
            dst_id: a.dst_id.clone(),
            rel: a.rel.clone(),
            created_at: now,
        },
    )?;

    if a.rel == "supersedes" {
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

    #[tokio::test]
    async fn supersedes_transitions_dst_status() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        artifact::upsert(&cat, &mk_row("b")).unwrap();
        let ctx = mk_ctx(cat);

        let v = call(
            &ctx,
            json!({"src_id": "a", "dst_id": "b", "rel": "supersedes"}),
        )
        .await
        .unwrap();

        assert_eq!(v, json!("ok"));
        let dst = artifact::get(&ctx.catalog.lock(), "b").unwrap().unwrap();
        assert_eq!(dst.status, "superseded");
    }

    #[tokio::test]
    async fn link_supersedes_emits_event() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        artifact::upsert(&cat, &mk_row("b")).unwrap();
        let ctx = mk_ctx(cat);

        call(
            &ctx,
            json!({"src_id": "a", "dst_id": "b", "rel": "supersedes"}),
        )
        .await
        .unwrap();

        // Expect a superseded_by event on artifact "a".
        let count: i64 = ctx
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT count(*) FROM events WHERE artifact_id='a' AND kind='superseded_by'",
                [],
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
