use super::{LibrarianRecoverableError, ToolContext};
use crate::librarian::catalog::{artifact, augmentation};
use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct Args {
    id: String,
    /// Omit for a PROSE ledger — one whose entries live as `## PREFIX-N` body
    /// sections rather than params rows. The call then allocates an id, and either
    /// writes the section itself (when `title` + `body` + `anchor_heading` are
    /// given) or reserves the id and writes nothing. See
    /// `augmentation::allocate_entry_id`.
    #[serde(default)]
    entry_collection: Option<String>,
    id_prefix: String,
    #[serde(default = "default_entry")]
    entry: Value,
    #[serde(default)]
    cites: Vec<String>,
    /// Prose-ledger section writing. All three or none: the server formats the
    /// heading as `<level> <ID> — <title>` and inserts it before `anchor_heading`,
    /// in the same file write that records the high-water mark.
    ///
    /// Supplying them is strictly better than reserving and writing yourself: a
    /// hand-written heading missing its dash-and-title defines no token under
    /// `link_scan`'s `def_re`, and every citation of the entry dangles.
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    anchor_heading: Option<String>,
    /// Index-table row, written in the SAME file write as the section. Both or
    /// neither with `index_after_line`, and only alongside a section.
    ///
    /// `{id}` is substituted with the allocated id — a template rather than a
    /// literal because the caller cannot know the id before the call. Supplying
    /// these closes the interval a two-call protocol guarantees: the row cannot be
    /// written FIRST (the allocator counts a row as a claimed id, so it would
    /// consume the number it names), so a caller writing it afterwards always
    /// leaves the entry row-less in between.
    #[serde(default)]
    index_row: Option<String>,
    /// Existing line to insert `index_row` immediately AFTER, compared with
    /// surrounding whitespace trimmed. For a newest-first table that is the
    /// separator, e.g. `|----|-------|`. Explicit, never inferred.
    #[serde(default)]
    index_after_line: Option<String>,
}

