use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::artifact::{self, ArtifactRow};
use crate::librarian::frontmatter::Frontmatter;

fn validate_rel_path(rel: &str) -> Result<()> {
    use std::path::{Component, Path};
    let p = Path::new(rel);
    if p.is_absolute() {
        bail!("rel_path must be relative: {}", rel);
    }
    for c in p.components() {
        match c {
            Component::ParentDir => bail!("rel_path must not contain `..`: {}", rel),
            Component::Prefix(_) | Component::RootDir => {
                bail!("rel_path must be relative: {}", rel)
            }
            _ => {}
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct AugmentSpec {
    pub prompt: String,
    pub params: Option<Value>,
}

#[derive(Deserialize)]
pub struct Args {
    pub repo: Option<String>,
    pub rel_path: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub owners: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub status: Option<String>,
    pub time_scope: Option<String>,
    pub augment: Option<AugmentSpec>,
}
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let mut a: Args = serde_json::from_value(args)?;

    // Resolve base directory: explicit repo arg looks up in workspace.roots
    // (legacy compatibility), otherwise derive from current_project.abs_path.
    let base_dir: std::path::PathBuf = match a.repo.as_deref() {
        Some(r) => {
            let root = ctx
                .workspace
                .roots
                .iter()
                .find(|root| root.name == r)
                .ok_or_else(|| {
                    let valid = ctx
                        .workspace
                        .roots
                        .iter()
                        .map(|root| root.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    RecoverableError::with_hint(
                        format!("unknown repo `{r}`"),
                        format!("Valid repo names: {valid}"),
                    )
                })?;
            root.path.clone()
        }
        None => ctx
            .current_project
            .as_ref()
            .map(|p| p.abs_path.clone())
            .ok_or_else(|| {
                RecoverableError::with_hint(
                    "no active project — cannot resolve rel_path",
                    "Pass repo=<name> or activate a project via workspace(action='activate', ...)",
                )
            })?,
    };

    validate_rel_path(&a.rel_path)?;
    a.rel_path = crate::librarian::util::normalize_rel_path(&a.rel_path);
    let full = base_dir.join(&a.rel_path);
    if full.exists() {
        bail!("path exists: {}", full.display());
    }
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let id = crate::librarian::ids::artifact_id_from_abs(&full);
    let status = a.status.as_deref().unwrap_or("draft").to_string();
    let fm = Frontmatter {
        id: Some(id.clone()),
        kind: Some(a.kind.clone()),
        status: Some(status.clone()),
        title: Some(a.title.clone()),
        owners: a.owners.clone(),
        tags: a.tags.clone(),
        topic: None,
        time_scope: a.time_scope.clone(),
    };
    let content = crate::librarian::frontmatter::write(&fm, &format!("\n{}\n", a.body));
    let now = chrono::Utc::now().timestamp_millis();
    let row = ArtifactRow {
        id: id.clone(),
        abs_path: full.clone(),
        kind: a.kind.clone(),
        status: status.clone(),
        title: Some(a.title),
        owners: a.owners,
        tags: a.tags,
        topic: None,
        time_scope: a.time_scope,
        source: Some("generated".into()),
        created_at: now,
        updated_at: now,
        file_mtime: now,
        file_sha256: crate::librarian::util::sha_of_bytes(content.as_bytes()),
        confidence: 1.0,
    };
    artifact::upsert(&ctx.catalog.lock(), &row)?;
    if let Some(aug_spec) = &a.augment {
        let params_str = aug_spec
            .params
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?
            .unwrap_or_else(|| "{}".to_string());
        let now_ts = chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let cat = ctx.catalog.lock();
        crate::librarian::catalog::augmentation::upsert(
            &cat,
            &crate::librarian::catalog::augmentation::AugmentationRow {
                artifact_id: id.clone(),
                prompt: aug_spec.prompt.clone(),
                params: params_str,
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: now_ts.clone(),
                updated_at: now_ts,
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
            },
        )?;
    }
    // Disk write last — the file is the user-visible side effect; the DB row
    // is the durable record. If a catalog upsert above fails, no orphan file
    // is left on disk to block a retry (BUG-058).
    std::fs::write(&full, &content)?;
    let mut result = json!({"id": id, "abs_path": row.abs_path.display().to_string()});
    if a.kind == "tracker" && a.augment.is_none() {
        result["tracker_hint"] = json!(
            "Tracker created without augmentation. \
             Call librarian(tracker_design) to pick an archetype \
             and attach a refresh prompt via artifact_augment."
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::current_project::CurrentProject;
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
            artifact_store: None,
            current_project: None,
        }
    }

    #[tokio::test]
    async fn creates_file_and_row() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let v = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/specs/x.md",
                "kind": "spec", "title": "X", "body": "hello"
            }),
        )
        .await
        .unwrap();
        let path = tmp.path().join("docs/specs/x.md");
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("---\n"));
        assert!(content.contains("title: X"));
        let id = v["id"].as_str().unwrap();
        assert!(artifact::get(&ctx.catalog.lock(), id).unwrap().is_some());
    }

    #[tokio::test]
    async fn refuses_if_file_exists() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs/x.md"), "").unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/x.md",
                "kind": "doc", "title": "X", "body": "hi"
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("path exists"));
    }

    #[tokio::test]
    async fn create_does_not_leave_orphan_file_when_upsert_fails() {
        // BUG-058: if the artifact upsert fails after the file has been
        // written, future create calls bail with "path exists" even though
        // the artifact is not in the DB. Disk write must come AFTER all
        // catalog writes so a DB error leaves the disk untouched.
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // Force every artifact INSERT to abort, simulating the constraint
        // violation that BUG-058 reported under partial v6 migration state.
        ctx.catalog
            .lock()
            .conn
            .execute_batch(
                "CREATE TRIGGER fail_artifact BEFORE INSERT ON artifact \
                 BEGIN SELECT RAISE(ABORT, 'simulated upsert failure'); END;",
            )
            .unwrap();

        let result = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "docs/orphan.md",
                "kind": "doc", "title": "X", "body": "hi"
            }),
        )
        .await;

        assert!(result.is_err(), "upsert must fail with abort trigger");
        let target = tmp.path().join("docs/orphan.md");
        assert!(
            !target.exists(),
            "no orphan file must remain after failed upsert: {}",
            target.display()
        );
    }

    #[tokio::test]
    async fn rejects_parent_dir_traversal() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "../escape.md",
                "kind": "doc", "title": "X", "body": "hi"
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains(".."), "got: {err}");
    }

    #[tokio::test]
    async fn rejects_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "repo": "r", "rel_path": "/etc/passwd",
                "kind": "doc", "title": "X", "body": "hi"
            }),
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("relative"), "got: {err}");
    }

    #[tokio::test]
    async fn create_with_augment_writes_augmentation_row() {
        use crate::librarian::catalog::augmentation;
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        let result = call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/my-tracker.md",
                "kind": "tracker",
                "title": "My Tracker",
                "body": "initial body",
                "status": "active",
                "augment": {
                    "prompt": "Keep this tracker up to date.",
                    "params": {"threshold": 5}
                }
            }),
        )
        .await
        .unwrap();

        let id = result["id"].as_str().unwrap().to_string();
        let cat = ctx.catalog.lock();
        let aug = augmentation::get(&cat, &id).unwrap();
        assert!(aug.is_some(), "augmentation row must be created");
        let aug = aug.unwrap();
        assert_eq!(aug.prompt, "Keep this tracker up to date.");
        let params: serde_json::Value = serde_json::from_str(&aug.params).unwrap();
        assert_eq!(params["threshold"], 5);
    }

    #[tokio::test]
    async fn create_with_explicit_status_active() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/active.md",
                "kind": "tracker",
                "title": "Active",
                "body": "",
                "status": "active"
            }),
        )
        .await
        .unwrap();

        let cat = ctx.catalog.lock();
        let row = crate::librarian::catalog::artifact::get(
            &cat,
            &crate::librarian::ids::artifact_id_from_abs(&tmp.path().join("trackers/active.md")),
        )
        .unwrap()
        .unwrap();
        assert_eq!(row.status, "active");
    }

    #[tokio::test]
    async fn create_with_time_scope_persists_to_row_and_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        call(
            &ctx,
            json!({
                "repo": "r",
                "rel_path": "trackers/scoped.md",
                "kind": "tracker",
                "title": "Scoped",
                "body": "",
                "time_scope": "2026-W25"
            }),
        )
        .await
        .unwrap();

        let abs = tmp.path().join("trackers/scoped.md");
        let row = crate::librarian::catalog::artifact::get(
            &ctx.catalog.lock(),
            &crate::librarian::ids::artifact_id_from_abs(&abs),
        )
        .unwrap()
        .unwrap();
        assert_eq!(row.time_scope.as_deref(), Some("2026-W25"));

        // The value must also land in the YAML frontmatter, not just the catalog.
        let on_disk = std::fs::read_to_string(&abs).unwrap();
        let (fm, _) = crate::librarian::frontmatter::parse(&on_disk).unwrap();
        assert_eq!(fm.unwrap().time_scope.as_deref(), Some("2026-W25"));
    }

    #[tokio::test]
    async fn tracker_without_augment_returns_hint() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "repo": "r",
                "rel_path": "docs/trackers/my-tracker.md",
                "kind": "tracker",
                "title": "My Tracker",
                "body": ""
            }),
        )
        .await
        .unwrap();
        assert!(
            result["tracker_hint"].is_string(),
            "tracker without augment must include tracker_hint"
        );
    }

    #[tokio::test]
    async fn tracker_with_augment_no_hint() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "repo": "r",
                "rel_path": "docs/trackers/augmented-tracker.md",
                "kind": "tracker",
                "title": "Augmented Tracker",
                "body": "",
                "augment": {"prompt": "track the state of X"}
            }),
        )
        .await
        .unwrap();
        assert!(
            result.get("tracker_hint").is_none(),
            "tracker with augment must not include tracker_hint"
        );
    }

    #[tokio::test]
    async fn non_tracker_kind_no_hint() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let result = call(
            &ctx,
            serde_json::json!({
                "repo": "r",
                "rel_path": "docs/plans/my-plan.md",
                "kind": "plan",
                "title": "My Plan",
                "body": ""
            }),
        )
        .await
        .unwrap();
        assert!(
            result.get("tracker_hint").is_none(),
            "non-tracker kind must not include tracker_hint"
        );
    }

    #[tokio::test]
    async fn creates_with_inferred_repo() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().to_path_buf();
        let mut ctx = mk_ctx(path.clone());
        ctx.current_project = Some(Arc::new(CurrentProject {
            abs_path: path.clone(),
            git_root: path.clone(),
            umbrella: None,
        }));
        let result = call(
            &ctx,
            json!({
                "rel_path": "docs/inferred.md",
                "kind": "spec",
                "title": "Inferred",
                "body": "body"
            }),
        )
        .await
        .unwrap();
        let abs = result["abs_path"].as_str().unwrap();
        assert!(abs.ends_with("docs/inferred.md"), "got: {abs}");
        assert!(
            abs.starts_with(path.to_string_lossy().as_ref()),
            "got: {abs}"
        );
    }

    #[tokio::test]
    async fn creates_with_subdir_prepend() {
        let tmp = TempDir::new().unwrap();
        let root_path = tmp.path().to_path_buf();
        let proj_path = root_path.join("myproj");
        std::fs::create_dir_all(&proj_path).unwrap();
        let mut ctx = mk_ctx(root_path.clone());
        ctx.current_project = Some(Arc::new(CurrentProject {
            abs_path: proj_path.clone(),
            git_root: root_path.clone(),
            umbrella: None,
        }));
        let result = call(
            &ctx,
            json!({
                "rel_path": "docs/foo.md",
                "kind": "spec",
                "title": "Subdir",
                "body": "body"
            }),
        )
        .await
        .unwrap();
        let abs = result["abs_path"].as_str().unwrap();
        let expected = proj_path.join("docs/foo.md");
        assert_eq!(abs, expected.to_string_lossy());
    }

    #[tokio::test]
    async fn wrong_repo_error_lists_valid_names() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        let err = call(
            &ctx,
            json!({
                "repo": "no-such-repo",
                "rel_path": "docs/x.md",
                "kind": "spec",
                "title": "X",
                "body": ""
            }),
        )
        .await
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("no-such-repo"), "should name the bad repo");
        assert!(
            msg.contains('"') || msg.contains('r'),
            "should list valid repos"
        );
    }
}
