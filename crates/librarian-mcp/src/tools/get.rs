use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{RecoverableError, Tool, ToolContext};
use crate::catalog::{artifact, augmentation, links, observations};
use rusqlite;

use crate::frontmatter;
use crate::preview::headings;
use std::path::PathBuf;

const SOFT_CAP_LINES: usize = 500;
const OVERFLOW_HEADING_LIMIT: usize = 10;

fn resolve_file_path(
    ctx: &ToolContext,
    row: &crate::catalog::artifact::ArtifactRow,
) -> Option<PathBuf> {
    ctx.workspace
        .roots
        .iter()
        .find(|r| r.name == row.repo)
        .map(|r| r.path.join(&row.rel_path))
}

fn normalize_heading(s: &str) -> String {
    s.trim().trim_start_matches('#').trim().to_lowercase()
}

fn find_heading_section(hs: &[headings::Heading], body: &str, query: &str) -> Option<String> {
    let normalized_query = normalize_heading(query);
    let idx = hs
        .iter()
        .position(|h| normalize_heading(&h.text) == normalized_query)?;
    let start_line = hs[idx].line;
    let start_level = hs[idx].level;
    let end_line = hs[idx + 1..]
        .iter()
        .find(|h| h.level <= start_level)
        .map(|h| h.line)
        .unwrap_or(usize::MAX);
    let lines: Vec<&str> = body.lines().collect();
    let slice_end = std::cmp::min(end_line.saturating_sub(1), lines.len());
    Some(lines[start_line - 1..slice_end].join("\n"))
}

fn slice_lines(body: &str, start: usize, end: usize) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if start == 0 || start > lines.len() {
        return String::new();
    }
    let end = std::cmp::min(end, lines.len());
    lines[start - 1..end].join("\n")
}

fn apply_soft_cap(body: &str) -> (String, Option<(usize, usize, Vec<String>)>) {
    let lines: Vec<&str> = body.lines().collect();
    let total = lines.len();
    if total <= SOFT_CAP_LINES {
        return (body.to_string(), None);
    }
    let shown: String = lines[..SOFT_CAP_LINES].join("\n");
    let top_headings: Vec<String> = headings::parse(body)
        .into_iter()
        .filter(|h| h.level <= 2)
        .take(OVERFLOW_HEADING_LIMIT)
        .map(|h| h.text)
        .collect();
    (shown, Some((SOFT_CAP_LINES, total, top_headings)))
}

pub struct ArtifactGet;

#[derive(Deserialize)]
struct Args {
    id: String,
    #[serde(default)]
    include_observations: Option<bool>,
    #[serde(default)]
    include_links: Option<bool>,
    /// Filter links by direction: "out"|"in"|"both". Only applies when include_links=true. Default: "both".
    #[serde(default)]
    links_direction: Option<String>,
    /// Filter links to only this rel type. Only applies when include_links=true.
    #[serde(default)]
    links_rel: Option<String>,
    #[serde(default)]
    full: Option<bool>,
    #[serde(default)]
    heading: Option<String>,
    #[serde(default)]
    headings: Option<Vec<String>>,
    #[serde(default)]
    start_line: Option<usize>,
    #[serde(default)]
    end_line: Option<usize>,
}

