use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ToolContext;
use crate::librarian::catalog::artifact;

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct UpdatePatch {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    owners: Option<Vec<String>>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    topic: Option<String>,
    /// Temporal scope tag (e.g. "2026-W25", a date, or "dated_snapshot").
    /// Recognized first-class frontmatter + catalog field.
    #[serde(default)]
    time_scope: Option<String>,
    /// Custom frontmatter keys to merge. Each entry is upserted into the
    /// artifact's frontmatter; a `null` value deletes the key. Not
    /// catalog-indexed (YAML-only, not filterable via find). Keys not present
    /// here are preserved (round-trip-safe).
    #[serde(default)]
    extra: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    /// Full body replacement. Total-overwrite — destroys existing body content.
    /// Gated by a 50% shrink guard unless `force=true` is passed on the call.
    /// Mutually exclusive with `body_edits`.
    #[serde(default)]
    body: Option<String>,
    /// Surgical body edits — array of edit-markdown-shaped entries
    /// `{heading, action, content?, old_string?, new_string?, replace_all?, at?, occurrence?, include_subsections?}`.
    /// Applied atomically (all-or-nothing). Mirrors edit_markdown's batch-mode `edits` array.
    /// Mutually exclusive with `body`.
    #[serde(default)]
    body_edits: Option<Vec<serde_json::Value>>,
    /// RFC 7396 merge-patch applied to the augmentation params.
    /// Requires an existing augmentation; ignored silently if none.
    #[serde(default)]
    params: Option<serde_json::Value>,
}

impl UpdatePatch {
    /// True when the patch names no change at all.
    ///
    /// `patch={}` and an absent `patch` both land here, and both must be refused for the
    /// same reason: an update that reports `updated: true` while touching neither the
    /// file nor the event log. Measured 2026-08-27 — `patch={}` did exactly that.
    ///
    /// Destructured rather than read field-by-field off `self`: a struct pattern without
    /// `..` is exhaustive, so adding a field to `UpdatePatch` without deciding whether it
    /// counts as a change becomes a COMPILE ERROR here. The alternative fails the other
    /// way — a silently-wrong `true` that refuses a valid call — which is the same shape
    /// of defect `lift_top_level_param!` above was written twice to close.
    fn is_empty(&self) -> bool {
        let UpdatePatch {
            status,
            title,
            owners,
            tags,
            topic,
            time_scope,
            extra,
            body,
            body_edits,
            params,
        } = self;
        status.is_none()
            && title.is_none()
            && owners.is_none()
            && tags.is_none()
            && topic.is_none()
            && time_scope.is_none()
            && extra.is_none()
            && body.is_none()
            && body_edits.is_none()
            && params.is_none()
    }
}

#[derive(Deserialize)]
struct Args {
    id: String,
    /// Defaulted so an absent `patch` reaches the lifts below instead of dying at
    /// deserialization. Serde's bare `missing field \`patch\`` names the field but not
    /// the action, and — worse — it fires BEFORE `lift_top_level_param!`, which exists
    /// specifically to repair `doc(update, id, status=...)`. The repair for a
    /// mistake was unreachable by the call shape that makes it. An empty patch is
    /// refused after the lifts run, not before.
    #[serde(default)]
    patch: UpdatePatch,
    // The tool schema advertises these as top-level params for `create` AND
    // `update`. `create` honors them; `update`'s canonical form is inside
    // `patch`, so `call()` lifts each one rather than letting serde drop it
    // silently. Every field here exists for that reason alone — see
    // `lift_top_level_param!` and the note at the top of `call()`.
    //
    // Do not remove one because it "looks unused": `Args` has no
    // `deny_unknown_fields`, so deleting a field turns a working param back
    // into a silent no-op that still reports `updated: true`.
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    owners: Option<Vec<String>>,
    #[serde(default)]
    tags: Option<Vec<String>>,
    #[serde(default)]
    topic: Option<String>,
    #[serde(default)]
    time_scope: Option<String>,
    #[serde(default)]
    extra: Option<std::collections::BTreeMap<String, serde_json::Value>>,
    /// When true, also call augmentation::commit_refresh after the update.
    #[serde(default)]
    commit_refresh: bool,
    /// Bypass the body-shrink guard. Required when a body write would cut the
    /// file by more than 50% in EITHER bytes or lines. Use only when the
    /// shrinkage is intentional (e.g. archiving stale sections, full rewrite).
    #[serde(default)]
    force: bool,
}

/// Merge a custom-frontmatter patch into `fm.extra`: each provided key is
/// upserted, a `null` value deletes the key, omitted keys are preserved
/// (round-trip-safe). No-op when the patch carries no `extra`.
fn merge_extra(
    fm: &mut crate::librarian::frontmatter::Frontmatter,
    extra: &Option<std::collections::BTreeMap<String, serde_json::Value>>,
) {
    if let Some(map) = extra {
        for (k, v) in map {
            if v.is_null() {
                fm.extra.remove(k);
            } else {
                fm.extra.insert(k.clone(), v.clone());
            }
        }
    }
}

/// Copy the patchable scalar frontmatter fields (`status`/`title`/`owners`/
/// `tags`/`topic`/`time_scope`) plus `extra` from `patch` onto `fm`, in place.
/// Shared by all three of `call`'s frontmatter-touching branches (full-body
/// overwrite, `body_edits` with frontmatter change, and plain in-place patch).
fn apply_frontmatter_patch(
    fm: &mut crate::librarian::frontmatter::Frontmatter,
    patch: &UpdatePatch,
) {
    if let Some(v) = &patch.status {
        fm.status = Some(v.clone());
    }
    if let Some(v) = &patch.title {
        fm.title = Some(v.clone());
    }
    if let Some(v) = &patch.owners {
        fm.owners = v.clone();
    }
    if let Some(v) = &patch.tags {
        fm.tags = v.clone();
    }
    if let Some(v) = &patch.topic {
        fm.topic = Some(v.clone());
    }
    if let Some(v) = &patch.time_scope {
        fm.time_scope = Some(v.clone());
    }
    merge_extra(fm, &patch.extra);
}

/// Whether this patch writes any frontmatter field at all. Shared by the two
/// branches in [`call`] that must choose between the preserving and normalizing
/// writers — a body-only patch must not touch the block, and a patch that does
/// write fields still tries the splice first.
fn patch_changes_frontmatter(patch: &UpdatePatch) -> bool {
    patch.status.is_some()
        || patch.title.is_some()
        || patch.owners.is_some()
        || patch.tags.is_some()
        || patch.topic.is_some()
        || patch.time_scope.is_some()
        || patch.extra.is_some()
}

/// Attempt the *preserving* write of a frontmatter patch: splice each targeted
/// scalar's own line and leave every other byte of the document alone.
///
/// Returns `None` — meaning "fall back to
/// [`crate::librarian::frontmatter::rewrite_frontmatter_normalizing`]" — whenever
/// a splice cannot express the patch:
///
/// - `owners`/`tags` are sequences and `extra` is a map, so none of them has a
///   single line to replace;
/// - a targeted key is absent from the block, so there is no line to splice and
///   writing the field at all requires re-emitting;
/// - the value carries a newline, which cannot be one line
///   (`replace_scalar_line` declines).
///
/// The fallback is not a defect, it is the honest boundary. This path exists so
/// the *common* single-scalar patch stops reformatting a hand-authored file — it
/// does not replace the serializer. BL-36.
fn try_preserving_frontmatter_patch(doc: &str, patch: &UpdatePatch) -> Option<String> {
    if patch.owners.is_some() || patch.tags.is_some() || patch.extra.is_some() {
        return None;
    }
    let mut out = doc.to_string();
    let mut spliced = false;
    for (key, value) in [
        ("status", patch.status.as_deref()),
        ("title", patch.title.as_deref()),
        ("topic", patch.topic.as_deref()),
        ("time_scope", patch.time_scope.as_deref()),
    ] {
        let Some(value) = value else { continue };
        // `?` on the first key that has no line: an all-or-nothing splice. A
        // partial one would leave the block half-spliced and half-stale, which is
        // worse than one honest re-serialization.
        out = crate::librarian::frontmatter::replace_scalar_line(&out, key, value)?;
        spliced = true;
    }
    spliced.then_some(out)
}

