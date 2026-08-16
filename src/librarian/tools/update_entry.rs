use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::augmentation;
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Args {
    id: String,
    entry_collection: String,
    entry_id: String,
    #[serde(default = "default_fields")]
    fields: Value,
}

fn default_fields() -> Value {
    json!({})
}

/// Patch one entry of a tracker's `entry_collection` in place.
///
/// The counterpart `append_entry` never had. Without it the only way to change a
/// row was `artifact(update, patch={params:{…}})`, whose RFC 7396 array semantics
/// replace the whole collection — so flipping one row's status meant re-sending
/// every other row, and getting that wrong silently deleted them.
/// docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)?;
    if !a.fields.is_object() {
        return Err(RecoverableError::new(
            "update_entry: `fields` must be a JSON object",
        ));
    }
    let mut cat = ctx.catalog.lock();
    let target = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
    let outcome = augmentation::update_entry(
        &mut cat,
        &target,
        &a.entry_collection,
        &a.entry_id,
        a.fields,
    )?;
    Ok(json!({
        "entry_id": outcome.entry_id,
        "artifact_id": target,
        "changed_fields": outcome.changed_fields,
        // Reported so a caller can assert cheaply that an entry update did not
        // change the row count — the failure mode this action exists to remove.
        "entries_total": outcome.entries_total,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact, augmentation, Catalog};
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx() -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build()
    }

    fn seed(ctx: &ToolContext, id: &str) {
        let cat = ctx.catalog.lock();
        artifact::upsert(
            &cat,
            &artifact::TestArtifactRowBuilder::new(id)
                .with_abs_path(format!("/repo/docs/trackers/{id}.md"))
                .with_kind("tracker")
                .build(),
        )
        .unwrap();
        augmentation::upsert(
            &cat,
            &augmentation::AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "p".into(),
                params: r#"{"tasks":[{"id":"T-1","status":"open"},{"id":"T-2","status":"open"}]}"#
                    .to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".into(),
                updated_at: "2026-01-01T00:00:00.000Z".into(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("tasks".into()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
    }

    #[tokio::test]
    async fn call_patches_one_row_and_reports_what_changed() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let out = call(
            &ctx,
            json!({
                "action": "update_entry",
                "id": "art1",
                "entry_collection": "tasks",
                "entry_id": "T-2",
                "fields": {"status": "done"}
            }),
        )
        .await
        .unwrap();

        assert_eq!(out["entry_id"], "T-2");
        assert_eq!(out["changed_fields"], json!(["status"]));
        assert_eq!(out["entries_total"], 2);

        let cat = ctx.catalog.lock();
        let row = augmentation::get(&cat, "art1").unwrap().unwrap();
        let params: Value = serde_json::from_str(&row.params).unwrap();
        assert_eq!(params["tasks"][0]["status"], "open");
        assert_eq!(params["tasks"][1]["status"], "done");
    }

    #[tokio::test]
    async fn call_rejects_non_object_fields() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let err = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "tasks",
                "entry_id": "T-1",
                "fields": ["status"]
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("fields"), "got: {err}");
    }

    #[tokio::test]
    async fn call_surfaces_the_known_ids_when_the_entry_id_is_wrong() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let err = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "tasks",
                "entry_id": "T-7",
                "fields": {"status": "done"}
            }),
        )
        .await
        .unwrap_err();
        let msg = format!("{err:?}");
        assert!(msg.contains("T-7"), "must name the missing id: {msg}");
        assert!(
            msg.contains("T-1") && msg.contains("T-2"),
            "must list the ids that do exist, or the caller re-reads the whole \
             collection to find its typo: {msg}"
        );
    }
}