fn default_entry() -> Value {
    json!({})
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(format!("doc(action=\"append_entry\") requires 'id' and 'id_prefix': {e}"), "Name the ledger and its id namespace, e.g. doc(action=\"append_entry\", id=\"<16-hex>\", id_prefix=\"R\"). For a PROSE ledger pass anchor_heading + title + body TOGETHER and the section is written for you; for a params ledger pass entry_collection + entry.")
    })?;
    if !a.entry.is_object() {
        return Err(LibrarianRecoverableError::new(
            "append_entry: `entry` must be a JSON object",
        ));
    }
    // Above the branch, so the params path and the prose path refuse alike: the params
    // allocator checks no declaration at all, so this is the only place both reach. A
    // ledger can still carry such a prefix — hand-written, or declared before the write
    // paths refused it — and allocating under it is exactly the uncitable-id defect.
    if !crate::util::librarian_guard::is_citable_entry_prefix(&a.id_prefix) {
        return Err(LibrarianRecoverableError::with_hint(
            format!(
                "append_entry: `{}` cannot be cited — an entry token is `[A-Z]{{1,3}}-<n>`, so \
                 an id under it would be written but no citation could address it",
                a.id_prefix
            ),
            format!(
                "Use one to three uppercase letters. If this ledger already declares `{p}`, \
                 move the whole namespace first: doc(action=\"rekey_prefix\", id=\"{id}\", \
                 from=\"{p}\", to=\"<one to three letters>\") — a dry run unless force=true. \
                 Nothing has been allocated.",
                p = a.id_prefix,
                id = a.id
            ),
        ));
    }
    // SECTION CONSTRUCTION, HOISTED ABOVE THE BRANCH — and the hoist is the fix, not a
    // tidy-up. These three steps used to live INSIDE the prose branch, which is this
    // defect stated as code: the routine that honours `title`/`body`/`anchor_heading`/
    // `index_row` sat inside the arm that excluded the params path, so a params caller
    // passing them got `Ok`, no section, no row and no diagnostic
    // (docs/issues/archive/2026-09-12-append-entry-drops-section-and-index-row-on-the-params-path.md).
    //
    // ALL THREE move together, not just the construction. The two refusals are what keep
    // the params path from gaining a way to write a row whose id nothing defines — the
    // dangling-citation shape the second refusal exists to prevent, arriving through the
    // branch that was supposed to be getting safer.
    //
    // Built ONCE and shared rather than constructed per-branch: a second copy reproduces
    // the original defect the moment either drifts.
    //
    // Built BEFORE `a.entry` is moved into `augmentation::append_entry` below, so the
    // params call can pass `section.as_ref()` without restructuring for the borrow.
    //
    // Both-or-neither, refused at the boundary rather than half-applied. Named
    // separately from the section triple because the missing half must be NAMED:
    // `Args` has no `deny_unknown_fields`, so before this existed a caller passing
    // `index_row` alone got `Ok` with no row and no error — a silent drop.
    let index_row = match (&a.index_row, &a.index_after_line) {
        (None, None) => None,
        (Some(row), Some(after)) => Some(augmentation::PendingIndexRow {
            row: row.clone(),
            after_line: augmentation::RowAnchor::Explicit(after.clone()),
        }),
        // A row with no per-call anchor defers to the ARTIFACT's own `snapshot_anchor`
        // frontmatter declaration, resolved in `splice_pending_section` against the
        // bytes about to be written. This is still an explicit target — declared once
        // by the author rather than repeated per call — so it does not reach the
        // auto-guessing the ADR above forbids; an artifact that declares nothing is
        // refused there by name, not silently placed.
        //
        // It was previously the other half of a both-or-neither refusal. That refusal
        // was right while no explicit target existed to defer TO, and the caller's only
        // remaining option was a second call — the capture window
        // `docs/issues/archive/2026-09-02-append-entry-two-call-protocol-manufactures-a-capture-window.md`
        // names. `index_after_line` stays available and still wins when passed.
        (Some(row), None) => Some(augmentation::PendingIndexRow {
            row: row.clone(),
            after_line: augmentation::RowAnchor::Declared,
        }),
        // The reverse half stays refused, and is NOT symmetric with the arm above:
        // an anchor with no row names a placement for nothing, and no artifact-level
        // declaration can supply the row's text.
        (None, Some(_)) => {
            return Err(LibrarianRecoverableError::with_hint(
                "doc(action=\"append_entry\"): `index_after_line` was passed with no \
                 `index_row`, so there is no row to place"
                    .to_string(),
                "Pass `index_row` too — the row text, with `{id}` where the allocated id \
                 goes. To place a row without naming the anchor per call, omit \
                 `index_after_line` and declare the block once on the artifact: \
                 doc(action=\"update\", id=…, patch={\"extra\": {\"snapshot_anchor\": \
                 \"<the table's header line, verbatim>\"}}).",
            ));
        }
    };
    // All three or none. A partial trio is an incomplete intent, and the two
    // halves fail differently: without `anchor_heading` the server would have to
    // GUESS placement, and this project's input-handling law is that a write
    // accepts an explicit target and never infers one — a wrong guess on a write
    // needs manual repair (docs/adrs/2026-07-10-repair-and-continue-input-handling.md).
    // Without `title` there is no `— <title>` to format, which is the entire
    // reason this path exists.
    let section = match (&a.title, &a.body, &a.anchor_heading) {
        (None, None, None) if index_row.is_some() => {
            return Err(LibrarianRecoverableError::with_hint(
                "doc(action=\"append_entry\"): `index_row` needs a section — pass \
                 `title` + `body` + `anchor_heading` too"
                    .to_string(),
                "A row on its own would cite an id whose entry nothing defines, which is \
                 the dangling-citation shape this path exists to prevent."
                    .to_string(),
            ));
        }
        (None, None, None) => None,
        (Some(title), Some(body), Some(anchor)) => Some(augmentation::PendingSection {
            title: title.clone(),
            body: body.clone(),
            anchor_heading: anchor.clone(),
            index_row,
        }),
        _ => {
            let missing: Vec<&str> = [
                ("title", a.title.is_none()),
                ("body", a.body.is_none()),
                ("anchor_heading", a.anchor_heading.is_none()),
            ]
            .into_iter()
            .filter(|(_, absent)| *absent)
            .map(|(name, _)| name)
            .collect();
            return Err(LibrarianRecoverableError::with_hint(
                format!(
                    "append_entry: writing a prose entry needs `title`, `body` and \
                     `anchor_heading` together — missing: {}",
                    missing.join(", ")
                ),
                "Pass all three to have the server write the section (heading formatted \
                 as `<ID> — <title>`, so it cannot be born undefined), or pass none of \
                 them to reserve an id only and write the section yourself."
                    .to_string(),
            ));
        }
    };

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
            return Err(LibrarianRecoverableError::with_hint(
                "append_entry: `entry` fields cannot be stored without an `entry_collection`"
                    .to_string(),
                "This ledger has no params collection, so those fields would be silently \
                 dropped. Omit `entry` to reserve an id, then write the fields into the \
                 markdown body yourself."
                    .to_string(),
            ));
        }
        if !a.cites.is_empty() {
            return Err(LibrarianRecoverableError::with_hint(
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
        // docs/issues/archive/2026-08-17-prose-ledger-worktree-id-collision.md
        if let Some(cp) = ctx.current_project.as_deref() {
            if let Some(row) = artifact::get(&cat, &a.id)? {
                if super::worktree::is_main_checkout_artifact(cp, &row.abs_path) {
                    return Err(LibrarianRecoverableError::with_hint(
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
        // SIBLING of the worktree guard above, deliberately NOT nested inside the
        // `current_project` block: this refusal does not depend on a current project
        // (the majority of callers, and every test built on `mk_ctx()`, have none), so
        // nesting it there would make it unreachable outside a workspace project. The
        // row is fetched again here rather than reused, which is the cost of being a
        // sibling rather than nested inside the block that already fetched one.
        //
        // Sited BEFORE resolve_write_target as defense-in-depth, matching the
        // worktree guard's placement above — NOT because this guard can reach that
        // hazard today. `resolve_write_target` forks only when `current_project` and
        // `main_root` are both `Some`, the row exists, and `is_main_checkout_artifact`
        // is true; the worktree guard above already refuses on exactly that
        // condition set (`is_main_checkout_artifact` itself returns `false` when
        // `main_root` is `None`). So every call that reaches this point has already
        // been proven, by the guard above, to hit `resolve_write_target`'s early
        // return with no side effect — reordering this guard is unobservable, not
        // merely untested. Kept here in case that identity ever stops holding (e.g.
        // the worktree guard becomes conditional), not because it is load-bearing now.
        //
        // ALLOCATE OPTIMISTICALLY — this used to refuse when `ledger_unpushed_commits`
        // found unpushed commits touching the ledger. Removed 2026-09-11: the refusal's
        // only remedy ("push this ledger's commits") named an action every session's
        // standing instruction forbids performing unasked, so the guard was a certain
        // block traded for a mitigation its own doc comment called "PARTIAL BY
        // CONSTRUCTION" — it never prevented the underlying collision (a peer at
        // origin allocates from origin's mark regardless of whether this caller is
        // refused), only converted an invisible divergence into a pushed one.
        //
        // The collision this guarded against is now caught where it is actually
        // publishable and where the party facing it has real agency: at `pre-push`,
        // on the merge commit that resolves a rejected push (`scripts/pre-push-
        // foreign-session-guard.sh`), not at allocate time, where the competing
        // allocation is by definition still invisible.
        // docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md
        // docs/issues/archive/2026-09-10-append-entry-refuses-on-unpushed-commits-with-a-remedy-no-session-may-perform.md
        // All three or none. A partial trio is an incomplete intent, and the two
        // halves fail differently: without `anchor_heading` the server would have to
        // GUESS placement, and this project's input-handling law is that a write
        // accepts an explicit target and never infers one — a wrong guess on a write
        // needs manual repair (docs/adrs/2026-07-10-repair-and-continue-input-handling.md).
        // Without `title` there is no `— <title>` to format, which is the entire
        // reason this path exists.
        let target = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
        let outcome =
            augmentation::allocate_entry_id(&mut cat, &target, &a.id_prefix, section.as_ref())?;
        // Phrase the hint in the LEDGER'S shape, never in one we picked. The hard-coded
        // `##` here told the `###` U-N ledger to write H2 — against its 36 siblings, its
        // own augmentation prompt, and docs/TAXONOMY.md, all three of which say H3. When
        // the body heads nothing there is no observation to report, and saying so is the
        // point: a default announced as a default is not a lie; a default announced as a
        // convention is. U-40 in docs/trackers/codescout-usage-frictions.md.
        let (heading, level_note) = match outcome.heading_level {
            Some(n) => (
                "#".repeat(n),
                " That is the level this ledger's existing entries use.",
            ),
            None => (
                "##".to_string(),
                " That level is a DEFAULT — this ledger heads no entry yet, so match the \
                 surrounding entries if any turn up.",
            ),
        };
        // Which input governed is the diagnostic the caller could not see. Only one
        // relation earns words: the committed mark leading BOTH the live body and this
        // machine's reservation table, so the mark alone accounts for the number.
        //
        // `frontmatter_max > body_max` on its own does NOT mean compaction — it is also
        // true immediately after any ordinary reservation, which is why the strict
        // comparison is against both other inputs.
        //
        // Stated as fact in the guidance prose and deliberately NOT under `warning`:
        // that register means "off-golden-path, reconsider before proceeding"
        // (PROGRESSIVE_DISCOVERABILITY Pattern 5a), and a compacted ledger is a CORRECT
        // state the archive cadence produced on purpose. Tagging it would train agents
        // to repair it. The cause is left as alternatives rather than asserted, because
        // the three integers cannot tell compaction from a fresh clone (Anti-Pattern 5).
        // docs/issues/archive/2026-08-17-allocate-outcome-frontmatter-max-dropped-at-the-mcp-boundary.md
        let compaction_note = match outcome.frontmatter_max {
            Some(fm)
                if fm > outcome.body_max.unwrap_or(0) && fm > outcome.reserved_max.unwrap_or(0) =>
            {
                format!(
                    " The committed frontmatter mark ({fm}) alone accounts for this id — it \
                     leads both the live body ({body}) and this machine's reservation table. \
                     Expected where entries were compacted out to an archive companion, or \
                     where the reservation table postdates them (a fresh clone, or an \
                     doc(move)); neither is drift.",
                    body = outcome
                        .body_max
                        .map_or_else(|| "none".to_string(), |b| b.to_string()),
                )
            }
            _ => String::new(),
        };
        // Two different outcomes, and the response must not describe one as the other.
        // A caller told to "write the section" after the server already wrote it would
        // write a duplicate heading — two active definers for one token, which is worse
        // than the dangling case this path exists to prevent.
        let next_step = if outcome.section_written {
            format!(
                "Wrote {id} and recorded the ledger's high-water mark, in one file write. \
                 The heading is `{heading} {id} — <title>`, which is the shape link_scan \
                 requires to define the token, so the entry is already citable. Do NOT \
                 write the section again.{compaction_note}",
                id = outcome.id
            )
        } else {
            format!(
                "Reserved {id} and recorded the ledger's high-water mark in frontmatter; the \
                 entry itself is yours to write. Add the section as \
                 `{heading} {id} — <title>` — link_scan defines an entry token only \
                 in that shape, so a heading without the dash-and-title defines nothing and \
                 every citation of {id} dangles.{level_note} Next time, pass `title`, `body` \
                 and `anchor_heading` to have the server write it and remove that \
                 failure mode entirely.{compaction_note}",
                id = outcome.id
            )
        };
        return Ok(json!({
            "id": outcome.id,
            "artifact_id": target,
            "reserved": !outcome.section_written,
            "section_written": outcome.section_written,
            "body_max": outcome.body_max,
            "reserved_max": outcome.reserved_max,
            "frontmatter_max": outcome.frontmatter_max,
            "next_step": next_step,
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
                    return Err(LibrarianRecoverableError::with_hint(
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
        // The wiring this hoist existed for. `section` is built once above the branch,
        // so the params path honours the same five fields the prose path does and a
        // params caller no longer gets `Ok` with nothing written.
        section.as_ref(),
    )?;
    // `section_written` mirrors the prose path's field, and it is what tells a caller the
    // server already wrote the heading. Without it the fix is invisible to exactly the
    // caller it serves: the two hints below tell you to write a section and a row, and a
    // caller who did ask for them has no way to know the request was honoured rather than
    // dropped — which is the shape of the bug this wiring closes.
    let mut out = json!({
        "id": outcome.id,
        "artifact_id": target,
        "section_written": outcome.section_written,
    });
    if let Some(w) = outcome.warning {
        out["warning"] = json!(w);
    }
    if !outcome.snapshot_missing.is_empty() {
        out["snapshot_missing"] = json!(outcome.snapshot_missing);
        out["snapshot_hint"] = json!(format!(
            "This tracker keeps a rendered snapshot in its body, and {} row(s) are not in it. \
             Entry rows live in the catalog, which is machine-local and git-ignored — a row \
             absent from the body is in no repo. Add the row(s) to the body's table/section \
             via doc(action=\"update\", patch={{body_edits: [...]}}).",
            outcome.snapshot_missing.len()
        ));
    }
    // Separate from snapshot_missing on purpose: that one is satisfied by an index row,
    // and a row defines no citable token. Both can be present at once, and they ask for
    // different things — a row, and a heading.
    if let Some(note) = outcome.undefined_in_body {
        out["undefined_in_body"] = json!(note);
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

    /// docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
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

    /// `append_entry` refuses an `id_prefix` the token grammar cannot express. The PARAMS
    /// path is the load-bearing one: its allocator checks no declaration at all, so nothing
    /// but this refusal stands between `DCTX` and a committed uncitable id. On the prose path
    /// the allocator's own "not declared" refusal would shadow it, and a test there would
    /// stay green with this check deleted.
    ///
    /// The `DCX` control proves the fixture is otherwise admissible, so the refusal is the
    /// prefix's and not the ledger's.
    #[tokio::test]
    async fn append_refuses_an_id_prefix_that_cannot_be_cited() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        seed_with_body(&ctx, "art1", &path, "# Q\n", &[]);

        let err = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "failures",
                   "id_prefix": "DCTX", "entry": {"status": "fail"}}),
        )
        .await
        .expect_err("an uncitable id_prefix must be refused on the params path");
        assert!(err.to_string().contains("cannot be cited"), "got: {err}");

        let ok = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "failures",
                   "id_prefix": "DCX", "entry": {"status": "fail"}}),
        )
        .await
        .expect("a three-letter prefix on the same ledger must allocate");
        assert_eq!(
            ok["id"], "DCX-1",
            "and nothing was allocated under the refused one"
        );
    }

    /// docs/issues/archive/2026-08-18-an-index-row-satisfies-the-drift-check-but-defines-no-citable-token.md
    ///
    /// `append_entry` emits `undefined_in_body` from its own call site, so it needs
    /// its own test — the update_entry tests cover the classification but not this
    /// wiring. Reuses the fixture above deliberately: the id it just minted is
    /// reported as needing a row AND as uncitable, which is the pair of facts a
    /// single `snapshot_missing` could never carry. Telling the author to "add the
    /// row" is what let ten A-N entries and 117 BL-N citations go dark.
    #[tokio::test]
    async fn append_also_says_the_new_id_is_not_yet_citable() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "# Q\n\n| ID |\n| F-1 |\n| F-2 |\n| F-3 |\n",
            &["F-1", "F-2", "F-3"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "failures",
                   "id_prefix": "F", "entry": {"status": "fail"}}),
        )
        .await
        .unwrap();

        assert_eq!(result["id"], "F-4");
        let note = result["undefined_in_body"]
            .as_str()
            .expect("a row-only ledger must say the new id is uncitable");
        assert!(
            note.contains("defines NO"),
            "no F-N is defined anywhere, so it is the whole-ledger message: {note}"
        );
        assert!(
            result["snapshot_missing"].is_array(),
            "and the row half still reports independently: {result}"
        );
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

    /// U-40. The hint asserted `## <id> — <title>` for every ledger. The U-N ledger
    /// keeps entries at `###` — as do its 36 siblings, its own augmentation prompt, and
    /// `docs/TAXONOMY.md` — so an agent following the hint wrote a heading matching
    /// nothing around it. The level was derivable from the body the allocator already
    /// reads; asserting it instead is the whole defect, and it is the same shape as the
    /// two other lies fixed today: a tool stating a convention it never looked up.
    #[tokio::test]
    async fn reservation_hint_uses_the_ledgers_own_heading_level() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: U\n---\n\n# Ledger\n\n### U-38 — a\n\n### U-39 — b\n",
        )
        .unwrap();

        let ctx = mk_ctx();
        seed_prose(&ctx, "art1", &md);

        let result = call(&ctx, json!({"id": "art1", "id_prefix": "U"}))
            .await
            .unwrap();
        let hint = result["next_step"].as_str().unwrap();

        // Backticked so `### U-40` cannot be satisfied by a `## U-40` substring match.
        assert!(
            hint.contains("`### U-40 — <title>`"),
            "the hint must name the level this ledger actually uses, got: {hint}"
        );
    }

    /// The complement, and the half that keeps the fix honest. With nothing headed —
    /// a first entry, or an index of rows — there IS no observed level, and the hint
    /// must say its suggestion is a default rather than quietly pick one. A tool that
    /// cannot tell you which of those it is doing is the original bug at one remove.
    #[tokio::test]
    async fn reservation_hint_admits_when_the_heading_level_is_a_default() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: F\n---\n\n# Ledger\n\n| ID | Title |\n|----|-------|\n| F-7 | a row |\n",
        )
        .unwrap();

        let ctx = mk_ctx();
        seed_prose(&ctx, "art1", &md);

        let result = call(&ctx, json!({"id": "art1", "id_prefix": "F"}))
            .await
            .unwrap();
        let hint = result["next_step"].as_str().unwrap();

        assert!(
            hint.contains("DEFAULT"),
            "with no headed entry the hint must flag its level as a default, got: {hint}"
        );
        assert!(
            !hint.contains("this ledger's existing entries use"),
            "nothing is headed here, so the hint must not claim to have observed a \
             level, got: {hint}"
        );
    }

    /// `AllocateOutcome` carries three derivation inputs and the prose branch reported
    /// one. Which input governed is the diagnostic — the caller saw `body_max` with
    /// nothing to compare it against. These are facts about the allocation, so they go
    /// out as data rather than under a severity-tagged guidance key.
    /// `docs/issues/archive/2026-08-17-allocate-outcome-frontmatter-max-dropped-at-the-mcp-boundary.md`
    #[tokio::test]
    async fn reservation_reports_all_three_derivation_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: R\n---\n\n# Ledger\n\n## R-41 — an entry\n",
        )
        .unwrap();

        let ctx = mk_ctx();
        seed_prose(&ctx, "art1", &md);

        let first = call(&ctx, json!({"id": "art1", "id_prefix": "R"}))
            .await
            .unwrap();
        assert_eq!(first["body_max"], 41);
        assert!(
            first.get("reserved_max").is_some(),
            "reserved_max must be present even when null, or absent reads as zero: {first}"
        );
        assert!(
            first.get("frontmatter_max").is_some(),
            "frontmatter_max must be present even when null: {first}"
        );
        assert!(first["frontmatter_max"].is_null(), "no mark existed yet");

        // The second call is where all three are populated: the first wrote the
        // committed mark and recorded the reservation.
        let second = call(&ctx, json!({"id": "art1", "id_prefix": "R"}))
            .await
            .unwrap();
        assert_eq!(second["id"], "R-43");
        assert_eq!(second["body_max"], 41, "the body did not move");
        assert_eq!(second["reserved_max"], 42);
        assert_eq!(second["frontmatter_max"], 42);
    }

    /// The one state worth naming in words: the committed mark leads the body, which
    /// means entries were compacted out to an archive companion. It is a CORRECT state
    /// produced by the archive cadence, so it must not arrive under `warning` — that
    /// register means "off-golden-path, reconsider before proceeding" and would train
    /// agents to repair a ledger that policy deliberately shaped this way.
    #[tokio::test]
    async fn reservation_names_compaction_without_calling_it_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        let md = dir.path().join("ledger.md");
        std::fs::write(
            &md,
            "---\nkind: tracker\nentry_prefix: HY\nentry_high_water_HY: 11\n---\n\n\
             # Ledger\n\nEntries through HY-11 live in the archive companion.\n",
        )
        .unwrap();

        let ctx = mk_ctx();
        seed_prose(&ctx, "art1", &md);

        let result = call(&ctx, json!({"id": "art1", "id_prefix": "HY"}))
            .await
            .unwrap();

        assert_eq!(result["id"], "HY-12", "the committed mark governs");
        assert!(result["body_max"].is_null(), "the live body claims no id");
        assert_eq!(result["frontmatter_max"], 11);

        let next_step = result["next_step"].as_str().unwrap();
        assert!(
            next_step.contains("compact"),
            "the governing input must be named in words, not left as three integers \
             for the caller to compare: {next_step}"
        );
        assert!(
            result.get("warning").is_none(),
            "a compacted ledger is correct, not off-golden-path: {result}"
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

        assert!(err.downcast_ref::<LibrarianRecoverableError>().is_some());
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

        assert!(err.downcast_ref::<LibrarianRecoverableError>().is_some());
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
    /// `docs/issues/archive/2026-08-17-prose-ledger-worktree-id-collision.md`.
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
            augmentation::allocate_entry_id(&mut cat, &main_id, "HY", None)
                .unwrap()
                .id
        };
        assert_eq!(main_alloc, "HY-11", "the main checkout must still allocate");

        let err = call(&ctx, json!({"id": main_id, "id_prefix": "HY", "entry": {}}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<LibrarianRecoverableError>().is_some());
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
        assert!(err.downcast_ref::<LibrarianRecoverableError>().is_some());
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
        assert!(err.downcast_ref::<LibrarianRecoverableError>().is_some());
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

    fn commit_all(repo: &git2::Repository, msg: &str) {
        let mut idx = repo.index().unwrap();
        idx.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        idx.write().unwrap();
        let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("t", "t@e").unwrap();
        let parents: Vec<git2::Commit> = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .into_iter()
            .collect();
        let refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &refs)
            .unwrap();
    }

    fn commit_path(root: &std::path::Path, rel: &str, msg: &str) {
        let repo = git2::Repository::open(root).unwrap();
        let mut idx = repo.index().unwrap();
        idx.add_path(std::path::Path::new(rel)).unwrap();
        idx.write().unwrap();
        let tree = repo.find_tree(idx.write_tree().unwrap()).unwrap();
        let sig = git2::Signature::now("t", "t@e").unwrap();
        let parents: Vec<git2::Commit> = repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok())
            .into_iter()
            .collect();
        let refs: Vec<&git2::Commit> = parents.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &refs)
            .unwrap();
    }

    /// Bare origin + a clone whose branch tracks it, both holding `ledger.md`, `other.md`,
    /// and a nested `docs/trackers/ledger.md`. TWO repos is the load-bearing detail: with
    /// one, the per-file and per-branch implementations are indistinguishable and both
    /// pass. The NESTED ledger is equally load-bearing: with only a top-level `ledger.md`,
    /// a ledger's basename and its repo-relative path are the same string, so an
    /// implementation that keys off `file_name()` instead of the real repo-relative path
    /// is indistinguishable from the correct one. A real ledger always lives under
    /// `docs/trackers/*.md` or similar, never at the repo root.
    fn repo_with_upstream() -> (tempfile::TempDir, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let origin = tmp.path().join("origin.git");
        git2::Repository::init_bare(&origin).unwrap();

        let work = tmp.path().join("work");
        let repo = git2::Repository::init(&work).unwrap();
        std::fs::write(work.join("ledger.md"), "base").unwrap();
        std::fs::write(work.join("other.md"), "base").unwrap();
        std::fs::create_dir_all(work.join("docs/trackers")).unwrap();
        std::fs::write(work.join("docs/trackers/ledger.md"), "base").unwrap();
        commit_all(&repo, "base");

        repo.remote("origin", origin.to_str().unwrap()).unwrap();
        let head = repo.head().unwrap();
        let branch_name = head.shorthand().unwrap().to_string();
        repo.find_remote("origin")
            .unwrap()
            .push(
                &[&format!(
                    "refs/heads/{branch_name}:refs/heads/{branch_name}"
                )],
                None,
            )
            .unwrap();
        let mut branch = repo
            .find_branch(&branch_name, git2::BranchType::Local)
            .unwrap();
        branch
            .set_upstream(Some(&format!("origin/{branch_name}")))
            .unwrap();
        (tmp, work)
    }
    /// THE REGRESSION TEST FOR "ALLOCATE OPTIMISTICALLY." Until 2026-09-11 this exact
    /// fixture (a ledger with an unpushed commit touching it) refused with "push this
    /// ledger's commits, then allocate" — a remedy no session may perform unasked. The
    /// check that produced that refusal is gone; the collision it guarded against is now
    /// caught at `pre-push`, on the merge commit that would actually publish it, not here.
    /// docs/issues/archive/2026-09-10-append-entry-refuses-on-unpushed-commits-with-a-remedy-no-session-may-perform.md
    #[tokio::test]
    async fn allocation_succeeds_while_the_ledger_has_unpushed_commits() {
        let (tmp, work) = repo_with_upstream();
        let ledger = work.join("ledger.md");
        std::fs::write(&ledger, "---\nentry_prefix: R\n---\n\n# L\n\n## R-1 — a\n").unwrap();
        commit_path(&work, "ledger.md", "add ledger, deliberately left unpushed");

        let ctx = mk_ctx();
        seed_prose(&ctx, "led", &ledger);

        let result = call(
            &ctx,
            json!({
                "id": "led", "id_prefix": "R",
                "anchor_heading": "## L", "title": "t", "body": "b"
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            result["id"], "R-2",
            "allocation must succeed against a ledger with unpushed commits touching it, \
             not refuse and name an unperformable remedy"
        );
        let _ = tmp;
    }

    /// REACHABILITY. `allocate_entry_id` gaining the capability is not the same as a
    /// caller being able to use it — a feature registered nowhere carries a passing
    /// suite and cannot be reached (`IC-3`, and CLAUDE.md § Testing Discipline names
    /// two tools that shipped in exactly that state for months).
    ///
    /// This is the only test that fails if the tool's `Args` never learn the fields,
    /// because `Args` has no `deny_unknown_fields`: a caller passing `index_row`
    /// today gets no error and no row, which is a silent drop rather than a refusal.
    #[tokio::test]
    async fn the_tool_writes_the_index_row_in_the_same_call() {
        let (tmp, work) = repo_with_upstream();
        let ledger = work.join("ledger.md");
        std::fs::write(
            &ledger,
            "---\nentry_prefix: R\n---\n\n| ID |\n|----|\n| R-1 |\n\n## L\n\n## R-1 — a\n",
        )
        .unwrap();
        let ctx = mk_ctx();
        seed_prose(&ctx, "led", &ledger);

        let out = call(
            &ctx,
            json!({
                "id": "led", "id_prefix": "R",
                "anchor_heading": "## L", "title": "t", "body": "b",
                "index_row": "| {id} |", "index_after_line": "|----|"
            }),
        )
        .await
        .unwrap();
        assert_eq!(out["id"], "R-2");

        let text = std::fs::read_to_string(&ledger).unwrap();
        assert!(text.contains("## R-2 — t"), "the section must land: {text}");
        assert!(
            text.contains("| R-2 |"),
            "the row must land in the SAME call — this is the whole feature: {text}"
        );
        let _ = tmp;
    }

    /// The PARAMS-path twin of `the_tool_writes_the_index_row_in_the_same_call`, and the
    /// discriminating test for
    /// `docs/issues/archive/2026-09-12-append-entry-drops-section-and-index-row-on-the-params-path.md`.
    ///
    /// **Asserts against the FILE, never against the response.** The response carries an
    /// allocated id whether or not anything was written — that IS the defect being fixed:
    /// the call returned `Ok` with an id, no section, no row and no diagnostic. A test that
    /// asserts on `result["id"]` passes under the mutation it exists to catch while feeling
    /// like a test of the write, which is the shape that let this ship.
    ///
    /// The `snapshot_missing` assertion is the second half and is not decoration. That list
    /// is derived from a body read taken BEFORE the write, so left alone it names the id
    /// whose row this very call just added — asking the caller to do by hand the exact
    /// thing that was just done for them. That is this bug re-appearing one field over.
    #[tokio::test]
    async fn a_params_append_writes_its_section_and_index_row_in_the_same_call() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "# Q\n\n| ID |\n|----|\n| F-1 |\n\n## Template\n",
            &["F-1"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "failures",
                   "id_prefix": "F", "entry": {"status": "fail"},
                   "anchor_heading": "## Template", "title": "t", "body": "b",
                   "index_row": "| {id} |", "index_after_line": "|----|"}),
        )
        .await
        .unwrap();
        assert_eq!(result["id"], "F-2");

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("## F-2 — t"),
            "the section must land on the PARAMS path too — an id with no heading defines \
             no citable token: {text}"
        );
        assert!(
            text.contains("| F-2 |"),
            "the index row must land in the SAME call; a second call is the capture window \
             this closes: {text}"
        );
        assert_eq!(
            result["section_written"], true,
            "the caller must be told the server wrote the section, or the hint sends them \
             to write it again: {result}"
        );
        let missing: Vec<String> =
            serde_json::from_value(result["snapshot_missing"].clone()).unwrap_or_default();
        assert!(
            !missing.contains(&"F-2".to_string()),
            "`snapshot_missing` is derived from a body read taken BEFORE the write, so it \
             must not ask for the row this call just added: {missing:?}"
        );
    }

    /// The point of the whole `snapshot_anchor` mechanism: a caller passes `index_row`
    /// with NO `index_after_line`, and the row lands at the block's TAIL because the
    /// artifact declared where its block is.
    ///
    /// **Asserts on POSITION, not merely presence.** `text.contains("| F-2 |")` would
    /// pass with the row spliced under the header, which is the wrong end of a
    /// newest-last ledger and the exact mistake a caller naming the separator by hand
    /// makes. The index comparison against `| F-1 |` is what discriminates, and it is
    /// why this test cannot be collapsed into the `contains` style of its siblings.
    ///
    /// Fixture detail that is load-bearing: `## Template` after the table. It gives the
    /// walk a non-table line to stop at, so a locator that ran to EOF would still place
    /// the row correctly here and pass — without it this test is monotone under that
    /// mutation.
    #[tokio::test]
    async fn a_declared_snapshot_anchor_places_the_row_at_the_blocks_tail() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "---\nkind: tracker\nsnapshot_anchor: '| ID |'\n---\n\n# Q\n\n| ID |\n|----|\n| F-1 |\n\n## Template\n",
            &["F-1"],
        );

        let result = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "failures",
                   "id_prefix": "F", "entry": {"status": "fail"},
                   "anchor_heading": "## Template", "title": "t", "body": "b",
                   "index_row": "| {id} |"}),
        )
        .await
        .unwrap();
        assert_eq!(result["id"], "F-2");

        let text = std::fs::read_to_string(&path).unwrap();
        let f1 = text.find("| F-1 |").expect("the seeded row must survive");
        let f2 = text
            .find("| F-2 |")
            .unwrap_or_else(|| panic!("the row must land with no index_after_line: {text}"));
        assert!(
            f2 > f1,
            "the row must land AFTER the last existing row, not under the header: {text}"
        );
        assert_eq!(
            result["section_written"], true,
            "the section still lands on this path: {result}"
        );
    }

    /// The negative control, and the reason the mechanism does not quietly guess: an
    /// artifact that declares no `snapshot_anchor` is REFUSED by name, and the ledger is
    /// left byte-identical.
    ///
    /// Most trackers are this shape on purpose — measured 2026-09-13, only 3 of the 15
    /// codescout artifacts with a `render_template` render a table into the body at all
    /// — so this path is the common one, not an edge case.
    #[tokio::test]
    async fn an_undeclared_anchor_refuses_by_name_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        let before = "---\nkind: tracker\n---\n\n# Q\n\n| ID |\n|----|\n| F-1 |\n\n## Template\n";
        seed_with_body(&ctx, "art1", &path, before, &["F-1"]);

        let err = call(
            &ctx,
            json!({"id": "art1", "entry_collection": "failures",
                   "id_prefix": "F", "entry": {"status": "fail"},
                   "anchor_heading": "## Template", "title": "t", "body": "b",
                   "index_row": "| {id} |"}),
        )
        .await
        .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("snapshot_anchor"),
            "the refusal must NAME the missing declaration, or the reader cannot act on \
             it: {msg}"
        );
        // Byte-identical: the abort happens before any write, so the promise the error
        // text makes ("nothing was written") is checked rather than trusted.
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            before,
            "a refused placement must leave the ledger untouched"
        );
    }

    /// An explicit `index_after_line` still wins when passed, so the new path is
    /// additive. Without this, a change that made `Declared` unconditional would pass
    /// every other test here.
    #[tokio::test]
    async fn an_explicit_after_line_still_overrides_the_declaration() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("queue.md");
        let ctx = mk_ctx();
        seed_with_body(
            &ctx,
            "art1",
            &path,
            "---\nkind: tracker\nsnapshot_anchor: '| ID |'\n---\n\n# Q\n\n| ID |\n|----|\n| F-1 |\n\n## Template\n",
            &["F-1"],
        );

        call(
            &ctx,
            json!({"id": "art1", "entry_collection": "failures",
                   "id_prefix": "F", "entry": {"status": "fail"},
                   "anchor_heading": "## Template", "title": "t", "body": "b",
                   "index_row": "| {id} |", "index_after_line": "|----|"}),
        )
        .await
        .unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let sep = text.find("|----|").unwrap();
        let f1 = text.find("| F-1 |").unwrap();
        let f2 = text.find("| F-2 |").unwrap();
        assert!(
            f2 > sep && f2 < f1,
            "the explicitly named separator must win over the declared tail: {text}"
        );
    }

    /// A row with no anchor ANYWHERE — none passed per call, none declared on the
    /// artifact — is refused at the boundary, and the refusal must name the field the
    /// caller can act on.
    ///
    /// **This was a both-or-neither test and is no longer one.** `index_row` alone is
    /// now legal when the artifact declares `snapshot_anchor`; what survives is the
    /// narrower and more durable claim, that a refusal names an actionable field rather
    /// than reporting an incomplete pair. Rationale updated rather than the test
    /// deleted: the contract outlived the rule that motivated it.
    ///
    /// Pairs with `an_undeclared_anchor_refuses_by_name_and_writes_nothing`, which pins
    /// the OTHER addressee — that the same refusal also names `snapshot_anchor`. One
    /// test per route, because a message naming only one of the two leaves half the
    /// readers with no next step, and neither assertion reds for the other's deletion.
    #[tokio::test]
    async fn an_index_row_without_its_anchor_is_refused_and_names_the_missing_half() {
        let (tmp, work) = repo_with_upstream();
        let ledger = work.join("ledger.md");
        std::fs::write(&ledger, "---\nentry_prefix: R\n---\n\n## L\n\n## R-1 — a\n").unwrap();
        let ctx = mk_ctx();
        seed_prose(&ctx, "led", &ledger);

        let err = call(
            &ctx,
            json!({
                "id": "led", "id_prefix": "R",
                "anchor_heading": "## L", "title": "t", "body": "b",
                "index_row": "| {id} |"
            }),
        )
        .await
        .unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("index_after_line"),
            "the refusal must name the MISSING field, not merely that a pair is incomplete: {text}"
        );
        let _ = tmp;
    }
}
/// Doc-to-code gate: every `append_entry` recipe in `docs/TAXONOMY.md`'s *Main taxonomy* table
/// must be one `call` accepts. Spec: docs/superpowers/specs/2026-09-27-taxonomy-append-recipes-test-design.md.
#[cfg(test)]
mod taxonomy_recipes {
    use crate::librarian::catalog::augmentation::declared_prefixes_from_frontmatter;
    use crate::librarian::frontmatter::{self, Frontmatter};
    use crate::librarian::tools::doctor::{parse_declaration, Declaration};
    use crate::util::librarian_guard::is_citable_entry_prefix;
    use crate::util::markdown_fence::FenceState;
    use std::path::{Path, PathBuf};