/// Apply a batch of edit-markdown-shaped body edits to `working` in sequence.
/// Mirrors the batch semantics of `edit_markdown`'s `edits=[...]`. Used by
/// `doc(update, patch={body_edits: [...]})` to provide surgical body
/// mutation on librarian-managed files — `edit_markdown` itself refuses to
/// touch them (see `librarian_guard::guard_not_librarian_managed`).
///
/// `consumed` collects the headings that an opted-in
/// `replace` + `include_subsections: true` destroyed. `replace` always consumes
/// its section's children; the flag only decides whether that is an error or a
/// permitted operation, so without this list the caller gets no signal at all
/// when the whole-file shrink guard is satisfied by a net-larger write.
/// See `docs/issues/archive/2026-08-06-body-edits-section-replace-silent-data-loss.md`.
fn apply_body_edits(working: &str, edits: &[Value], consumed: &mut Vec<String>) -> Result<String> {
    let mut buf = working.to_string();
    for (i, edit) in edits.iter().enumerate() {
        let heading = edit["heading"].as_str().ok_or_else(|| {
            super::RecoverableError::with_hint(
                format!("body_edits[{i}]: missing required 'heading' field"),
                "Each entry must have shape {heading, action, content?|old_string+new_string?, at?, occurrence?, replace_all?, include_subsections?}.",
            )
        })?;
        let action = edit["action"].as_str().ok_or_else(|| {
            super::RecoverableError::with_hint(
                format!("body_edits[{i}]: missing required 'action' field"),
                "Allowed actions: replace, insert_before, insert_after, remove, edit.",
            )
        })?;
        // 1-indexed selector among identical headings — the only way to reach either of
        // two byte-identical ones. `body_edits` is a managed artifact's ONLY edit
        // surface, so without this such a section would be permanently uneditable.
        let query = crate::tools::file_summary::HeadingQuery::new(
            heading,
            edit["occurrence"].as_u64().map(|n| n as usize),
        );

        buf = if action == "edit" {
            let old_string = edit["old_string"].as_str().ok_or_else(|| {
                super::RecoverableError::with_hint(
                    format!("body_edits[{i}]: old_string is required for action='edit'"),
                    "Pass {action: \"edit\", heading, old_string, new_string, replace_all?}.",
                )
            })?;
            let new_string = crate::tools::markdown::edit_markdown::require_new_string(
                edit,
                &format!("body_edits[{i}]: "),
            )?;
            let replace_all = edit["replace_all"].as_bool().unwrap_or(false);
            crate::tools::markdown::edit_markdown::perform_scoped_edit(
                &buf,
                query,
                old_string,
                new_string,
                replace_all,
            )
            .map_err(|e| {
                crate::tools::markdown::edit_markdown::prefix_scoped_error(
                    e,
                    &format!("body_edits[{i}]: "),
                    "Check heading name and old_string content.",
                )
            })?
        } else {
            // Compute the victim list for EVERY replace, not only the ones we
            // are about to refuse. `include_subsections: true` used to skip this
            // block entirely, which meant the one guard purpose-built for
            // "replace is about to wipe a nested heading" never ran on the exact
            // calls that do the wiping.
            if action == "replace" {
                if let Ok(victims) =
                    crate::tools::markdown::edit_markdown::find_consumed_subsections(&buf, query)
                {
                    if !victims.is_empty() {
                        if edit["include_subsections"].as_bool().unwrap_or(false) {
                            consumed.extend(victims);
                        } else {
                            return Err(super::RecoverableError::with_hint(
                                format!(
                                    "body_edits[{i}]: replace on '{heading}' would wipe {n} nested heading(s): {list}. \
                                     Pass include_subsections: true to opt into consuming children.",
                                    n = victims.len(),
                                    list = victims.join(", "),
                                ),
                                "Prefer action=\"edit\" with old_string/new_string to target text inside the section without touching its subsections.",
                            ));
                        }
                    }
                }
            }
            crate::tools::markdown::edit_markdown::perform_section_edit_ext(
                &buf,
                query,
                action,
                edit["content"].as_str(),
                edit["at"].as_str(),
                false,
            )
            .map_err(|e| {
                super::RecoverableError::with_hint(
                    format!("body_edits[{i}]: {e}"),
                    "Check heading name and action.",
                )
            })?
        };
    }
    Ok(buf)
}

