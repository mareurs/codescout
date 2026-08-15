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
    /// `{heading, action, content?, old_string?, new_string?, replace_all?, at?, include_subsections?}`.
    /// Applied atomically (all-or-nothing). Mirrors edit_markdown's batch-mode `edits` array.
    /// Mutually exclusive with `body`.
    #[serde(default)]
    body_edits: Option<Vec<serde_json::Value>>,
    /// RFC 7396 merge-patch applied to the augmentation params.
    /// Requires an existing augmentation; ignored silently if none.
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct Args {
    id: String,
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
    /// Bypass the body-shrink guard. Required when a body write would reduce
    /// the existing body by more than 50%. Use only when the shrinkage is
    /// intentional (e.g. archiving stale sections, full rewrite).
    #[serde(default)]
    force: bool,
}
/// Body writes smaller than this byte count skip the shrink guard. Files this
/// small are typically just-created frontmatter shells where a shrink ratio
/// would be misleading. A real tracker with content is always many KB.
const SHRINK_GUARD_MIN_BYTES: usize = 200;

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

/// Apply a batch of edit-markdown-shaped body edits to `working` in sequence.
/// Mirrors the batch semantics of `edit_markdown`'s `edits=[...]`. Used by
/// `artifact(update, patch={body_edits: [...]})` to provide surgical body
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
                "Each entry must have shape {heading, action, content?|old_string+new_string?, at?, replace_all?, include_subsections?}.",
            )
        })?;
        let action = edit["action"].as_str().ok_or_else(|| {
            super::RecoverableError::with_hint(
                format!("body_edits[{i}]: missing required 'action' field"),
                "Allowed actions: replace, insert_before, insert_after, remove, edit.",
            )
        })?;

        buf = if action == "edit" {
            let old_string = edit["old_string"].as_str().ok_or_else(|| {
                super::RecoverableError::with_hint(
                    format!("body_edits[{i}]: old_string is required for action='edit'"),
                    "Pass {action: \"edit\", heading, old_string, new_string, replace_all?}.",
                )
            })?;
            let new_string = edit["new_string"].as_str().unwrap_or("");
            let replace_all = edit["replace_all"].as_bool().unwrap_or(false);
            crate::tools::markdown::edit_markdown::perform_scoped_edit(
                &buf,
                heading,
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
                    crate::tools::markdown::edit_markdown::find_consumed_subsections(&buf, heading)
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
                heading,
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
                            "artifact(action=\"update\"): conflicting `{}` values — the top-level param and `patch.{}` disagree",
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
    if args.get("patch").and_then(|p| p.get("rel_path")).is_some() {
        return Err(super::RecoverableError::with_hint(
            "artifact(action=\"update\") cannot change `rel_path` — the file location is owned by the `move` action",
            "Use artifact(action=\"move\", id=..., new_rel_path=...) to rename the backing file and update the catalog atomically. `update` only modifies frontmatter fields (status, title, owners, tags, topic, time_scope, extra, body, body_edits, params).",
        ));
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

    // Checked once here rather than inside `apply_frontmatter_patch`, which all three
    // frontmatter-touching branches below call and which is infallible by design.
    //
    // Rejected whatever the value, including `null`. A null would be an RFC-7396 delete,
    // which is a no-op for these keys — parsing routes them to typed fields, so `extra`
    // never holds one to delete — and answering a mistaken repair attempt with an error
    // that names the right parameter beats answering it with silence.
    if let Some(extra) = &patch.extra {
        super::create::reject_reserved_extra_keys(extra)?;
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
        crate::librarian::frontmatter::write(&fm, &format!("\n{actual_body}\n"))
    } else if let Some(edits) = &patch.body_edits {
        let mut working = original.clone();
        let fm_changing = patch.status.is_some()
            || patch.title.is_some()
            || patch.owners.is_some()
            || patch.tags.is_some()
            || patch.topic.is_some()
            || patch.time_scope.is_some()
            || patch.extra.is_some();
        if fm_changing {
            working = crate::librarian::frontmatter::update_in_place(&working, |fm| {
                apply_frontmatter_patch(fm, patch);
            })?;
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
        crate::librarian::frontmatter::update_in_place(&original, |fm| {
            apply_frontmatter_patch(fm, patch);
        })?
    };

    if body_changing && !a.force && original.len() >= SHRINK_GUARD_MIN_BYTES {
        let allow_history_trim = matches!(
            crate::librarian::catalog::augmentation::get(&cat, &a.id)?,
            Some(aug) if aug.append_mode && aug.history_cap.is_some()
        );
        if !allow_history_trim && new_content.len() * 2 < original.len() {
            let pct = 100 - (new_content.len() * 100 / original.len().max(1));
            return Err(super::RecoverableError::with_hint(
                format!(
                    "body-shrink guard: write to {} would reduce {} → {} bytes ({}% reduction)",
                    full.display(),
                    original.len(),
                    new_content.len(),
                    pct
                ),
                "Use patch={body_edits:[{heading, action, content?|old_string+new_string?, ...}]} for surgical per-section edits (mirrors edit_markdown's batch shape). \
                 If the shrinkage is intentional (e.g. archiving stale sections, full rewrite), re-call with force=true.",
            ));
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
    artifact::upsert(&cat, &updated_row)?;

    if let Some(params_patch) = &patch.params {
        crate::librarian::catalog::augmentation::merge_params(&cat, &a.id, params_patch)?;
    }

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
        // artifact(get) can report commits_behind_head from an unforgeable source.
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
    let new_content =
        crate::librarian::frontmatter::update_in_place(&original, |fm| match field {
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
        })?;
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