    const CALL_OPENER: &str = "doc(action=\"append_entry\"";
    const SECTION: &str = "## Main taxonomy";

    #[derive(Debug, Clone, PartialEq)]
    enum Shape {
        Prose,
        Params {
            collection: String,
        },
        /// The F-N row: `<topic>` in its target stands for every session log. The W-N row has
        /// no call of its own ("Same"), so the template check requires W alongside F.
        Template,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Recipe {
        line: usize,
        label: String,
        id_prefix: String,
        target: String,
        shape: Shape,
    }

    #[derive(Debug, Default)]
    struct Scan {
        recipes: Vec<Recipe>,
        unparseable: Vec<String>,
        non_recipe_rows: Vec<String>,
        rows_seen: usize,
    }

    fn at(line: usize, label: &str) -> String {
        format!("docs/TAXONOMY.md:{line} ({label})")
    }

    /// Argument text of the call whose `(` is at byte `open`; `None` if it never closes.
    /// Tracks double-quoted strings (with `\` escapes) and `([{` depth.
    fn call_args(text: &str, open: usize) -> Option<&str> {
        let bytes = text.as_bytes();
        let (mut depth, mut in_str, mut i) = (0usize, false, open);
        while i < bytes.len() {
            let b = bytes[i];
            if in_str {
                match b {
                    b'\\' => i += 1,
                    b'"' => in_str = false,
                    _ => {}
                }
            } else {
                match b {
                    b'"' => in_str = true,
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => {
                        depth = depth.checked_sub(1)?;
                        if depth == 0 {
                            if b == b')' {
                                return Some(&text[open + 1..i]);
                            }
                            return None;
                        }
                    }
                    _ => {}
                }
            }
            i += 1;
        }
        None
    }

