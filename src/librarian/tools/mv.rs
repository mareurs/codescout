use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ToolContext;
use crate::librarian::catalog::artifact;
use crate::util::fs::to_forward_slash;

#[derive(Deserialize)]
struct Args {
    id: String,
    new_rel_path: String,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        super::RecoverableError::new(format!("move requires 'id' and 'new_rel_path': {e}"))
    })?;

    // Defense-in-depth: new_rel_path must stay within the resolved root. Reject
    // absolute paths and `..` segments so a move can never escape the project
    // even if root resolution is wrong (1a5acfc0).
    if a.new_rel_path.is_empty()
        || std::path::Path::new(&a.new_rel_path).components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
    {
        return Err(super::RecoverableError::new(format!(
            "new_rel_path '{}' must be a non-empty relative path with no '..' or absolute segments",
            a.new_rel_path
        )));
    }

    // The catalog guard lives in an EXPLICIT BLOCK, and the block is the fix rather
    // than a tidy-up. `refile` below is `async`, and this guard is a
    // `parking_lot::MutexGuard` -- not `Send`, so holding it across that `.await`
    // does not compile. `drop(cat)` does NOT satisfy that: the binding stays in
    // scope for the generator's state machine even once moved, so only ending the
    // scope works. The `Send` error is the surface symptom; the substance is that
    // `SqliteVecArtifactStore::refile` re-acquires THIS mutex, and parking_lot's is
    // not reentrant -- so a version that merely silenced the compiler would hang
    // that backend on every move.
    let (old_full, new_full, new_id) = {
        let cat = ctx.catalog.lock();
        let row = artifact::get(&cat, &a.id)?
            .ok_or_else(|| super::RecoverableError::new(format!("unknown id `{}`", a.id)))?;

        // Fork-on-first-write gate: a worktree session may not move an artifact
        // that belongs to the main checkout — that would rename the shared
        // file/row out from under the main checkout. Merge first, or run from
        // the main checkout.
        if let Some(cp) = ctx.current_project.as_deref() {
            if super::worktree::is_main_checkout_artifact(cp, &row.abs_path) {
                return Err(super::RecoverableError::new(
                "refused from a worktree session: this artifact belongs to the main checkout. \
                 Merge the worktree (librarian action=\"merge_worktree\") or run this from the main checkout.",
            ));
            }
        }

        // Find the managed root that contains this artifact — a workspace
        // `[[roots]]` entry or the active project. `new_rel_path` is interpreted
        // relative to that root. See `super::managed_roots`.
        let roots = super::managed_roots(ctx);
        let root_path = super::containing_root(&roots, &row.abs_path).ok_or_else(|| {
            anyhow::anyhow!("no managed root contains {}", row.abs_path.display())
        })?;

        let old_full = row.abs_path.clone();
        let new_full = root_path.join(&a.new_rel_path);

        if new_full.exists() {
            return Err(super::RecoverableError::new(format!(
                "destination '{}' already exists — choose a different path or delete it first",
                a.new_rel_path
            )));
        }

        if let Some(parent) = new_full.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(&old_full, &new_full)?;

        let now = chrono::Utc::now().timestamp_millis();

        // Catalog identity is `id == artifact_id_from_abs(abs_path)` — stated in
        // `doctor.rs` and relied on by `migrate_v6`'s implicit id migration. Keeping
        // the old id while rewriting `abs_path` leaves that invariant broken, and the
        // next reindex's `artifact::upsert` pre-clean (`DELETE FROM artifact WHERE
        // abs_path=? AND id != ?`) deletes the row — cascading its events, links,
        // observations and augmentation away, silently and later.
        //
        // So do what `doctor`'s `reseat_worktree` does for the same situation: seed a
        // row at the path-derived id, then graft the history across and drop the old
        // row. `graft_rows` re-points `artifact_link` on BOTH endpoints, so a
        // `worktree_of` lineage edge survives whether it was the shadow or the main
        // twin that moved.
        // docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md
        let new_id = crate::librarian::ids::artifact_id_from_abs(&new_full);

        // The file's own `id:` now asserts an identity this move just invalidated.
        // Repair it here, in the same call, because nothing downstream can: every
        // write path into a managed artifact refuses one (`edit_file`'s markdown and raw
        // routes both guard on the frontmatter id; `doc(update)`'s `extra`
        // writes custom keys but never `id`), so a later repair pass has no route to
        // the file. BL-23.
        let content = repair_frontmatter_id(&new_full, &new_id)?;

        // Both derived AFTER the repair. A digest taken before it describes a file
        // that no longer exists on disk, which leaves the row looking dirty on every
        // subsequent walk.
        let file_mtime = std::fs::metadata(&new_full)
            .ok()
            .and_then(|m| {
                m.modified().ok().and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_millis() as i64)
                })
            })
            .unwrap_or(now);
        let file_sha256 = crate::librarian::util::sha_of_bytes(content.as_bytes());

        let updated_row = crate::librarian::catalog::artifact::ArtifactRow {
            id: new_id.clone(),
            abs_path: new_full.clone(),
            updated_at: now,
            file_mtime,
            file_sha256,
            ..row.clone()
        };
        artifact::upsert(&cat, &updated_row)?;

        (old_full, new_full, new_id)
    };

    // Re-file the artifact's chunk vectors onto the new id, BETWEEN the upsert
    // and the graft. Both neighbours are load-bearing: before the upsert the new
    // artifact row does not exist and sqlite's FK rejects the re-point; after the
    // graft the old row is gone and there is nothing left to re-point.
    //
    // A move is a RE-KEY, so re-filing rather than deleting is the whole point —
    // the bytes did not change, and dropping the vectors would take the artifact
    // out of semantic search until someone ran a `reembed`, trading one silent
    // degradation for another. This is a payload/join-row write on both backends;
    // nothing is re-embedded.
    //
    // Until 2026-09-04 neither backend did anything here, and they failed in
    // OPPOSITE directions: Qdrant stranded the vectors under the dead id (7
    // orphan artifacts / 126 points measured on this checkout), while sqlite let
    // the graft's FK cascade delete the chunk rows and the
    // artifact_vec_v2_cascade_delete trigger take their vectors with them.
    // docs/issues/2026-09-04-artifact-vector-delete-has-no-production-caller-so-every-archive-strands-its-vectors.md
    let (vectors_refiled, vectors_refile_error) = {
        // The guard is already gone — the block above ended its scope, which is what
        // makes this `.await` legal at all. Nothing to drop here.
        if new_id != a.id {
            match ctx.artifact_store.as_ref() {
                // A store that CONSTRUCTED and then failed at call time is the same
                // operational fact as one that could not be constructed — the vector
                // backend is down — reached by a different path. Only the second had
                // a fallback until 2026-09-06, and `refile`'s own doc in
                // `artifact_store.rs` already forbade the first: "a `refile` that
                // failed there would turn a working archive into a refused one."
                // It did. CI's `--features server-stack` lane ran without a Qdrant
                // and red for four days, invisible to the documented gate because
                // `server-stack` is not in `default`.
                //
                // So degrade rather than refuse: the catalog half is the half that
                // matters, it is already written, and a `reindex` heals the vectors.
                Some(store) => match store.refile(&a.id, &new_id).await {
                    Ok(n) => (Some(n), None),
                    Err(e) => (None, Some(format!("{e:#}"))),
                },
                // No backend configured (a lean build, or one that could not be
                // constructed). Reported as null rather than 0: "no store to ask" and
                // "asked, the artifact had none" are different facts, and a 0 here
                // would assert the second while meaning the first.
                None => (None, None),
            }
        } else {
            (None, None)
        }
    };

    // Re-acquired under a FRESH binding rather than assigning back to `cat`.
    // Assigning back keeps the original binding live across the `.await` above as
    // far as the generator analysis is concerned — which is why `drop` alone did
    // not fix the `Send` error — so the rebind is load-bearing, not style.
    let mut cat = ctx.catalog.lock();

    // Two transactions (`upsert` autocommits, `graft_rows` runs its own IMMEDIATE
    // tx) — same shape as `reseat_worktree`. A crash between them leaves both rows
    // present with the history still on the old one: recoverable by re-running,
    // not data loss.
    let grafted = if new_id != a.id {
        Some(crate::librarian::catalog::graft::graft_rows(
            &mut cat, &a.id, &new_id,
        )?)
    } else {
        None
    };

    Ok(json!({
        "id": new_id,
        // The id is derived from the path, so a move mints a new one. Reported
        // explicitly: prose that cites the old id has to be re-pointed, and a
        // caller that assumed stability would otherwise find out via a later
        // `unknown id` error.
        "previous_id": a.id,
        "id_changed": grafted.is_some(),
        "history_grafted": grafted.map(|r| json!({
            "events": r.events_repointed,
            "observations": r.observations_repointed,
            "links": r.links_repointed,
            "event_edges": r.event_edges_repointed,
        })),
        "old_abs_path": to_forward_slash(&old_full),
        "new_abs_path": to_forward_slash(&new_full),
        // How many chunk vectors followed the artifact onto its new id.
        // Reported because a re-file nobody can observe is indistinguishable
        // from the bug it fixes: the pre-2026-09-04 behaviour also produced a
        // clean `moved: true`. `null` means no vector backend was reachable —
        // deliberately not 0, which would claim the artifact had none.
        "vectors_refiled": vectors_refiled,
        // Carried BESIDE `vectors_refiled` rather than omitted when absent, because
        // the two fields only disambiguate as a pair. `refiled: null` alone is now
        // two different facts: `error: null` means no backend was configured to ask,
        // and `error: <text>` means one was asked and failed — the artifact's vectors
        // are still filed under its dead id and a `reindex` is owed. Dropping the
        // field on success would leave a reader inferring the difference from an
        // absence, which is the shape this whole field family exists to avoid.
        "vectors_refile_error": vectors_refile_error,
        // A move is a tracked DELETION plus an untracked ADDITION, so every
        // selector defined over index entries — `git add -u`, `git commit -a` —
        // enumerates only the first half and silently undoes the archive with a
        // green commit. Both paths were always reported; what the caller could not
        // read off two adjacent fields is that they must be staged TOGETHER, so the
        // imperative is carried here rather than left to be inferred. At the catalog
        // layer this move IS atomic — the split only becomes visible one tool call
        // later, at the git layer, to a different observer.
        // docs/issues/archive/2026-09-02-tracked-only-staging-commits-half-an-archive-move.md
        "stage_together": [to_forward_slash(&old_full), to_forward_slash(&new_full)],
        // Deliberately path-free. `stage_together` is relativized by
        // `path_strip::PATH_KEYS` and a prose string is not, so a path embedded here
        // would render absolute beside a relative sibling.
        //
        // The confirmation names two PROPERTIES — staged-ness and content — and warns
        // against the LETTER, because `R` is monotone in both directions and failed in
        // each. It vanishes when the archive is most correct (writing the outcome, SHA
        // and patch-id into the body before moving drops similarity to 44%, under git's
        // 50% default), and it appears when the archive is most broken (a destination
        // holding a stale copy is ~95% similar, so a bad move renders identically).
        // Two sessions were misled in opposite directions by the same sentence:
        // docs/issues/archive/2026-09-08-the-archive-move-confirmation-signal-is-a-letter-not-staged-ness.md
        // docs/trackers/bug-fix-session-log.md § F-111
        "stage_hint": "Stage both halves of this move together — `git add -- <old> <new>` \
                       using the two entries of `stage_together` — then confirm in `git \
                       status --short` that both halves are STAGED, i.e. lettered in \
                       column 1: either one `R` line, or a `D` plus an `A`. A leading \
                       space (` D`) or a `??` is half-staged. Do NOT confirm on the `R` \
                       alone — it is a SIMILARITY verdict, not a staging or a content one: \
                       it drops out when the move also rewrote the body (measured 44%, \
                       under git's 50% default), and it appears identically when the \
                       destination holds a STALE copy. For content, check the destination \
                       for something you wrote just before the move. `git add -u` and `git \
                       commit -a` are defined over paths that already have an index entry, \
                       so they take the deletion and never enumerate the addition, which \
                       undoes the archive without reporting anything.",
        "moved": true
    }))
}