#[async_trait]
impl Tool for ArtifactGet {
    fn name(&self) -> &'static str {
        "artifact_get"
    }

    fn description(&self) -> &'static str {
        "Fetch a single artifact by id. Optionally include observations and links."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": {"type": "string"},
                "include_observations": {"type": "boolean", "default": false},
                "include_links": {"type": "boolean", "default": false},
                "links_direction": {
                    "type": "string",
                    "enum": ["out", "in", "both"],
                    "description": "Filter links by direction. Default: both. Only applies when include_links=true."
                },
                "links_rel": {
                    "type": "string",
                    "description": "Filter links to only this rel type. Only applies when include_links=true."
                },
                "full": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include full body (subject to soft cap)."
                },
                "heading": {
                    "type": "string",
                    "description": "Fetch one section by heading match (case-insensitive, trimmed)."
                },
                "headings": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Fetch multiple sections by heading match."
                },
                "start_line": {
                    "type": "integer",
                    "description": "1-indexed start of line slice. Pair with end_line."
                },
                "end_line": {
                    "type": "integer",
                    "description": "1-indexed inclusive end of line slice."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        if args.get("include_body").is_some() {
            anyhow::bail!(
                "parameter `include_body` was removed; use `full: true` for the full body, or `heading=\"<section>\"` for a targeted section"
            );
        }
        let a: Args = serde_json::from_value(args)?;
        let body_selectors = [
            a.full.unwrap_or(false),
            a.heading.is_some(),
            a.headings.as_ref().is_some_and(|v| !v.is_empty()),
            a.start_line.is_some() || a.end_line.is_some(),
        ];
        if body_selectors.iter().filter(|b| **b).count() > 1 {
            anyhow::bail!(
                "at most one of `full`, `heading`, `headings`, `start_line`+`end_line` may be set"
            );
        }
        if let (Some(s), Some(e)) = (a.start_line, a.end_line) {
            if s > e {
                anyhow::bail!("start_line ({s}) must be <= end_line ({e})");
            }
        }

        // Scope the catalog lock narrowly: fetch the row plus any optional
        // observations/links inside this block, then drop the guard before
        // calling `preview::extract` (which may re-acquire the lock for
        // certain kinds, e.g. memory). `parking_lot::Mutex` is not reentrant,
        // so holding it across `preview::extract` would deadlock.
        let want_observations = a.include_observations.unwrap_or(false);
        let want_links = a.include_links.unwrap_or(false);
        let (row, observations_json, links_json, latest_event_row, latest_reviewed_at, aug) = {
            let cat = ctx.catalog.lock();
            let row = match artifact::get(&cat, &a.id)? {
                Some(r) => r,
                None => return Ok(Value::Null),
            };

            let observations_json = if want_observations {
                let obs = observations::list_for_artifact(&cat, &a.id)?;
                Some(json!(obs
                    .into_iter()
                    .map(|o| json!({
                        "id": o.id,
                        "text": o.text,
                        "source": o.source,
                        "created_at": o.created_at,
                    }))
                    .collect::<Vec<_>>()))
            } else {
                None
            };

            let links_json = if want_links {
                let direction = a.links_direction.as_deref().unwrap_or("both");
                if !matches!(direction, "out" | "in" | "both") {
                    return Err(RecoverableError::new(format!(
                        "invalid links_direction '{}' — must be \"out\", \"in\", or \"both\"",
                        direction
                    )));
                }
                let rel_filter = a.links_rel.as_deref();

                let outgoing_items: Vec<Value> = if direction == "out" || direction == "both" {
                    links::outgoing(&cat, &a.id)?
                        .into_iter()
                        .filter(|l| rel_filter.is_none_or(|r| l.rel == r))
                        .map(|l| json!({"dst_id": l.dst_id, "rel": l.rel}))
                        .collect()
                } else {
                    vec![]
                };

                let incoming_items: Vec<Value> = if direction == "in" || direction == "both" {
                    links::incoming(&cat, &a.id)?
                        .into_iter()
                        .filter(|l| rel_filter.is_none_or(|r| l.rel == r))
                        .map(|l| json!({"src_id": l.src_id, "rel": l.rel}))
                        .collect()
                } else {
                    vec![]
                };

                Some(json!({
                    "outgoing": outgoing_items,
                    "incoming": incoming_items,
                }))
            } else {
                None
            };

            let latest_event_row = crate::catalog::events::latest_for_artifact(&cat, &a.id)?;
            let latest_reviewed_at: Option<i64> = cat
                .conn
                .query_row(
                    "SELECT MAX(created_at) FROM events WHERE artifact_id=?1 AND kind='reviewed'",
                    rusqlite::params![&a.id],
                    |r| r.get::<_, Option<i64>>(0),
                )
                .unwrap_or(None);

            let aug = augmentation::get(&cat, &a.id)?;

            (
                row,
                observations_json,
                links_json,
                latest_event_row,
                latest_reviewed_at,
                aug,
            )
        };

        let mut out = json!({
            "id": row.id,
            "repo": row.repo,
            "rel_path": row.rel_path,
            "kind": row.kind,
            "status": row.status,
            "title": row.title,
            "owners": row.owners,
            "tags": row.tags,
            "topic": row.topic,
            "time_scope": row.time_scope,
            "created_at": row.created_at,
            "updated_at": row.updated_at,
        });

        if let Some(v) = observations_json {
            out["observations"] = v;
        }
        if let Some(v) = links_json {
            out["links"] = v;
        }

        // TimeMachine freshness + latest_event annotations.
        let freshness = crate::freshness::compute(crate::freshness::FreshnessInputs {
            latest_event_kind: latest_event_row.as_ref().map(|e| e.kind.as_str()),
            latest_reviewed_at,
            file_updated_at: row.file_mtime,
            topo_distance_from_head: None,
            freshness_horizon: crate::freshness::FRESHNESS_HORIZON_DEFAULT,
        });
        out["freshness"] = serde_json::to_value(freshness)?;
        out["latest_event"] = match latest_event_row {
            Some(ref e) => json!({
                "id": e.id,
                "kind": e.kind,
                "created_at": e.created_at,
                "head_commit": e.head_commit,
            }),
            None => Value::Null,
        };

        out["augmentation"] = match aug {
            Some(a) => json!({
                "prompt": a.prompt,
                "params": serde_json::from_str::<Value>(&a.params).unwrap_or_else(|_| json!({})),
                "last_refreshed_at": a.last_refreshed_at,
                "refresh_count": a.refresh_count,
                "created_at": a.created_at,
                "updated_at": a.updated_at,
            }),
            None => Value::Null,
        };

        let file_path = resolve_file_path(ctx, &row);
        let body_selected = a.full.unwrap_or(false)
            || a.heading.is_some()
            || a.headings.as_ref().is_some_and(|v| !v.is_empty())
            || a.start_line.is_some()
            || a.end_line.is_some();

        let file_content = match &file_path {
            Some(p) => match std::fs::read_to_string(p) {
                Ok(c) => Some(c),
                Err(e) => {
                    out["preview"] = Value::Null;
                    out["body_error"] = json!(e.to_string());
                    None
                }
            },
            None => {
                out["preview"] = Value::Null;
                out["body_error"] = json!(format!("repo {:?} not in workspace.roots", row.repo));
                None
            }
        };

        let parsed_body: Option<String> =
            file_content
                .as_ref()
                .map(|content| match frontmatter::parse(content) {
                    Ok((_, b)) => b.to_string(),
                    Err(_) => content.clone(),
                });

        if let Some(body) = parsed_body.as_deref() {
            out["preview"] = crate::preview::extract(&row.kind, &row, body, ctx);

            if body_selected {
                // Parse headings once; reused by heading/headings selectors
                // and by the overflow hint on `full`.
                let parsed_headings = headings::parse(body);
                let (final_body, overflow_meta, body_meta_extra) = if let Some(ref name) = a.heading
                {
                    match find_heading_section(&parsed_headings, body, name) {
                        Some(section) => (section, None, json!({ "heading": name })),
                        None => (
                            String::new(),
                            None,
                            json!({ "heading": name, "heading_missing": true }),
                        ),
                    }
                } else if let Some(ref list) = a.headings {
                    let mut parts = Vec::new();
                    let mut missing = Vec::new();
                    for name in list {
                        match find_heading_section(&parsed_headings, body, name) {
                            Some(s) => parts.push(s),
                            None => missing.push(name.clone()),
                        }
                    }
                    let joined = parts.join("\n\n");
                    let extra = if missing.is_empty() {
                        json!({ "headings": list })
                    } else {
                        json!({ "headings": list, "headings_missing": missing })
                    };
                    (joined, None, extra)
                } else if let (Some(s), Some(e)) = (a.start_line, a.end_line) {
                    (
                        slice_lines(body, s, e),
                        None,
                        json!({ "start_line": s, "end_line": e }),
                    )
                } else {
                    // full = true
                    let (shown, overflow) = apply_soft_cap(body);
                    (shown, overflow, json!({}))
                };

                let source_line_count = body.lines().count();
                let returned_line_count = if final_body.is_empty() {
                    0
                } else {
                    final_body.lines().count()
                };
                let bytes = final_body.len();
                out["body"] = json!(final_body);
                let mut meta = json!({
                    "line_count": returned_line_count,
                    "source_line_count": source_line_count,
                    "bytes": bytes,
                });
                if let Some(extra) = body_meta_extra.as_object() {
                    for (k, v) in extra {
                        meta[k] = v.clone();
                    }
                }
                out["body_meta"] = meta;

                if let Some((shown, total, headings)) = overflow_meta {
                    let hint = format!(
                        "Body exceeds soft cap ({SOFT_CAP_LINES} lines). Narrow with heading=\"<section>\" or start_line=N, end_line=M. Top-level headings: {headings:?}"
                    );
                    out["overflow"] = json!({
                        "shown_lines": shown,
                        "total_lines": total,
                        "hint": hint,
                    });
                }
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{self, ArtifactRow};
    use crate::catalog::links::{self, LinkRow};
    use crate::catalog::observations::{self, ObservationRow};
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        }
    }

    fn mk_row(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: "r".into(),
            rel_path: format!("{id}.md"),
            kind: "spec".into(),
            status: "active".into(),
            title: Some(id.to_uppercase()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 1,
            file_mtime: 0,
            file_sha256: "".into(),
            confidence: 1.0,
        }
    }

    #[tokio::test]
    async fn get_with_links_and_observations() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        artifact::upsert(&cat, &mk_row("b")).unwrap();
        links::insert(
            &cat,
            &LinkRow {
                src_id: "a".into(),
                dst_id: "b".into(),
                rel: "implements".into(),
                created_at: 0,
            },
        )
        .unwrap();
        observations::insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "a".into(),
                text: "note".into(),
                source: None,
                created_at: 0,
            },
        )
        .unwrap();

        let ctx = mk_ctx(cat);
        let v = ArtifactGet
            .call(
                &ctx,
                json!({"id": "a", "include_links": true, "include_observations": true}),
            )
            .await
            .unwrap();

        assert_eq!(v["id"], "a");
        assert_eq!(
            v["links"]["outgoing"].as_array().unwrap().len(),
            1,
            "expected 1 outgoing link"
        );
        assert_eq!(
            v["observations"].as_array().unwrap().len(),
            1,
            "expected 1 observation"
        );
        // Preview is null here because mk_ctx has no roots configured.
        assert!(v["preview"].is_null());
    }

    #[tokio::test]
    async fn get_missing_returns_null() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat);
        let v = ArtifactGet
            .call(&ctx, json!({"id": "nonexistent"}))
            .await
            .unwrap();
        assert!(v.is_null());
    }

    #[tokio::test]
    async fn include_body_param_returns_migration_error() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = ArtifactGet
            .call(&ctx, json!({"id": "a", "include_body": true}))
            .await;
        let err = res.expect_err("include_body must error");
        let msg = format!("{err}");
        assert!(
            msg.contains("include_body") && msg.contains("full"),
            "error should mention migration: got {msg}"
        );
    }

    #[tokio::test]
    async fn conflicting_body_selectors_error() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = ArtifactGet
            .call(&ctx, json!({"id": "a", "full": true, "heading": "X"}))
            .await;
        assert!(res.is_err(), "conflicting selectors must error");
    }

    #[tokio::test]
    async fn start_line_greater_than_end_line_errors() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = ArtifactGet
            .call(&ctx, json!({"id": "a", "start_line": 10, "end_line": 5}))
            .await;
        assert!(res.is_err(), "inverted line range must error");
    }

    use crate::workspace::Root;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: build a context with one root pointing at a tempdir.
    fn mk_ctx_with_root(cat: Catalog) -> (ToolContext, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "r".into(),
                    path: dir.path().to_path_buf(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
        };
        (ctx, dir)
    }

    #[tokio::test]
    async fn full_true_returns_body_within_cap() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\nShort body.\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "full": true}))
            .await
            .unwrap();
        assert!(v["body"].as_str().unwrap().contains("Short body."));
        assert!(v.get("overflow").is_none(), "short body must not overflow");
    }

    #[tokio::test]
    async fn full_true_triggers_overflow_over_cap() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        let mut body = String::from("---\nkind: spec\n---\n\n");
        body.push_str("# Top\n\n");
        body.push_str("## Section One\n\n");
        for i in 0..600 {
            body.push_str(&format!("Line {i}\n"));
        }
        body.push_str("## Section Two\n");
        fs::write(dir.path().join("a.md"), body).unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "full": true}))
            .await
            .unwrap();
        let overflow = v["overflow"].as_object().expect("overflow present");
        assert!(overflow["total_lines"].as_u64().unwrap() > 500);
        assert_eq!(overflow["shown_lines"], 500);
        let hint = overflow["hint"].as_str().unwrap();
        assert!(
            hint.contains("heading="),
            "hint must suggest heading= usage"
        );
        assert!(hint.contains("Top"), "hint lists top-level headings");
    }

    #[tokio::test]
    async fn heading_targeted_read_returns_single_section() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# Title\n\n## Alpha\n\nalpha body\n\n## Beta\n\nbeta body\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "heading": "Alpha"}))
            .await
            .unwrap();
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("alpha body"));
        assert!(!body.contains("beta body"));
    }

    #[tokio::test]
    async fn heading_missing_sets_meta_flag() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# T\n\n## A\n\nx\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "heading": "Nonexistent"}))
            .await
            .unwrap();
        assert_eq!(v["body"], "");
        assert_eq!(v["body_meta"]["heading_missing"], true);
    }

    #[tokio::test]
    async fn line_slice_returns_requested_range() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        // NOTE: no blank line between the closing `---` and the content so that
        // start_line=1 corresponds to L1 in the parsed body.
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\nL1\nL2\nL3\nL4\nL5\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "start_line": 2, "end_line": 4}))
            .await
            .unwrap();
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("L2"));
        assert!(body.contains("L3"));
        assert!(body.contains("L4"));
        assert!(!body.contains("L1"));
        assert!(!body.contains("L5"));
    }

    #[tokio::test]
    async fn preview_present_by_default() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = mk_row("a");
        row.kind = "spec".into();
        artifact::upsert(&cat, &row).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# A\n\nHello world.\n",
        )
        .unwrap();

        let v = ArtifactGet.call(&ctx, json!({"id": "a"})).await.unwrap();
        assert_eq!(v["preview"]["shape"], "spec");
        assert!(v.get("body").is_none(), "body absent when not selected");
    }

    #[tokio::test]
    async fn preview_null_when_file_missing() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, _dir) = mk_ctx_with_root(cat);
        // Note: file was never written.

        let v = ArtifactGet.call(&ctx, json!({"id": "a"})).await.unwrap();
        assert!(v["preview"].is_null());
        assert!(v["body_error"].as_str().is_some());
    }

    #[tokio::test]
    async fn preview_null_when_repo_not_in_roots() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat); // mk_ctx has roots: vec![]

        let v = ArtifactGet.call(&ctx, json!({"id": "a"})).await.unwrap();
        assert!(v["preview"].is_null());
        assert!(v["body_error"]
            .as_str()
            .unwrap()
            .contains("workspace.roots"));
    }

    #[tokio::test]
    async fn end_to_end_plan_across_all_modes() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = mk_row("pl");
        row.kind = "plan".into();
        artifact::upsert(&cat, &row).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("pl.md"),
            "---\nkind: plan\n---\n\n\
# Big Plan\n\n\
## Phase 1\n\n\
- [ ] Alpha task\n\
- [x] Beta done\n\
- [ ] Gamma task\n\n\
## Phase 2\n\n\
- [ ] Delta task\n",
        )
        .unwrap();

        // Mode 1: preview default
        let v = ArtifactGet.call(&ctx, json!({"id": "pl"})).await.unwrap();
        assert_eq!(v["preview"]["shape"], "plan");
        assert_eq!(v["preview"]["tasks"]["total"], 4);
        assert_eq!(v["preview"]["tasks"]["done"], 1);
        let open = v["preview"]["tasks"]["open_next"].as_array().unwrap();
        assert_eq!(open[0], "Alpha task");
        assert!(v.get("body").is_none());

        // Mode 2: full body
        let v = ArtifactGet
            .call(&ctx, json!({"id": "pl", "full": true}))
            .await
            .unwrap();
        assert!(v["body"].as_str().unwrap().contains("Alpha task"));
        assert!(v["body"].as_str().unwrap().contains("Phase 2"));
        assert!(v.get("overflow").is_none());

        // Mode 3: heading-targeted read
        let v = ArtifactGet
            .call(&ctx, json!({"id": "pl", "heading": "Phase 1"}))
            .await
            .unwrap();
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("Alpha task"));
        assert!(body.contains("Gamma task"));
        assert!(
            !body.contains("Delta task"),
            "Phase 2 content must be excluded"
        );
    }

    #[tokio::test]
    async fn memory_kind_does_not_deadlock_on_preview() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut row = mk_row("m");
        row.kind = "memory".into();
        artifact::upsert(&cat, &row).unwrap();
        observations::insert(
            &cat,
            &ObservationRow {
                id: None,
                artifact_id: "m".into(),
                text: "test observation".into(),
                source: None,
                created_at: 100,
            },
        )
        .unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        std::fs::write(
            dir.path().join("m.md"),
            "---\nkind: memory\n---\n\nMemory body.\n",
        )
        .unwrap();

        // This call would deadlock if `call` holds the catalog lock across
        // `preview::extract` on a memory-kind artifact.
        let v = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            ArtifactGet.call(&ctx, json!({"id": "m"})),
        )
        .await
        .expect("artifact_get should not deadlock on memory kind")
        .unwrap();

        assert_eq!(v["preview"]["shape"], "memory");
        assert_eq!(v["preview"]["observation_count"], 1);
    }

    #[tokio::test]
    async fn body_meta_line_count_reflects_returned_body_for_heading() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# Title\n\n## Alpha\n\nline1\nline2\n\n## Beta\n\nbeta1\nbeta2\nbeta3\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(&ctx, json!({"id": "a", "heading": "Alpha"}))
            .await
            .unwrap();
        let returned = v["body"].as_str().unwrap();
        let expected_returned = returned.lines().count();
        assert_eq!(
            v["body_meta"]["line_count"].as_u64().unwrap() as usize,
            expected_returned,
            "line_count should reflect lines in returned body, not full source"
        );
        let src_lines = v["body_meta"]["source_line_count"].as_u64().unwrap() as usize;
        assert!(
            src_lines > expected_returned,
            "source_line_count should be total body lines"
        );
    }

    #[tokio::test]
    async fn multi_heading_selector_finds_all_sections() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# Title\n\n## Alpha\n\nalpha body\n\n## Beta\n\nbeta body\n\n## Gamma\n\ngamma body\n",
        )
        .unwrap();

        let v = ArtifactGet
            .call(
                &ctx,
                json!({"id": "a", "headings": ["Alpha", "Gamma", "Missing"]}),
            )
            .await
            .unwrap();
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("alpha body"));
        assert!(body.contains("gamma body"));
        assert!(!body.contains("beta body"));
        let missing = v["body_meta"]["headings_missing"].as_array().unwrap();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].as_str().unwrap(), "Missing");
    }

    #[tokio::test]
    async fn artifact_get_includes_freshness_unknown_by_default() {
        use crate::catalog::events;
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = ArtifactGet.call(&ctx, json!({"id": "a"})).await.unwrap();
        assert_eq!(res["freshness"], "unknown");
        assert!(res["latest_event"].is_null());
        let _ = events::latest_for_artifact; // keep import used
    }

    #[tokio::test]
    async fn artifact_get_freshness_after_reviewed_event() {
        use crate::catalog::events;
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        // Seed a reviewed event directly.
        events::insert(
            &cat,
            &events::EventRow {
                id: "ev1".into(),
                artifact_id: "a".into(),
                kind: "reviewed".into(),
                payload: "{}".into(),
                anchor_commit: None,
                head_commit: None,
                author: None,
                created_at: 1,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let res = ArtifactGet.call(&ctx, json!({"id": "a"})).await.unwrap();
        assert_eq!(res["freshness"], "fresh");
        assert_eq!(res["latest_event"]["kind"], "reviewed");
    }

    #[tokio::test]
    async fn get_includes_augmentation_when_present() {
        use crate::catalog::augmentation::{self, AugmentationRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("aug-art")).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "aug-art".to_string(),
                prompt: "Keep updated".to_string(),
                params: r#"{"format":"table"}"#.to_string(),
                last_refreshed_at: Some("2026-05-01T00:00:00.000Z".to_string()),
                refresh_count: 5,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = ArtifactGet
            .call(&ctx, json!({"id": "aug-art"}))
            .await
            .unwrap();
        let aug = &result["augmentation"];
        assert_eq!(aug["prompt"], "Keep updated");
        assert_eq!(aug["refresh_count"], 5);
        assert_eq!(aug["last_refreshed_at"], "2026-05-01T00:00:00.000Z");
        assert_eq!(aug["params"]["format"], "table");
    }

    #[tokio::test]
    async fn get_omits_augmentation_when_absent() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("plain-art")).unwrap();
        let ctx = mk_ctx(cat);
        let result = ArtifactGet
            .call(&ctx, json!({"id": "plain-art"}))
            .await
            .unwrap();
        assert!(result["augmentation"].is_null());
    }

    #[tokio::test]
    async fn include_links_direction_out_hides_incoming() {
        use crate::catalog::links as lcat;
        let cat = Catalog::open_in_memory().unwrap();
        let base = mk_row("center");
        let src = mk_row("other");
        artifact::upsert(&cat, &base).unwrap();
        artifact::upsert(&cat, &src).unwrap();
        lcat::insert(
            &cat,
            &lcat::LinkRow {
                src_id: "center".into(),
                dst_id: "other".into(),
                rel: "implements".into(),
                created_at: 0,
            },
        )
        .unwrap();
        lcat::insert(
            &cat,
            &lcat::LinkRow {
                src_id: "other".into(),
                dst_id: "center".into(),
                rel: "supersedes".into(),
                created_at: 0,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = ArtifactGet
            .call(
                &ctx,
                json!({"id": "center", "include_links": true, "links_direction": "out"}),
            )
            .await
            .unwrap();
        let outgoing = result["links"]["outgoing"].as_array().unwrap();
        let incoming = result["links"]["incoming"].as_array().unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(incoming.len(), 0);
    }

    #[tokio::test]
    async fn include_links_rel_filters_by_rel_type() {
        use crate::catalog::links as lcat;
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        artifact::upsert(&cat, &mk_row("b")).unwrap();
        artifact::upsert(&cat, &mk_row("c")).unwrap();
        lcat::insert(
            &cat,
            &lcat::LinkRow {
                src_id: "a".into(),
                dst_id: "b".into(),
                rel: "implements".into(),
                created_at: 0,
            },
        )
        .unwrap();
        lcat::insert(
            &cat,
            &lcat::LinkRow {
                src_id: "a".into(),
                dst_id: "c".into(),
                rel: "supersedes".into(),
                created_at: 0,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = ArtifactGet
            .call(
                &ctx,
                json!({"id": "a", "include_links": true, "links_rel": "implements"}),
            )
            .await
            .unwrap();
        let outgoing = result["links"]["outgoing"].as_array().unwrap();
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0]["rel"], "implements");
    }

    #[tokio::test]
    async fn invalid_links_direction_errors() {
        use crate::catalog::Catalog;
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("x")).unwrap();
        let ctx = mk_ctx(cat);
        let err = ArtifactGet
            .call(
                &ctx,
                json!({"id": "x", "include_links": true, "links_direction": "sideways"}),
            )
            .await;
        assert!(err.is_err());
    }
}
