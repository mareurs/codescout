use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::catalog::augmentation;
use crate::tools::{RecoverableError, Tool, ToolContext};

use super::scope::Scope;

pub struct ArtifactRefreshStale;

#[derive(serde::Deserialize)]
struct Args {
    threshold_hours: Option<u32>,
    limit: Option<usize>,
    scope: Option<Scope>,
}

const MAX_LIMIT: usize = 50;
const DEFAULT_THRESHOLD_HOURS: u32 = 24;
const DEFAULT_LIMIT: usize = 10;

#[async_trait]
impl Tool for ArtifactRefreshStale {
    fn name(&self) -> &'static str {
        "artifact_refresh_stale"
    }

    fn description(&self) -> &'static str {
        "List augmented artifacts whose last refresh is older than threshold_hours (default 24h). \
         Returns them oldest-first (never-refreshed first) so you know what needs attention. \
         Scope defaults to project. Call artifact_refresh(id) on each result to refresh."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "threshold_hours": {
                    "type": "integer",
                    "description": "Hours since last refresh to consider stale (default 24).",
                    "default": 24
                },
                "limit": {
                    "type": "integer",
                    "description": "Max results returned (default 10, max 50).",
                    "default": 10
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "all"],
                    "description": "Scope: project = current sub-project (default), repo = whole root, all = workspace-wide.",
                    "default": "project"
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let threshold_hours = a.threshold_hours.unwrap_or(DEFAULT_THRESHOLD_HOURS);
        let limit = a.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
        let scope = a.scope.unwrap_or(Scope::Project);

        let current = ctx.current_project.as_deref();

        let (repo, subdir_prefix): (Option<&str>, Option<&str>) = match scope {
            Scope::All => (None, None),
            Scope::Repo => {
                let cp = current.ok_or_else(|| {
                    RecoverableError::new(
                        "scope=repo requires a resolved current project. Pass scope=\"all\".",
                    )
                })?;
                (Some(cp.root.as_str()), None)
            }
            Scope::Project => {
                let cp = current.ok_or_else(|| {
                    RecoverableError::new(
                        "scope=project requires a resolved current project. Pass scope=\"all\".",
                    )
                })?;
                let subdir = if cp.subdir.is_empty() {
                    None
                } else {
                    Some(cp.subdir.as_str())
                };
                (Some(cp.root.as_str()), subdir)
            }
            Scope::Umbrella => {
                return Err(RecoverableError::new(
                    "scope=umbrella is not supported. Use scope=project|repo|all.",
                ));
            }
        };

        let threshold_iso = {
            let cutoff = chrono::Utc::now() - chrono::Duration::hours(i64::from(threshold_hours));
            cutoff.to_rfc3339()
        };

        let entries = {
            let cat = ctx.catalog.lock();
            augmentation::list_stale(&cat, &threshold_iso, limit, repo, subdir_prefix)?
        };

        let now = chrono::Utc::now();
        let items: Vec<Value> = entries
            .iter()
            .map(|e| {
                let age_hours = e.last_refreshed_at.as_deref().and_then(|t| {
                    chrono::DateTime::parse_from_rfc3339(t)
                        .ok()
                        .map(|dt| (now - dt.with_timezone(&chrono::Utc)).num_hours())
                });
                json!({
                    "id": e.artifact_id,
                    "kind": e.kind,
                    "title": e.title,
                    "rel_path": e.rel_path,
                    "last_refreshed_at": e.last_refreshed_at,
                    "refresh_count": e.refresh_count,
                    "age_hours": age_hours,
                })
            })
            .collect();

        let next_step = if items.is_empty() {
            "No stale augmented artifacts in scope.".to_string()
        } else {
            "Call artifact_refresh(id) on each item, synthesize updates, \
             then artifact_update(id, commit_refresh=true)."
                .to_string()
        };

        Ok(json!({
            "count": items.len(),
            "threshold_hours": threshold_hours,
            "items": items,
            "next_step": next_step,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::ArtifactRow;
    use crate::catalog::{artifact, augmentation, Catalog};
    use serde_json::json;

    fn sample_art(id: &str, repo: &str, rel_path: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: repo.into(),
            rel_path: rel_path.into(),
            kind: "tracker".into(),
            status: "active".into(),
            title: Some(format!("Tracker {id}")),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime: 0,
            file_sha256: "abc".into(),
            confidence: 1.0,
        }
    }

    fn aug_row(
        artifact_id: &str,
        last_refreshed_at: Option<&str>,
    ) -> augmentation::AugmentationRow {
        augmentation::AugmentationRow {
            artifact_id: artifact_id.into(),
            prompt: "keep updated".into(),
            params: "{}".into(),
            last_refreshed_at: last_refreshed_at.map(str::to_string),
            refresh_count: 0,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-01T00:00:00Z".into(),
            render_template: None,
            params_schema: None,
        }
    }

    #[test]
    fn list_stale_returns_never_refreshed_first() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_art("a1", "claude", "proj/t1.md")).unwrap();
        artifact::upsert(&cat, &sample_art("a2", "claude", "proj/t2.md")).unwrap();
        augmentation::upsert(&cat, &aug_row("a1", None)).unwrap();
        augmentation::upsert(&cat, &aug_row("a2", Some("2000-01-01T00:00:00Z"))).unwrap();

        // threshold far in the future so both are stale
        let entries =
            augmentation::list_stale(&cat, "9999-01-01T00:00:00Z", 10, None, None).unwrap();
        assert_eq!(entries.len(), 2);
        // never-refreshed (NULL) sorts first
        assert_eq!(entries[0].artifact_id, "a1");
        assert!(entries[0].last_refreshed_at.is_none());
    }

    #[test]
    fn list_stale_threshold_filters_fresh() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_art("a1", "claude", "proj/t1.md")).unwrap();
        artifact::upsert(&cat, &sample_art("a2", "claude", "proj/t2.md")).unwrap();
        // a1: refreshed in the future (fresh), a2: never refreshed (stale)
        augmentation::upsert(&cat, &aug_row("a1", Some("9999-01-01T00:00:00Z"))).unwrap();
        augmentation::upsert(&cat, &aug_row("a2", None)).unwrap();

        let threshold = chrono::Utc::now().to_rfc3339();
        let entries = augmentation::list_stale(&cat, &threshold, 10, None, None).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].artifact_id, "a2");
    }

    #[test]
    fn list_stale_repo_scope_filters() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_art("a1", "claude", "proj/t1.md")).unwrap();
        artifact::upsert(&cat, &sample_art("a2", "other", "proj/t2.md")).unwrap();
        augmentation::upsert(&cat, &aug_row("a1", None)).unwrap();
        augmentation::upsert(&cat, &aug_row("a2", None)).unwrap();

        let entries =
            augmentation::list_stale(&cat, "9999-01-01T00:00:00Z", 10, Some("claude"), None)
                .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].artifact_id, "a1");
    }

    #[test]
    fn list_stale_subdir_scope_filters() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_art("a1", "claude", "code-explorer/t1.md")).unwrap();
        artifact::upsert(&cat, &sample_art("a2", "claude", "mempalace/t2.md")).unwrap();
        augmentation::upsert(&cat, &aug_row("a1", None)).unwrap();
        augmentation::upsert(&cat, &aug_row("a2", None)).unwrap();

        let entries = augmentation::list_stale(
            &cat,
            "9999-01-01T00:00:00Z",
            10,
            Some("claude"),
            Some("code-explorer"),
        )
        .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].artifact_id, "a1");
    }

    #[test]
    fn list_stale_limit_respected() {
        let cat = Catalog::open_in_memory().unwrap();
        for i in 0..5 {
            let id = format!("a{i}");
            artifact::upsert(&cat, &sample_art(&id, "claude", &format!("proj/t{i}.md"))).unwrap();
            augmentation::upsert(&cat, &aug_row(&id, None)).unwrap();
        }
        let entries =
            augmentation::list_stale(&cat, "9999-01-01T00:00:00Z", 3, None, None).unwrap();
        assert_eq!(entries.len(), 3);
    }
}