    /// Value of `key="…"` at depth 0 of `args`, outside strings, at a word boundary.
    fn top_level_arg(args: &str, key: &str) -> Option<String> {
        let bytes = args.as_bytes();
        let needle = format!("{key}=\"");
        let (mut depth, mut in_str, mut i) = (0usize, false, 0);
        while i < bytes.len() {
            let b = bytes[i];
            if in_str {
                match b {
                    b'\\' => i += 1,
                    b'"' => in_str = false,
                    _ => {}
                }
            } else if depth == 0
                && bytes[i..].starts_with(needle.as_bytes())
                && (i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_'))
            {
                let start = i + needle.len();
                let len = args[start..].find('"')?;
                return Some(args[start..start + len].to_string());
            } else {
                match b {
                    b'"' => in_str = true,
                    b'(' | b'[' | b'{' => depth += 1,
                    b')' | b']' | b'}' => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
            i += 1;
        }
        None
    }

    fn first_tracker_path(cell: &str) -> Option<String> {
        cell.split('`')
            .skip(1)
            .step_by(2)
            .find(|s| s.starts_with("docs/trackers/") && s.ends_with(".md"))
            .map(str::to_string)
    }

    fn parse_row(line: &str) -> Result<(String, Option<String>, String), String> {
        let mut calls: Vec<(String, Option<String>)> = Vec::new();
        for (open, _) in line.match_indices(CALL_OPENER) {
            let args = call_args(line, open + "doc".len())
                .ok_or("the append_entry call never closes its `(`")?;
            let prefix = top_level_arg(args, "id_prefix")
                .ok_or("the append_entry call passes no id_prefix=\"…\"")?;
            calls.push((prefix, top_level_arg(args, "entry_collection")));
        }
        if calls.windows(2).any(|w| w[0] != w[1]) {
            return Err(format!(
                "the row holds append_entry calls that disagree: {calls:?}"
            ));
        }
        // Only the Lives-in cell (the second) is split out: it never holds a pipe, while the
        // append-tool cell can hold unescaped ones inside a code span.
        let lives_in = line.split(" | ").nth(1).unwrap_or_default();
        let target = first_tracker_path(lives_in)
            .ok_or("the Lives-in cell names no backticked docs/trackers/…md target")?;
        let (prefix, collection) = calls.swap_remove(0);
        Ok((prefix, collection, target))
    }

    fn scan_main_taxonomy(text: &str) -> Scan {
        let mut scan = Scan::default();
        let mut fence = FenceState::new();
        let mut in_section = false;
        for (idx, line) in text.lines().enumerate() {
            if fence.feed(line) || fence.in_fence() {
                continue;
            }
            if line.starts_with("## ") {
                in_section = line.trim_end() == SECTION;
                continue;
            }
            if !in_section || !line.starts_with("| **") {
                continue;
            }
            scan.rows_seen += 1;
            let label = line["| **".len()..]
                .split(['*', '|'])
                .next()
                .unwrap_or_default()
                .trim()
                .to_string();
            if !line.contains(CALL_OPENER) {
                scan.non_recipe_rows.push(label);
                continue;
            }
            match parse_row(line) {
                Ok((id_prefix, collection, target)) => {
                    let shape = match collection {
                        Some(collection) => Shape::Params { collection },
                        None if target.contains("<topic>") => Shape::Template,
                        None => Shape::Prose,
                    };
                    scan.recipes.push(Recipe {
                        line: idx + 1,
                        label,
                        id_prefix,
                        target,
                        shape,
                    });
                }
                Err(why) => scan
                    .unparseable
                    .push(format!("{}: {why}", at(idx + 1, &label))),
            }
        }
        scan
    }
    /// Session logs that declared no `entry_prefix` when this gate landed (2026-09-27), so the
    /// F-N recipe is refused there. SHRINK-ONLY: declare `entry_prefix: [F, W]` in one, then
    /// delete its line; the test reds until you do.
    const TEMPLATE_EXEMPT: &[&str] = &[
        "local-onnx-embedding-session-log.md",
        "pr-review-session-log.md",
        "release-promotion-session-log.md",
        "structural-edit-gate-session-log.md",
        "worktree-semantic-search-session-log.md",
    ];

    impl Recipe {
        fn at(&self) -> String {
            at(self.line, &self.label)
        }
    }

    fn read_fm(path: &Path) -> Result<Option<Frontmatter>, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot be read ({e})"))?;
        frontmatter::parse(&text)
            .map(|(fm, _)| fm)
            .map_err(|e| format!("has frontmatter that does not parse ({e})"))
    }

    /// Mirrors `allocate_entry_id`, the only reader of the declaration: the prose path refuses
    /// an empty declared set, and one lacking the recipe's prefix.
    fn check_prose(root: &Path, r: &Recipe) -> Option<String> {
        let fm = match read_fm(&root.join(&r.target)) {
            Ok(fm) => fm,
            Err(e) => {
                return Some(format!(
                "{}: routes {}-N writes to `{}`, which {e} — archived or moved? Update the row.",
                r.at(),
                r.id_prefix,
                r.target
            ))
            }
        };
        let declared = declared_prefixes_from_frontmatter(fm.as_ref());
        if declared.contains(&r.id_prefix) {
            return None;
        }
        let refusal = if declared.is_empty() {
            format!(
                "allocate_entry_id: `{}` does not declare an entry_prefix",
                r.target
            )
        } else {
            format!(
                "allocate_entry_id: `{}` is not declared by this ledger (it declares {})",
                r.id_prefix,
                declared.join(", ")
            )
        };
        Some(format!(
            "{}: prose recipe for id_prefix=\"{p}\" is refused — {refusal}. Repair ONE side: declare \
         it (doc(action=\"update\", id=<artifact id of {t}>, patch={{extra: {{\"entry_prefix\": \
         \"{p}\"}}}})), or correct the TAXONOMY row if the recipe is what is wrong.",
            r.at(),
            p = r.id_prefix,
            t = r.target
        ))
    }

    /// The params path checks no declaration; it refuses when no augmentation declares the
    /// collection. Offline, the committed sidecar is what a fresh clone re-attaches.
    fn check_params(root: &Path, r: &Recipe, collection: &str) -> Option<String> {
        let fm = match read_fm(&root.join(&r.target)) {
            Ok(fm) => fm.unwrap_or_default(),
            Err(e) => {
                return Some(format!(
                    "{}: routes writes to `{}`, which {e}.",
                    r.at(),
                    r.target
                ))
            }
        };
        let missing = |what: &str| {
            format!(
                "{}: params recipe (entry_collection=\"{collection}\") targets `{}`, which {what} — on a \
             fresh clone no augmentation re-attaches, so append_entry refuses it. Export the shape: \
             librarian(action=\"doctor\", fix=\"export_augmentations\") (a dry run; then confirm=true).",
                r.at(),
                r.target
            )
        };
        let sidecar = match fm.extra.get("expects_augmentation").map(parse_declaration) {
            Some(Declaration::Declared { sidecar: Some(rel) }) => rel,
            Some(Declaration::Declared { sidecar: None }) => {
                return Some(missing(
                    "declares `expects_augmentation: true` but names no committed sidecar",
                ))
            }
            Some(Declaration::Unparseable) => {
                return Some(missing(
                    "carries an `expects_augmentation` value that declares nothing",
                ))
            }
            Some(Declaration::Absent) | None => {
                return Some(missing("declares no `expects_augmentation` sidecar"))
            }
        };
        match crate::librarian::augmentation_sidecar::read(&root.join(&sidecar)) {
            Err(e) => Some(format!("{}: sidecar `{sidecar}` does not read: {e:#}", r.at())),
            Ok(s) if s.entry_collection.as_deref() == Some(collection) => None,
            Ok(s) => Some(format!(
                "{}: names entry_collection=\"{collection}\", but `{sidecar}` declares {:?} — append_entry \
             refuses a collection the augmentation does not declare. Repair ONE side: the TAXONOMY \
             row, or the augmentation.",
                r.at(),
                s.entry_collection
            )),
        }
    }

    fn check_template(root: &Path, r: &Recipe, exempt: &[&str]) -> Vec<String> {
        let Some((dir, suffix)) = r.target.split_once("<topic>") else {
            return vec![format!(
                "{}: template target `{}` has no `<topic>`",
                r.at(),
                r.target
            )];
        };
        let dir_path = root.join(dir);
        let mut names: Vec<String> = match std::fs::read_dir(&dir_path) {
            Ok(entries) => entries
                .flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .filter(|n| n.ends_with(suffix))
                .collect(),
            Err(e) => return vec![format!("{}: cannot list `{dir}`: {e}", r.at())],
        };
        names.sort();
        let mut out = Vec::new();
        if names.is_empty() {
            out.push(format!(
                "{}: `{}` matches no file — the template shape went vacuous",
                r.at(),
                r.target
            ));
        }
        for name in &names {
            let rel = format!("{dir}{name}");
            let declared = match read_fm(&dir_path.join(name)) {
                Ok(fm) => declared_prefixes_from_frontmatter(fm.as_ref()),
                Err(e) => {
                    out.push(format!("{}: `{rel}` {e}", r.at()));
                    continue;
                }
            };
            let is_exempt = exempt.contains(&name.as_str());
            let has_both = ["F", "W"].iter().all(|p| declared.iter().any(|d| d == p));
            match (is_exempt, declared.is_empty(), has_both) {
                (true, true, _) | (false, _, true) => {}
                (true, false, _) => out.push(format!(
                    "`{rel}` now declares {} — delete its line from TEMPLATE_EXEMPT \
                 (src/librarian/tools/append_entry.rs); the list only shrinks.",
                    declared.join(", ")
                )),
                (false, true, _) => out.push(format!(
                    "{}: `{rel}` declares no entry_prefix, so the F-N recipe is refused there. If this log \
                 is YOURS: declare it before appending, as docs/templates/session-log.md says \
                 (doc(action=\"update\", id=<its artifact id>, patch={{extra: {{\"entry_prefix\": \
                 [\"F\", \"W\"]}}}})). If it is NOT yours, it is likely a peer's log in progress — this \
                 test reads the working tree: attribute it with scripts/file-provenance.py and ask \
                 them; do not declare it for them.",
                    r.at()
                )),
                (false, false, false) => out.push(format!(
                    "{}: `{rel}` declares {} — a session log owns both F and W (the W-N row is \"Same\").",
                    r.at(),
                    declared.join(", ")
                )),
            }
        }
        for ex in exempt {
            if !names.iter().any(|n| n == ex) {
                out.push(format!(
                    "TEMPLATE_EXEMPT names `{dir}{ex}`, which no longer exists — delete the line."
                ));
            }
        }
        out
    }

    /// Asserts the CODE contract per recipe shape. Params recipes are deliberately NOT required
    /// to declare `entry_prefix` — only the prose path reads it (bug-fix-session-log:F-176).
    /// Covers TAXONOMY only: CLAUDE.md, sidecar prompts and ledger templates also route writers
    /// to append_entry and are not read here.
    #[test]
    fn every_taxonomy_append_entry_recipe_is_one_the_code_accepts() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(root.join("docs/TAXONOMY.md"))
            .expect("docs/TAXONOMY.md is the surface under test");
        let scan = scan_main_taxonomy(&text);

        let count =
            |want: fn(&Shape) -> bool| scan.recipes.iter().filter(|r| want(&r.shape)).count();
        let prose = count(|s| matches!(s, Shape::Prose));
        let params = count(|s| matches!(s, Shape::Params { .. }));
        let template = count(|s| matches!(s, Shape::Template));
        let population = format!(
            "examined {} rows under `{SECTION}`: {prose} prose, {params} params, {template} template \
         recipe(s); rows with no recipe: {:?}",
            scan.rows_seen, scan.non_recipe_rows
        );
        assert!(
            prose > 0 && params > 0 && template > 0,
            "a recipe shape is missing — the scanner lost it or the section moved; this is not a \
         clean corpus. {population}"
        );

        let mut failures = scan.unparseable.clone();
        for r in &scan.recipes {
            if !is_citable_entry_prefix(&r.id_prefix) {
                failures.push(format!(
                    "{}: id_prefix=\"{}\" is refused by append_entry before either branch — an entry \
                 token is `[A-Z]{{1,3}}-<n>`. Correct the TAXONOMY row.",
                    r.at(),
                    r.id_prefix
                ));
                continue;
            }
            match &r.shape {
                Shape::Prose => failures.extend(check_prose(&root, r)),
                Shape::Params { collection } => failures.extend(check_params(&root, r, collection)),
                Shape::Template => failures.extend(check_template(&root, r, TEMPLATE_EXEMPT)),
            }
        }
        assert!(
            failures.is_empty(),
            "{} TAXONOMY append_entry recipe finding(s) — each would be refused, or the gate cannot \
         read it:\n  {}\n\n{population}",
            failures.len(),
            failures.join("\n  ")
        );
    }

    /// A miniature TAXONOMY. Load-bearing details: the fenced ZZ row and the Q row under the
    /// NEXT section must be ignored; the R row's `index_row="| {id} | … |"` has unescaped pipes
    /// that break any cell-splitting parser; A has no call (not a recipe); B has a call with no
    /// id_prefix (unparseable, line 12).
    const FIXTURE: &str = r##"# T
## Main taxonomy
```sh
| **ZZ-N** | `docs/trackers/in-fence.md` | c | `doc(action="append_entry", id="z", id_prefix="ZZ")` | p |
```
| Prefix | Lives in | Captures | Append tool | Promotes to |
|---|---|---|---|---|
| **R-N** | `docs/trackers/r.md` (artifact `abc`) | c | `doc(action="append_entry", id="abc", id_prefix="R", index_row="| {id} | … |", title=…)` | p |
| **T-N** | `docs/trackers/t.md` | c | `doc(action="append_entry", id="t", entry_collection="observations", id_prefix="T", entry={…})` | p |
| **F-N** | `docs/trackers/<topic>-session-log.md` | c | `doc(action="append_entry", id=<log's artifact id>, id_prefix="F", title=…)` | p |
| **A-N** | `docs/trackers/a.md` | c | per the tracker's convention | p |
| **B-N** | `docs/trackers/b.md` | c | `doc(action="append_entry", id="b", title=…)` | p |
## Next section
| **Q-N** | `docs/trackers/q.md` | c | `doc(action="append_entry", id="q", id_prefix="Q")` | p |
"##;

    fn open_of(row: &str) -> usize {
        row.find(CALL_OPENER).unwrap() + "doc".len()
    }

    #[test]
    fn a_call_span_closes_over_a_nested_literal() {
        let row = r##"x `doc(action="append_entry", id_prefix="T", entry={a: [1, (2)]})` tail"##;
        assert_eq!(
            call_args(row, open_of(row)),
            Some(r##"action="append_entry", id_prefix="T", entry={a: [1, (2)]}"##)
        );
    }

    #[test]
    fn a_paren_inside_a_quoted_string_does_not_close_the_call() {
        let row = r##"`doc(action="append_entry", title="a ) b", id_prefix="R")`"##;
        let args = call_args(row, open_of(row)).expect("closes at the real paren");
        assert_eq!(top_level_arg(args, "id_prefix").as_deref(), Some("R"));
    }

    #[test]
    fn an_unclosed_call_span_is_none() {
        let row = r##"`doc(action="append_entry", id_prefix="R", entry={…)`"##;
        assert_eq!(call_args(row, open_of(row)), None);
    }

    #[test]
    fn only_a_top_level_argument_at_a_word_boundary_counts() {
        // Three decoys, each load-bearing: a quoted mention, a nested object and a longer key all
        // spell `id_prefix="…"`; only the last, top-level one is the argument.
        let args =
            r#"body="id_prefix=\"Q\"", entry={id_prefix="Z"}, xid_prefix="Y", id_prefix="R""#;
        assert_eq!(top_level_arg(args, "id_prefix").as_deref(), Some("R"));
    }

    #[test]
    fn the_scanner_classifies_each_row_and_ignores_fenced_and_foreign_rows() {
        let scan = scan_main_taxonomy(FIXTURE);
        let got: Vec<(&str, &str, Shape)> = scan
            .recipes
            .iter()
            .map(|r| (r.label.as_str(), r.id_prefix.as_str(), r.shape.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("R-N", "R", Shape::Prose),
                (
                    "T-N",
                    "T",
                    Shape::Params {
                        collection: "observations".into()
                    }
                ),
                ("F-N", "F", Shape::Template),
            ]
        );
        assert_eq!(scan.recipes[0].target, "docs/trackers/r.md");
        assert_eq!(scan.recipes[0].line, 8);
        assert_eq!(scan.non_recipe_rows, vec!["A-N".to_string()]);
        assert_eq!(scan.unparseable.len(), 1, "{:?}", scan.unparseable);
        assert!(
            scan.unparseable[0].starts_with("docs/TAXONOMY.md:12 (B-N)"),
            "{:?}",
            scan.unparseable
        );
        assert_eq!(
            scan.rows_seen, 5,
            "the fenced ZZ row and the Q row under the next section are not rows"
        );
    }

    #[test]
    fn two_calls_that_disagree_are_unparseable_not_a_guess() {
        let row = r##"| **X-N** | `docs/trackers/x.md` | c | `doc(action="append_entry", id_prefix="X")` or `doc(action="append_entry", id_prefix="Y")` | p |"##;
        let err = parse_row(row).unwrap_err();
        assert!(err.contains("disagree"), "{err}");
    }

    #[test]
    fn a_target_is_read_only_from_the_lives_in_cell() {
        // Load-bearing: the Captures cell names a real-looking tracker. Binding to it would
        // check the wrong file and pass.
        let row = r##"| **X-N** | Same file as F-N | see `docs/trackers/other.md` | `doc(action="append_entry", id_prefix="X")` | p |"##;
        let err = parse_row(row).unwrap_err();
        assert!(
            err.contains("Lives-in cell names no backticked docs/trackers"),
            "{err}"
        );
    }

    #[test]
    fn a_renamed_section_yields_no_rows() {
        let scan =
            scan_main_taxonomy(&FIXTURE.replace("## Main taxonomy", "## Main taxonomy (renamed)"));
        assert_eq!(scan.rows_seen, 0);
        assert!(scan.recipes.is_empty());
    }

    #[test]
    fn crlf_scans_like_lf() {
        let lf = scan_main_taxonomy(FIXTURE);
        let crlf = scan_main_taxonomy(&FIXTURE.replace('\n', "\r\n"));
        assert_eq!(crlf.recipes, lf.recipes);
        assert_eq!(crlf.unparseable, lf.unparseable);
        assert_eq!(crlf.rows_seen, lf.rows_seen);
    }
    fn recipe(target: &str, id_prefix: &str, shape: Shape) -> Recipe {
        Recipe {
            line: 1,
            label: "X-N".into(),
            id_prefix: id_prefix.into(),
            target: target.into(),
            shape,
        }
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn prose_check_mirrors_both_allocator_refusals() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(
            root,
            "docs/trackers/ok.md",
            "---\nkind: tracker\nentry_prefix: R\n---\n# ok\n",
        );
        put(
            root,
            "docs/trackers/none.md",
            "---\nkind: tracker\n---\n# none\n",
        );
        put(
            root,
            "docs/trackers/other.md",
            "---\nkind: tracker\nentry_prefix: Q\n---\n# other\n",
        );
        assert_eq!(
            check_prose(root, &recipe("docs/trackers/ok.md", "R", Shape::Prose)),
            None
        );
        let none = check_prose(root, &recipe("docs/trackers/none.md", "R", Shape::Prose)).unwrap();
        assert!(none.contains("does not declare an entry_prefix"), "{none}");
        let other =
            check_prose(root, &recipe("docs/trackers/other.md", "R", Shape::Prose)).unwrap();
        assert!(
            other.contains("is not declared by this ledger") && other.contains("declares Q"),
            "{other}"
        );
        let gone = check_prose(root, &recipe("docs/trackers/gone.md", "R", Shape::Prose)).unwrap();
        assert!(gone.contains("archived or moved"), "{gone}");
    }

    #[test]
    fn params_check_reads_the_committed_sidecar_not_entry_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // No entry_prefix on purpose: the params path never reads one (bug-fix-session-log:F-176).
        put(
            root,
            "docs/trackers/p.md",
            "---\nkind: tracker\nexpects_augmentation: docs/augmentations/p.yaml\n---\n# p\n",
        );
        put(
            root,
            "docs/augmentations/p.yaml",
            "prompt: \"p\"\nentry_collection: issues\n",
        );
        put(
            root,
            "docs/trackers/bare.md",
            "---\nkind: tracker\n---\n# bare\n",
        );
        put(
            root,
            "docs/trackers/yes.md",
            "---\nkind: tracker\nexpects_augmentation: true\n---\n# yes\n",
        );
        let p = |c: &str| {
            recipe(
                "docs/trackers/p.md",
                "P",
                Shape::Params {
                    collection: c.into(),
                },
            )
        };
        assert_eq!(check_params(root, &p("issues"), "issues"), None);
        let wrong = check_params(root, &p("items"), "items").unwrap();
        assert!(wrong.contains("declares Some(\"issues\")"), "{wrong}");
        let bare = check_params(
            root,
            &recipe("docs/trackers/bare.md", "P", Shape::Prose),
            "issues",
        )
        .unwrap();
        assert!(
            bare.contains("declares no `expects_augmentation` sidecar"),
            "{bare}"
        );
        let yes = check_params(
            root,
            &recipe("docs/trackers/yes.md", "P", Shape::Prose),
            "issues",
        )
        .unwrap();
        assert!(yes.contains("names no committed sidecar"), "{yes}");
    }

    #[test]
    fn template_check_enforces_f_and_w_with_a_shrink_only_exemption_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        put(
            root,
            "docs/trackers/a-session-log.md",
            "---\nentry_prefix: [F, W]\n---\n",
        );
        put(
            root,
            "docs/trackers/b-session-log.md",
            "---\nkind: tracker\n---\n",
        );
        put(
            root,
            "docs/trackers/c-session-log.md",
            "---\nentry_prefix: F\n---\n",
        );
        put(
            root,
            "docs/trackers/d-session-log.md",
            "---\nentry_prefix: [F, W]\n---\n",
        );
        let r = recipe("docs/trackers/<topic>-session-log.md", "F", Shape::Template);
        let out = check_template(root, &r, &["d-session-log.md", "gone-session-log.md"]);
        let joined = out.join("\n");
        assert!(
            !joined.contains("a-session-log.md"),
            "a declares [F, W]: {joined}"
        );
        assert!(
            joined.contains("b-session-log.md` declares no entry_prefix")
                && joined.contains("If this log is YOURS"),
            "{joined}"
        );
        assert!(
            joined.contains("c-session-log.md` declares F —"),
            "{joined}"
        );
        assert!(
            joined.contains("d-session-log.md` now declares F, W"),
            "{joined}"
        );
        assert!(
            joined.contains("gone-session-log.md`, which no longer exists"),
            "{joined}"
        );
        assert_eq!(out.len(), 4, "{joined}");
    }
}
