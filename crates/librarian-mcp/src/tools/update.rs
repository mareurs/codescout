use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ToolContext;
use crate::catalog::artifact;

#[derive(Deserialize, Default)]
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
    #[serde(default)]
    body: Option<String>,
}

#[derive(Deserialize)]
struct Args {
    id: String,
    patch: UpdatePatch,
    /// When true, also call augmentation::commit_refresh after the update.
    #[serde(default)]
    commit_refresh: bool,
}
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)?;
    let cat = ctx.catalog.lock();
    let row =
        artifact::get(&cat, &a.id)?.ok_or_else(|| anyhow::anyhow!("unknown id `{}`", a.id))?;

    let root = ctx
        .workspace
        .roots
        .iter()
        .find(|r| r.name == row.repo)
        .ok_or_else(|| anyhow::anyhow!("unknown repo `{}`", row.repo))?;
    let full = root.path.join(&row.rel_path);

    let original = std::fs::read_to_string(&full)?;
    let patch = &a.patch;

    let new_content = if let Some(new_body) = &patch.body {
        let (fm_opt, old_body) = crate::frontmatter::parse(&original)?;
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
        let actual_body = match crate::catalog::augmentation::get(&cat, &a.id)? {
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
        crate::frontmatter::write(&fm, &format!("\n{actual_body}\n"))
    } else {
        crate::frontmatter::update_in_place(&original, |fm| {
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

    let updated_row = crate::catalog::artifact::ArtifactRow {
        id: row.id.clone(),
        repo: row.repo.clone(),
        rel_path: row.rel_path.clone(),
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
        file_sha256: crate::util::sha_of_bytes(new_content.as_bytes()),
        confidence: row.confidence,
    };
    artifact::upsert(&cat, &updated_row)?;

    let committed = if a.commit_refresh {
        Some(crate::catalog::augmentation::commit_refresh(&cat, &a.id)?)
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
        return Err(crate::tools::RecoverableError::with_hint(
            format!("frontmatter field `{field}` is not writable"),
            format!("writable scalar fields: {}", WRITABLE.join(", ")),
        ));
    }
    let cat = ctx.catalog.lock();
    let row = artifact::get(&cat, artifact_id)?
        .ok_or_else(|| anyhow::anyhow!("unknown artifact `{artifact_id}`"))?;
    let root = ctx
        .workspace
        .roots
        .iter()
        .find(|r| r.name == row.repo)
        .ok_or_else(|| anyhow::anyhow!("unknown repo `{}`", row.repo))?;
    let full = root.path.join(&row.rel_path);
    let original = std::fs::read_to_string(&full).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            crate::tools::RecoverableError::with_hint(
                format!("artifact file not found on disk: {}", full.display()),
                "the file may have been deleted or moved outside of librarian",
            )
        } else {
            crate::tools::RecoverableError::with_hint(
                format!("failed to read {}: {e}", full.display()),
                "check file permissions",
            )
        }
    })?;
    let new_content = crate::frontmatter::update_in_place(&original, |fm| match field {
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
    use crate::catalog::artifact;
    use crate::catalog::augmentation;
    use crate::catalog::Catalog;
    use crate::workspace::{Root, WorkspaceConfig};
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
        let v = crate::tools::create::call(
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
    async fn update_status_archived_persisted() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = crate::tools::create::call(
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
        let v = crate::tools::create::call(
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
        use crate::catalog::augmentation;
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // Create artifact via ArtifactCreate so the file exists on disk
        let v = crate::tools::create::call(
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

    async fn seed_with_augment(
        ctx: &ToolContext,
        rel_path: &str,
        append_mode: bool,
        history_cap: Option<i64>,
    ) -> String {
        let v = crate::tools::create::call(
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
            crate::tools::update::call(
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
}
