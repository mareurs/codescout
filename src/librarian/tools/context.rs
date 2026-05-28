use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::librarian::catalog::{artifact, augmentation, links};
use crate::librarian::filter::FilterNode;

use super::scope::{apply_scope, Scope};
use super::ToolContext;

use super::HIDDEN_STATUSES;

#[derive(Deserialize)]

struct Args {
    #[serde(default)]
    topic: Option<String>,
    #[serde(default)]
    anchor_id: Option<String>,
    #[serde(default)]
    max_tokens: Option<usize>,
    #[serde(default)]
    scope: Option<Scope>,
    #[serde(default)]
    include_archived: bool,
}

const DEFAULT_MAX_TOKENS: usize = 4000;

fn scope_summary(
    scope: Scope,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    fallback: bool,
) -> Value {
    json!({
        "applied": match scope {
            Scope::Project => "project",
            Scope::Repo => "repo",
            Scope::Umbrella => "umbrella",
            Scope::All => "all",
        },
        "root": current.map(|c| c.git_root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
        "subdir": current.map(|_| String::new()),
        "umbrella": current.and_then(|c| c.umbrella.clone()),
        "scope_fallback": fallback,
    })
}
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    use crate::librarian::catalog::find::{find, FindOpts};
    use std::collections::HashMap;

    let a: Args = serde_json::from_value(args)?;
    let max_tokens = a.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    let char_cap = max_tokens * 4;

    let requested_scope = a.scope.unwrap_or_default();
    let (effective_scope, scope_fallback) = match (requested_scope, ctx.current_project.is_some()) {
        (Scope::Project | Scope::Repo, false) => (Scope::All, true),
        (s, _) => (s, false),
    };
    let current = ctx.current_project.as_deref();

    let topic_vec: Option<Vec<f32>> =
        if let (Some(ref topic), Some(ref svc)) = (&a.topic, &ctx.embedding) {
            Some(svc.embedder.embed_query(topic).await?)
        } else {
            None
        };

    let candidate_ids: Vec<String> = {
        let cat = ctx.catalog.lock();
        if let Some(ref anchor_id) = a.anchor_id {
            let mut ids: Vec<String> = vec![anchor_id.clone()];
            let out = links::outgoing(&cat, anchor_id)?;
            let inc = links::incoming(&cat, anchor_id)?;
            for link in out {
                if !ids.contains(&link.dst_id) {
                    ids.push(link.dst_id);
                }
            }
            for link in inc {
                if !ids.contains(&link.src_id) {
                    ids.push(link.src_id);
                }
            }
            ids
        } else if a.topic.is_some() {
            let archived_clause = if a.include_archived {
                None
            } else {
                Some(FilterNode::Leaf(
                    [("status".to_string(), json!({"nin": HIDDEN_STATUSES}))]
                        .into_iter()
                        .collect(),
                ))
            };
            let (scoped_filter, _) =
                apply_scope(archived_clause, effective_scope, &ctx.workspace, current)?;

            if let Some(vec) = topic_vec {
                let rows = find(
                    &cat,
                    &FindOpts {
                        filter: scoped_filter,
                        limit: 50,
                        offset: 0,
                        semantic: Some(vec),
                    },
                )?;
                rows.into_iter().map(|r| r.id).collect()
            } else {
                let topic = a.topic.as_deref().unwrap_or("");
                let topic_clause = FilterNode::Or {
                    or: vec![
                        FilterNode::Leaf(
                            [("title".to_string(), json!({"contains": topic}))]
                                .into_iter()
                                .collect(),
                        ),
                        FilterNode::Leaf(
                            [("topic".to_string(), json!({"contains": topic}))]
                                .into_iter()
                                .collect(),
                        ),
                    ],
                };
                let combined = match scoped_filter {
                    Some(s) => FilterNode::And {
                        and: vec![s, topic_clause],
                    },
                    None => topic_clause,
                };
                let rows = find(
                    &cat,
                    &FindOpts {
                        filter: Some(combined),
                        limit: 50,
                        offset: 0,
                        semantic: None,
                    },
                )?;
                rows.into_iter().map(|r| r.id).collect()
            }
        } else {
            // No anchor, no topic: surface active goal-trackers.
            let mut clauses: Vec<FilterNode> = vec![
                FilterNode::Leaf(
                    [("kind".to_string(), json!({"eq": "tracker"}))]
                        .into_iter()
                        .collect(),
                ),
                FilterNode::Leaf(
                    [("tags".to_string(), json!({"contains": "goal"}))]
                        .into_iter()
                        .collect(),
                ),
                FilterNode::Leaf(
                    [("status".to_string(), json!({"eq": "active"}))]
                        .into_iter()
                        .collect(),
                ),
            ];
            if !a.include_archived {
                clauses.push(FilterNode::Leaf(
                    [("status".to_string(), json!({"nin": HIDDEN_STATUSES}))]
                        .into_iter()
                        .collect(),
                ));
            }
            let goal_filter = FilterNode::And { and: clauses };
            let (scoped_filter, _) =
                apply_scope(Some(goal_filter), effective_scope, &ctx.workspace, current)?;
            let rows = find(
                &cat,
                &FindOpts {
                    filter: scoped_filter,
                    limit: 10,
                    offset: 0,
                    semantic: None,
                },
            )?;
            rows.into_iter().map(|r| r.id).collect()
        }
    };

    let rows_map: HashMap<String, artifact::ArtifactRow> = {
        let cat = ctx.catalog.lock();
        if candidate_ids.is_empty() {
            HashMap::new()
        } else {
            let placeholders = (0..candidate_ids.len())
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT id, abs_path, kind, status, title, owners, tags, topic, \
                 time_scope, source, created_at, updated_at, file_mtime, \
                 file_sha256, confidence FROM artifact WHERE id IN ({placeholders})"
            );
            let mut stmt = cat.conn.prepare(&sql)?;
            let params = rusqlite::params_from_iter(candidate_ids.iter());
            let rows: Vec<artifact::ArtifactRow> = stmt
                .query_map(params, artifact::row_from_sql)?
                .collect::<Result<_, _>>()?;
            rows.into_iter().map(|r| (r.id.clone(), r)).collect()
        }
    };

    let aug_map: std::collections::HashMap<String, augmentation::AugmentationRow> = {
        let cat = ctx.catalog.lock();
        augmentation::get_batch(&cat, &candidate_ids)?
    };

    let mut sorted_ids = candidate_ids.clone();
    sorted_ids.sort_by_key(|id| {
        let is_tracker = rows_map
            .get(id.as_str())
            .is_some_and(|r| r.kind == "tracker");
        let is_augmented = aug_map.contains_key(id.as_str());
        match (is_tracker, is_augmented) {
            (true, _) => 0u8,
            (false, true) => 1,
            _ => 2,
        }
    });

    let active_goals_header =
        matches!((&a.topic, &a.anchor_id), (None, None)) && !sorted_ids.is_empty();
    let mut markdown = if active_goals_header {
        String::from("## Active goals\n\n")
    } else {
        String::new()
    };
    let mut included_ids: Vec<String> = Vec::new();

    for id in &sorted_ids {
        let row = match rows_map.get(id) {
            Some(r) => r,
            None => continue,
        };
        let full_path = row.abs_path.clone();
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let body = match crate::librarian::frontmatter::parse(&content) {
            Ok((_, body)) => body.to_string(),
            Err(_) => content.clone(),
        };
        let first_30: String = body.lines().take(30).collect::<Vec<_>>().join("\n");
        let title = row.title.as_deref().unwrap_or("(untitled)");
        let section = if let Some(aug) = aug_map.get(id.as_str()) {
            let refreshed = aug.last_refreshed_at.as_deref().unwrap_or("never");
            let rendered = aug.render_template.as_deref().map(|tmpl| {
                let params: Value =
                    serde_json::from_str(&aug.params).unwrap_or(Value::Object(Default::default()));
                match crate::librarian::tools::render::render_params(tmpl, &params) {
                    Ok(s) => format!("{s}\n\n"),
                    Err(e) => format!("<!-- render_template error: {e} -->\n\n"),
                }
            });
            format!(
                "<!-- [LIVE]: {} | last refreshed: {} | refresh #{} -->\n\
                 > Prompt: {}\n\n\
                 {}## {}  — {}/{}  ({})\n{}\n\n",
                title,
                refreshed,
                aug.refresh_count,
                aug.prompt,
                rendered.as_deref().unwrap_or(""),
                title,
                row.kind,
                row.status,
                row.abs_path.display(),
                first_30
            )
        } else {
            format!(
                "## {}  — {}/{}  ({})\n{}\n\n",
                title,
                row.kind,
                row.status,
                row.abs_path.display(),
                first_30
            )
        };
        if !markdown.is_empty() && (markdown.len() + section.len()) > char_cap {
            break;
        }
        markdown.push_str(&section);
        included_ids.push(id.clone());
        if markdown.len() >= char_cap {
            break;
        }
    }

    Ok(json!({
        "markdown": markdown,
        "included_ids": included_ids,
        "scope": scope_summary(effective_scope, current, scope_fallback),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact::ArtifactRow, Catalog};
    use crate::librarian::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn sample_row(
        id: &str,
        repo: &str,
        rel_path: &str,
        title: &str,
        topic: Option<&str>,
    ) -> ArtifactRow {
        let now = chrono::Utc::now().timestamp_millis();
        ArtifactRow {
            id: id.into(),
            abs_path: std::path::PathBuf::from(format!("/{repo}/{rel_path}")),
            kind: "spec".into(),
            status: "active".into(),
            title: Some(title.into()),
            owners: vec![],
            tags: vec![],
            topic: topic.map(|s| s.into()),
            time_scope: None,
            source: None,
            created_at: now,
            updated_at: now,
            file_mtime: now,
            file_sha256: "abc".into(),
            confidence: 1.0,
        }
    }

    fn mk_ctx(tmp_root: std::path::PathBuf, cat: Catalog) -> ToolContext {
        // Realign rows whose `sample_row` placeholder abs_path is `/r/{rel}`
        // to point under `tmp_root`, so files written under tmp_root resolve.
        let new_prefix = format!("{}/", tmp_root.display());
        cat.conn
            .execute(
                "UPDATE artifact SET abs_path = REPLACE(abs_path, '/r/', ?1)",
                rusqlite::params![new_prefix],
            )
            .unwrap();
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
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
    async fn topic_search_returns_matching_artifacts() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create 3 real .md files
        std::fs::write(root.join("auth_login.md"), "# Auth Login\nsome body\n").unwrap();
        std::fs::write(root.join("auth_signup.md"), "# Auth Signup\nsome body\n").unwrap();
        std::fs::write(root.join("billing.md"), "# Billing\nsome body\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_login.md", "r", "auth_login.md", "Auth Login", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row(
                "r/auth_signup.md",
                "r",
                "auth_signup.md",
                "Auth Signup",
                None,
            ),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/billing.md", "r", "billing.md", "Billing", None),
        )
        .unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        let v = call(&ctx, json!({"topic": "auth"})).await.unwrap();

        let ids = v["included_ids"].as_array().unwrap();
        assert_eq!(ids.len(), 2, "only auth artifacts should be included");

        let md = v["markdown"].as_str().unwrap();
        assert!(
            md.contains("Auth Login"),
            "markdown should contain Auth Login title"
        );
        assert!(
            md.contains("Auth Signup"),
            "markdown should contain Auth Signup title"
        );
        assert!(
            !md.contains("Billing"),
            "markdown should not contain Billing"
        );
    }

    #[tokio::test]
    async fn topic_search_hides_retired_artifacts() {
        // Regression for the HIDDEN_STATUSES split-brain: the context topic
        // branch must hide `retired` artifacts exactly as find() does.
        // See docs/issues/2026-05-25-hidden-statuses-context-missing-retired.md
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        std::fs::write(root.join("auth_live.md"), "# Auth Live\nsome body\n").unwrap();
        std::fs::write(root.join("auth_retired.md"), "# Auth Retired\nsome body\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_live.md", "r", "auth_live.md", "Auth Live", None),
        )
        .unwrap();
        let mut retired = sample_row(
            "r/auth_retired.md",
            "r",
            "auth_retired.md",
            "Auth Retired",
            None,
        );
        retired.status = "retired".into();
        artifact::upsert(&cat, &retired).unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        let v = call(&ctx, json!({"topic": "auth"})).await.unwrap();

        let ids = v["included_ids"].as_array().unwrap();
        assert_eq!(
            ids.len(),
            1,
            "retired artifact must be hidden in topic context, like find()"
        );
        let md = v["markdown"].as_str().unwrap();
        assert!(md.contains("Auth Live"), "live artifact should be present");
        assert!(
            !md.contains("Auth Retired"),
            "retired artifact must not leak into context markdown"
        );
    }

    #[tokio::test]
    async fn max_tokens_caps_inclusion() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create 2 auth files
        std::fs::write(root.join("auth_a.md"), "# Auth A\n".repeat(5)).unwrap();
        std::fs::write(root.join("auth_b.md"), "# Auth B\n".repeat(5)).unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_a.md", "r", "auth_a.md", "Auth A", None),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("r/auth_b.md", "r", "auth_b.md", "Auth B", None),
        )
        .unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        // max_tokens=1 means char_cap=4 — way too small for any full section, but first
        // artifact is always included (budget check only triggers on subsequent artifacts).
        // Use a slightly larger budget that fits exactly 1 section.
        // Each section header is ~50+ chars; set max_tokens=15 (60 chars) → fits 1, not 2.
        let v = call(&ctx, json!({"topic": "auth", "max_tokens": 15}))
            .await
            .unwrap();

        let ids = v["included_ids"].as_array().unwrap();
        assert_eq!(
            ids.len(),
            1,
            "max_tokens should cap inclusion to 1 artifact"
        );
    }

    #[tokio::test]
    async fn no_args_with_no_active_goals_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf(), cat);

        let v = call(&ctx, json!({})).await.unwrap();

        assert_eq!(v["markdown"].as_str().unwrap(), "");
        assert_eq!(v["included_ids"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn no_args_returns_active_goals_header() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // Create the real .md file the no-anchor branch will read.
        let goal_dir = root.join("docs/trackers");
        std::fs::create_dir_all(&goal_dir).unwrap();
        std::fs::write(goal_dir.join("goal-a.md"), "# Ship Feature X\nsome body\n").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut goal_row = sample_row(
            "r/docs/trackers/goal-a.md",
            "r",
            "docs/trackers/goal-a.md",
            "Ship Feature X",
            None,
        );
        goal_row.kind = "tracker".into();
        goal_row.tags = vec!["goal".into()];
        artifact::upsert(&cat, &goal_row).unwrap();

        let ctx = mk_ctx(root.to_path_buf(), cat);

        let v = call(&ctx, json!({})).await.unwrap();

        let md = v["markdown"].as_str().unwrap();
        assert!(
            md.contains("## Active goals"),
            "expected '## Active goals' header in markdown; got: {md}"
        );
        assert!(
            md.contains("Ship Feature X"),
            "expected goal title in active-goals section; got: {md}"
        );
    }

    #[tokio::test]
    async fn repo_scope_excludes_other_repos() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let cat = Catalog::open_in_memory().unwrap();

        // Active project lives at root/code-explorer with file inside.
        let proj_dir = root.join("code-explorer");
        std::fs::create_dir_all(&proj_dir).unwrap();
        std::fs::write(proj_dir.join("auth.md"), "# auth\nbody").unwrap();

        let mut in_proj = sample_row(
            "in",
            "claude",
            "code-explorer/auth.md",
            "auth notes",
            Some("auth"),
        );
        in_proj.abs_path = proj_dir.join("auth.md");
        let mut out_proj = sample_row("out", "agents", "x/auth.md", "auth elsewhere", Some("auth"));
        // Place the other repo's row outside the active git_root so scope=Repo excludes it.
        let other_root = std::path::PathBuf::from("/some/other/repo");
        out_proj.abs_path = other_root.join("x/auth.md");
        artifact::upsert(&cat, &in_proj).unwrap();
        artifact::upsert(&cat, &out_proj).unwrap();

        let ctx = ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "claude".into(),
                    path: root.clone(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: Some(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: proj_dir.clone(),
                    git_root: root.clone(),
                    umbrella: None,
                },
            )),
        };

        let v = call(&ctx, json!({"topic": "auth"})).await.unwrap();
        let included = v["included_ids"].as_array().unwrap();
        assert_eq!(included.len(), 1);
        assert_eq!(included[0], "in");
        assert_eq!(v["scope"]["applied"], "repo");
    }

    #[tokio::test]
    async fn live_header_present_for_augmented_artifact() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        use std::io::Write;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();

        // Write the artifact file to disk.
        let mut f = std::fs::File::create(root.join("tracker.md")).unwrap();
        writeln!(f, "# My Tracker\n\nsome content").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut row = sample_row(
            "ctx-aug",
            "r",
            "tracker.md",
            "My Tracker",
            Some("live-test"),
        );
        row.kind = "tracker".into();
        artifact::upsert(&cat, &row).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "ctx-aug".to_string(),
                prompt: "Maintain state".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
            },
        )
        .unwrap();

        let ctx = mk_ctx(root, cat);
        let result = call(&ctx, json!({"topic": "live-test"})).await.unwrap();

        let md = result["markdown"].as_str().unwrap();
        assert!(md.contains("[LIVE]"), "expected [LIVE] in:\n{md}");
        assert!(md.contains("Maintain state"), "expected prompt in:\n{md}");
    }

    #[tokio::test]
    async fn render_template_projects_params_into_context() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        use std::io::Write;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();

        let mut f = std::fs::File::create(root.join("tracker.md")).unwrap();
        writeln!(f, "# Eval Tracker\n\nProse-only body.").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut row = sample_row(
            "ctx-tmpl",
            "r",
            "tracker.md",
            "Eval Tracker",
            Some("render-test"),
        );
        row.kind = "tracker".into();
        artifact::upsert(&cat, &row).unwrap();

        let template = "**Status:** {{ status }} ({{ failures|length }} failing)";
        let params = r#"{"status":"red","failures":[{"id":"F-1"},{"id":"F-2"}]}"#;
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "ctx-tmpl".to_string(),
                prompt: "Maintain F-N table".to_string(),
                params: params.to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: Some(template.to_string()),
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
            },
        )
        .unwrap();

        let ctx = mk_ctx(root, cat);
        let result = call(&ctx, json!({"topic": "render-test"})).await.unwrap();

        let md = result["markdown"].as_str().unwrap();
        assert!(md.contains("[LIVE]"), "expected [LIVE] in:\n{md}");
        assert!(
            md.contains("**Status:** red (2 failing)"),
            "expected rendered template line in:\n{md}"
        );
    }

    #[tokio::test]
    async fn render_template_error_surfaces_in_context() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        use std::io::Write;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let mut f = std::fs::File::create(root.join("t.md")).unwrap();
        writeln!(f, "# T\n\nbody").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        let mut row = sample_row("ctx-bad", "r", "t.md", "T", Some("bad-tmpl"));
        row.kind = "tracker".into();
        artifact::upsert(&cat, &row).unwrap();

        // Intentionally malformed template
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "ctx-bad".to_string(),
                prompt: "p".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: Some("{% for x in %}".to_string()),
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
            },
        )
        .unwrap();

        let ctx = mk_ctx(root, cat);
        let result = call(&ctx, json!({"topic": "bad-tmpl"})).await.unwrap();

        let md = result["markdown"].as_str().unwrap();
        assert!(
            md.contains("render_template error"),
            "expected error comment in:\n{md}"
        );
    }

    #[tokio::test]
    async fn augmented_artifacts_sorted_before_plain() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};

        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();

        // Write files for both artifacts.
        std::fs::write(root.join("plain.md"), "# Plain\nbody").unwrap();
        std::fs::write(root.join("aug.md"), "# Augmented\nbody").unwrap();

        let cat = Catalog::open_in_memory().unwrap();
        // Insert plain first so it would appear first without sorting.
        artifact::upsert(
            &cat,
            &sample_row("plain", "r", "plain.md", "Plain", Some("sort-test")),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &sample_row("aug", "r", "aug.md", "Augmented", Some("sort-test")),
        )
        .unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "aug".to_string(),
                prompt: "keep fresh".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
            },
        )
        .unwrap();

        let ctx = mk_ctx(root, cat);
        let result = call(&ctx, json!({"topic": "sort-test"})).await.unwrap();

        let included = result["included_ids"].as_array().unwrap();
        assert_eq!(included.len(), 2);
        // Augmented artifact should appear before plain.
        assert_eq!(included[0], "aug");
        assert_eq!(included[1], "plain");
    }
}
