use super::{RecoverableError, ToolContext};
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
}

fn default_entry() -> Value {
    json!({})
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args).map_err(|e| {
        crate::tools::RecoverableError::with_hint(format!("doc(action=\"append_entry\") requires 'id' and 'id_prefix': {e}"), "Name the ledger and its id namespace, e.g. doc(action=\"append_entry\", id=\"<16-hex>\", id_prefix=\"R\"). For a PROSE ledger pass anchor_heading + title + body TOGETHER and the section is written for you; for a params ledger pass entry_collection + entry.")
    })?;
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
        // docs/issues/archive/2026-08-17-prose-ledger-worktree-id-collision.md
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
        // PARTIAL BY CONSTRUCTION, and labelled so. This does not prevent the
        // collision — a peer at origin allocates from origin's mark and collides
        // with these unpushed entries whether or not this caller is refused. What
        // it converts is an invisible divergence into a pushed one, which is why
        // the hint names pushing rather than the refusal.
        //
        // `ledger_has_unpushed_commits` allows (returns `false`) when `row.abs_path`
        // does not exist on disk — `git2::Repository::discover()` errs for a
        // nonexistent path even inside a valid repo, and every failure path in the
        // helper allows by design (Task 3). A catalog row surviving its file's
        // deletion is a pre-existing, separately-tracked condition (a stale catalog
        // row), not one this guard is positioned to detect; treating a missing file
        // as a hard failure here would trade a real capability (allocation still
        // working against a momentarily-stale catalog) for no safety, since a
        // deleted file cannot itself collide.
        // docs/issues/archive/2026-08-31-append-entry-high-water-mark-collides-across-hosts.md
        // Scoped to a LEDGER. This guard sits in front of `allocate_entry_id`, which
        // is the layer that actually decides whether `a.id` declares an
        // `entry_prefix` (`augmentation.rs:975-995`) — so without a check here, a
        // non-ledger artifact with unrelated unpushed commits on its own file is
        // refused with "this ledger has commits..." and a "push, then allocate"
        // remedy that does not unblock the call: push, retry, and
        // `allocate_entry_id` refuses again with "does not declare an
        // entry_prefix". Reading `row.abs_path` here (rather than adding a third
        // `artifact::get`) is the same file access `ledger_has_unpushed_commits`
        // already needs the path for.
        if let Some(row) = artifact::get(&cat, &a.id)? {
            let text = std::fs::read_to_string(&row.abs_path).unwrap_or_default();
            let is_ledger =
                !crate::util::librarian_guard::declared_entry_prefixes(&text).is_empty();
            if is_ledger && ledger_has_unpushed_commits(std::path::Path::new(&row.abs_path)) {
                return Err(RecoverableError::with_hint(
                    "append_entry: this ledger has commits that are not on its upstream \
                     branch, so its `entry_high_water_` mark is ahead of what any other \
                     host can see"
                        .to_string(),
                    "Push this ledger's commits, then allocate. Another clone reads its \
                     own committed high-water mark, so until yours is pushed both hosts \
                     resolve the same next id and the collision is only visible after \
                     the branches merge — as one token with two definitions. If you \
                     cannot push right now (no network, no push access), do not write \
                     the entry by hand instead — a declared `entry_prefix` puts this \
                     file off-limits to direct `edit_markdown`. Note the entry \
                     somewhere worktree-local instead, and fold it into the ledger once \
                     these commits are pushed."
                        .to_string(),
                ));
            }
        }
        // All three or none. A partial trio is an incomplete intent, and the two
        // halves fail differently: without `anchor_heading` the server would have to
        // GUESS placement, and this project's input-handling law is that a write
        // accepts an explicit target and never infers one — a wrong guess on a write
        // needs manual repair (docs/adrs/2026-07-10-repair-and-continue-input-handling.md).
        // Without `title` there is no `— <title>` to format, which is the entire
        // reason this path exists.
        let section = match (&a.title, &a.body, &a.anchor_heading) {
            (None, None, None) => None,
            (Some(title), Some(body), Some(anchor)) => Some(augmentation::PendingSection {
                title: title.clone(),
                body: body.clone(),
                anchor_heading: anchor.clone(),
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
                return Err(RecoverableError::with_hint(
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

/// Does this ledger's own file carry commits in `@{upstream}..HEAD`?
///
/// PER-FILE, not per-branch, and that is the whole design. Measured on codescout
/// 2026-09-02: HEAD was 34 commits ahead of `origin/experiments` — the normal state
/// on a branch that is pushed rarely — while 2 of 3 ledgers had zero unpushed
/// commits touching them. A branch-wide check refuses every ledger permanently and
/// gets disabled within a day.
///
/// EVERY FAILURE PATH ALLOWS. No repository, no configured upstream, an unreadable
/// ref, and — notably — a `abs_path` that does not exist on disk: `git2::Repository::
/// discover()` errs for a nonexistent path even inside a valid repo, so a ledger
/// absent at call time silently allows. Each of these returns `false`. A repo with
/// no remote has no second host, so refusing there is a false positive with no
/// recoverable reading, and this guard is partial by construction — degrading it to
/// a hard failure trades a real capability for no safety.
fn ledger_has_unpushed_commits(abs_path: &std::path::Path) -> bool {
    let Ok(repo) = git2::Repository::discover(abs_path) else {
        return false;
    };
    let Ok(head) = repo.head() else { return false };
    let Some(shorthand) = head.shorthand() else {
        return false;
    };
    let Ok(branch) = repo.find_branch(shorthand, git2::BranchType::Local) else {
        return false;
    };
    let Ok(upstream) = branch.upstream() else {
        return false;
    };
    let (Some(head_oid), Some(up_oid)) = (head.target(), upstream.get().target()) else {
        return false;
    };
    if head_oid == up_oid {
        return false;
    }

    let Ok(workdir) = repo.workdir().ok_or(()) else {
        return false;
    };
    let Ok(rel) = abs_path.strip_prefix(workdir) else {
        return false;
    };
    let rel = rel.to_string_lossy().replace('\\', "/");

    let mut walk = match repo.revwalk() {
        Ok(w) => w,
        Err(_) => return false,
    };
    if walk.push(head_oid).is_err() || walk.hide(up_oid).is_err() {
        return false;
    }
    for oid in walk.flatten() {
        let Ok(commit) = repo.find_commit(oid) else {
            continue;
        };
        let Ok(tree) = commit.tree() else { continue };
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
        let Ok(diff) = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) else {
            continue;
        };
        let touched = diff.deltas().any(|d| {
            d.new_file()
                .path()
                .map(|p| p.to_string_lossy() == rel)
                .unwrap_or(false)
                || d.old_file()
                    .path()
                    .map(|p| p.to_string_lossy() == rel)
                    .unwrap_or(false)
        });
        if touched {
            return true;
        }
    }
    false
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

    /// A repo with NO configured upstream must ALLOW. A repo with no remote has no
    /// second host, so refusing there is a pure false positive with no recoverable
    /// reading. This is spec § Error handling, and it is the arm most likely to be
    /// dropped as an edge case — it is the common case for a fresh clone.
    #[test]
    fn no_upstream_reports_no_unpushed_commits() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(tmp.path()).unwrap();
        let led = tmp.path().join("ledger.md");
        std::fs::write(&led, "x").unwrap();
        commit_all(&repo, "first");
        assert!(!ledger_has_unpushed_commits(&led));
    }

    /// A path outside any git repository must ALLOW, not panic.
    #[test]
    fn non_git_root_reports_no_unpushed_commits() {
        let tmp = tempfile::tempdir().unwrap();
        let led = tmp.path().join("ledger.md");
        std::fs::write(&led, "x").unwrap();
        assert!(!ledger_has_unpushed_commits(&led));
    }

    /// THE DISCRIMINATION THAT MATTERS. A branch-wide check passes a refusal-only
    /// test; measured on codescout 2026-09-02, HEAD was 34 commits ahead of
    /// origin/experiments while 2 of 3 ledgers had ZERO unpushed commits touching
    /// them. Only a per-file check separates these two assertions, so removing
    /// either one makes the pair satisfiable by an unusable implementation.
    ///
    /// The NESTED-vs-top-level assertions additionally kill an implementation that
    /// derives `rel` from `abs_path.file_name()` instead of the real repo-relative
    /// path: with two files sharing the basename `ledger.md` (one at the root, one
    /// under `docs/trackers/`), a basename-keyed implementation cannot tell an
    /// unpushed commit on one from an unpushed commit on the other, and would
    /// falsely refuse the nested ledger for a commit that never touched it.
    #[test]
    fn unpushed_is_per_file_not_per_branch() {
        let (tmp, origin_clone) = repo_with_upstream();
        let ledger = origin_clone.join("ledger.md");
        let other = origin_clone.join("other.md");
        let nested = origin_clone.join("docs/trackers/ledger.md");
        std::fs::write(&other, "changed").unwrap();
        commit_path(&origin_clone, "other.md", "touch other");

        assert!(
            !ledger_has_unpushed_commits(&ledger),
            "an unpushed commit on ANOTHER file must not block this ledger"
        );
        assert!(
            !ledger_has_unpushed_commits(&nested),
            "an unpushed commit on an unrelated file must not block the nested ledger either"
        );

        std::fs::write(&ledger, "changed").unwrap();
        commit_path(&origin_clone, "ledger.md", "touch ledger");
        assert!(
            ledger_has_unpushed_commits(&ledger),
            "an unpushed commit on THIS ledger must be reported"
        );
        assert!(
                !ledger_has_unpushed_commits(&nested),
                "a commit on the top-level ledger.md must not falsely mark the same-named nested ledger unpushed"
            );

        std::fs::write(&nested, "changed").unwrap();
        commit_path(
            &origin_clone,
            "docs/trackers/ledger.md",
            "touch nested ledger",
        );
        assert!(
            ledger_has_unpushed_commits(&nested),
            "an unpushed commit on THIS nested ledger must be reported"
        );
        let _ = tmp;
    }

    /// Refusal names the PUSH remedy, not the refusal. The guard does not prevent
    /// the collision — a peer at origin collides with these unpushed entries whether
    /// or not this caller is refused. What it converts is an INVISIBLE divergence into
    /// a pushed one, so the hint is the entire value and the assertion is on the hint.
    #[tokio::test]
    async fn allocation_is_refused_while_the_ledger_has_unpushed_commits() {
        let (tmp, work) = repo_with_upstream();
        let ledger = work.join("ledger.md");
        std::fs::write(&ledger, "---\nentry_prefix: R\n---\n\n# L\n\n## R-1 — a\n").unwrap();
        commit_path(&work, "ledger.md", "add ledger");

        let ctx = mk_ctx();
        seed_prose(&ctx, "led", &ledger);

        let err = call(
            &ctx,
            json!({
                "id": "led", "id_prefix": "R",
                "anchor_heading": "## L", "title": "t", "body": "b"
            }),
        )
        .await
        .unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Push this ledger's commits, then allocate."),
            "hint must name the actual remedy sentence, not just any occurrence of \
             the word \"push\" (the explanatory second sentence also contains it): {msg}"
        );
        let _ = tmp;
    }

    /// The guard checks LEDGER-ness, not merely "unpushed commits touch this
    /// file". Before this fix a non-ledger artifact (no `entry_prefix` in
    /// frontmatter) with unpushed commits on its own file was refused with "this
    /// ledger has commits..." and a "push, then allocate" remedy that would not
    /// actually unblock the call — the real refusal, from `allocate_entry_id`,
    /// is "does not declare an entry_prefix", and no amount of pushing fixes
    /// that. LOAD-BEARING DETAIL: `plain.md` has NO `entry_prefix` at all, so
    /// `declared_entry_prefixes` returns empty and this guard must be a no-op —
    /// only the later, correctly-named refusal may fire.
    #[tokio::test]
    async fn unpushed_commits_on_a_non_ledger_file_are_not_refused_by_the_ledger_guard() {
        let (tmp, work) = repo_with_upstream();
        let plain = work.join("plain.md");
        std::fs::write(&plain, "# Not a ledger\n\nno entry_prefix here\n").unwrap();
        commit_path(&work, "plain.md", "add non-ledger file");

        let ctx = mk_ctx();
        seed_prose(&ctx, "plain", &plain);

        let err = call(&ctx, json!({"id": "plain", "id_prefix": "R", "entry": {}}))
            .await
            .unwrap_err();
        let msg = format!("{err}");
        assert!(
            !msg.contains("this ledger has commits"),
            "a non-ledger file's unpushed commits must not trip the ledger guard: {msg}"
        );
        assert!(
            msg.contains("does not declare an entry_prefix"),
            "the call must fall through to the REAL refusal (allocate_entry_id's), not \
             be silently swallowed by the guard skip: {msg}"
        );
        let _ = tmp;
    }

    /// The allow side, built from the SAME fixture machinery as the refusal test
    /// above, so the machinery is proven live rather than the guard being proven
    /// merely unreached. `repo_with_upstream`'s base commit is already pushed to
    /// `origin`, and the working-tree edit below is left UNCOMMITTED — the guard
    /// walks `@{upstream}..HEAD`, so with HEAD still equal to upstream there is no
    /// unpushed commit on this file (or any file) regardless of what the working
    /// tree holds, and allocation must proceed normally.
    ///
    /// `other.md` is committed but left UNPUSHED, so the repository as a whole DOES
    /// have unpushed commits — only not on the ledger. This is the distinguishing
    /// case between a per-file and a per-branch implementation at the `call()`
    /// wiring site (the property itself is already covered at the helper's own
    /// site by `unpushed_is_per_file_not_per_branch`): a branch-wide check would
    /// wrongly refuse this call.
    #[tokio::test]
    async fn allocation_proceeds_when_the_ledger_has_no_unpushed_commits() {
        let (tmp, work) = repo_with_upstream();
        let ledger = work.join("ledger.md");
        std::fs::write(&ledger, "---\nentry_prefix: R\n---\n\n## L\n\n## R-1 — a\n").unwrap();
        std::fs::write(work.join("other.md"), "changed").unwrap();
        commit_path(&work, "other.md", "touch other.md, left unpushed");

        let ctx = mk_ctx();
        seed_prose(&ctx, "led", &ledger);

        let out = call(
            &ctx,
            json!({
                "id": "led", "id_prefix": "R",
                "anchor_heading": "## L", "title": "t", "body": "b"
            }),
        )
        .await
        .unwrap();
        assert_eq!(out["id"], "R-2");
        let _ = tmp;
    }
}
