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
/// row was `doc(update, patch={params:{…}})`, whose RFC 7396 array semantics
/// replace the whole collection — so flipping one row's status meant re-sending
/// every other row, and getting that wrong silently deleted them.
/// docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    // `append_entry` names its payload `entry`; this action names its patch
    // `fields`. Nothing in the action name says which noun applies, so `entry` is
    // a natural guess — and an undeclared key is dropped before it reaches the
    // handler, which turned that guess into an empty patch and a success envelope.
    // Catch it by name so the error can say which parameter to use, instead of the
    // generic "nothing to patch" the catalog layer would give.
    // docs/issues/archive/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md
    //
    // Refused whenever `entry` is present, not only when `fields` is absent. The
    // first version carried an `&& args.get("fields").is_none()` conjunct, which
    // narrowed the guard to the case that had been tested and let the both-present
    // call drop `entry` exactly as before — measured 2026-08-16:
    // `entry={"status":"done","task":"SENTINEL"}` + `fields={"status":"open"}`
    // returned `changed_fields: ["status"]` with the row's `task` untouched. Same
    // defect shape as the edit_file guard that covered one write path of three.
    if args.get("entry").is_some() {
        return Err(RecoverableError::with_hint(
            "update_entry: `entry` is append_entry's parameter — this action takes `fields`"
                .to_string(),
            "Re-send the patch as fields={...}. `entry` is the whole row for a NEW entry; \
             `fields` is the subset to change on an existing one."
                .to_string(),
        ));
    }
    let a: Args = serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(format!("doc(action=\"update_entry\") requires 'id', 'entry_collection' and 'entry_id': {e}"), "e.g. doc(action=\"update_entry\", id=\"<16-hex>\", entry_collection=\"observations\", entry_id=\"T-17\", fields={\"status\": \"closed\"}). This patches ONE row; patch={params:...} would replace the whole collection.")
    })?;
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
    let mut out = json!({
        "entry_id": outcome.entry_id,
        "artifact_id": target,
        "changed_fields": outcome.changed_fields,
        // Reported so a caller can assert cheaply that an entry update did not
        // change the row count — the failure mode this action exists to remove.
        "entries_total": outcome.entries_total,
    });
    // The catalog is machine-local and git-ignored, so a params change that
    // never reaches the body is a change no repo has. Advisory, and only for
    // trackers that demonstrably keep a body snapshot.
    if let Some(note) = outcome.snapshot_stale {
        out["snapshot_stale"] = json!(note);
    }
    // The citation half, and independent of the row half above: `snapshot_stale` is
    // satisfied by an index row showing current values, while a row defines no token at
    // all. An entry can be perfectly in-sync and still be uncitable.
    if let Some(note) = outcome.undefined_in_body {
        out["undefined_in_body"] = json!(note);
    }
    Ok(out)
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

    /// Seed a tracker whose file really exists. The default `seed` points at
    /// `/repo/docs/trackers/<id>.md`, which does not — fine for params-only
    /// assertions, useless for anything that reads the body.
    ///
    /// `rows` is explicit because the snapshot signal gates on the body carrying
    /// a MAJORITY of them; a fixture that leaves the ratio implicit lands on the
    /// wrong side of that gate without saying so.
    fn seed_with_body(
        ctx: &ToolContext,
        id: &str,
        path: &std::path::Path,
        body: &str,
        rows: &[&str],
    ) {
        std::fs::write(path, body).unwrap();
        let cat = ctx.catalog.lock();
        artifact::upsert(
            &cat,
            &artifact::TestArtifactRowBuilder::new(id)
                .with_abs_path(crate::util::fs::RepoPath::from(path).into_string())
                .with_kind("tracker")
                .build(),
        )
        .unwrap();
        let tasks: Vec<Value> = rows
            .iter()
            .map(|r| json!({"id": r, "status": "open"}))
            .collect();
        augmentation::upsert(
            &cat,
            &augmentation::AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "p".into(),
                params: json!({ "tasks": tasks }).to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".into(),
                updated_at: "2026-01-01T00:00:00.000Z".into(),
                // None on purpose — the signal must not depend on it.
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

    /// docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
    ///
    /// The sub-shape no id comparison can see: the row IS in the body, showing
    /// its previous values. `append_entry`'s missing-id check would report
    /// nothing here, which is why this path needed its own signal.
    #[tokio::test]
    async fn patching_a_rendered_row_says_the_committed_table_now_disagrees() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "# Q\n\n| ID | status |\n| T-1 | open |\n| T-2 | open |\n| T-3 | open |\n",
            &["T-1", "T-2", "T-3", "T-4"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "tasks",
                   "entry_id": "T-1", "fields": {"status": "done"}}),
        )
        .await
        .unwrap();

        assert_eq!(result["changed_fields"], json!(["status"]));
        let note = result["snapshot_stale"]
            .as_str()
            .expect("a rendered row that changed value must say so");
        assert!(
            note.contains("PREVIOUS"),
            "the row is present but outdated — that is the distinguishing case: {note}"
        );
        assert!(note.contains("T-1"), "{note}");
    }

    /// The other branch: the row is not rendered at all, so it exists only in
    /// the git-ignored catalog.
    #[tokio::test]
    async fn patching_an_unrendered_row_says_it_is_absent_from_the_body_entirely() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "# Q\n\n| ID | status |\n| T-1 | open |\n| T-2 | open |\n| T-3 | open |\n",
            &["T-1", "T-2", "T-3", "T-4"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "tasks",
                   "entry_id": "T-4", "fields": {"status": "done"}}),
        )
        .await
        .unwrap();

        let note = result["snapshot_stale"].as_str().unwrap();
        assert!(
            note.contains("not in it at all"),
            "T-4 is absent, not merely stale — the two need different remedies: {note}"
        );
    }

    /// The gate again: a prose-only tracker keeps its rows in params by design.
    #[tokio::test]
    async fn patching_a_prose_only_tracker_says_nothing_about_snapshots() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("prose.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "# Notes\n\nprose only.\n",
            &["T-1", "T-2"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "tasks",
                   "entry_id": "T-1", "fields": {"status": "done"}}),
        )
        .await
        .unwrap();

        assert!(
            result.get("snapshot_stale").is_none(),
            "no body snapshot means nothing can be behind, got: {result}"
        );
    }

    /// docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md
    ///
    /// The bug, end to end. This body is the same row-only shape
    /// `patching_a_rendered_row_says_the_committed_table_now_disagrees` uses, and
    /// that is the point: the row question was always answered here while the
    /// citation question went unasked. Asserting BOTH fields on one response is
    /// what pins them as orthogonal — an entry can be perfectly in-sync with its
    /// rendered row and still be uncitable.
    #[tokio::test]
    async fn patching_a_row_only_ledger_also_reports_that_nothing_can_cite_it() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "# Q\n\n| ID | status |\n| T-1 | open |\n| T-2 | open |\n| T-3 | open |\n",
            &["T-1", "T-2", "T-3"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "tasks",
                   "entry_id": "T-1", "fields": {"status": "done"}}),
        )
        .await
        .unwrap();

        let note = result["undefined_in_body"]
            .as_str()
            .expect("a ledger with no heading anywhere must say so");
        assert!(
            note.contains("defines NO"),
            "no T-N is defined anywhere, so this is a whole-ledger format issue and \
                 must not read as one row's omission: {note}"
        );
        assert!(
            note.contains("T-N"),
            "the message has to name the prefix, since the remedy is per-ledger: {note}"
        );
        assert!(
            result["snapshot_stale"].is_string(),
            "the row half still fires independently — the two are orthogonal, which \
                 is the whole reason a second field was needed: {result}"
        );
    }

    /// The negative control. Without it, a mutation that made
    /// `undefined_in_body` unconditional would pass every other test here.
    #[tokio::test]
    async fn patching_an_entry_that_has_its_own_heading_reports_no_citation_gap() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("log.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "# L\n\n## T-1 — first\n\nbody\n\n## T-2 — second\n\nbody\n",
            &["T-1", "T-2"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "tasks",
                   "entry_id": "T-1", "fields": {"status": "done"}}),
        )
        .await
        .unwrap();

        assert!(
            result.get("undefined_in_body").is_none(),
            "`## T-1 — first` defines the token, so there is no citation gap: {result}"
        );
    }

    /// The third case, and the one that justifies three outcomes instead of a bool:
    /// this ledger demonstrably writes definitions and T-2 missed one. The remedy is
    /// one heading, so the message must blame the entry — telling this author it is a
    /// whole-ledger format issue would send them to migrate a format that is fine.
    #[tokio::test]
    async fn patching_an_undefined_entry_in_a_defining_ledger_blames_the_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("log.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "# L\n\n## T-1 — first\n\nbody\n\n| ID | status |\n| T-2 | open |\n",
            &["T-1", "T-2"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "tasks",
                   "entry_id": "T-2", "fields": {"status": "done"}}),
        )
        .await
        .unwrap();

        let note = result["undefined_in_body"]
            .as_str()
            .expect("T-2 has no heading");
        assert!(
            note.contains("has no `## T-2 — <title>` heading"),
            "must name the exact heading to write: {note}"
        );
        assert!(
            note.contains("defines its other entries"),
            "and must distinguish itself from the whole-ledger case: {note}"
        );
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

    /// `append_entry` names its payload `entry`; this action names its patch
    /// `fields`. Nothing in the action name says which noun applies, so `entry` is
    /// a natural guess — and an undeclared key is dropped before it reaches the
    /// handler, so the guess used to become an empty patch and a success envelope
    /// with `changed_fields: []`.
    ///
    /// Reported from a real session: ~1.4 KB of text sent as `entry=`, exit
    /// success, row unchanged.
    /// docs/issues/archive/2026-08-16-update-entry-ignores-an-unknown-patch-param-and-reports-success.md
    ///
    /// **Both rows matter.** The first guard shipped as
    /// `entry.is_some() && fields.is_none()`, which left the both-present case
    /// dropping `entry` exactly as before — measured 2026-08-16:
    /// `entry={"status":"done","task":"SENTINEL"}` + `fields={"status":"open"}`
    /// returned `changed_fields: ["status"]` with the row's `task` untouched. A
    /// conjunct that narrows a guard to the case you happened to test is the same
    /// defect shape as the edit_file guard that covered one write path of three.
    #[tokio::test]
    async fn call_rejects_the_entry_param_and_names_fields() {
        for (label, extra) in [
            ("entry alone", json!({"entry": {"status": "done"}})),
            (
                "entry alongside fields",
                json!({"entry": {"status": "done"}, "fields": {"status": "open"}}),
            ),
        ] {
            let ctx = mk_ctx();
            seed(&ctx, "art1");

            let mut args = json!({
                "action": "update_entry",
                "id": "art1",
                "entry_collection": "tasks",
                "entry_id": "T-1",
            });
            for (k, v) in extra.as_object().unwrap() {
                args[k] = v.clone();
            }

            let err = call(&ctx, args)
                .await
                .expect_err(&format!("{label}: `entry` must be refused"));

            let msg = format!("{err:?}");
            assert!(
                msg.contains("fields"),
                "{label}: the error must name the parameter that IS accepted: {msg}"
            );

            // And it must not have written anything on the way to that error.
            let cat = ctx.catalog.lock();
            let row = augmentation::get(&cat, "art1").unwrap().unwrap();
            let params: Value = serde_json::from_str(&row.params).unwrap();
            assert_eq!(params["tasks"][0]["status"], "open", "{label}");
        }
    }

    #[tokio::test]
    async fn call_rejects_an_empty_fields_patch() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let err = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "tasks",
                "entry_id": "T-1",
                "fields": {}
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("empty"), "got: {err}");
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