/// Lift a top-level param the schema advertises for `update` into its canonical
/// `patch.<field>` slot, per the Repair-and-Continue convention: one correct
/// reading is repaired and noted; two conflicting readings are refused, because a
/// wrong guess on a write is unrecoverable.
///
/// Why a macro rather than seven hand-written blocks: this defect has now shipped
/// twice from the same cause. `Args` cannot carry `deny_unknown_fields` — the
/// dispatcher passes `action` through and the shared artifact schema carries
/// create-only keys — so any advertised param *missing* from `Args` is discarded by
/// serde while the call still returns `updated: true`. A silent partial success on a
/// write. It was fixed for `status` alone (2026-07-20), which left the identical
/// hole in `extra`, `owners`, `tags`, `topic` and `time_scope` (found 2026-08-14).
/// One mechanism means the next param added to the schema either joins this list or
/// is visibly absent from it.
macro_rules! lift_top_level_param {
    ($corrections:expr, $top:expr, $patch:expr, $name:literal) => {
        if let Some(top) = $top.take() {
            match &$patch {
                Some(existing) if *existing != top => {
                    return Err(super::RecoverableError::with_hint(
                        format!(
                            "doc(action=\"update\"): conflicting `{}` values — the top-level param and `patch.{}` disagree",
                            $name, $name
                        ),
                        format!(
                            "Pass `{}` once. The canonical form for update is patch={{\"{}\": ...}}.",
                            $name, $name
                        ),
                    ));
                }
                // Two readings that agree is not ambiguity — repair silently.
                Some(_) => {}
                None => {
                    $patch = Some(top);
                    $corrections.push(format!(
                        "lifted top-level `{}` into `patch.{}` — the canonical form is patch={{\"{}\": ...}}",
                        $name, $name, $name
                    ));
                }
            }
        }
    };
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    // `rel_path` is owned by `move`, at BOTH nesting levels. Checking only
    // `patch.rel_path` let a top-level one fall through to the generic empty-patch
    // guard below, which would answer a specific mistake with a generic route.
    if args.get("patch").and_then(|p| p.get("rel_path")).is_some() || args.get("rel_path").is_some()
    {
        return Err(super::RecoverableError::with_hint(
            "doc(action=\"update\") cannot change `rel_path` — the file location is owned by the `move` action",
            "Use doc(action=\"move\", id=..., new_rel_path=...) to rename the backing file and update the catalog atomically. `update` only modifies frontmatter fields (status, title, owners, tags, topic, time_scope, extra, body, body_edits, params).",
        ));
    }

    if let Some(p) = args.get("patch") {
        if !p.is_null() && !p.is_object() {
            return Err(super::RecoverableError::with_hint(
                "doc(action=\"update\") patch must be a JSON object mapping field names to new values",
                "e.g. patch={\"status\": \"fixed\"}. A patch that is an array or scalar is not a valid RFC 7396 merge document.",
            ));
        }
    }

    let mut a: Args = serde_json::from_value(args)?;

    // Every top-level param the artifact schema advertises for `update` is lifted
    // into `patch`, which is where the write actually reads from. Skipping one does
    // not error — it silently no-ops while reporting `updated: true`, which is how
    // `status` shipped broken until 2026-07-20 and how the other five shipped broken
    // until 2026-08-14. See `lift_top_level_param!` above.
    //
    // `title` is lifted too. The schema documents it as create-only, so a top-level
    // `title` on update is off-schema rather than advertised — but `UpdatePatch` has
    // the field, it is the same class of mistake, and repairing it with a note beats
    // discarding a rename in silence.
    let mut corrections: Vec<String> = Vec::new();
    lift_top_level_param!(corrections, a.status, a.patch.status, "status");
    lift_top_level_param!(corrections, a.title, a.patch.title, "title");
    lift_top_level_param!(corrections, a.owners, a.patch.owners, "owners");
    lift_top_level_param!(corrections, a.tags, a.patch.tags, "tags");
    lift_top_level_param!(corrections, a.topic, a.patch.topic, "topic");
    lift_top_level_param!(corrections, a.time_scope, a.patch.time_scope, "time_scope");
    lift_top_level_param!(corrections, a.extra, a.patch.extra, "extra");

    // An update that changes nothing must say so rather than report success. Two call
    // shapes reach here: `patch={}`, which returned `updated: true` while touching
    // neither the file nor the event log (measured 2026-08-27), and — now that `patch`
    // defaults — an update with no `patch` at all. Same defect in different clothes.
    //
    // AFTER the lifts on purpose: `doc(update, id, status="fixed")` arrives with an
    // empty patch and has a populated one by this line, so the guard must not see it.
    // `commit_refresh` is the one legitimate empty patch — recording that a refresh cycle
    // ran is a real effect even when the body did not move.
    if a.patch.is_empty() && !a.commit_refresh {
        return Err(super::RecoverableError::with_hint(
            "doc(action=\"update\") was given nothing to change",
            "Pass patch={...} naming at least one of status, title, owners, tags, topic, time_scope, extra, body, body_edits, params — e.g. patch={\"status\": \"fixed\"}. A top-level status/title/owners/tags/topic/time_scope/extra is lifted into `patch` for you and reported under `corrections`. To record a refresh cycle without changing the body, pass commit_refresh=true.",
        ));
    }

    let a = {
        let mut cat = ctx.catalog.lock();
        let id = super::worktree::resolve_write_target(&mut cat, ctx, &a.id)?;
        Args { id, ..a }
    };
    let cat = ctx.catalog.lock();
    let row =
        artifact::get(&cat, &a.id)?.ok_or_else(|| anyhow::anyhow!("unknown id `{}`", a.id))?;

    let full = row.abs_path.clone();
    let original = std::fs::read_to_string(&full)?;
    let patch = &a.patch;

    if patch.body.is_some() && patch.body_edits.is_some() {
        return Err(super::RecoverableError::with_hint(
            "patch fields `body` and `body_edits` are mutually exclusive",
            "Use `body_edits` for surgical per-section edits, or `body` for full-document overwrite (pair with `force=true` if it would shrink the file by >50%).",
        ));
    }

    // The same defect `create` carries, at the other write surface: a full-body
    // replacement whose first line is `---` lands a SECOND frontmatter block below the
    // catalog's, which parses fine and is therefore silent. Measured by probe
    // 2026-09-01 — `update` reproduced it identically after `create` was fixed, which is
    // why the guard is shared rather than reimplemented here.
    //
    // `body_edits` deliberately NOT guarded: those splice content at a heading inside an
    // existing document, so a fragment opening with `---` is a horizontal rule mid-body,
    // not a second block at position 0. Guarding it would refuse legitimate content.
    if let Some(new_body) = patch.body.as_deref() {
        super::create::reject_body_leading_frontmatter(new_body)?;
    }

    // Checked once here rather than inside `apply_frontmatter_patch`, which all three
    // frontmatter-touching branches below call and which is infallible by design.
    //
    // Rejected whatever the value, including `null`. A null would be an RFC-7396 delete,
    // which is a no-op for these keys — parsing routes them to typed fields, so `extra`
    // never holds one to delete — and answering a mistaken repair attempt with an error
    // that names the right parameter beats answering it with silence.
    if let Some(extra) = &patch.extra {
        super::create::reject_reserved_extra_keys(extra, super::create::ExtraKeySurface::Update)?;
    }

    let body_changing = patch.body.is_some() || patch.body_edits.is_some();
    // Headings destroyed by an opted-in `replace` + `include_subsections: true`.
    // Surfaced in the response and the `field_patch` payload so a section-level
    // loss is visible even when the whole-file write grew.
    let mut consumed_subsections: Vec<String> = Vec::new();

    let new_content = if let Some(new_body) = &patch.body {
        let (fm_opt, old_body) = crate::librarian::frontmatter::parse(&original)?;
        let mut fm = fm_opt.unwrap_or_default();
        apply_frontmatter_patch(&mut fm, patch);
        let actual_body = match crate::librarian::catalog::augmentation::get(&cat, &a.id)? {
            Some(aug) if aug.append_mode => {
                let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
                let mut appended = format!("## {date}\n\n{new_body}\n\n{}", old_body.trim_start());
                if let Some(cap) = aug.history_cap {
                    appended = trim_history(&appended, cap as usize);
                }
                appended
            }
            _ => new_body.clone(),
        };
        let rendered_body = format!("\n{actual_body}\n");
        // A body overwrite has no business re-emitting the frontmatter, and a
        // field patch alongside it usually splices. Fall back to `write` only
        // when a splice cannot express the change — an absent key, a sequence
        // field, or a document with no frontmatter block to preserve. BL-36.
        let preserved_head = if patch_changes_frontmatter(patch) {
            try_preserving_frontmatter_patch(&original, patch)
        } else {
            Some(original.clone())
        };
        preserved_head
            .and_then(|head| crate::librarian::frontmatter::replace_body(&head, &rendered_body))
            .unwrap_or_else(|| crate::librarian::frontmatter::write(&fm, &rendered_body))
    } else if let Some(edits) = &patch.body_edits {
        let mut working = original.clone();
        if patch_changes_frontmatter(patch) {
            working = match try_preserving_frontmatter_patch(&working, patch) {
                Some(preserved) => preserved,
                None => crate::librarian::frontmatter::rewrite_frontmatter_normalizing(
                    &working,
                    |fm| {
                        apply_frontmatter_patch(fm, patch);
                    },
                )?,
            };
        }
        apply_body_edits(&working, edits, &mut consumed_subsections).map_err(|e| {
            // Extract nudge inputs in a scoped block so the borrow of `e` ends
            // before we either rebuild or return it.
            let rebuilt = {
                let is_augmented = crate::librarian::catalog::augmentation::get(&cat, &a.id)
                    .ok()
                    .flatten()
                    .is_some();
                if is_augmented {
                    e.downcast_ref::<crate::tools::RecoverableError>().and_then(|rec| {
                        if rec.extra.get("scoped_miss_tier").and_then(|v| v.as_str())
                            == Some("visible_drift")
                        {
                            Some((
                                rec.message.clone(),
                                rec.hint().unwrap_or("").to_string(),
                                (*rec.extra).clone(),
                            ))
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            };
            match rebuilt {
                Some((msg, base_hint, extra)) => {
                    let mut r = crate::tools::RecoverableError::with_hint(
                        msg,
                        format!(
                            "{base_hint} This artifact is augmented — a drifted value usually means the body is a render of `params`; update patch={{params:{{…}}}} and re-render rather than hand-editing the rendered text."
                        ),
                    );
                    for (k, v) in extra {
                        r = r.with_extra(k, v);
                    }
                    r.into()
                }
                None => e,
            }
        })?
    } else {
        match try_preserving_frontmatter_patch(&original, patch) {
            Some(preserved) => preserved,
            None => {
                crate::librarian::frontmatter::rewrite_frontmatter_normalizing(&original, |fm| {
                    apply_frontmatter_patch(fm, patch);
                })?
            }
        }
    };

    // The report is computed first so the augmentation lookup below only runs
    // on a write that already looks destructive — the common, safe case pays
    // nothing for the guard beyond two length counts.
    if body_changing && !a.force {
        if let Some(report) = crate::util::shrink_guard::check(&original, &new_content) {
            let allow_history_trim = matches!(
                crate::librarian::catalog::augmentation::get(&cat, &a.id)?,
                Some(aug) if aug.append_mode && aug.history_cap.is_some()
            );
            if !allow_history_trim {
                return Err(super::RecoverableError::with_hint(
                    format!(
                        "body-shrink guard: write to {} {}",
                        full.display(),
                        report.describe()
                    ),
                    "Use patch={body_edits:[{heading, action, content?|old_string+new_string?, ...}]} for surgical per-section edits (mirrors edit_markdown's batch shape). \
                     A write that loses LINES while keeping bytes is usually a truncated read fed back in — rebuild the body from the file or from git, never from a `get` response, whose body is capped at 500 lines. \
                     If the shrinkage is intentional (e.g. archiving stale sections, full rewrite), re-call with force=true.",
                ));
            }
        }
    }

    // Validate the params patch against the stored schema BEFORE writing the
    // file or upserting the row. merge_params (below) re-validates and persists;
    // pre-checking here keeps the update atomic — a schema violation must abort
    // before any mutation, never after the body has already been written.
    // docs/issues/archive/2026-06-13-artifact-update-body-applies-before-params-validation.md
    if let Some(params_patch) = &patch.params {
        crate::librarian::catalog::augmentation::validate_params_patch(&cat, &a.id, params_patch)?;
    }

    std::fs::write(&full, &new_content)?;

    let now = chrono::Utc::now().timestamp_millis();
    let file_mtime = std::fs::metadata(&full)
        .ok()
        .and_then(|m| {
            m.modified().ok().and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_millis() as i64)
            })
        })
        .unwrap_or(now);

    let updated_row = crate::librarian::catalog::artifact::ArtifactRow {
        id: row.id.clone(),
        abs_path: row.abs_path.clone(),
        kind: row.kind.clone(),
        status: patch.status.clone().unwrap_or(row.status),
        title: patch.title.clone().or(row.title),
        owners: patch.owners.clone().unwrap_or(row.owners),
        tags: patch.tags.clone().unwrap_or(row.tags),
        topic: patch.topic.clone().or(row.topic),
        time_scope: patch.time_scope.clone().or(row.time_scope),
        source: row.source,
        created_at: row.created_at,
        updated_at: now,
        file_mtime,
        file_sha256: crate::librarian::util::sha_of_bytes(new_content.as_bytes()),
        confidence: row.confidence,
    };
    artifact::upsert_and_mint_slug(&cat, &updated_row)?;

    // Keep the entry-count report: a params patch replaces an array wholesale
    // (RFC 7396), so a caller re-sending a trimmed collection silently deletes the
    // rest — and the catalog is not in git, so nothing else can notice.
    // docs/issues/archive/2026-08-16-params-merge-patch-wipes-entry-arrays-with-no-guard.md
    let params_merge = match &patch.params {
        Some(params_patch) => Some(crate::librarian::catalog::augmentation::merge_params(
            &cat,
            &a.id,
            params_patch,
        )?),
        None => None,
    };

    if body_changing {
        let _ = crate::librarian::catalog::events::insert(
            &cat,
            &crate::librarian::catalog::events::EventRow {
                id: ulid::Ulid::new().to_string(),
                artifact_id: a.id.clone(),
                kind: "field_patch".into(),
                payload: serde_json::json!({
                    "field": "body",
                    "prev_bytes": original.len(),
                    "new_bytes": new_content.len(),
                    "edits_count": patch.body_edits.as_ref().map(|v| v.len()).unwrap_or(0),
                    "mode": if patch.body.is_some() { "overwrite" } else { "edits" },
                    "forced": a.force,
                    // Forensic trail for section-level loss. `prev_bytes` /
                    // `new_bytes` are whole-file aggregates and read as a benign
                    // append when a replace drops a child but the file grows.
                    "replaced_subsections": consumed_subsections,
                })
                .to_string(),
                anchor_commit: None,
                head_commit: None,
                author: None,
                created_at: now,
            },
        );
    }

    let committed = if a.commit_refresh {
        // Server-computed provenance: record the repo HEAD at refresh time so
        // doc(get) can report commits_behind_head from an unforgeable source.
        let head = ctx
            .current_project
            .as_ref()
            .map(|p| crate::util::fs::RepoPath::from(&p.git_root).into_string())
            .and_then(|gr| {
                crate::librarian::catalog::commits::head_commit(&cat, &gr)
                    .ok()
                    .flatten()
            });
        Some(crate::librarian::catalog::augmentation::commit_refresh(
            &cat,
            &a.id,
            head.as_deref(),
        )?)
    } else {
        None
    };

    let mut out = json!({"id": a.id, "updated": true});
    if let Some(c) = committed {
        out["committed"] = json!(c);
    }
    // Report what the params write did to the entry collection, always — not only
    // when it shrank. A count that appears solely on loss is a count nobody learns
    // to read.
    if let Some(m) = &params_merge {
        if let (Some(before), Some(after)) = (m.entries_before, m.entries_after) {
            out["entries_before"] = json!(before);
            out["entries_after"] = json!(after);
            if after < before {
                out["entries_removed"] = json!(before - after);
                out["warning"] = json!(format!(
                    "params patch replaced the entry collection wholesale: {before} entries -> \
                     {after} ({} removed). RFC 7396 replaces arrays, it does not merge them. To \
                     change one row use doc(action=\"update_entry\", entry_id=…, fields=…); \
                     to add one use append_entry.",
                    before - after
                ));
            }
        }
    }
    if !corrections.is_empty() {
        out["corrections"] = json!(corrections);
    }
    if !consumed_subsections.is_empty() {
        out["replaced_subsections"] = json!(consumed_subsections);
    }
    Ok(out)
}

/// Write a single named frontmatter field to the artifact's file on disk.
///
/// Supported field names: `"status"`, `"title"`, `"topic"`, `"time_scope"`.
/// Any other field name is rejected with a [`RecoverableError`] so callers
/// (e.g. `event_create::call` for `field_patch` events) can surface a
/// useful error rather than silently writing an event row that has no
/// matching change on disk.
pub(crate) fn write_field_to_frontmatter(
    ctx: &ToolContext,
    artifact_id: &str,
    field: &str,
    value: &Value,
) -> Result<()> {
    const WRITABLE: &[&str] = &["status", "title", "topic", "time_scope"];
    if !WRITABLE.contains(&field) {
        return Err(crate::librarian::tools::RecoverableError::with_hint(
            format!("frontmatter field `{field}` is not writable"),
            format!("writable scalar fields: {}", WRITABLE.join(", ")),
        ));
    }
    let cat = ctx.catalog.lock();
    let row = artifact::get(&cat, artifact_id)?
        .ok_or_else(|| anyhow::anyhow!("unknown artifact `{artifact_id}`"))?;
    let full = row.abs_path.clone();
    let original = std::fs::read_to_string(&full).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            crate::librarian::tools::RecoverableError::with_hint(
                format!("artifact file not found on disk: {}", full.display()),
                "the file may have been deleted or moved outside of librarian",
            )
        } else {
            crate::librarian::tools::RecoverableError::with_hint(
                format!("failed to read {}: {e}", full.display()),
                "check file permissions",
            )
        }
    })?;
    let new_content = crate::librarian::frontmatter::rewrite_frontmatter_normalizing(
        &original,
        |fm| match field {
            "status" => {
                if let Some(s) = value.as_str() {
                    fm.status = Some(s.into());
                }
            }
            "title" => {
                if let Some(s) = value.as_str() {
                    fm.title = Some(s.into());
                }
            }
            "topic" => {
                if let Some(s) = value.as_str() {
                    fm.topic = Some(s.into());
                }
            }
            "time_scope" => {
                if let Some(s) = value.as_str() {
                    fm.time_scope = Some(s.into());
                }
            }
            _ => unreachable!("guarded by WRITABLE check above"),
        },
    )?;
    std::fs::write(&full, &new_content)?;
    Ok(())
}
fn trim_history(body: &str, cap: usize) -> String {
    use std::sync::LazyLock;
    static RE: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(r"(?m)^## \d{4}-\d{2}-\d{2}").unwrap());
    let positions: Vec<usize> = RE.find_iter(body).map(|m| m.start()).collect();
    if positions.len() <= cap {
        return body.to_string();
    }
    let cutoff = positions[cap];
    body[..cutoff].trim_end().to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact;
    use crate::librarian::catalog::augmentation;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::Root;
    use tempfile::TempDir;

    fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_root(Root {
                name: "r".into(),
                path: tmp_root,
            })
            .build()
    }

    #[tokio::test]
    async fn update_title_roundtrips() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc.md",
                "kind": "spec", "title": "Old", "body": "content"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"title": "New"}}),
        )
        .await
        .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("doc.md")).unwrap();
        assert!(content.contains("title: New"), "file should have new title");
        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(row.title.as_deref(), Some("New"));
    }

    /// One artifact, created and ready to update. Returns its id.
    async fn mk_doc(ctx: &ToolContext) -> String {
        let v = crate::librarian::tools::create::call(
            ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc.md",
                "kind": "spec", "title": "Old", "body": "content"
            }),
        )
        .await
        .unwrap();
        v["id"].as_str().unwrap().to_string()
    }

    /// `doc(update, id, status=...)` is the exact call `lift_top_level_param!` was
    /// written to repair — twice, after the same defect shipped twice. Until `patch`
    /// gained `#[serde(default)]` the repair was unreachable by the call shape that needs
    /// it: serde rejected the request for a missing `patch` before any lift could run.
    /// docs/issues/archive/2026-08-27-required-param-failures-neither-correct-nor-suggest.md
    #[tokio::test]
    async fn top_level_status_is_lifted_when_no_patch_is_supplied() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = mk_doc(&ctx).await;

        let v = call(&ctx, serde_json::json!({"id": id, "status": "fixed"}))
            .await
            .expect("a liftable top-level param must not require an explicit patch");

        // Mutation control: removing `#[serde(default)]` from `Args::patch` fails here
        // with a bare `missing field \`patch\`` before any lift runs.
        assert_eq!(v["updated"], true);
        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(row.status, "fixed", "the lift must actually write");

        // Repaired is not enough — an unreported repair teaches the caller nothing and
        // makes the next call identical.
        let corrections = v["corrections"]
            .as_array()
            .expect("the lift must be reported under `corrections`");
        assert!(
            corrections
                .iter()
                .any(|c| c.as_str().unwrap_or_default().contains("status")),
            "{corrections:?}"
        );
    }

    /// `patch={}` returned `updated: true` while touching neither the file nor the event
    /// log (measured 2026-08-27) — a write whose report did not describe what happened.
    /// An absent `patch` now reaches the same place and must be refused the same way.
    #[tokio::test]
    async fn an_update_that_changes_nothing_is_refused() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = mk_doc(&ctx).await;

        for args in [
            serde_json::json!({"id": id}),
            serde_json::json!({"id": id, "patch": {}}),
        ] {
            // Mutation control: dropping the guard makes both of these Ok(updated: true).
            let err = call(&ctx, args.clone())
                .await
                .expect_err("an update that changes nothing must not report success")
                .to_string();
            assert!(err.contains("nothing to change"), "{args}: {err}");
            // And it must name the action — the bare serde message named only the field.
            assert!(err.contains("update"), "{args}: {err}");
            assert!(
                !err.contains("missing field"),
                "bare serde message leaked: {err}"
            );
        }
    }

    /// The one legitimate empty patch: recording that a refresh cycle ran is a real
    /// effect even when the body did not move. Before this fix the call shape was
    /// rejected outright at deserialization.
    #[tokio::test]
    async fn commit_refresh_alone_is_a_legitimate_empty_patch() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = mk_doc(&ctx).await;

        // Mutation control: dropping `&& !a.commit_refresh` from the guard refuses this.
        let v = call(&ctx, serde_json::json!({"id": id, "commit_refresh": true}))
            .await
            .expect("commit_refresh with no body change must be allowed");
        assert_eq!(v["updated"], true);
    }

    /// `rel_path` is owned by `move`, and the route to it already existed — it just could
    /// not be reached from the top-level spelling, which died on `missing field patch`.
    #[tokio::test]
    async fn top_level_rel_path_routes_to_move() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = mk_doc(&ctx).await;

        let err = call(
            &ctx,
            serde_json::json!({"id": id, "rel_path": "docs/moved.md"}),
        )
        .await
        .expect_err("rel_path is not updatable")
        .to_string();

        // Mutation control: checking only `patch.rel_path` sends this to the generic
        // "nothing to change" guard, which names no remedy for the mistake actually made.
        assert!(err.contains("rel_path"), "{err}");
        assert!(err.contains("move"), "{err}");
        assert!(!err.contains("nothing to change"), "{err}");
    }

    #[tokio::test]
    async fn update_mints_a_slug_for_a_row_that_was_never_given_one() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let full = tmp.path().join("doc.md");
        std::fs::write(
            &full,
            "---\nid: aabbccdd11223344\nkind: spec\ntitle: Old\n---\ncontent\n",
        )
        .unwrap();
        let row = artifact::ArtifactRow {
            id: "aabbccdd11223344".into(),
            abs_path: full.clone(),
            kind: "spec".into(),
            status: "active".into(),
            title: Some("Old".into()),
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
        artifact::upsert(&ctx.catalog.lock(), &row).unwrap();

        call(
            &ctx,
            serde_json::json!({"id": "aabbccdd11223344", "patch": {"title": "New"}}),
        )
        .await
        .unwrap();

        let slug: Option<String> = ctx
            .catalog
            .lock()
            .conn
            .query_row(
                "SELECT slug FROM artifact WHERE id='aabbccdd11223344'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            slug.as_deref(),
            Some("new"),
            "update() reaches the same upsert chokepoint as create() and must mint too"
        );
    }

    #[tokio::test]
    async fn update_rejects_rel_path_with_move_hint() {
        // F-010: passing rel_path in the update patch used to silently no-op
        // (returns updated:true while the file location was never changed).
        // Now: explicit rejection pointing at the `move` action.
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let err = call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": {"rel_path": "new/path.md"}
            }),
        )
        .await
        .expect_err("update with patch.rel_path should error");

        let msg = err.to_string();
        assert!(
            msg.contains("rel_path"),
            "error must mention rel_path; got: {msg}"
        );
        assert!(
            msg.contains("move"),
            "error must point at the move action; got: {msg}"
        );
    }

    #[tokio::test]
    async fn update_status_archived_persisted() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc2.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"status": "archived"}}),
        )
        .await
        .unwrap();

        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(row.status, "archived");
    }

    /// Regression: docs/issues/archive/2026-08-16-artifact-update-reserializes-frontmatter-on-a-field-patch.md
    /// (BL-36). A single-field patch used to re-emit the entire block from the
    /// parsed struct, so a hand-authored file came back requoted, reordered, with
    /// flow sequences exploded to block style, null keys dropped, and — worst —
    /// `created: {YYYY-MM-DD}` expanded into `created:\n  YYYY-MM-DD: null`,
    /// because a `{Placeholder}` is valid YAML for a flow mapping. Measured on a
    /// probe: one field patched, seven lines changed, six unrequested.
    ///
    /// Two things this test does deliberately. The fixture is **hostile to
    /// normalization** — asserting only that `status` changed is exactly what let
    /// BL-34's mechanism survive at this call site. And it drives the **real
    /// tool**, not `rewrite_frontmatter_normalizing` directly, because a unit test
    /// on a shared primitive cannot catch a caller that routes around it.
    #[tokio::test]
    async fn patching_one_scalar_field_leaves_every_other_frontmatter_byte_alone() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "hand.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        // Overwrite with YAML the librarian would never emit: a double-quoted
        // title, a flow sequence, a `{Placeholder}` flow mapping, an explicit
        // null, a nested map at four-space indent, and a key order the struct
        // does not produce.
        let hostile = format!(
            r#"---
id: {id}
kind: spec
status: draft
title: "Hand Authored"
tags: [alpha, beta]
created: {{YYYY-MM-DD}}
owner: marius
topic: null
nested:
    deep: value
---

# Body

text
"#
        );
        let path = tmp.path().join("hand.md");
        std::fs::write(&path, &hostile).unwrap();

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"status": "archived"}}),
        )
        .await
        .unwrap();

        let expected = hostile.replace("status: draft", "status: archived");
        let actual = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            actual, expected,
            "only the `status:` line may change — every other byte is the author's"
        );
    }

    /// The sibling of the test above, written because BL-36's own lesson is that
    /// a fix's prose excusing an unmeasured call site is a claim, not a decision.
    /// `patch.body` is the third frontmatter-touching branch in `call`; it rebuilds
    /// the document as `write(&fm, new_body)`, so the block is re-emitted even
    /// though the caller asked only for a new body.
    #[tokio::test]
    async fn overwriting_the_body_leaves_the_hand_authored_frontmatter_alone() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "hand2.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let hostile = format!(
            r#"---