/// Rewrite a moved file's frontmatter `id:` to the id the move just minted, and
/// return the file's post-repair content.
///
/// **Only a present 16-hex CATALOG id is rewritten**, and both halves of that matter.
/// A file carrying no `id:` is not asserting anything false, and
/// `frontmatter::rewrite_frontmatter_normalizing` would *insert* a block rather than
/// skip — stamping an `id:` is exactly what subjects a file to the librarian guard, so
/// archiving a prose tracker like `docs/trackers/skill-frictions.md` would quietly make
/// `edit_file` refuse the workflow CLAUDE.md documents for it.
///
/// A file carrying a value that is *not* a catalog id — `id: ADR-{NUMBER}` in a template,
/// a hand-written slug — is the **same harm reached by a different route**, and the gate
/// missed it until 2026-08-18 because it tested "is there an `id:` value?" rather than "is
/// there a catalog id?". Splicing such a value to a 16-hex id destroys the placeholder AND
/// newly guards every copy of the template, since `is_librarian_id` is false for it today.
/// Reproduced before the fix by
/// `doctor::tests::repair_frontmatter_id_never_rewrites_a_value_that_was_never_a_catalog_id`.
/// The skip is silent here, as befits a best-effort post-rename step; `doctor` reports the
/// condition as `frontmatter_id_is_not_a_catalog_id`.
///
/// **Best-effort by design.** The rename has already happened by the time this
/// runs, so unparseable frontmatter must not abort the move and strand the
/// catalog mid-update. A failure is logged and the original content returned; the
/// catalog still re-keys correctly, and the file is left exactly as it was.
///
/// BL-23 / `docs/issues/archive/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md`
pub(super) fn repair_frontmatter_id(
    path: &std::path::Path,
    new_id: &str,
) -> anyhow::Result<String> {
    let content = std::fs::read_to_string(path)?;

    // READ through the parser — it is authoritative about what YAML actually sees,
    // including quoting and type coercion. WRITE through a line splice: re-emitting
    // the block would reformat every other key, and a `{Placeholder}` would stop
    // being one. BL-34.
    let needs_repair = match crate::librarian::frontmatter::parse(&content) {
        Ok((Some(fm), _)) => fm
            .id
            .as_deref()
            .is_some_and(|id| id != new_id && crate::util::librarian_guard::is_librarian_id(id)),
        Ok((None, _)) => false,
        Err(err) => {
            tracing::warn!(
                "move: frontmatter unparseable at {}, leaving its id alone: {err:#}",
                path.display()
            );
            false
        }
    };
    if !needs_repair {
        return Ok(content);
    }

    match crate::librarian::frontmatter::replace_scalar_line(&content, "id", new_id) {
        Some(rewritten) => {
            std::fs::write(path, &rewritten)?;
            Ok(rewritten)
        }
        // The parser found an id the line scan could not: a shape neither anticipated
        // (a folded or flow-mapped `id`, say). Leave the file alone rather than fall
        // back to a whole-block rewrite — the catalog re-key still stands on its own.
        None => {
            tracing::warn!(
                "move: frontmatter declares an id at {} that is not on a plain `id:` line \
                 — left unrepaired",
                path.display()
            );
            Ok(content)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::librarian::{
        catalog::{artifact, artifact::ArtifactRow, Catalog},
        tools::{mv, TestToolContextBuilder, ToolContext},
        workspace::{Root, WorkspaceConfig},
    };

    fn mk_ctx(tmp: &std::path::Path) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();

        let row = ArtifactRow {
            id: "aabbccdd11223344".into(),
            abs_path: tmp.join("docs/trackers/foo.md"),
            kind: "tracker".into(),
            status: "active".into(),
            title: Some("Foo Tracker".into()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        };
        artifact::upsert(&cat, &row).unwrap();

        let src = tmp.join("docs/trackers/foo.md");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(
            &src,
            "---\nid: aabbccdd11223344\nkind: tracker\n---\n# Foo\n",
        )
        .unwrap();

        TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "test-repo".into(),
                path: tmp.to_path_buf(),
            })
            .build()
    }

    #[tokio::test]
    async fn move_renames_file_and_updates_catalog() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(result["old_abs_path"]
            .as_str()
            .unwrap()
            .ends_with("docs/trackers/foo.md"));
        assert!(result["new_abs_path"]
            .as_str()
            .unwrap()
            .ends_with("docs/archive/foo.md"));

        assert!(tmp.path().join("docs/archive/foo.md").exists());
        assert!(!tmp.path().join("docs/trackers/foo.md").exists());

        // The id is derived from the path, so a move mints a new one and reports
        // both. History follows via `graft_rows` — see
        // `move_carries_history_onto_the_new_id_and_survives_a_reindex`.
        assert_eq!(result["previous_id"], "aabbccdd11223344");
        assert_eq!(result["id_changed"], true);

        let cat = ctx.catalog.lock();
        let new_id = result["id"].as_str().unwrap();
        let row = artifact::get(&cat, new_id).unwrap().unwrap();
        assert!(row.abs_path.ends_with("docs/archive/foo.md"));
        assert!(
            artifact::get(&cat, "aabbccdd11223344").unwrap().is_none(),
            "the old id must not linger as a second row"
        );
    }

    /// A move must name the STAGING ACTION, not merely the two paths.
    ///
    /// **The regression test the bug file proposed would have passed against the
    /// defect, and that is the finding.**
    /// `docs/issues/archive/2026-09-02-tracked-only-staging-commits-half-an-archive-move.md`
    /// § *Tests added* specifies that "a regression test would assert that a `move`
    /// response names both paths" — but `old_abs_path` / `new_abs_path` were already
    /// emitted unconditionally when that file was written, so the proposed assertion
    /// is green on the broken shape. What was missing was never the data.
    ///
    /// Measured 2026-09-02: one session ran six archive moves in a row, read both
    /// path fields in all six responses, and still had to consult that file's
    /// § *Workarounds* to learn that `git add -u` takes only the tracked deletion.
    /// Data presence did not produce the action across six consecutive
    /// opportunities, which is why this asserts on the imperative.
    ///
    /// `stage_hint` deliberately carries no path of its own: `stage_together` is
    /// relativized by `path_strip::PATH_KEYS` and a prose string is not, so a path
    /// embedded here would print absolute beside a relative sibling.
    ///
    /// Mutation this kills: deleting `stage_together` / `stage_hint` and keeping the
    /// two path fields — exactly the shape that shipped.
    #[tokio::test]
    async fn move_names_the_staging_action_not_only_the_two_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        let pair = result["stage_together"]
            .as_array()
            .expect("stage_together must be an array naming both halves of the move");
        assert_eq!(pair.len(), 2, "both halves, in `git add` order: {pair:?}");
        assert!(
            pair[0].as_str().unwrap().ends_with("docs/trackers/foo.md"),
            "first element is the vanished path — the tracked deletion: {pair:?}"
        );
        assert!(
            pair[1].as_str().unwrap().ends_with("docs/archive/foo.md"),
            "second element is the new path — the untracked addition: {pair:?}"
        );

        let hint = result["stage_hint"]
            .as_str()
            .expect("stage_hint must name the command rather than leave it inferred");
        assert!(
            hint.contains("git add --"),
            "the hint must name the pathspec form that works: {hint}"
        );
        assert!(
            hint.contains("git add -u"),
            "the hint must name the selector that silently half-stages: {hint}"
        );
        assert!(
            hint.contains("git status --short"),
            "the hint must name the command that confirms the staging landed: {hint}"
        );
        // WHAT that command must show is asserted by
        // `the_archive_confirmation_names_staged_ness_and_content_on_both_surfaces`.
        // This assertion read `hint.contains('R')` until 2026-09-08 and was doubly wrong:
        // it pinned the letter this bug removed, and a bare `contains('R')` matches any
        // capital R in any word, so it never discriminated the rename line at all.
    }

    /// The archive confirmation must name two PROPERTIES — staged-ness and content — on
    /// BOTH surfaces that state it, and must not rest on git's `R` letter.
    ///
    /// `R` is a SIMILARITY verdict, and it is monotone in opposite directions across the
    /// two failures that actually happened, so a check written on the letter is blind to
    /// both:
    ///
    /// - It VANISHES when the archive is most correct. Writing the outcome, fix SHA and
    ///   patch-id into a bug file before moving it is the *normal* archive flow, and it
    ///   drops similarity under git's 50% default — measured 44% at `f7d61237`, six
    ///   points under. A correct, fully staged move then renders `D` + `A`, a shape the
    ///   old wording named nowhere, so a correct archive read as unconfirmed.
    ///   docs/issues/archive/2026-09-08-the-archive-move-confirmation-signal-is-a-letter-not-staged-ness.md
    /// - It APPEARS when the archive is most broken. A destination holding a stale copy
    ///   of its source is similar enough to pair, so a bad move renders `R` identically
    ///   to a good one (F-111 estimates ~95%; not re-measured here — what is certain is
    ///   that it paired, which is how that session was misled). One session cited that
    ///   `R` as proof the destination held fresh bytes, twice in one day.
    ///   docs/trackers/bug-fix-session-log.md § F-111
    ///
    /// **All THREE surfaces are asserted here because nothing else checks that they
    /// agree.** They were written together and say the same thing, so a reader
    /// cross-checking one against another finds agreement and learns nothing — three
    /// surfaces, one claim, no independent check. This is that check.
    ///
    /// The bug file said *two* surfaces and this test covered two. The third
    /// (`get_guide("librarian")` § *Archiving / Moving Trackers*) surfaced only when the
    /// tool auto-injected it during the fix's own archive move, still reading "expect one
    /// `R` line, never ` D` + `??`" — a defect surviving in the guide that was explaining
    /// the very operation being fixed. Enumerating the call sites from the bug file's
    /// list, rather than from the corpus, would have shipped it.
    ///
    /// The guide assertions are SCOPED to the staging paragraph, and the scoping is
    /// load-bearing rather than tidiness: `stale` occurs 10 times elsewhere in
    /// `tracker-conventions.md` and 5 more in `librarian.md`, so an unscoped
    /// `contains("stale")` stays green with the entire paragraph deleted.
    ///
    /// Each slice is then bounded ABOVE rather than below, which is the non-obvious half.
    /// A missing OPEN anchor panics. A missing CLOSE anchor is the one that hurts — the
    /// slice runs to end-of-body and absorbs unrelated prose, so the assertions pass on
    /// the wrong text; a lower bound is monotone under exactly that failure and cannot
    /// see it. The first version used a lower bound and it did active harm: it fired
    /// before the content assertions and masked them, reddening site 3 for the wrong
    /// reason.
    ///
    /// Mutation once per guarded SITE, since one kill says nothing about the others:
    /// reverting ANY surface to the `R`-letter form reds this test, each naming its own
    /// surface in the failure message, and no reversion is caught by another surface's
    /// assertions.
    #[tokio::test]
    async fn the_archive_confirmation_names_staged_ness_and_content_on_every_surface() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        let hint = result["stage_hint"]
            .as_str()
            .expect("stage_hint must be emitted on every move");

        let guide_slice = |topic: &str, open: &str, close: &str| -> &'static str {
            let body = crate::prompts::topic_body(topic)
                .unwrap_or_else(|| panic!("the {topic} guide must compile in"));
            let from = body.find(open).unwrap_or_else(|| {
                panic!("{topic}: the staging paragraph's opening anchor {open:?} must exist")
            });
            let rest = &body[from..];
            match rest.find(close) {
                Some(i) => &rest[..i],
                None => rest,
            }
        };
        let tracker_conventions = guide_slice(
            "tracker-conventions",
            "stage BOTH halves",
            "**Then re-point",
        );
        let librarian = guide_slice("librarian", "**Stage both halves:", "Two consequences");

        // Each slice is bounded ABOVE, and the direction is the whole point. A missing
        // OPEN anchor panics in `guide_slice`. A missing CLOSE anchor is the dangerous
        // one: the slice runs to end-of-body and silently absorbs unrelated prose —
        // measured, `librarian` would reach 3151 bytes and pick up 2 further `stale`
        // hits, so every content assertion below would pass on text that is not this
        // paragraph. A lower bound cannot see that direction at all. Worse, a lower
        // bound large enough to be interesting fires FIRST and MASKS the content
        // assertions it was meant to protect: the first mutation run reddened site 3 on
        // "not a real slice" rather than on the missing discriminator, which is a red
        // that proves sensitivity to the file without proving the assertions discriminate.
        const SLICE_CEILING: usize = 2_000; // slices measured 1655 and 462
        for (surface, text) in [
            ("tracker-conventions guide", tracker_conventions),
            ("librarian guide", librarian),
        ] {
            assert!(
                !text.is_empty() && text.len() < SLICE_CEILING,
                "non-vacuity: {surface}'s slice is {} bytes, outside (0, {SLICE_CEILING}) — a \
                 closing anchor that moved lets the slice absorb unrelated prose, and every \
                 assertion below then passes on the wrong text",
                text.len()
            );
        }

        for (surface, text) in [
            ("stage_hint", hint),
            ("tracker-conventions guide", tracker_conventions),
            ("librarian guide", librarian),
        ] {
            let lower = text.to_ascii_lowercase();
            assert!(
                lower.contains("column 1"),
                "{surface} must state the STAGED-NESS discriminator — both halves lettered \
                 in column 1 — rather than a letter whose presence depends on how much of \
                 the body the archiver rewrote: {text}"
            );
            assert!(
                lower.contains("similarit"),
                "{surface} must say `R` is a SIMILARITY verdict; without that, a reader \
                 takes the letter for a staging check or a content check: {text}"
            );
            assert!(
                lower.contains("stale"),
                "{surface} must name the STALE-destination case, which renders `R` \
                 identically to a correct move (F-111): {text}"
            );
        }
    }

    /// BL-23 / `docs/issues/archive/2026-08-16-a-moved-artifacts-frontmatter-asserts-its-pre-move-id.md`.
    ///
    /// A move mints a new id, and the file's own `id:` keeps asserting the old one —
    /// which resolves to nothing. This has to be repaired **here**, in the same call
    /// as the graft, because by the time anyone notices, no write path can reach the
    /// file: `edit_file`'s markdown and raw routes both refuse a librarian-managed
    /// artifact, and `doc(update)`'s `extra` writes custom keys but never `id`.
    ///
    /// The `file_sha256` assertion is the load-bearing one. It fails if the rewrite
    /// happens after the hash is taken — the catalog would then record a digest of a
    /// file that no longer exists on disk, and the next reindex would see the row as
    /// dirty on every walk.
    #[tokio::test]
    async fn move_rewrites_the_frontmatter_id_it_just_invalidated() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["id_changed"], true);
        let new_id = result["id"].as_str().unwrap().to_string();

        let moved = tmp.path().join("docs/archive/foo.md");
        let text = std::fs::read_to_string(&moved).unwrap();
        let (fm, body) = crate::librarian::frontmatter::parse(&text).unwrap();
        let fm = fm.expect("frontmatter must survive the move");

        assert_eq!(
            fm.id.as_deref(),
            Some(new_id.as_str()),
            "the file must assert the id it now has, not the one it was moved away from"
        );
        assert_eq!(
            fm.kind.as_deref(),
            Some("tracker"),
            "rewriting `id` must not disturb the other frontmatter fields"
        );
        assert!(
            body.contains("# Foo"),
            "the body must be byte-untouched, got: {body:?}"
        );

        let cat = ctx.catalog.lock();
        let row = artifact::get(&cat, &new_id).unwrap().unwrap();
        assert_eq!(
            row.file_sha256,
            crate::librarian::util::sha_of_bytes(text.as_bytes()),
            "the recorded sha must describe the file AFTER the frontmatter rewrite — \
             hashing before it leaves the row permanently dirty"
        );
    }

    /// The other half, and the reason this is not simply the normalizing writer.
    ///
    /// `frontmatter::rewrite_frontmatter_normalizing` inserts a frontmatter block when none exists,
    /// so applying it unconditionally would stamp an `id:` onto files that never had
    /// one — and a stamped id is exactly what subjects a file to the librarian guard
    /// (BL-33). Archiving `docs/trackers/skill-frictions.md` would silently make it
    /// unreachable by `edit_file`, the workflow CLAUDE.md documents for it.
    ///
    /// A file with no `id:` is not asserting anything false. Only a wrong id is repaired.
    #[tokio::test]
    async fn move_does_not_stamp_an_id_onto_a_file_that_never_had_one() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let prose = tmp.path().join("docs/trackers/prose.md");
        std::fs::write(&prose, "---\nkind: tracker\nstatus: active\n---\n# Prose\n").unwrap();
        {
            let cat = ctx.catalog.lock();
            let row = ArtifactRow {
                id: "1111222233334444".into(),
                abs_path: prose.clone(),
                kind: "tracker".into(),
                status: "active".into(),
                title: Some("Prose Tracker".into()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: 0,
                updated_at: 0,
                file_mtime: 0,
                file_sha256: String::new(),
                confidence: 1.0,
            };
            artifact::upsert(&cat, &row).unwrap();
        }

        mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "1111222233334444",
                "new_rel_path": "docs/archive/prose.md"
            }),
        )
        .await
        .unwrap();

        let text = std::fs::read_to_string(tmp.path().join("docs/archive/prose.md")).unwrap();
        let (fm, _) = crate::librarian::frontmatter::parse(&text).unwrap();
        assert!(
            fm.expect("frontmatter block preserved").id.is_none(),
            "a file with no id must not gain one — stamping it would newly subject a \
             prose tracker to the librarian guard. Got: {text:?}"
        );
    }

    /// BL-34, asserted at the caller.
    ///
    /// `frontmatter::replace_scalar_line`'s own tests prove the splice; they cannot prove
    /// `move` *reaches for* it. That gap is exactly how the re-serialization shipped —
    /// `move_rewrites_the_frontmatter_id_it_just_invalidated` was green throughout,
    /// because it only ever asserted that the id changed.
    ///
    /// The fixture is hand-authored YAML: flow sequence, double-quoted title, a
    /// `{Placeholder}` (valid YAML for a flow mapping), a null key. Every one is
    /// something a parse→write round-trip rewrites.
    #[tokio::test]
    async fn move_preserves_hand_authored_frontmatter_outside_the_id_line() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let original = concat!(
            "---\n",
            "kind: tracker\n",
            "title: \"Foo Tracker\"\n",
            "id: aabbccdd11223344\n",
            "tags: [alpha, beta]\n",
            "created: {YYYY-MM-DD}\n",
            "topic: null\n",
            "---\n",
            "\n# Foo\n",
        );
        std::fs::write(tmp.path().join("docs/trackers/foo.md"), original).unwrap();

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();
        let new_id = result["id"].as_str().unwrap().to_string();

        let moved = std::fs::read_to_string(tmp.path().join("docs/archive/foo.md")).unwrap();

        let before: Vec<&str> = original.lines().collect();
        let after: Vec<&str> = moved.lines().collect();
        assert_eq!(
            before.len(),
            after.len(),
            "the line count must not change — a re-serialized block drops null keys and \
             expands flow values. Got:\n{moved}"
        );
        for (b, a) in before.iter().zip(&after) {
            if b.starts_with("id:") {
                continue;
            }
            assert_eq!(b, a, "only the id line may change. Got:\n{moved}");
        }

        // Quoting is the splice's call (`scalar_can_be_bare`), so accept either form —
        // what matters is that the line now names the id the move minted.
        assert!(
            moved.contains(&format!("id: {new_id}")) || moved.contains(&format!("id: '{new_id}'")),
            "the id line must carry the new id, got:\n{moved}"
        );
    }

    /// A move must carry the artifact's history onto the new id, and that id
    /// must survive the next reindex.
    ///
    /// Catalog identity is `id == artifact_id_from_abs(abs_path)` — stated in
    /// `src/librarian/tools/doctor.rs` and relied on by `migrate_v6`. A move that
    /// kept the old id while rewriting `abs_path` leaves that invariant broken,
    /// and the next reindex's `artifact::upsert` pre-clean (`DELETE FROM artifact
    /// WHERE abs_path=? AND id != ?`) deletes the row — cascading its events,
    /// links, observations and augmentation away.
    ///
    /// Measured 2026-08-16 against the live catalog: one reindex following a
    /// 22-tracker archive sweep took the event count from 1845 to 1834 while
    /// reporting `removed: 0`.
    /// docs/issues/archive/2026-08-16-reindex-rekeys-moved-artifacts-and-cascades-away-their-events.md
    ///
    /// **The reindex step is the whole test.** Asserting only that history
    /// follows the move passes the moment `graft_rows` is wired up, and would
    /// still pass if the row were left mismatched — the deletion happens later,
    /// on a walk the test never runs.
    #[tokio::test]
    async fn move_carries_history_onto_the_new_id_and_survives_a_reindex() {
        use crate::librarian::catalog::events;

        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let old_id = "aabbccdd11223344";

        // History the artifact accumulated while it was live.
        {
            let cat = ctx.catalog.lock();
            events::insert(
                &cat,
                &events::TestEventRowBuilder::new(old_id, "note").build(),
            )
            .unwrap();
        }

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": old_id,
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        let archived = tmp.path().join("docs/archive/foo.md");
        let new_id = crate::librarian::ids::artifact_id_from_abs(&archived);

        assert_eq!(
            result["id"].as_str(),
            Some(new_id.as_str()),
            "move must report the id the artifact now has, not the one it had"
        );

        {
            let cat = ctx.catalog.lock();
            assert!(
                artifact::get(&cat, old_id).unwrap().is_none(),
                "the old id must not survive — it no longer matches the path it hashes from"
            );
            let row = artifact::get(&cat, &new_id)
                .unwrap()
                .expect("the artifact must live under the path-derived id");
            assert!(row.abs_path.ends_with("docs/archive/foo.md"));
            assert!(
                events::latest_for_artifact(&cat, &new_id)
                    .unwrap()
                    .is_some(),
                "the event history must be grafted onto the new id, not cascade-deleted"
            );
        }

        // The step that matters: a walk over the repo must now hit ON CONFLICT(id)
        // rather than the abs_path pre-clean, and leave the history alone.
        {
            let cat = ctx.catalog.lock();
            let rules = crate::librarian::classify::load_rules(
                "[[rule]]\nglob = \"**/docs/**/*.md\"\nkind = \"tracker\"\n",
            )
            .unwrap();
            crate::librarian::indexer::index_repo_sync(
                &cat,
                &rules,
                tmp.path(),
                &globset::GlobSet::empty(),
                false,
                false,
                false,
            )
            .unwrap();

            assert!(
                artifact::get(&cat, &new_id).unwrap().is_some(),
                "the reindex must not re-key the artifact it just found in place"
            );
            assert!(
                events::latest_for_artifact(&cat, &new_id)
                    .unwrap()
                    .is_some(),
                "the event history must survive the reindex"
            );
        }
    }

    /// A move RE-FILES the artifact's chunk vectors onto the new id — it does not
    /// delete them, and it does not leave them under the dead one.
    ///
    /// This is the test that would have caught the shipped bug, and it has to be
    /// here rather than in `artifact_store`: the store's own tests prove `refile`
    /// works when called, which was never in doubt. What was broken is that
    /// nothing called it. `move_renames_file_and_updates_catalog` passes on the
    /// pre-fix code, and so does every other test in this module, because a
    /// successful move and a successful move that stranded its vectors produce
    /// byte-identical responses apart from the field added for exactly that reason.
    ///
    /// Both directions are asserted, and both matter, because the two backends
    /// failed in OPPOSITE directions from this one missing call: Qdrant left the
    /// vectors under the old id (orphans that answer queries and resolve to
    /// nothing), while sqlite's FK cascade deleted them outright when the graft
    /// dropped the old row. So `new == 3` alone would pass the Qdrant bug if it
    /// also copied, and `old == 0` alone would pass a delete.
    #[tokio::test]
    async fn move_refiles_chunk_vectors_onto_the_new_id() {
        use crate::librarian::artifact_store::test_support::InMemoryArtifactStore;
        // The trait, for `upsert` on the concrete fixture type.
        use crate::librarian::artifact_store::ArtifactVectorStore;

        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let row = ArtifactRow {
            id: "aabbccdd11223344".into(),
            abs_path: tmp.path().join("docs/trackers/foo.md"),
            kind: "tracker".into(),
            status: "active".into(),
            title: Some("Foo Tracker".into()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        };
        artifact::upsert(&cat, &row).unwrap();
        let src = tmp.path().join("docs/trackers/foo.md");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(
            &src,
            "---\nid: aabbccdd11223344\nkind: tracker\n---\n# Foo\n",
        )
        .unwrap();

        let store = Arc::new(InMemoryArtifactStore::default());
        // Three chunks, not one: a per-chunk bug that moved only the first would be
        // invisible against a single-chunk fixture, which behaves identically under
        // "re-file all" and "re-file one".
        for (c, v) in [("c1", 0.25f32), ("c2", 0.5), ("c3", 0.75)] {
            store
                .upsert("p", c, "aabbccdd11223344", &[v, 1.0 - v])
                .await
                .unwrap();
        }
        // A second artifact that must not move. Without it, a `refile` that
        // re-pointed the entire store would pass every assertion below.
        store
            .upsert("p", "c9", "other-art", &[9.0, 9.0])
            .await
            .unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "test-repo".into(),
                path: tmp.path().to_path_buf(),
            })
            .with_artifact_store(store.clone())
            .build();

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        let new_id = result["id"].as_str().unwrap().to_string();
        assert_eq!(
            result["vectors_refiled"], 3,
            "the count is reported so a caller can tell a re-file from the no-op it replaced"
        );

        assert_eq!(
            store.chunks_under(&new_id),
            3,
            "vectors did not follow the artifact"
        );
        assert_eq!(
            store.chunks_under("aabbccdd11223344"),
            0,
            "a vector still answers under the dead id — the Qdrant-shaped orphan"
        );
        assert_eq!(
            store.chunks_under("other-art"),
            1,
            "an unrelated artifact was re-filed"
        );

        // Vectors and chunk ids unchanged: re-filing must not re-embed, and the
        // catalog's `artifact_chunk` rows still name these chunk ids, so a re-keyed
        // point would be an orphan wearing the right artifact id.
        assert_eq!(
            store.chunk("c1"),
            Some((new_id.clone(), vec![0.25, 0.75])),
            "the vector was recomputed or dropped rather than re-filed"
        );
    }

    /// A vector store that is REACHABLE-BUT-FAILING must not refuse the move.
    ///
    /// `artifact_store.rs`'s own `refile` doc already states the requirement —
    /// *"`mv` calls this on every id-changing move, including … an unreachable
    /// Qdrant. A `refile` that failed there would turn a working archive into a
    /// refused one."* — and until this test the code could not honour it. `mv`
    /// handled a store that could not be CONSTRUCTED (`None` → `vectors_refiled:
    /// null`, whose own field doc reads *"no vector backend was reachable"*), and
    /// propagated a store that constructed and then failed at CALL time. Those are
    /// the same operational fact — Qdrant is down — reached by two paths, and only
    /// one had a fallback.
    ///
    /// **The failure was invisible to the documented gate.** `server-stack` is not
    /// in `default`, so `cargo test --workspace` never compiles the Qdrant backend;
    /// CI's `--features server-stack` lane did, found no Qdrant, and red for four
    /// days while every local gate stayed green. This test needs neither feature
    /// nor daemon: it injects the failure directly, so it runs in the lean lane too.
    ///
    /// **Why `null` and not `0`.** The distinction is already load-bearing in this
    /// file — `0` asserts "asked, the artifact had none", which would be a lie here
    /// and would make a stranded-vector bug read as a clean move. The error text is
    /// carried in a sibling field rather than dropped, because a re-file that
    /// silently did not happen is the defect the neighbouring test exists to catch.
    #[tokio::test]
    async fn a_failing_vector_store_does_not_refuse_the_move() {
        use crate::librarian::artifact_store::ArtifactVectorStore;

        /// Mirrors an unreachable Qdrant: constructs fine, errors on the call.
        /// `list_collections(artifact)` is the verbatim context string CI saw.
        struct UnreachableStore;
        #[async_trait::async_trait]
        impl ArtifactVectorStore for UnreachableStore {
            async fn upsert(&self, _: &str, _: &str, _: &str, _: &[f32]) -> anyhow::Result<()> {
                anyhow::bail!("list_collections(artifact)")
            }
            async fn delete(&self, _: &str) -> anyhow::Result<()> {
                anyhow::bail!("list_collections(artifact)")
            }
            async fn refile(&self, _: &str, _: &str) -> anyhow::Result<u64> {
                anyhow::bail!("list_collections(artifact)")
            }
            async fn knn(
                &self,
                _: Option<&str>,
                _: &[f32],
                _: usize,
            ) -> anyhow::Result<Vec<(String, f32)>> {
                anyhow::bail!("list_collections(artifact)")
            }
        }

        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let row = ArtifactRow {
            id: "aabbccdd11223344".into(),
            abs_path: tmp.path().join("docs/trackers/foo.md"),
            kind: "tracker".into(),
            status: "active".into(),
            title: Some("Foo Tracker".into()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        };
        artifact::upsert(&cat, &row).unwrap();
        let src = tmp.path().join("docs/trackers/foo.md");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(
            &src,
            "---\nid: aabbccdd11223344\nkind: tracker\n---\n# Foo\n",
        )
        .unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "test-repo".into(),
                path: tmp.path().to_path_buf(),
            })
            .with_artifact_store(Arc::new(UnreachableStore))
            .build();

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .expect(
            "a move must not be refused because the vector store is down — the catalog \
             half is the half that matters, and `reindex` heals the vectors",
        );

        assert_eq!(result["moved"], true);
        assert!(
            result["id"]
                .as_str()
                .is_some_and(|s| s != "aabbccdd11223344"),
            "the re-key must still have happened: {result:#}"
        );
        assert!(
            result["vectors_refiled"].is_null(),
            "null means 'no vector backend was reachable'; 0 would claim the artifact \
             had no vectors, which is a different and unverified fact: {result:#}"
        );
        // The degradation must be legible. Silence here is the failure mode the
        // sibling test above was written to catch, one layer down.
        assert!(
            result["vectors_refile_error"]
                .as_str()
                .is_some_and(|e| e.contains("list_collections")),
            "a re-file that did not happen must say so, or a stranded-vector bug \
             reads as a clean move: {result:#}"
        );
        // The file moved on disk regardless of the store.
        assert!(tmp.path().join("docs/archive/foo.md").exists());
        assert!(!src.exists());
    }

    #[tokio::test]
    async fn move_errors_if_destination_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let dst = tmp.path().join("docs/archive/foo.md");
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::write(&dst, "already here").unwrap();

        let err = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[tokio::test]
    async fn move_errors_on_unknown_id() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());

        let err = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "deadbeefdeadbeef",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("unknown id"));
    }

    #[tokio::test]
    async fn move_succeeds_for_active_project_absent_from_legacy_roots() {
        // Regression for docs/issues/archive/2026-06-03-artifact-delete-refuses-in-workspace-artifact.md
        // (mv shares delete's guard): under the `[[project]]` model the active project is in
        // `current_project`, not `workspace.roots`. `new_rel_path` must resolve relative to the
        // active project's git_root.
        let tmp = tempfile::tempdir().unwrap();
        let mut ctx = mk_ctx(tmp.path());
        ctx.workspace = Arc::new(WorkspaceConfig {
            roots: vec![],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        });
        ctx.current_project = Some(Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: tmp.path().to_path_buf(),
                git_root: tmp.path().to_path_buf(),
                main_root: None,
                umbrella: None,
            },
        ));

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(tmp.path().join("docs/archive/foo.md").exists());
        assert!(!tmp.path().join("docs/trackers/foo.md").exists());
        // The id is derived from the path, so a move mints a new one and reports
        // both. History follows via `graft_rows` — see
        // `move_carries_history_onto_the_new_id_and_survives_a_reindex`.
        assert_eq!(result["previous_id"], "aabbccdd11223344");
        assert_eq!(result["id_changed"], true);

        let cat = ctx.catalog.lock();
        let new_id = result["id"].as_str().unwrap();
        let row = artifact::get(&cat, new_id).unwrap().unwrap();
        assert!(row.abs_path.ends_with("docs/archive/foo.md"));
        assert!(
            artifact::get(&cat, "aabbccdd11223344").unwrap().is_none(),
            "the old id must not linger as a second row"
        );
    }

    #[tokio::test]
    async fn move_resolves_under_nested_project_not_ancestor_root() {
        // 1a5acfc0: active project nested under an ancestor [[roots]] entry.
        // The move must resolve against the nested project, not the ancestor.
        let tmp = tempfile::tempdir().unwrap();
        let ancestor = tmp.path().to_path_buf();
        let child = ancestor.join("child");
        std::fs::create_dir_all(&child).unwrap();
        let mut ctx = mk_ctx(&child); // seeds artifact at child/docs/trackers/foo.md

        // Workspace registers the ANCESTOR as a legacy [[roots]] entry; the
        // active project is the nested child (its own repo), absent from roots.
        ctx.workspace = Arc::new(WorkspaceConfig {
            roots: vec![Root {
                name: "ancestor".into(),
                path: ancestor.clone(),
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        });
        ctx.current_project = Some(Arc::new(
            crate::librarian::current_project::CurrentProject {
                abs_path: child.clone(),
                git_root: child.clone(),
                main_root: None,
                umbrella: None,
            },
        ));

        let result = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "docs/archive/foo.md"
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(
            child.join("docs/archive/foo.md").exists(),
            "move resolved under the nested active project"
        );
        assert!(
            !ancestor.join("docs/archive/foo.md").exists(),
            "move did NOT escape to the ancestor [[roots]] entry"
        );
    }

    #[tokio::test]
    async fn move_rejects_new_rel_path_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = mk_ctx(tmp.path());
        let err = mv::call(
            &ctx,
            serde_json::json!({
                "action": "move",
                "id": "aabbccdd11223344",
                "new_rel_path": "../escape/foo.md"
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("..") || err.to_string().contains("relative"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn mv_of_main_artifact_from_worktree_is_refused() {
        let ctx = crate::librarian::tools::worktree::test_support::wt_ctx(
            Catalog::open_in_memory().unwrap(),
        );
        let main_id = {
            let c = ctx.catalog.lock();
            crate::librarian::tools::worktree::test_support::seed_main_tracker(&c)
        };

        let err = mv::call(
            &ctx,
            serde_json::json!({"id": main_id, "new_rel_path": "docs/trackers/moved.md"}),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("worktree"),
            "refusal names the worktree overlay: {err}"
        );
    }

    #[tokio::test]
    async fn mv_of_worktree_born_artifact_is_allowed() {
        // Mirror of the delete-side test: an artifact born under the
        // worktree's own root (nested inside main_root) must not be refused.
        let tmp = tempfile::tempdir().unwrap();
        let main_root = tmp.path().to_path_buf();
        let wt_root = main_root.join(".worktrees/feat");
        std::fs::create_dir_all(wt_root.join("docs")).unwrap();
        let file_path = wt_root.join("docs/new.md");
        std::fs::write(
            &file_path,
            "---\nid: mvwtbornmvwtbo1\nkind: tracker\n---\n# New\n",
        )
        .unwrap();

        let id = "mvwtbornmvwtbo1";
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &ArtifactRow {
                id: id.into(),
                abs_path: file_path.clone(),
                kind: "tracker".into(),
                status: "active".into(),
                title: Some("Worktree-born".into()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: 0,
                updated_at: 0,
                file_mtime: 0,
                file_sha256: String::new(),
                confidence: 1.0,
            },
        )
        .unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: wt_root.clone(),
                    git_root: wt_root.clone(),
                    main_root: Some(main_root.clone()),
                    umbrella: None,
                },
            ))
            .build();

        let result = mv::call(
            &ctx,
            serde_json::json!({"id": id, "new_rel_path": "docs/moved.md"}),
        )
        .await
        .unwrap();

        assert_eq!(result["moved"], true);
        assert!(wt_root.join("docs/moved.md").exists());
        assert!(
            !file_path.exists(),
            "worktree-born artifact must actually be moved, not refused"
        );
    }
}
