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

/// Apply a batch of edit-markdown-shaped body edits to `working` in sequence.
/// Mirrors the batch semantics of `edit_markdown`'s `edits=[...]`. Used by
/// `artifact(update, patch={body_edits: [...]})` to provide surgical body
/// mutation on librarian-managed files — `edit_markdown` itself refuses to
/// touch them (see `librarian_guard::guard_not_librarian_managed`).
fn apply_body_edits(working: &str, edits: &[Value]) -> Result<String> {
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
                super::RecoverableError::with_hint(
                    format!("body_edits[{i}]: {e}"),
                    "Check heading name and old_string content.",
                )
            })?
        } else {
            if action == "replace" && !edit["include_subsections"].as_bool().unwrap_or(false) {
                if let Ok(victims) =
                    crate::tools::markdown::edit_markdown::find_consumed_subsections(&buf, heading)
                {
                    if !victims.is_empty() {
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

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    if args.get("patch").and_then(|p| p.get("rel_path")).is_some() {
        return Err(super::RecoverableError::with_hint(
            "artifact(action=\"update\") cannot change `rel_path` — the file location is owned by the `move` action",
            "Use artifact(action=\"move\", id=..., new_rel_path=...) to rename the backing file and update the catalog atomically. `update` only modifies frontmatter fields (status, title, owners, tags, topic, body, body_edits, params).",
        ));
    }

    let a: Args = serde_json::from_value(args)?;
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

    let body_changing = patch.body.is_some() || patch.body_edits.is_some();

    let new_content = if let Some(new_body) = &patch.body {
        let (fm_opt, old_body) = crate::librarian::frontmatter::parse(&original)?;
        let mut fm = fm_opt.unwrap_or_default();
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
            || patch.topic.is_some();
        if fm_changing {
            working = crate::librarian::frontmatter::update_in_place(&working, |fm| {
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
            })?;
        }
        apply_body_edits(&working, edits)?
    } else {
        crate::librarian::frontmatter::update_in_place(&original, |fm| {
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
    // docs/issues/2026-06-13-artifact-update-body-applies-before-params-validation.md
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
        time_scope: row.time_scope,
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
        Some(crate::librarian::catalog::augmentation::commit_refresh(
            &cat, &a.id,
        )?)
    } else {
        None
    };

    let mut out = json!({"id": a.id, "updated": true});
    if let Some(c) = committed {
        out["committed"] = json!(c);
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
    use crate::librarian::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn mk_ctx(tmp_root: std::path::PathBuf) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap())),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: tmp_root,
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
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
        let msg = apply_body_edits("## Foo", &edits).unwrap_err().to_string();
        assert!(
            msg.contains("action='edit'"),
            "replace-without-content error must name action='edit'; got: {msg}"
        );
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
        // Regression: docs/issues/2026-06-13-artifact-update-body-applies-before-params-validation.md
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