id: {id}
kind: spec
status: draft
title: "Hand Authored"
tags: [alpha, beta]
created: {{YYYY-MM-DD}}
owner: marius
topic: null
nested:
    deep: value
---

# Body

text
"#
        );
        let path = tmp.path().join("hand2.md");
        std::fs::write(&path, &hostile).unwrap();

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body": "# New\n\nreplaced prose\n"}}),
        )
        .await
        .unwrap();

        // Everything up to and including the closing delimiter is the author's.
        let fm_end = hostile[4..].find("\n---\n").map(|i| i + 4 + 5).unwrap();
        let fm_block = &hostile[..fm_end];
        let actual = std::fs::read_to_string(&path).unwrap();
        assert!(
            actual.starts_with(fm_block),
            "a body overwrite must not touch the frontmatter block\n  want prefix: {fm_block:?}\n  got:         {actual:?}"
        );
        assert!(
            actual.contains("replaced prose"),
            "the body should still have been replaced; got: {actual:?}"
        );
    }

    /// Regression: docs/issues/archive/2026-07-20-artifact-update-toplevel-status-param-silently-dropped.md
    /// The tool schema documents `create/update: set status` as a top-level
    /// param. `create` honored it; `update` dropped it via serde while still
    /// returning `updated: true` — a silent partial success.
    #[tokio::test]
    async fn update_lifts_top_level_status_into_the_patch() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc_tls.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let out = call(
            &ctx,
            serde_json::json!({"id": id, "status": "fixed", "patch": {"title": "T2"}}),
        )
        .await
        .unwrap();

        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(row.status, "fixed", "top-level status must reach the row");
        assert_eq!(row.title.as_deref(), Some("T2"), "patch must still apply");
        assert!(
            out["corrections"].is_array(),
            "the lift should be advertised via a corrections note: {out}"
        );
    }

    /// The frontmatter on disk must agree with the catalog row — the original
    /// bug was caught only because both read `draft`, and a fix that updated
    /// just the row would be a subtler version of the same defect.
    #[tokio::test]
    async fn update_top_level_status_reaches_the_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc_tlf.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(
            &ctx,
            serde_json::json!({"id": id, "status": "fixed", "patch": {}}),
        )
        .await
        .unwrap();

        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        let on_disk = std::fs::read_to_string(&row.abs_path).unwrap();
        assert!(
            on_disk.contains("status: fixed"),
            "frontmatter must agree with the row: {on_disk}"
        );
    }

    /// Two readings that agree is not ambiguity — repair silently, no note.
    #[tokio::test]
    async fn update_top_level_status_agreeing_with_patch_is_not_flagged() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc_agree.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let out = call(
            &ctx,
            serde_json::json!({
                "id": id, "status": "fixed", "patch": {"status": "fixed"}
            }),
        )
        .await
        .unwrap();

        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(row.status, "fixed");
        assert!(out.get("corrections").is_none());
    }

    /// Two readings that conflict IS ambiguity — refuse rather than guess,
    /// per the Repair-and-Continue convention. A wrong guess on a write is
    /// unrecoverable.
    #[tokio::test]
    async fn update_conflicting_status_sources_are_refused() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc_conflict.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let err = call(
            &ctx,
            serde_json::json!({
                "id": id, "status": "fixed", "patch": {"status": "archived"}
            }),
        )
        .await
        .unwrap_err();

        assert!(err
            .downcast_ref::<super::super::RecoverableError>()
            .is_some());
        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_ne!(row.status, "fixed", "a refused call must not write");
        assert_ne!(row.status, "archived", "a refused call must not write");
    }

    /// Every top-level param the artifact schema advertises for `update` must reach
    /// the file. `status` alone was fixed on 2026-07-20; the other five kept
    /// returning `updated: true` while writing nothing until 2026-08-14.
    ///
    /// Table-driven on purpose. The defect is per-param, and a fix that lifts one
    /// while forgetting the rest is precisely what happened the first time — a test
    /// covering a single param would have passed through the entire second bug.
    #[tokio::test]
    async fn update_lifts_every_advertised_top_level_param() {
        // (param, top-level value, distinctive substring that must appear in the
        // written frontmatter). Values are unique, so their presence cannot be
        // explained by anything already in the file.
        let cases: Vec<(&str, serde_json::Value, &str)> = vec![
            ("status", serde_json::json!("probestatus"), "probestatus"),
            ("title", serde_json::json!("probetitle"), "probetitle"),
            ("owners", serde_json::json!(["probeowner"]), "probeowner"),
            ("tags", serde_json::json!(["probetag"]), "probetag"),
            ("topic", serde_json::json!("probetopic"), "probetopic"),
            ("time_scope", serde_json::json!("probescope"), "probescope"),
            (
                "extra",
                serde_json::json!({"probe_key": "probeextra"}),
                "probeextra",
            ),
        ];

        for (param, value, needle) in cases {
            let tmp = TempDir::new().unwrap();
            let ctx = mk_ctx(tmp.path().to_path_buf());
            let rel = format!("doc_{param}.md");
            let v = crate::librarian::tools::create::call(
                &ctx,
                serde_json::json!({
                    "repo": "r", "rel_path": rel,
                    "kind": "spec", "title": "T", "body": "body text"
                }),
            )
            .await
            .unwrap();
            let id = v["id"].as_str().unwrap().to_string();

            // `patch` is deliberately non-empty and unrelated, so the call has real
            // work either way. That reproduces the shape of the bug, where a
            // succeeding patch masked the dropped top-level param.
            let out = call(
                &ctx,
                serde_json::json!({ "id": id, param: value, "patch": {"body": "body text v2"} }),
            )
            .await
            .unwrap_or_else(|e| panic!("update with top-level `{param}` failed: {e}"));

            let content = std::fs::read_to_string(tmp.path().join(&rel)).unwrap();
            assert!(
                content.contains(needle),
                "top-level `{param}` was accepted (`updated: true`) but never reached \
                 the frontmatter — the silent-drop bug. File:\n{content}"
            );
            assert!(
                out["corrections"].is_array(),
                "the lift of `{param}` must be advertised via a corrections note: {out}"
            );
        }
    }

    /// The conflict arm must hold for non-`status` params too — a wrong guess on a
    /// write is unrecoverable regardless of which field carries it.
    #[tokio::test]
    async fn update_conflicting_non_status_sources_are_refused() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc_tag_conflict.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let err = call(
            &ctx,
            serde_json::json!({
                "id": id, "tags": ["fromtop"], "patch": {"tags": ["frompatch"]}
            }),
        )
        .await
        .unwrap_err();

        assert!(err
            .downcast_ref::<super::super::RecoverableError>()
            .is_some());
        let content = std::fs::read_to_string(tmp.path().join("doc_tag_conflict.md")).unwrap();
        assert!(
            !content.contains("fromtop") && !content.contains("frompatch"),
            "a refused call must not write either reading. File:\n{content}"
        );
    }

    /// Agreement is not ambiguity — repair silently with no note, matching the
    /// `status` arm.
    #[tokio::test]
    async fn update_agreeing_non_status_sources_are_not_flagged() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc_tag_agree.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let out = call(
            &ctx,
            serde_json::json!({
                "id": id, "tags": ["sametag"], "patch": {"tags": ["sametag"]}
            }),
        )
        .await
        .unwrap();

        assert!(out.get("corrections").is_none(), "got: {out}");
        let content = std::fs::read_to_string(tmp.path().join("doc_tag_agree.md")).unwrap();
        assert!(
            content.contains("sametag"),
            "the agreed value must still be written"
        );
    }

    #[tokio::test]
    async fn update_time_scope_persists_to_row_and_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "scoped.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"time_scope": "2026-Q3"}}),
        )
        .await
        .unwrap();

        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(row.time_scope.as_deref(), Some("2026-Q3"));

        let on_disk = std::fs::read_to_string(&row.abs_path).unwrap();
        let (fm, _) = crate::librarian::frontmatter::parse(&on_disk).unwrap();
        assert_eq!(fm.unwrap().time_scope.as_deref(), Some("2026-Q3"));
    }

    #[tokio::test]
    async fn update_extra_merges_preserves_and_deletes() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "custom.md",
                "kind": "spec", "title": "T", "body": "b",
                "extra": {"origin_session_id": "abc", "branch": "x"}
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();
        let abs = artifact::get(&ctx.catalog.lock(), &id)
            .unwrap()
            .unwrap()
            .abs_path;

        let read_extra = |path: &std::path::Path| {
            let s = std::fs::read_to_string(path).unwrap();
            crate::librarian::frontmatter::parse(&s)
                .unwrap()
                .0
                .unwrap()
                .extra
        };

        // 1. Round-trip safety: changing an UNRELATED field must NOT wipe extra.
        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"status": "active"}}),
        )
        .await
        .unwrap();
        let after_status = read_extra(&abs);
        assert_eq!(
            after_status.get("origin_session_id"),
            Some(&serde_json::json!("abc"))
        );
        assert_eq!(after_status.get("branch"), Some(&serde_json::json!("x")));

        // 2. Merge: upsert a new key + overwrite one + delete one via null.
        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"extra": {
                "branch": "y",
                "pr": 42,
                "origin_session_id": null
            }}}),
        )
        .await
        .unwrap();
        let after_merge = read_extra(&abs);
        assert_eq!(
            after_merge.get("branch"),
            Some(&serde_json::json!("y")),
            "overwritten"
        );
        assert_eq!(after_merge.get("pr"), Some(&serde_json::json!(42)), "added");
        assert!(
            !after_merge.contains_key("origin_session_id"),
            "null deletes the key"
        );
    }

    #[tokio::test]
    async fn update_rejects_an_extra_key_that_names_a_frontmatter_field() {
        // The create path is not the only way in: an update carrying the same
        // colliding key would corrupt an artifact that was created correctly.
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "custom.md",
                "kind": "spec", "title": "T", "body": "b"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();
        let abs = artifact::get(&ctx.catalog.lock(), &id)
            .unwrap()
            .unwrap()
            .abs_path;
        let before = std::fs::read_to_string(&abs).unwrap();

        let err = call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"extra": {"status": "done"}}}),
        )
        .await
        .expect_err("a reserved key in `extra` must be refused");
        let msg = err.to_string();
        assert!(msg.contains("status"), "must name the clash: {msg}");

        // Refused before any write: a partially-applied patch that leaves the file
        // unparseable is the exact failure this guards against.
        assert_eq!(
            std::fs::read_to_string(&abs).unwrap(),
            before,
            "the file must be untouched"
        );

        // And a null value is refused too — it would be a no-op delete (parse routes
        // reserved keys to typed fields, so `extra` never holds one), and answering
        // a mistaken repair with an error that names the right parameter beats
        // answering it with silence.
        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"extra": {"kind": null}}}),
        )
        .await
        .expect_err("a reserved key is refused whatever its value");
    }
    /// The second guarded SITE, and it gets its own test for the reason CLAUDE.md
    /// § *Testing Discipline* gives: a mutation answers a question about one line, so
    /// `create`'s kill says nothing about this call. Measured before the guard existed
    /// — `update` reproduced the doubled block identically, four `---` lines, with
    /// `status: scratch` inert below a catalog block reading `draft`.
    ///
    /// Load-bearing fixture detail: the replacement body is deliberately LONGER than the
    /// original. A shorter one would trip the 50% shrink guard first and the call would
    /// fail for an unrelated reason, leaving this test green while proving nothing about
    /// frontmatter — the assertion below would still see an `expect_err`.
    #[tokio::test]
    async fn update_refuses_a_full_body_replacement_that_opens_a_frontmatter_block() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "custom.md",
                "kind": "spec", "title": "T", "body": "original body text, short"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();
        let abs = artifact::get(&ctx.catalog.lock(), &id)
            .unwrap()
            .unwrap()
            .abs_path;
        let before = std::fs::read_to_string(&abs).unwrap();

        let err = call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body":
                "---\nstatus: scratch\n---\n\n# Heading\n\nA replacement body that is \
                 comfortably longer than the original, so the shrink guard cannot be \
                 what refuses this call.\n"}}),
        )
        .await
        .expect_err("a full-body replacement opening its own frontmatter must be refused");

        let msg = err.to_string();
        assert!(
            msg.contains("frontmatter block"),
            "the error must name what it found, not merely refuse: {msg}"
        );
        assert!(
            !msg.contains("shrink"),
            "must be refused BY THIS GUARD, not incidentally by the shrink guard: {msg}"
        );
        assert_eq!(
            std::fs::read_to_string(&abs).unwrap(),
            before,
            "the file must be untouched"
        );
    }

    /// `body_edits` splices into an EXISTING document at a heading, so a fragment
    /// opening with `---` is a horizontal rule mid-body, never a second block at
    /// position 0. Guarding it would refuse legitimate content, so the guard
    /// deliberately does not — and this pins that exemption, which is otherwise
    /// indistinguishable from having forgotten the site.
    ///
    /// Mutation caught: extending the guard to `body_edits` content.
    #[tokio::test]
    async fn body_edits_may_splice_content_that_begins_with_dashes() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "custom.md",
                "kind": "spec", "title": "T", "body": "# Section\n\nbody text here\n"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body_edits": [{
                "heading": "# Section", "action": "insert_after",
                "content": "---\n\nA horizontal rule, spliced mid-document.\n"
            }]}}),
        )
        .await
        .expect("a body_edits splice beginning with `---` is legitimate content");
    }

    #[tokio::test]
    async fn missing_id_errors() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            serde_json::json!({"id": "nonexistent", "patch": {"title": "X"}}),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("unknown id"));
    }

    #[tokio::test]
    async fn body_patch_preserves_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc3.md",
                "kind": "spec", "title": "Keep", "body": "old body"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body": "brand new"}}),
        )
        .await
        .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("doc3.md")).unwrap();
        assert!(content.starts_with("---\n"), "frontmatter must be present");
        let row = artifact::get(&ctx.catalog.lock(), &id).unwrap().unwrap();
        assert_eq!(
            row.title.as_deref(),
            Some("Keep"),
            "title should be unchanged"
        );
    }

    #[tokio::test]
    async fn update_with_commit_refresh_increments_refresh_count() {
        use crate::librarian::catalog::augmentation;
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // Create artifact via ArtifactCreate so the file exists on disk
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "tracker.md",
                "kind": "tracker", "title": "T", "body": "body"
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        // Seed augmentation row
        {
            let ts = "2026-01-01T00:00:00.000Z".to_string();
            let cat = ctx.catalog.lock();
            augmentation::upsert(
                &cat,
                &augmentation::AugmentationRow {
                    artifact_id: id.clone(),
                    prompt: "p".into(),
                    params: "{}".into(),
                    last_refreshed_at: None,
                    refresh_count: 0,
                    created_at: ts.clone(),
                    updated_at: ts,
                    render_template: None,
                    params_schema: None,
                    append_mode: false,
                    history_cap: None,
                    entry_collection: None,
                    refreshed_at_commit: None,
                },
            )
            .unwrap();
        }

        // Update body + commit refresh in one call
        let result = call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": {"body": "new body"},
                "commit_refresh": true
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["id"].as_str().unwrap(), id);
        assert_eq!(result["updated"], true);
        assert_eq!(result["committed"], true);

        let cat = ctx.catalog.lock();
        let aug = augmentation::get(&cat, &id).unwrap().unwrap();
        assert_eq!(aug.refresh_count, 1);
        assert!(aug.last_refreshed_at.is_some());
    }

    #[test]
    fn trim_history_keeps_all_when_under_cap() {
        let body = "## 2026-01-03\n\nnewest\n\n## 2026-01-02\n\nmiddle\n";
        assert_eq!(trim_history(body, 5), body);
    }

    #[test]
    fn trim_history_drops_oldest_entries() {
        let body =
            "## 2026-01-03\n\nnewest\n\n## 2026-01-02\n\nmiddle\n\n## 2026-01-01\n\noldest\n";
        let result = trim_history(body, 2);
        assert!(result.contains("newest"), "newest missing");
        assert!(result.contains("middle"), "middle missing");
        assert!(!result.contains("oldest"), "oldest should be dropped");
    }

    #[test]
    fn trim_history_preserves_intro_prose() {
        let body = "Intro paragraph.\n\n## 2026-01-02\n\nnew\n\n## 2026-01-01\n\nold\n";
        let result = trim_history(body, 1);
        assert!(result.contains("Intro paragraph"), "intro prose missing");
        assert!(result.contains("new"), "new section missing");
        assert!(!result.contains("old"), "old section should be dropped");
    }

    #[test]
    fn trim_history_no_dated_sections_unchanged() {
        let body = "Just prose, no dated headers.\n";
        assert_eq!(trim_history(body, 2), body);
    }
    #[test]
    fn body_edits_replace_without_content_points_at_edit_action() {
        // Regression (2026-06-09): old_string/new_string with action="replace"
        // is the intuitive-but-wrong guess for a scoped text swap. It used to
        // fail with a bare "content is required" and no recovery path; the
        // error must now name action='edit' so the caller recovers in one step.
        let edits = vec![serde_json::json!({
            "heading": "## Foo",
            "action": "replace",
            "old_string": "x",
            "new_string": "y",
        })];
        let msg = apply_body_edits("## Foo", &edits, &mut Vec::new())
            .unwrap_err()
            .to_string();
        assert!(
            msg.contains("action='edit'"),
            "replace-without-content error must name action='edit'; got: {msg}"
        );
    }

    /// `apply_body_edits` is a THIRD independent read site for `new_string` —
    /// separate from `edit_markdown`'s single-edit and `plan_batch` paths, not a
    /// wrapper around either. It carried the same `unwrap_or("")` default, so
    /// `doc(update, patch={body_edits: [...]})` could delete a tracker or bug
    /// section and report success. Trackers are the artifacts least likely to have
    /// their content asserted by any test, which makes this the worst of the three
    /// places to lose text silently.
    ///
    /// See `docs/issues/archive/2026-08-17-edit-markdown-edit-action-deletes-when-new-string-is-omitted.md`.
    #[test]
    fn body_edits_edit_action_with_content_is_refused_and_changes_nothing() {
        let body = "## Foo\n\nkeep this sentence\n";
        let edits = vec![serde_json::json!({
            "heading": "## Foo",
            "action": "edit",
            "old_string": "keep this sentence",
            "content": "replaced sentence",
        })];
        let msg = apply_body_edits(body, &edits, &mut Vec::new())
            .expect_err("body_edits edit without new_string must be refused")
            .to_string();
        assert!(
            msg.contains("new_string is required"),
            "must refuse rather than silently delete; got: {msg}"
        );
        assert!(
            msg.contains("body_edits[0]"),
            "the error must locate the entry using THIS path's prefix, not \
             edit_markdown's `edits[0]`; got: {msg}"
        );
    }

    /// Deliberate deletion through `body_edits` stays reachable.
    #[test]
    fn body_edits_edit_action_with_explicit_empty_new_string_still_deletes() {
        let body = "## Foo\n\ndrop this. keep this.\n";
        let edits = vec![serde_json::json!({
            "heading": "## Foo",
            "action": "edit",
            "old_string": "drop this. ",
            "new_string": "",
        })];
        let out = apply_body_edits(body, &edits, &mut Vec::new())
            .expect("explicit empty replacement must apply");
        assert!(
            !out.contains("drop this"),
            "deletion should have applied: {out}"
        );
        assert!(
            out.contains("keep this."),
            "only the match should go: {out}"
        );
    }

    /// An opted-in `include_subsections` replace proceeds, but the headings
    /// it destroyed come back in `consumed`. Before this, the ONE guard built
    /// to notice "replace is about to wipe a nested heading" was skipped by
    /// the very flag that permits the wiping, and the whole-file shrink guard
    /// cannot compensate: a net-larger write passes it by construction.
    /// See `docs/issues/archive/2026-08-06-body-edits-section-replace-silent-data-loss.md`.
    #[test]
    fn body_edits_include_subsections_reports_what_it_destroyed() {
        let body = "## Wins\n\n### W-1 — first win\n\nBody of W-1.\n\n### W-2 — second win\n\nBody of W-2.\n";
        let edits = vec![serde_json::json!({
            "heading": "## Wins",
            "action": "replace",
            "content": "## Wins\n\n### W-3 — a brand new win with plenty of text to grow the file\n\nLots of replacement prose here so the whole-file byte count rises.\n",
            "include_subsections": true,
        })];
        let mut consumed = Vec::new();
        let out = apply_body_edits(body, &edits, &mut consumed).unwrap();

        assert!(
            out.contains("W-3"),
            "the opted-in replace must still apply: {out}"
        );
        assert!(
            !out.contains("W-1"),
            "replace consumes children — this test exists because it does: {out}"
        );
        assert_eq!(
            consumed.len(),
            2,
            "both destroyed subsections must be reported, got {consumed:?}"
        );
        assert!(
            consumed.iter().any(|h| h.contains("W-1"))
                && consumed.iter().any(|h| h.contains("W-2")),
            "consumed must name the lost headings, got {consumed:?}"
        );
        assert!(
            out.len() > body.len(),
            "precondition: the write must be net-LARGER, so the whole-file \
                 shrink guard would never have fired ({} -> {})",
            body.len(),
            out.len()
        );
    }

    /// A librarian-managed artifact's ONLY edit surface is `body_edits`, and it shares the
    /// one heading resolver with `edit_markdown` -- so before `occurrence` existed, two
    /// byte-identical headings made both sections permanently uneditable through every
    /// available path. `edit_markdown` refuses managed files, `edit_file` refuses them on
    /// every path, and the error's own hint named `edit_file` parameters that do not exist.
    /// See `docs/issues/archive/2026-08-27-identical-headings-make-a-section-permanently-unaddressable.md`.
    #[test]
    fn body_edits_occurrence_reaches_the_second_of_two_identical_headings() {
        let body = "## Fix\n\nthe plan\n\n## Middle\n\nm\n\n## Fix\n\nthe plan\n";
        let edits = vec![serde_json::json!({
            "heading": "## Fix",
            "action": "edit",
            "occurrence": 2,
            "old_string": "the plan",
            "new_string": "the shipped record",
        })];
        let out = apply_body_edits(body, &edits, &mut Vec::new())
            .expect("occurrence must disambiguate identical headings");

        // Mutation control: ignoring `occurrence` on this path -- or selecting indices[0]
        // in the resolver -- rewrites the FIRST section instead, which is precisely the
        // silent wrong-section edit the loud ambiguity error exists to prevent.
        let first_section = out.split("## Middle").next().unwrap();
        assert!(
            first_section.contains("the plan"),
            "first '## Fix' must be untouched: {out}"
        );
        assert!(
            out.ends_with("the shipped record\n"),
            "second '## Fix' must carry the edit: {out}"
        );
    }

    /// Without a selector the ambiguity error still fires on the managed path. Silently
    /// picking one is strictly worse than refusing: it edits the plan while the caller
    /// believes they edited the shipped record.
    #[test]
    fn body_edits_without_occurrence_still_refuses_identical_headings() {
        let body = "## Fix\n\nthe plan\n\n## Fix\n\nthe plan\n";
        let edits = vec![serde_json::json!({
            "heading": "## Fix",
            "action": "edit",
            "old_string": "the plan",
            "new_string": "x",
        })];
        let msg = apply_body_edits(body, &edits, &mut Vec::new())
            .expect_err("ambiguous heading must be refused, not guessed")
            .to_string();
        assert!(msg.contains("found 2 times"), "{msg}");
    }

    /// The refusal is unchanged when the caller has NOT opted in — moving the
    /// victim computation out of the `!include_subsections` branch must not
    /// turn the BUG-043 guard into a no-op.
    #[test]
    fn body_edits_replace_still_refuses_without_include_subsections() {
        let body = "## Wins\n\n### W-1 — first win\n\nBody of W-1.\n";
        let edits = vec![serde_json::json!({
            "heading": "## Wins",
            "action": "replace",
            "content": "## Wins\n\nreplacement\n",
        })];
        let mut consumed = Vec::new();
        let err = apply_body_edits(body, &edits, &mut consumed)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("W-1") && err.contains("include_subsections"),
            "refusal must name the victim and the opt-in flag; got: {err}"
        );
        assert!(
            consumed.is_empty(),
            "a refused edit destroyed nothing, so it must report nothing: {consumed:?}"
        );
    }

    /// A replace on a leaf section (no children) reports nothing — the list
    /// must mean "content was destroyed", not "a replace happened".
    #[test]
    fn body_edits_leaf_replace_reports_nothing_consumed() {
        let body = "## Alpha\n\nalpha text\n\n## Beta\n\nbeta text\n";
        let edits = vec![serde_json::json!({
            "heading": "## Alpha",
            "action": "replace",
            "content": "## Alpha\n\nnew alpha text\n",
            "include_subsections": true,
        })];
        let mut consumed = Vec::new();
        let out = apply_body_edits(body, &edits, &mut consumed).unwrap();
        assert!(out.contains("new alpha text"));
        assert!(out.contains("beta text"), "sibling must survive: {out}");
        assert!(consumed.is_empty(), "no children to consume: {consumed:?}");
    }

    async fn seed_with_augment(
        ctx: &ToolContext,
        rel_path: &str,
        append_mode: bool,
        history_cap: Option<i64>,
    ) -> String {
        let v = crate::librarian::tools::create::call(
            ctx,
            serde_json::json!({
                "repo": "r",
                "rel_path": rel_path,
                "kind": "spec",
                "title": "test",
                "body": "original body",
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();
        let cat = ctx.catalog.lock();
        augmentation::upsert(
            &cat,
            &augmentation::AugmentationRow {
                artifact_id: id.clone(),
                prompt: "test".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode,
                history_cap,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        id
    }

    #[tokio::test]
    async fn append_mode_prepends_dated_section() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "b1.md", true, None).await;

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body": "delta content"}}),
        )
        .await
        .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("b1.md")).unwrap();
        assert!(
            content.contains("\n## 20"),
            "dated header missing: {content}"
        );
        assert!(content.contains("delta content"), "delta missing");
        assert!(content.contains("original body"), "original body missing");
    }

    #[tokio::test]
    async fn second_append_newest_first() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "b2.md", true, None).await;

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body": "first delta"}}),
        )
        .await
        .unwrap();
        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body": "second delta"}}),
        )
        .await
        .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("b2.md")).unwrap();
        let pos_second = content.find("second delta").unwrap();
        let pos_first = content.find("first delta").unwrap();
        assert!(
            pos_second < pos_first,
            "second delta should appear before first delta"
        );
    }

    #[tokio::test]
    async fn history_cap_drops_oldest_section() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "b3.md", true, Some(2)).await;

        for entry in &["entry 1", "entry 2", "entry 3"] {
            crate::librarian::tools::update::call(
                &ctx,
                serde_json::json!({"id": id, "patch": {"body": entry}}),
            )
            .await
            .unwrap();
        }

        let content = std::fs::read_to_string(tmp.path().join("b3.md")).unwrap();
        assert!(content.contains("entry 3"), "newest missing");
        assert!(content.contains("entry 2"), "second missing");
        assert!(!content.contains("entry 1"), "oldest should be dropped");
    }

    #[tokio::test]
    async fn patch_params_updates_augmentation() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "p1.md", false, None).await;

        call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": {"params": {"entries": [{"id": "x", "title": "X"}]}}
            }),
        )
        .await
        .unwrap();

        let cat = ctx.catalog.lock();
        let aug = augmentation::get(&cat, &id).unwrap().unwrap();
        let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
        assert_eq!(params["entries"][0]["id"], "x");
    }

    #[tokio::test]
    async fn patch_params_with_commit_refresh() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "p2.md", false, None).await;

        let result = call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": {"params": {"count": 3}},
                "commit_refresh": true
            }),
        )
        .await
        .unwrap();

        assert_eq!(result["committed"], serde_json::json!(true));
        let cat = ctx.catalog.lock();
        let aug = augmentation::get(&cat, &id).unwrap().unwrap();
        let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
        assert_eq!(params["count"], 3);
        assert_eq!(aug.refresh_count, 1);
    }
    #[tokio::test]
    async fn params_schema_violation_leaves_body_unchanged() {
        // Regression: docs/issues/archive/2026-06-13-artifact-update-body-applies-before-params-validation.md
        // A schema-violating params patch must abort BEFORE the body write, so a
        // combined {body, params} update is atomic — never body-written-but-params-stale.
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "p1.md", false, None).await;

        // Attach a params_schema requiring `count: integer`, no extra keys.
        {
            let cat = ctx.catalog.lock();
            let schema = serde_json::json!({
                "type": "object",
                "properties": {"count": {"type": "integer"}},
                "additionalProperties": false
            });
            augmentation::upsert(
                &cat,
                &augmentation::AugmentationRow {
                    artifact_id: id.clone(),
                    prompt: "test".to_string(),
                    params: "{}".to_string(),
                    last_refreshed_at: None,
                    refresh_count: 0,
                    created_at: "2026-01-01T00:00:00.000Z".to_string(),
                    updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                    render_template: None,
                    params_schema: Some(serde_json::to_string(&schema).unwrap()),
                    append_mode: false,
                    history_cap: None,
                    entry_collection: None,
                    refreshed_at_commit: None,
                },
            )
            .unwrap();
        }

        let path = {
            let cat = ctx.catalog.lock();
            artifact::get(&cat, &id).unwrap().unwrap().abs_path
        };
        let before = std::fs::read_to_string(&path).unwrap();

        // Valid body overwrite + schema-violating params in the SAME update.
        let result = call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": {
                    "body": "REPLACEMENT BODY that must never reach disk on a failed params validation",
                    "params": {"count": "not-a-number"}
                }
            }),
        )
        .await;

        assert!(result.is_err(), "schema violation must error");
        assert!(
            result.unwrap_err().to_string().contains("params_schema"),
            "error should name the schema violation"
        );

        // Atomicity: the body write must NOT have happened.
        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            before, after,
            "body changed despite a failed params validation"
        );
        assert!(
            !after.contains("REPLACEMENT BODY"),
            "body overwrite leaked to disk"
        );
    }

    #[tokio::test]
    async fn no_append_mode_replace_unchanged() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "b4.md", false, None).await;

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body": "replacement body"}}),
        )
        .await
        .unwrap();

        let content = std::fs::read_to_string(tmp.path().join("b4.md")).unwrap();
        assert!(content.contains("replacement body"), "body missing");
        assert!(
            !content.contains("## 20"),
            "dated header should not appear in replace mode"
        );
    }

    // ── Layer 1: body-shrink guard ──────────────────────────────────────

    #[tokio::test]
    async fn body_shrink_guard_blocks_destructive_overwrite() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let big_body = "X".repeat(600);
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "big.md",
                "kind": "spec", "title": "T", "body": big_body,
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let err = call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body": "tiny"}}),
        )
        .await
        .expect_err("destructive overwrite should be blocked");
        let msg = err.to_string();
        assert!(
            msg.contains("body-shrink guard"),
            "error must name the guard; got: {msg}"
        );
        assert!(
            msg.contains("body_edits"),
            "hint must point at body_edits; got: {msg}"
        );
        assert!(
            msg.contains("force"),
            "hint must name the force escape; got: {msg}"
        );
    }

    /// The byte ratio and the line ratio diverge whenever a document is
    /// front-loaded with long lines, and a truncating write keeps the front.
    /// That is not hypothetical: it deleted 1047 of 1553 lines of
    /// `docs/trackers/prompt-hamsa-audit-log.md` on 2026-08-28 while losing only
    /// 29% of the bytes, because the capped prefix was an index table whose rows
    /// run 3-7 KB each. See
    /// `docs/issues/archive/2026-08-28-capped-get-body-round-trips-into-truncating-write.md`.
    ///
    /// The fixture has to be built from lines of UNEQUAL length. With uniform
    /// lines the two ratios move together, so a truncation that trips one trips
    /// the other and this test would pass with the line arm deleted.
    #[tokio::test]
    async fn body_shrink_guard_catches_a_line_truncation_that_keeps_the_bytes() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // 10 fat lines carry ~92% of the bytes; 90 thin ones carry ~92% of the
        // lines. Truncating to the fat prefix is the shape that slipped through.
        let fat: Vec<String> = (0..10).map(|_| "X".repeat(1000)).collect();
        let thin: Vec<String> = (0..90).map(|_| "y".repeat(10)).collect();
        let big_body = format!("{}\n{}", fat.join("\n"), thin.join("\n"));
        let truncated = fat.join("\n");

        // Pin the divergence itself, so a later change to the fixture that
        // quietly makes it uniform fails here rather than silently defanging
        // the assertion below.
        assert!(
            truncated.len() * 2 >= big_body.len(),
            "fixture must keep >=50% of BYTES, or the byte arm catches it and \
             this test proves nothing about the line arm"
        );
        assert!(
            truncated.lines().count() * 2 < big_body.lines().count(),
            "fixture must lose >50% of LINES, or there is nothing to catch"
        );

        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "wide.md",
                "kind": "spec", "title": "T", "body": big_body,
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let err = call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body": truncated}}),
        )
        .await
        .expect_err("a 90% line truncation must be blocked even when bytes survive");
        let msg = err.to_string();
        assert!(
            msg.contains("body-shrink guard"),
            "error must name the guard; got: {msg}"
        );
        assert!(
            msg.contains("lines"),
            "the message must name the dimension that actually tripped, or the \
             reader is told bytes shrank when they did not; got: {msg}"
        );
    }

    #[tokio::test]
    async fn body_shrink_guard_allows_with_force() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let big_body = "X".repeat(600);
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "big2.md",
                "kind": "spec", "title": "T", "body": big_body,
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": {"body": "intentionally small"},
                "force": true,
            }),
        )
        .await
        .expect("force=true must bypass the guard");

        let content = std::fs::read_to_string(tmp.path().join("big2.md")).unwrap();
        assert!(content.contains("intentionally small"));
    }

    #[tokio::test]
    async fn body_shrink_guard_skips_tiny_files() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "small.md",
                "kind": "spec", "title": "T", "body": "starting body",
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(&ctx, serde_json::json!({"id": id, "patch": {"body": "x"}}))
            .await
            .expect("small file shrink should not trigger the guard");
    }

    // ── Layer 2: deny unknown patch keys ────────────────────────────────

    #[tokio::test]
    async fn unknown_patch_key_rejected() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "doc-uk.md",
                "kind": "spec", "title": "T", "body": "b",
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let err = call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": {"body_prepend_section": null},
            }),
        )
        .await
        .expect_err("unknown patch key should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains("body_prepend_section") || msg.contains("unknown field"),
            "error must name the bad key; got: {msg}"
        );
    }

    // ── Layer 3: patch={body_edits: [...]} surgical surface ─────────────

    #[tokio::test]
    async fn body_edits_inserts_after_section() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let seed = "# Doc\n\n## Currently Shipped\n\nold content\n\n## Recent\n\nstuff\n";
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "be.md",
                "kind": "spec", "title": "T", "body": seed,
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": {
                    "body_edits": [{
                        "heading": "## Currently Shipped",
                        "action": "insert_after",
                        "at": "after-heading-line",
                        "content": "\n> scope note inserted\n",
                    }]
                }
            }),
        )
        .await
        .expect("body_edits insert_after must succeed");

        let content = std::fs::read_to_string(tmp.path().join("be.md")).unwrap();
        assert!(
            content.contains("scope note inserted"),
            "inserted content missing"
        );
        assert!(
            content.contains("old content"),
            "original body must survive"
        );
        assert!(content.contains("## Recent"), "siblings must survive");
    }

    #[tokio::test]
    async fn body_edits_visible_drift_on_augmented_nudges_params() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "docs/trackers/vt.md", false, None).await;

        // seed_with_augment writes a bare-literal body with no heading; give it
        // one to scope the body_edit against.
        call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": { "body": "## State\n\n_Last refresh: `ddf8215`_\n" }
            }),
        )
        .await
        .unwrap();

        let args = serde_json::json!({
            "id": id,
            "patch": { "body_edits": [{
                "heading": "## State",
                "action": "edit",
                "old_string": "_Last refresh: `8481bea`_",
                "new_string": "whatever",
            }]}
        });
        let err = call(&ctx, args).await.unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("recoverable");
        let hint = rec.hint().unwrap_or("").to_lowercase();
        assert!(
            hint.contains("params"),
            "augmented + visible-drift miss must nudge toward params: {hint:?}"
        );
    }
    #[tokio::test]
    async fn body_edits_preamble_sentinel_edits_a_guarded_artifacts_preamble() {
        // The bug this pins: text before an artifact's first heading has no section
        // to name, and `edit_markdown` is refused outright on a guarded (here:
        // augmented) file — so a preamble correction had no write surface at all.
        // `heading: "^"` is the reserved sentinel for that region.
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "docs/trackers/vt.md", false, None).await;

        call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": { "body": "Stale header note.\n\n## State\n\nbody\n" }
            }),
        )
        .await
        .unwrap();

        let args = serde_json::json!({
            "id": id,
            "patch": { "body_edits": [{
                "heading": "^",
                "action": "edit",
                "old_string": "Stale header note.",
                "new_string": "Corrected header note.",
            }]}
        });
        call(&ctx, args).await.unwrap();

        let cat = ctx.catalog.lock();
        let row = artifact::get(&cat, &id).unwrap().unwrap();
        let content = std::fs::read_to_string(&row.abs_path).unwrap();
        assert!(
            content.contains("Corrected header note.\n\n## State"),
            "{content}"
        );
        assert!(!content.contains("Stale header note."), "{content}");
    }

    #[tokio::test]
    async fn body_edits_visible_drift_on_non_augmented_does_not_nudge_params() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "plain.md",
                "kind": "spec", "title": "T",
                "body": "## State\n\n_Last refresh: `ddf8215`_\n",
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let args = serde_json::json!({
            "id": id,
            "patch": { "body_edits": [{
                "heading": "## State",
                "action": "edit",
                "old_string": "_Last refresh: `8481bea`_",
                "new_string": "whatever",
            }]}
        });
        let err = call(&ctx, args).await.unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("recoverable");
        let hint = rec.hint().unwrap_or("").to_lowercase();
        assert!(
            !hint.contains("params"),
            "non-augmented miss must not carry the params nudge: {hint:?}"
        );
    }

    #[tokio::test]
    async fn body_edits_whitespace_miss_on_augmented_does_not_nudge_params() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let id = seed_with_augment(&ctx, "docs/trackers/vt.md", false, None).await;
        call(
            &ctx,
            serde_json::json!({ "id": id, "patch": { "body": "## State\n\nalpha beta gamma\n" }}),
        )
        .await
        .unwrap();

        // old_string matches the body line EXCEPT a doubled interior space -> whitespace_invisible tier.
        let args = serde_json::json!({
            "id": id,
            "patch": { "body_edits": [{
                "heading": "## State",
                "action": "edit",
                "old_string": "alpha  beta gamma",   // two spaces after alpha
                "new_string": "whatever",
            }]}
        });
        let err = call(&ctx, args).await.unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("recoverable");
        let hint = rec.hint().unwrap_or("").to_lowercase();
        assert!(
            !hint.contains("params"),
            "a whitespace-tier miss (even on an augmented artifact) must NOT nudge params: {hint:?}"
        );
    }

    #[tokio::test]
    async fn body_and_body_edits_mutually_exclusive() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "mx.md",
                "kind": "spec", "title": "T", "body": "x",
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        let err = call(
            &ctx,
            serde_json::json!({
                "id": id,
                "patch": {
                    "body": "new",
                    "body_edits": [],
                }
            }),
        )
        .await
        .expect_err("body + body_edits together must error");
        let msg = err.to_string();
        assert!(
            msg.contains("mutually exclusive"),
            "error must say mutually exclusive; got: {msg}"
        );
    }

    // ── Layer 4: auto-emit body_patch event ─────────────────────────────

    #[tokio::test]
    async fn body_patch_event_emitted_on_body_change() {
        use crate::librarian::catalog::events;
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::librarian::tools::create::call(
            &ctx,
            serde_json::json!({
                "repo": "r", "rel_path": "ev.md",
                "kind": "spec", "title": "T", "body": "before",
            }),
        )
        .await
        .unwrap();
        let id = v["id"].as_str().unwrap().to_string();

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"status": "fixed"}}),
        )
        .await
        .unwrap();

        call(
            &ctx,
            serde_json::json!({"id": id, "patch": {"body": "after"}}),
        )
        .await
        .unwrap();

        let cat = ctx.catalog.lock();
        let evs = events::timeline_for_artifact(&cat, &id, None, None, 100).unwrap();
        let body_patches: Vec<_> = evs
            .iter()
            .filter(|e| {
                e.kind == "field_patch"
                    && serde_json::from_str::<serde_json::Value>(&e.payload)
                        .ok()
                        .and_then(|p| p["field"].as_str().map(|s| s.to_string()))
                        .as_deref()
                        == Some("body")
            })
            .collect();
        assert_eq!(
            body_patches.len(),
            1,
            "exactly one body field_patch event expected; got: {body_patches:?}"
        );
        let payload: serde_json::Value = serde_json::from_str(&body_patches[0].payload).unwrap();
        assert_eq!(payload["field"], "body");
        assert_eq!(payload["mode"], "overwrite");
        assert_eq!(payload["forced"], false);
        assert!(payload["prev_bytes"].is_number());
        assert!(payload["new_bytes"].is_number());
    }
}
