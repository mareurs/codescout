use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::{artifact, augmentation};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Args {
    id: String,
    /// Omit for a PROSE ledger — one whose entries live as `## PREFIX-N` body
    /// sections rather than params rows. The call then reserves an id and
    /// writes nothing. See `augmentation::allocate_entry_id`.
    #[serde(default)]
    entry_collection: Option<String>,
    id_prefix: String,
    #[serde(default = "default_entry")]
    entry: Value,
    #[serde(default)]
    cites: Vec<String>,
}

fn default_entry() -> Value {
    json!({})
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)?;
    if !a.entry.is_object() {
        return Err(RecoverableError::new(
            "append_entry: `entry` must be a JSON object",
        ));
    }
    // PROSE-LEDGER PATH. Nine of the ten numeric prefixes in `docs/TAXONOMY.md`
    // keep entries as `## PREFIX-N` body sections, not params rows, and so could
    // not reach the allocator at all — which is why they were allocated by hand,
    // and why R-N reused nine ids for unrelated lessons. Omitting
    // `entry_collection` declares this shape: the server reserves the next id
    // under a transaction and hands it back; the caller writes the body. The
    // reservation is what makes the split safe (a lookup alone would only move
    // the race) — see `augmentation::allocate_entry_id`.
    if a.entry_collection.is_none() {
        if a.entry.as_object().is_some_and(|o| !o.is_empty()) {
            return Err(RecoverableError::with_hint(
                "append_entry: `entry` fields cannot be stored without an `entry_collection`"
                    .to_string(),
                "This ledger has no params collection, so those fields would be silently \
                 dropped. Omit `entry` to reserve an id, then write the fields into the \
                 markdown body yourself."
                    .to_string(),
            ));
        }
        if !a.cites.is_empty() {
            return Err(RecoverableError::with_hint(
                "append_entry: `cites` is not supported on a prose ledger".to_string(),
                "Reserve the id, write the body, and cite in prose — link_scan derives the \
                 edges from the text."
                    .to_string(),
            ));
        }
        let mut cat = ctx.catalog.lock();
        // An entry id is a LEDGER-WIDE fact, and a worktree is by definition not the
        // ledger. Left unguarded, `resolve_write_target` forks a shadow whose distinct
        // `artifact_id` misses the reservation, so main and the worktree both issue the
        // same id — and unlike the params branch, nothing can repair it afterwards:
        // `merge_worktree`'s renumber runs inside `if let Some(coll_name) = &coll` over
        // params rows, and the `worktree_fork` event snapshots `base_params` with no
        // body counterpart to diff a prose section against. The two `## PREFIX-N`
        // sections just merge into one file, giving the token two active definers.
        //
        // Same refusal, same reasoning, and the same ORDERING as the `cites` guard
        // below: it must fire BEFORE resolve_write_target, or a refused call still
        // leaves behind a shadow row, augmentation, fork event and lineage link (the
        // 2026-07-17 regression). Hence `is_main_checkout_artifact` here rather than
        // inspecting the resolved target.
        // docs/issues/2026-08-17-prose-ledger-worktree-id-collision.md
        if let Some(cp) = ctx.current_project.as_deref() {
            if let Some(row) = artifact::get(&cat, &a.id)? {
                if super::worktree::is_main_checkout_artifact(cp, &row.abs_path) {
                    return Err(RecoverableError::with_hint(
                        "append_entry: id allocation is not supported from a worktree checkout"
                            .to_string(),
                        "An entry id is ledger-wide state and must key to the main tracker. \
                         Reserve the id from the main checkout, or record the entry in a \
                         worktree-local file and fold it into the ledger after the merge."
                            .to_string(),
                    ));
                }
            }
        }
        let target = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
        let outcome = augmentation::allocate_entry_id(&mut cat, &target, &a.id_prefix)?;
        return Ok(json!({
            "id": outcome.id,
            "artifact_id": target,
            "reserved": true,
            "body_max": outcome.body_max,
            "next_step": format!(
                "Reserved {id} and recorded the ledger's high-water mark in frontmatter; the \
                 entry itself is yours to write. Add the section, and make the \
                 heading exactly `## {id} — <title>` — link_scan defines an entry token only \
                 in that shape, so a heading without the dash-and-title defines nothing and \
                 every citation of {id} dangles.",
                id = outcome.id
            ),
        }));
    }

    let mut cat = ctx.catalog.lock();
    // Refuse cites-from-worktree BEFORE resolve_write_target can fork a shadow.
    // The old ordering forked first and refused after, so a refused call still
    // materialized an empty shadow row + augmentation + worktree_fork event +
    // worktree_of link (2026-07-17 regression) — contradicting the "aborts the
    // whole call / writes nothing" contract. This mirrors resolve_write_target's
    // own `is_main_checkout_artifact` check to predict `target != a.id` without
    // the forking side effect.
    if !a.cites.is_empty() {
        if let Some(cp) = ctx.current_project.as_deref() {
            if let Some(row) = artifact::get(&cat, &a.id)? {
                if super::worktree::is_main_checkout_artifact(cp, &row.abs_path) {
                    return Err(RecoverableError::with_hint(
                        "append_entry: `cites` is not supported from a worktree checkout".to_string(),
                        "Entry-graph edges must key to the main tracker. Omit `cites`, or append from the main checkout.".to_string(),
                    ));
                }
            }
        }
    }
    let target = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
    let outcome = augmentation::append_entry(
        &mut cat,
        &target,
        a.entry_collection
            .as_deref()
            .expect("the None case returned above"),
        &a.id_prefix,
        a.entry,
        &a.cites,
    )?;
    let mut out = json!({"id": outcome.id, "artifact_id": target});
    if let Some(w) = outcome.warning {
        out["warning"] = json!(w);
    }
    if !outcome.snapshot_missing.is_empty() {
        out["snapshot_missing"] = json!(outcome.snapshot_missing);
        out["snapshot_hint"] = json!(format!(
            "This tracker keeps a rendered snapshot in its body, and {} row(s) are not in it. \
             Entry rows live in the catalog, which is machine-local and git-ignored — a row \
             absent from the body is in no repo. Add the row(s) to the body's table/section \
             via artifact(action=\"update\", patch={{body_edits: [...]}}).",
            outcome.snapshot_missing.len()
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{upsert as art_upsert, ArtifactRow};
    use crate::librarian::catalog::augmentation::{upsert as aug_upsert, AugmentationRow};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx() -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build()
    }

    fn seed(ctx: &ToolContext, id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        let cat = ctx.catalog.lock();
        art_upsert(
            &cat,
            &ArtifactRow {
                id: id.to_string(),
                abs_path: std::path::PathBuf::from(format!("/test/{id}.md")),
                kind: "tracker".to_string(),
                status: "active".to_string(),
                title: Some("T".to_string()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "x".to_string(),
                confidence: 1.0,
            },
        )
        .unwrap();
        aug_upsert(
            &cat,
            &AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "test".to_string(),
                params: r#"{"failures":[]}"#.to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("failures".to_string()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
    }

    /// Seed a tracker whose markdown file really exists, so the body-reading
    /// half of `append_entry` has something to read. The default `seed` points
    /// at `/test/<id>.md`, which does not exist — fine for id allocation,
    /// useless for snapshot checks.
    fn seed_with_body(
        ctx: &ToolContext,
        id: &str,
        path: &std::path::Path,
        body: &str,
        rows: &[&str],
    ) {
        std::fs::write(path, body).unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let cat = ctx.catalog.lock();
        art_upsert(
            &cat,
            &ArtifactRow {
                id: id.to_string(),
                abs_path: path.to_path_buf(),
                kind: "tracker".to_string(),
                status: "active".to_string(),
                title: Some("T".to_string()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "x".to_string(),
                confidence: 1.0,
            },
        )
        .unwrap();
        let entries: Vec<Value> = rows
            .iter()
            .map(|r| json!({"id": r, "status": "open"}))
            .collect();
        aug_upsert(
            &cat,
            &AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "test".to_string(),
                params: json!({ "failures": entries }).to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                // None on purpose: the signal must NOT depend on
                // `render_template`, whose job is to project params into
                // `librarian(context)` so the body can stay prose-only.
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: Some("failures".to_string()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
    }

    /// docs/issues/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
    ///
    /// The append succeeds and the row lands in the catalog, which is
    /// machine-local and git-ignored. Without this the response was a bare
    /// `{id, artifact_id}` — indistinguishable from a row that reached git.
    ///
    /// The body carries a MAJORITY of the rows (3 of 5 after the append), which
    /// is what a maintained snapshot lagging at the tail looks like; below that
    /// the tracker is treated as params-canonical and stays silent (see
    /// `body_keeps_snapshot`).
    #[tokio::test]
    async fn append_names_the_rows_the_body_snapshot_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        // Body renders F-1..F-3; params already ran ahead with F-4.
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "# Q\n\n| ID |\n| F-1 |\n| F-2 |\n| F-3 |\n",
            &["F-1", "F-2", "F-3", "F-4"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "failures",
                   "id_prefix": "F", "entry": {"status": "fail"}}),
        )
        .await
        .unwrap();

        assert_eq!(result["id"], "F-5");
        let missing: Vec<String> = serde_json::from_value(result["snapshot_missing"].clone())
            .expect("snapshot_missing must be present when the body is behind");
        assert_eq!(
            missing,
            vec!["F-4".to_string(), "F-5".to_string()],
            "F-4 was already adrift and F-5 was just created; F-1..F-3 are rendered"
        );
        assert!(result["snapshot_hint"].as_str().unwrap().contains("git"));
    }

    /// The gate. A tracker whose body anchors no ids keeps its rows in params
    /// deliberately — flagging it would fire on every append forever.
    #[tokio::test]
    async fn append_says_nothing_about_snapshots_for_a_prose_only_tracker() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("prose.md");
        let ctx = mk_ctx();
        seed_with_body(&ctx, "art1", &path, "# Notes\n\nprose only.\n", &["F-1"]);

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "failures",
                   "id_prefix": "F", "entry": {"status": "fail"}}),
        )
        .await
        .unwrap();

        assert!(
            result.get("snapshot_missing").is_none(),
            "no body snapshot means nothing can be behind, got: {result}"
        );
    }

    #[tokio::test]
    async fn call_assigns_and_returns_next_id() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let result = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": {"status": "fail"}
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["id"], "F-1");
    }

    /// A prose ledger: augmented (so it is declared) but with NO
    /// `entry_collection`, because its entries are `## R-N` body sections.
    fn seed_prose(ctx: &ToolContext, id: &str, abs_path: &std::path::Path) {
        let now = chrono::Utc::now().timestamp_millis();
        let cat = ctx.catalog.lock();
        art_upsert(
            &cat,
            &ArtifactRow {
                id: id.to_string(),
                abs_path: abs_path.to_path_buf(),
                kind: "tracker".to_string(),
                status: "active".to_string(),
                title: Some("Prose ledger".to_string()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "x".to_string(),
                confidence: 1.0,
            },
        )
        .unwrap();
        aug_upsert(
            &cat,
            &AugmentationRow {
                artifact_id: id.to_string(),
                prompt: "prose ledger".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                // Left in place deliberately: the allocator no longer consults the
                // augmentation at all — the declaration is `entry_prefix` in
                // frontmatter — so an augmentation being present must not change
                // the outcome. This fixture is the control for that.
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
    }

    #[tokio::test]
    async fn omitting_entry_collection_reserves_an_id_and_writes_no_entry() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        let original =
            "---\nkind: tracker\nentry_prefix: R\n---\n\n# Ledger\n\n## R-41 — an entry\n";
        std::fs::write(&md, original).unwrap();

        let ctx = mk_ctx();
        seed_prose(&ctx, "art1", &md);

        let result = call(&ctx, json!({"id": "art1", "id_prefix": "R"}))
            .await
            .unwrap();

        assert_eq!(
            result["id"], "R-42",
            "reserved from the body max, not params"
        );
        assert_eq!(result["reserved"], true);
        assert_eq!(result["body_max"], 41);
        assert!(
            result["next_step"].as_str().unwrap().contains("— <title>"),
            "the hint must teach def_re's heading shape, got: {}",
            result["next_step"]
        );
        // A reservation writes the ledger's committed high-water mark and NOTHING
        // else: the entry is still the caller's to write. Asserted as exact equality
        // against `original` plus the one spliced line, so any additional or reordered
        // byte fails here — a normalizing frontmatter rewrite would change several
        // (BL-34), and that is the failure mode this guards.
        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            "---\nkind: tracker\nentry_prefix: R\nentry_high_water_R: 42\n---\n\n# Ledger\n\n## R-41 — an entry\n",
            "the reservation must add exactly the high-water line"
        );

        // The reservation has to survive the read, or the tool re-issues the
        // same id to the next caller — which is the collision this exists to
        // prevent.
        let again = call(&ctx, json!({"id": "art1", "id_prefix": "R"}))
            .await
            .unwrap();
        assert_eq!(again["id"], "R-43");
        // ...and the committed mark advances with it, in place rather than duplicated.
        assert_eq!(
            std::fs::read_to_string(&md).unwrap(),
            "---\nkind: tracker\nentry_prefix: R\nentry_high_water_R: 43\n---\n\n# Ledger\n\n## R-41 — an entry\n",
            "the second reservation must splice the existing line, not append a second"
        );
    }

    #[tokio::test]
    async fn a_prose_ledger_refuses_entry_fields_it_would_silently_drop() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(&md, "---\nentry_prefix: R\n---\n\n## R-1 — x\n").unwrap();
        let ctx = mk_ctx();
        seed_prose(&ctx, "art1", &md);

        let err = call(
            &ctx,
            json!({"id": "art1", "id_prefix": "R", "entry": {"status": "open"}}),
        )
        .await
        .unwrap_err();

        assert!(
            err.to_string().contains("cannot be stored"),
            "dropping the caller's fields silently would be worse than refusing: {err}"
        );
    }

    #[tokio::test]
    async fn a_prose_ledger_refuses_cites() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(&md, "---\nentry_prefix: R\n---\n\n## R-1 — x\n").unwrap();
        let ctx = mk_ctx();
        seed_prose(&ctx, "art1", &md);

        let err = call(
            &ctx,
            json!({"id": "art1", "id_prefix": "R", "cites": ["R-1"]}),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("cites"), "{err}");
    }

    #[tokio::test]
    async fn call_warns_when_params_lags_the_body() {
        // Regression: docs/issues/archive/2026-07-20-append-entry-id-drift-params-vs-body.md
        // Skipping the colliding id is only half the repair — params is still
        // missing the rows the body documents, so say so.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("tracker.md");
        std::fs::write(&path, "## F-8 — body-only entry\n").unwrap();

        let ctx = mk_ctx();
        seed(&ctx, "art1");
        {
            let cat = ctx.catalog.lock();
            cat.conn
                .execute(
                    "UPDATE artifact SET abs_path = ?1 WHERE id = 'art1'",
                    [path.to_str().unwrap()],
                )
                .unwrap();
        }

        let result = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": {"status": "fail"}
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["id"], "F-9");
        let warning = result["warning"].as_str().expect("expected a warning");
        assert!(
            warning.contains("F-8"),
            "warning should name the body's max: {warning}"
        );
    }

    #[tokio::test]
    async fn call_omits_warning_when_params_is_current() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let result = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": {"status": "fail"}
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["id"], "F-1");
        assert!(result.get("warning").is_none());
    }

    #[tokio::test]
    async fn call_rejects_non_object_entry() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");

        let err = call(
            &ctx,
            json!({
                "id": "art1",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": "not an object"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn call_missing_artifact_returns_recoverable_error() {
        let ctx = mk_ctx();

        let err = call(
            &ctx,
            json!({
                "id": "nope",
                "entry_collection": "failures",
                "id_prefix": "F",
                "entry": {}
            }),
        )
        .await
        .unwrap_err();

        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn append_from_worktree_lands_on_shadow_not_main() {
        let ctx = crate::librarian::tools::worktree::test_support::wt_ctx(
            Catalog::open_in_memory().unwrap(),
        );
        let main_id = {
            let c = ctx.catalog.lock();
            crate::librarian::tools::worktree::test_support::seed_main_tracker(&c)
        };

        let out = call(
            &ctx,
            json!({
                "id": main_id,
                "entry_collection": "items",
                "id_prefix": "F",
                "entry": {"t": "from-worktree"}
            }),
        )
        .await
        .unwrap();
        assert_eq!(out["id"], "F-2"); // base had F-1

        let c = ctx.catalog.lock();
        let main_aug = augmentation::get(&c, &main_id).unwrap().unwrap();
        assert!(!main_aug.params.contains("from-worktree"), "main untouched");
    }

    /// The regression guard for
    /// `docs/issues/2026-08-17-prose-ledger-worktree-id-collision.md`.
    ///
    /// The params branch is protected, and `append_from_worktree_lands_on_shadow_not_main`
    /// above is the proof: it lands on the shadow, and `merge_worktree` renumbers the
    /// collision on the way back via `graft::fold_entries`. The prose branch could
    /// inherit the fork but never that repair — `merge_worktree`'s renumber runs inside
    /// `if let Some(coll_name) = &coll` over params rows, and the `worktree_fork` event
    /// snapshots `base_params` with no body counterpart to diff a prose section against.
    /// Measured before the guard existed: main issued `HY-11`, the worktree issued
    /// `HY-11` again, and `merge_worktree` reported `entries_renumbered: 0`.
    ///
    /// So allocation is refused instead, on exactly the grounds `cites` is refused: an
    /// entry id is ledger-wide state and must key to the main tracker.
    ///
    /// Own fixture rather than `wt_ctx` / `seed_main_tracker`: those seed
    /// `/repo/docs/trackers/t.md`, a path with no file behind it, and the prose branch
    /// reads the ledger body off disk. The worktree root is nested inside the repo,
    /// matching this project's own layout (`.claude/worktrees/`, `.worktrees/`);
    /// `is_main_checkout_artifact` discriminates by `under(main) && !under(worktree)`,
    /// so the nesting resolves correctly.
    #[tokio::test]
    async fn prose_allocation_is_refused_from_a_worktree() {
        use crate::librarian::current_project::CurrentProject;
        use crate::librarian::ids;
        use std::sync::Arc;

        let dir = tempfile::tempdir().unwrap();
        let main_root = dir.path().join("repo");
        let wt_root = main_root.join(".worktrees/feat");
        let rel = "docs/trackers/ledger.md";
        let body = "---\nkind: tracker\nentry_prefix: HY\n---\n\n# Ledger\n\n## HY-10 — the newest entry\n";

        // Both checkouts hold the same file at fork time — what git gives a fresh
        // worktree, and why both trees would otherwise derive the same body_max.
        for root in [&main_root, &wt_root] {
            std::fs::create_dir_all(root.join("docs/trackers")).unwrap();
            std::fs::write(root.join(rel), body).unwrap();
        }

        let main_abs = main_root.join(rel);
        let main_id = ids::artifact_id_from_abs(&main_abs);

        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(Arc::new(CurrentProject {
                abs_path: wt_root.clone(),
                git_root: wt_root.clone(),
                main_root: Some(main_root.clone()),
                umbrella: None,
            }))
            .build();

        // A prose ledger: catalogued, frontmatter-declared, NO augmentation and no
        // entry_collection. Nine of the ten prefixes in TAXONOMY.md are this shape.
        let now = chrono::Utc::now().timestamp_millis();
        {
            let cat = ctx.catalog.lock();
            art_upsert(
                &cat,
                &ArtifactRow {
                    id: main_id.clone(),
                    abs_path: main_abs.clone(),
                    kind: "tracker".to_string(),
                    status: "active".to_string(),
                    title: Some("Ledger".to_string()),
                    owners: vec![],
                    tags: vec![],
                    topic: None,
                    time_scope: None,
                    source: None,
                    created_at: now,
                    updated_at: now,
                    file_mtime: now,
                    file_sha256: "x".to_string(),
                    confidence: 1.0,
                },
            )
            .unwrap();
        }

        // Discriminating half: the SAME ledger allocates fine from the main checkout.
        // Without this the test could pass because the fixture refuses everything.
        let main_alloc = {
            let mut cat = ctx.catalog.lock();
            augmentation::allocate_entry_id(&mut cat, &main_id, "HY")
                .unwrap()
                .id
        };
        assert_eq!(main_alloc, "HY-11", "the main checkout must still allocate");

        let err = call(&ctx, json!({"id": main_id, "id_prefix": "HY", "entry": {}}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
        assert!(
            err.to_string().contains("worktree"),
            "expected the worktree guard, got: {err}"
        );

        // The guard must refuse BEFORE resolve_write_target forks. The 2026-07-17
        // regression was a refusal that fired after, so a refused call still
        // materialized a shadow row, an augmentation, a fork event and a lineage link —
        // contradicting the "writes nothing" contract. Same assertions as
        // `append_with_cites_from_worktree_is_refused`.
        let cat = ctx.catalog.lock();
        let artifacts: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            artifacts, 1,
            "must refuse before resolve_write_target forks a shadow artifact row"
        );
        let fork_events: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE kind = 'worktree_fork'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            fork_events, 0,
            "must refuse before resolve_write_target emits a worktree_fork event"
        );
        let lineage: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_link WHERE rel = 'worktree_of'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            lineage, 0,
            "must refuse before resolve_write_target inserts a worktree_of lineage link"
        );
    }

    #[tokio::test]
    async fn append_with_cites_writes_entry_cite_and_not_artifact_link() {
        let ctx = mk_ctx();
        seed(&ctx, "art1"); // seeds an augmented tracker with entry_collection "failures"
        seed(&ctx, "art2");
        let out = call(
            &ctx,
            json!({
                "id": "art1", "entry_collection": "failures", "id_prefix": "F",
                "entry": {"status": "fail"}, "cites": ["art2.md"]
            }),
        )
        .await
        .unwrap();
        assert_eq!(out["id"], "F-1");
        let cat = ctx.catalog.lock();
        // slug minted on art1; one entry_cite row; zero artifact_link rows.
        let slug: String = cat
            .conn
            .query_row("SELECT slug FROM artifact WHERE id='art1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let ec = crate::librarian::catalog::entry_cite::outgoing(&cat, &slug).unwrap();
        assert_eq!(ec.len(), 1);
        assert_eq!(ec[0].dst_ref, "art2");
        let al: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_link", [], |r| r.get(0))
            .unwrap();
        assert_eq!(al, 0, "cites must not touch artifact_link");
    }

    #[tokio::test]
    async fn append_with_unresolvable_cite_writes_nothing() {
        let ctx = mk_ctx();
        seed(&ctx, "art1");
        let err = call(
            &ctx,
            json!({
                "id": "art1", "entry_collection": "failures", "id_prefix": "F",
                "entry": {"status": "fail"}, "cites": ["no-such-target"]
            }),
        )
        .await
        .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
        let cat = ctx.catalog.lock();
        // atomic: entry NOT appended.
        let aug = augmentation::get(&cat, "art1").unwrap().unwrap();
        assert!(
            !aug.params.contains("F-1"),
            "entry must not be written when a cite is bad"
        );
    }

    #[tokio::test]
    async fn append_with_cites_from_worktree_is_refused() {
        let ctx = crate::librarian::tools::worktree::test_support::wt_ctx(
            Catalog::open_in_memory().unwrap(),
        );
        let main_id = {
            let c = ctx.catalog.lock();
            crate::librarian::tools::worktree::test_support::seed_main_tracker(&c)
        };
        // Cite the main tracker's own id — resolvable via the 16-hex branch, so
        // WITHOUT the worktree guard this append would succeed. This makes the
        // guard the only possible source of the error (discriminating test).
        let err = call(
            &ctx,
            json!({
                "id": main_id, "entry_collection": "items", "id_prefix": "F",
                "entry": {"t": "x"}, "cites": [main_id.clone()]
            }),
        )
        .await
        .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
        assert!(
            err.to_string().contains("worktree"),
            "expected the worktree-guard error, got: {err}"
        );
        let c = ctx.catalog.lock();
        let n: i64 = c
            .conn
            .query_row("SELECT COUNT(*) FROM entry_cite", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            n, 0,
            "guard must refuse before any entry_cite row is written"
        );
        // 2026-07-17 regression: the refusal used to fire AFTER
        // resolve_write_target had already forked and committed a shadow row
        // for the worktree — the entry write is atomic, but the shadow fork
        // wasn't gated on it. Assert the guard now refuses BEFORE any shadow
        // materializes at all: exactly the one seeded main artifact, no
        // worktree_fork event, no worktree_of lineage link.
        let n_artifacts: i64 = c
            .conn
            .query_row("SELECT COUNT(*) FROM artifact", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            n_artifacts, 1,
            "guard must refuse before resolve_write_target forks a shadow artifact row"
        );
        let n_fork_events: i64 = c
            .conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE kind = 'worktree_fork'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            n_fork_events, 0,
            "guard must refuse before resolve_write_target emits a worktree_fork event"
        );
        let n_lineage_links: i64 = c
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_link WHERE rel = 'worktree_of'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            n_lineage_links, 0,
            "guard must refuse before resolve_write_target inserts a worktree_of lineage link"
        );
    }
}
