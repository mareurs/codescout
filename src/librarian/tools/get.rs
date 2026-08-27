use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::{artifact, augmentation, entry_cite, links, observations};
use rusqlite;
use rusqlite::OptionalExtension;

use crate::librarian::frontmatter;
use crate::librarian::preview::headings;
use std::path::PathBuf;

use crate::librarian::filter::{eval, FilterNode};

const SOFT_CAP_LINES: usize = 500;
const OVERFLOW_HEADING_LIMIT: usize = 10;

fn resolve_file_path(
    _ctx: &ToolContext,
    row: &crate::librarian::catalog::artifact::ArtifactRow,
) -> Option<PathBuf> {
    Some(row.abs_path.clone())
}

/// Resolve one heading to its section text.
///
/// Returns the resolver's error rather than an `Option`. `.ok()` here used to collapse
/// "found N times" and "not found" into the same `None`, and the caller then reported
/// `heading_missing` for both — naming the false one, in a response whose own
/// `preview.headings` array listed the heading twice.
/// docs/issues/2026-08-27-artifact-get-reports-a-doubly-defined-heading-as-missing.md
fn find_heading_section<'q>(
    body: &str,
    query: impl Into<crate::tools::file_summary::HeadingQuery<'q>>,
) -> Result<String, crate::tools::RecoverableError> {
    crate::tools::file_summary::extract_markdown_section(body, query).map(|r| r.content)
}

/// Describe a heading that did not resolve, distinguishing ABSENT from AMBIGUOUS.
///
/// `resolve_section_range` already knows which it was and attaches `heading_ambiguous`
/// plus `occurrences` (the 1-indexed body lines) to the error. This reads those back
/// rather than re-deriving the state from message text — the message is prose and its
/// wording is not a contract.
///
/// Staying `isError: false` is deliberate and unchanged; see the comment at
/// `src/usage/db.rs` on why `artifact(get)` reports a heading miss in `body_meta`
/// instead of raising. Only the *label* was wrong.
fn heading_miss_meta(name: &str, err: &crate::tools::RecoverableError) -> serde_json::Value {
    match err.extra.get("occurrences") {
        Some(occurrences) => json!({
            "heading": name,
            "heading_ambiguous": true,
            "occurrences": occurrences,
            "heading_hint": err.hint().unwrap_or_default(),
        }),
        None => json!({ "heading": name, "heading_missing": true }),
    }
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
    /// 1-indexed selector when `heading` matches several sections. Two byte-identical
    /// headings admit no distinguishing query, so this is the only way to reach either.
    #[serde(default)]
    occurrence: Option<usize>,
    #[serde(default)]
    headings: Option<Vec<String>>,
    #[serde(default)]
    start_line: Option<usize>,
    #[serde(default)]
    end_line: Option<usize>,
    #[serde(default)]
    entry_filter: Option<FilterNode>,
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    if args.get("include_body").is_some() {
        return Err(RecoverableError::new(
            "parameter `include_body` was removed; use `full: true` for the full body, or `heading=\"<section>\"` for a targeted section",
        ));
    }
    let a: Args = serde_json::from_value(args)?;
    let body_selectors = [
        a.full.unwrap_or(false),
        a.heading.is_some(),
        a.headings.as_ref().is_some_and(|v| !v.is_empty()),
        a.start_line.is_some() || a.end_line.is_some(),
    ];
    if body_selectors.iter().filter(|b| **b).count() > 1 {
        return Err(RecoverableError::new(
            "at most one of `full`, `heading`, `headings`, `start_line`+`end_line` may be set",
        ));
    }
    if let (Some(s), Some(e)) = (a.start_line, a.end_line) {
        if s > e {
            return Err(RecoverableError::new(format!(
                "start_line ({s}) must be <= end_line ({e})"
            )));
        }
    }

    let want_observations = a.include_observations.unwrap_or(false);
    let want_links = a.include_links.unwrap_or(false);
    let (
        row,
        observations_json,
        links_json,
        entry_links_json,
        latest_event_row,
        latest_reviewed_at,
        aug,
        refreshed_at_commit,
        commits_behind_head,
        head_commit,
    ) = {
        let cat = ctx.catalog.lock();
        let row = match artifact::get(&cat, &a.id)? {
            Some(r) => r,
            None => {
                return Err(RecoverableError::new(format!(
                    "unknown artifact id '{}'. If this id came from an earlier call, an \
                     artifact(action=\"move\") since then will have re-keyed it (id = \
                     sha256(abs_path)); find it by path with artifact(action=\"find\", \
                     filter={{\"rel_path\": {{\"contains\": …}}}}, include_archived=true). If \
                     it was never seen before, run librarian(action=\"reindex\").",
                    a.id
                )));
            }
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

        let entry_links_json = if want_links {
            let slug: Option<String> = cat
                .conn
                .query_row(
                    "SELECT slug FROM artifact WHERE id = ?1",
                    rusqlite::params![a.id],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();
            let in_by_id = entry_cite::incoming(&cat, &a.id)?;
            let (out_rows, in_like) = match &slug {
                Some(s) => (
                    entry_cite::outgoing(&cat, s)?,
                    entry_cite::incoming_like(&cat, &format!("{s}:%"))?,
                ),
                None => (vec![], vec![]),
            };
            let out_items: Vec<Value> = out_rows
                .into_iter()
                .map(|e| json!({"src_local": e.src_local, "dst_ref": e.dst_ref, "rel": e.rel}))
                .collect();
            let in_items: Vec<Value> = in_by_id
                .into_iter()
                .chain(in_like)
                .map(|e| json!({"src": format!("{}:{}", e.src_slug, e.src_local), "rel": e.rel}))
                .collect();
            if out_items.is_empty() && in_items.is_empty() {
                None
            } else {
                Some(json!({"outgoing": out_items, "incoming": in_items}))
            }
        } else {
            None
        };

        let latest_event_row = crate::librarian::catalog::events::latest_for_artifact(&cat, &a.id)?;
        let latest_reviewed_at: Option<i64> = cat
            .conn
            .query_row(
                "SELECT MAX(created_at) FROM events WHERE artifact_id=?1 AND kind='reviewed'",
                rusqlite::params![&a.id],
                |r| r.get::<_, Option<i64>>(0),
            )
            .unwrap_or(None);

        let aug = augmentation::get(&cat, &a.id)?;

        // Server-computed provenance (finding 7): report staleness from an unforgeable
        // channel (git state the server computes), not content the artifact carries.
        let git_root_str = ctx
            .current_project
            .as_ref()
            .map(|p| crate::util::fs::RepoPath::from(&p.git_root).into_string());
        let head_commit = git_root_str.as_deref().and_then(|gr| {
            crate::librarian::catalog::commits::head_commit(&cat, gr)
                .ok()
                .flatten()
        });
        // refreshed_at_commit: the tracker's last-refresh HEAD, else the latest reviewed
        // event's commit (both server-written); None when neither exists.
        let refreshed_at_commit = aug
            .as_ref()
            .and_then(|a| a.refreshed_at_commit.clone())
            .or_else(|| {
                cat.conn
                    .query_row(
                        "SELECT head_commit FROM events \
                         WHERE artifact_id=?1 AND kind='reviewed' AND head_commit IS NOT NULL \
                         ORDER BY created_at DESC, id DESC LIMIT 1",
                        rusqlite::params![&a.id],
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .ok()
                    .flatten()
            });
        let commits_behind_head = match (
            git_root_str.as_deref(),
            refreshed_at_commit.as_deref(),
            head_commit.as_deref(),
        ) {
            (Some(gr), Some(rc), Some(hd)) => {
                crate::librarian::catalog::commits::topo_distance(&cat, gr, rc, hd)
                    .ok()
                    .flatten()
            }
            _ => None,
        };

        (
            row,
            observations_json,
            links_json,
            entry_links_json,
            latest_event_row,
            latest_reviewed_at,
            aug,
            refreshed_at_commit,
            commits_behind_head,
            head_commit,
        )
    };

    let mut out = json!({
        "id": row.id,
        "abs_path": row.abs_path.display().to_string(),
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

    // Overlay hint: a worktree session `get`-ing a main id that already has a
    // shadow forked in THIS worktree gets pointed at it — advisory only. `get`
    // stays id-literal and never redirects; the caller decides whether to
    // re-`get`/write the shadow id instead. Gated on `main_root.is_some()` — a
    // plain (non-worktree) session never runs this query and is unaffected.
    if let Some(cp) = ctx
        .current_project
        .as_deref()
        .filter(|c| c.main_root.is_some())
    {
        let wt = crate::util::fs::RepoPath::from(cp.git_root.as_path()).into_string();
        let cat = ctx.catalog.lock();
        // `shadow_main_pairs` (shared with find.rs's overlay dedup)
        // wildcard-escapes the worktree-root LIKE pattern.
        let pairs = crate::librarian::tools::worktree::shadow_main_pairs(&cat, &wt)?;
        let shadow_id = pairs
            .into_iter()
            .find_map(|(main_id, shadow_id)| (main_id == a.id).then_some(shadow_id));
        if let Some(sid) = shadow_id {
            out["overlay_hint"] = json!({
                "shadow_id": sid,
                "hint": "This session has forked this artifact; reads of the worktree state and all writes use the shadow id.",
            });
        }
    }

    if let Some(v) = observations_json {
        out["observations"] = v;
    }
    if let Some(v) = links_json {
        out["links"] = v;
    }
    if let Some(v) = entry_links_json {
        out["entry_links"] = v;
    }

    let freshness =
        crate::librarian::freshness::compute(crate::librarian::freshness::FreshnessInputs {
            latest_event_kind: latest_event_row.as_ref().map(|e| e.kind.as_str()),
            latest_reviewed_at,
            file_updated_at: row.file_mtime,
            topo_distance_from_head: commits_behind_head,
            freshness_horizon: crate::librarian::freshness::FRESHNESS_HORIZON_DEFAULT,
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
    // Finding 7: server-computed provenance keys (unforgeable staleness signal).
    out["provenance"] = json!({
        "refreshed_at_commit": refreshed_at_commit,
        "commits_behind_head": commits_behind_head,
        "head_commit": head_commit,
    });

    if let Some(ref filter) = a.entry_filter {
        let aug_row = aug.as_ref().ok_or_else(|| {
            RecoverableError::new(
                "entry_filter set but this artifact is not augmented — declare \
                 entry_collection on its augmentation, or retrofit it \
                 (docs/conventions/retrofitting-trackers-for-filtering.md)",
            )
        })?;
        let collection = aug_row.entry_collection.as_deref().ok_or_else(|| {
            RecoverableError::new(
                "entry_filter set but the augmentation has no entry_collection — \
                 declare which params array holds the filterable rows",
            )
        })?;
        let params: Value = serde_json::from_str(&aug_row.params)?;
        let arr = params
            .get(collection)
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                RecoverableError::new(format!(
                    "entry_collection points at `{collection}` but params has no array there"
                ))
            })?;
        let mut matched: Vec<Value> = Vec::new();
        let mut considered = 0usize;
        for item in arr {
            if let Some(obj) = item.as_object() {
                considered += 1;
                if eval(filter, obj)? {
                    matched.push(item.clone());
                }
            }
        }
        // F-7: the in-memory eval path has no field allowlist, so a filter
        // naming a field absent from every entry silently matches nothing.
        // Warn when a referenced field is present in zero entries (a likely
        // typo) — distinct from a genuine zero-match.
        if !arr.is_empty() {
            let present: std::collections::BTreeSet<String> = arr
                .iter()
                .filter_map(|i| i.as_object())
                .flat_map(|o| o.keys().cloned())
                .collect();
            let unknown: Vec<String> = crate::librarian::filter::referenced_fields(filter)
                .into_iter()
                .filter(|f| !present.contains(f))
                .collect();
            if !unknown.is_empty() {
                out["filter_warnings"] = json!({
                    "unknown_fields": unknown,
                    "hint": "these entry_filter fields are absent from every entry — an empty or reduced result may be a field-name typo, not a true zero-match",
                });
            }
        }
        out["entry_total"] = json!(considered);
        out["entries"] = json!(matched);
    }

    out["augmentation"] = match aug {
        Some(a) => json!({
            "prompt": a.prompt,
            "params": serde_json::from_str::<Value>(&a.params).unwrap_or_else(|_| json!({})),
            "last_refreshed_at": a.last_refreshed_at,
            "refresh_count": a.refresh_count,
            "created_at": a.created_at,
            "updated_at": a.updated_at,
            "render_template": a.render_template,
            "params_schema": a.params_schema,
            "append_mode": a.append_mode,
            "history_cap": a.history_cap,
            "entry_collection": a.entry_collection,
            "refreshed_at_commit": a.refreshed_at_commit,
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
            out["body_error"] = json!(format!(
                "file not found on disk: {}",
                row.abs_path.display()
            ));
            None
        }
    };

    let parsed_body: Option<String> =
        file_content
            .as_ref()
            .map(|content| match frontmatter::parse(content) {
                // Files conventionally carry a single blank separator line between the
                // frontmatter's closing `---` and the body content. Strip exactly one so
                // `start_line`/`end_line`/`source_line_count` are 1-indexed against the
                // first VISIBLE content line, not the invisible separator.
                Ok((_, b)) => b
                    .strip_prefix("\r\n")
                    .or_else(|| b.strip_prefix('\n'))
                    .unwrap_or(b)
                    .to_string(),
                Err(_) => content.clone(),
            });

    // Surface custom (non-first-class) frontmatter keys. These are YAML-only —
    // not in the catalog and not filterable via find — so the file is the only
    // source; parse it here (the content is already read for the preview).
    if let Some(content) = file_content.as_ref() {
        if let Ok((Some(fm), _)) = frontmatter::parse(content) {
            if !fm.extra.is_empty() {
                out["extra"] = serde_json::to_value(&fm.extra).unwrap_or(Value::Null);
            }
        }
    }

    if let Some(body) = parsed_body.as_deref() {
        out["preview"] = crate::librarian::preview::extract(&row.kind, &row, body, ctx);

        if body_selected {
            let (final_body, overflow_meta, body_meta_extra) = if let Some(ref name) = a.heading {
                let query =
                    crate::tools::file_summary::HeadingQuery::new(name.as_str(), a.occurrence);
                match find_heading_section(body, query) {
                    Ok(section) => (section, None, json!({ "heading": name })),
                    Err(e) => (String::new(), None, heading_miss_meta(name, &e)),
                }
            } else if let Some(ref list) = a.headings {
                // A per-member `occurrence` has no shape here (members are bare strings),
                // so an ambiguous member is REPORTED distinctly rather than folded in with
                // the absent ones. Reach it with the singular `heading` + `occurrence`.
                let mut parts = Vec::new();
                let mut missing = Vec::new();
                let mut ambiguous = Vec::new();
                for name in list {
                    match find_heading_section(body, name.as_str()) {
                        Ok(s) => parts.push(s),
                        Err(e) if e.extra.contains_key("occurrences") => {
                            ambiguous.push(json!({
                                "heading": name,
                                "occurrences": e.extra.get("occurrences"),
                            }));
                        }
                        Err(_) => missing.push(name.clone()),
                    }
                }
                let joined = parts.join("\n\n");
                let mut extra = json!({ "headings": list });
                if !missing.is_empty() {
                    extra["headings_missing"] = json!(missing);
                }
                if !ambiguous.is_empty() {
                    extra["headings_ambiguous"] = json!(ambiguous);
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, ArtifactRow, TestArtifactRowBuilder};
    use crate::librarian::catalog::links::{self, LinkRow};
    use crate::librarian::catalog::observations::{self, ObservationRow};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        TestToolContextBuilder::new(cat).build()
    }

    fn mk_row(id: &str) -> ArtifactRow {
        TestArtifactRowBuilder::new(id)
            .with_title(id.to_uppercase())
            .with_updated_at(1)
            .build()
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
        let v = call(
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
        let err = call(&ctx, json!({"id": "nonexistent"}))
            .await
            .expect_err("unknown id must error, not return null");
        assert!(
            err.downcast_ref::<crate::librarian::tools::RecoverableError>()
                .is_some(),
            "unknown-id error must be recoverable (isError:false), not a fatal bail"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("nonexistent"),
            "error must name the id verbatim: {msg}"
        );
        assert!(
            msg.contains("reindex"),
            "error must point at the never-indexed recovery path: {msg}"
        );
        assert!(
            msg.contains("move") && msg.contains("find"),
            "error must point at the moved/re-keyed recovery path: {msg}"
        );
    }

    #[tokio::test]
    async fn get_existing_artifact_with_empty_body_is_distinguishable_from_unknown_id() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"id": "a"})).await.unwrap();
        assert!(
            v.is_object(),
            "an artifact that exists must return an object, never null — \
             otherwise a fix that errors on both cases would still pass: {v}"
        );
    }

    #[tokio::test]
    async fn include_body_param_returns_migration_error() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = call(&ctx, json!({"id": "a", "include_body": true})).await;
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
        let err = call(&ctx, json!({"id": "a", "full": true, "heading": "X"}))
            .await
            .expect_err("conflicting selectors must error");
        assert!(
            err.downcast_ref::<crate::librarian::tools::RecoverableError>()
                .is_some(),
            "conflicting-selector error must be recoverable (isError:false), not a fatal bail"
        );
    }

    #[tokio::test]
    async fn start_line_greater_than_end_line_errors() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = call(&ctx, json!({"id": "a", "start_line": 10, "end_line": 5})).await;
        assert!(res.is_err(), "inverted line range must error");
    }

    use crate::librarian::workspace::Root;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: build a context with one root pointing at a tempdir.
    /// Rewrites any pre-existing rows' `abs_path` from the placeholder
    /// `/test/r/...` (set by `mk_row`) to point under the new tempdir,
    /// so files written into `dir.path()` resolve correctly.
    fn mk_ctx_with_root(cat: Catalog) -> (ToolContext, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        // Forward-slash — see the note in context.rs's mk_ctx: the catalog's
        // abs_path column is forward-slash by invariant.
        let new_prefix = format!("{}/", crate::util::fs::to_forward_slash(dir.path()));
        cat.conn
            .execute(
                "UPDATE artifact SET abs_path = REPLACE(abs_path, '/test/r/', ?1)",
                rusqlite::params![new_prefix],
            )
            .unwrap();
        let ctx = TestToolContextBuilder::new(cat)
            .with_root(Root {
                name: "r".into(),
                path: dir.path().to_path_buf(),
            })
            .build();
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

        let v = call(&ctx, json!({"id": "a", "full": true})).await.unwrap();
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

        let v = call(&ctx, json!({"id": "a", "full": true})).await.unwrap();
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

        let v = call(&ctx, json!({"id": "a", "heading": "Alpha"}))
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

        let v = call(&ctx, json!({"id": "a", "heading": "Nonexistent"}))
            .await
            .unwrap();
        assert_eq!(v["body"], "");
        assert_eq!(v["body_meta"]["heading_missing"], true);
    }

    /// A heading present TWICE is not "missing". Reporting it so sends the caller hunting
    /// for a heading that is right there — in a response whose own `preview.headings`
    /// array lists both occurrences.
    /// docs/issues/2026-08-27-artifact-get-reports-a-doubly-defined-heading-as-missing.md
    #[tokio::test]
    async fn duplicate_heading_reports_ambiguous_not_missing() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# T\n\n## A\n\nfirst\n\n## B\n\nb\n\n## A\n\nsecond\n",
        )
        .unwrap();

        let v = call(&ctx, json!({"id": "a", "heading": "## A"}))
            .await
            .unwrap();

        // Mutation control: restoring the `.ok()` that collapsed both error states into
        // `None` fails both of these — the call would report `heading_missing` instead.
        assert_eq!(v["body_meta"]["heading_ambiguous"], true);
        assert_eq!(v["body_meta"]["heading_missing"], Value::Null);

        // Both line numbers, in document order, so the caller can pick one without
        // re-reading the file. Asserted structurally rather than by literal line number:
        // the frontmatter-stripping frame is exactly what this bug's sibling is about.
        let occ = v["body_meta"]["occurrences"]
            .as_array()
            .expect("occurrences must be an array");
        assert_eq!(occ.len(), 2, "both matches must be reported: {occ:?}");
        assert!(
            occ[0].as_u64().unwrap() < occ[1].as_u64().unwrap(),
            "document order: {occ:?}"
        );
    }

    /// The over-correction guard: a heading that genuinely is not there must still say
    /// `missing`, not `ambiguous`. Pairs with `heading_missing_sets_meta_flag` above.
    #[tokio::test]
    async fn absent_heading_still_reports_missing_not_ambiguous() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# T\n\n## A\n\nfirst\n\n## A\n\nsecond\n",
        )
        .unwrap();

        let v = call(&ctx, json!({"id": "a", "heading": "## Nowhere"}))
            .await
            .unwrap();
        assert_eq!(v["body_meta"]["heading_missing"], true);
        assert_eq!(v["body_meta"]["heading_ambiguous"], Value::Null);
    }

    /// Reporting the ambiguity is only half a remedy — without a selector the caller is
    /// told the heading is ambiguous and still cannot read either section.
    #[tokio::test]
    async fn heading_with_occurrence_returns_the_named_match() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# T\n\n## A\n\nfirst\n\n## B\n\nb\n\n## A\n\nsecond\n",
        )
        .unwrap();

        let v = call(&ctx, json!({"id": "a", "heading": "## A", "occurrence": 2}))
            .await
            .unwrap();

        // Mutation control: dropping `occurrence` on the way to the resolver, or selecting
        // indices[0], returns "first" here.
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("second"), "{body}");
        assert!(!body.contains("first"), "{body}");
        assert_eq!(v["body_meta"]["heading_ambiguous"], Value::Null);
    }

    /// The plural selector takes bare strings, so it has no place to put a per-member
    /// `occurrence` — but it must still not file an ambiguous member under `missing`.
    #[tokio::test]
    async fn multi_heading_selector_separates_ambiguous_from_missing() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n# T\n\n## A\n\nfirst\n\n## B\n\nb\n\n## A\n\nsecond\n",
        )
        .unwrap();

        let v = call(
            &ctx,
            json!({"id": "a", "headings": ["## A", "## Nowhere", "## B"]}),
        )
        .await
        .unwrap();

        // "## B" resolves, "## Nowhere" is absent, "## A" is doubly defined.
        assert_eq!(v["body_meta"]["headings_missing"], json!(["## Nowhere"]));
        let amb = v["body_meta"]["headings_ambiguous"]
            .as_array()
            .expect("an ambiguous member must be reported");
        assert_eq!(amb.len(), 1);
        assert_eq!(amb[0]["heading"], "## A");

        // Mutation control: folding ambiguous into missing lands "## A" in
        // headings_missing, which is the pre-fix behaviour.
        assert!(
            !v["body_meta"]["headings_missing"]
                .to_string()
                .contains("## A"),
            "{:?}",
            v["body_meta"]
        );
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

        let v = call(&ctx, json!({"id": "a", "start_line": 2, "end_line": 4}))
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

        let v = call(&ctx, json!({"id": "a"})).await.unwrap();
        assert_eq!(v["preview"]["shape"], "spec");
        assert!(v.get("body").is_none(), "body absent when not selected");
    }

    #[tokio::test]
    async fn preview_null_when_file_missing() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, _dir) = mk_ctx_with_root(cat);
        // Note: file was never written.

        let v = call(&ctx, json!({"id": "a"})).await.unwrap();
        assert!(v["preview"].is_null());
        assert!(v["body_error"].as_str().is_some());
    }

    #[tokio::test]
    async fn preview_null_when_repo_not_in_roots() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat); // roots: vec![], row abs_path is /test/r/a.md (nonexistent)

        let v = call(&ctx, json!({"id": "a"})).await.unwrap();
        assert!(v["preview"].is_null());
        // New model: file existence is the only criterion. The placeholder
        // path /test/r/a.md doesn't exist on disk, so body_error is set.
        assert!(v["body_error"].as_str().is_some());
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
        let v = call(&ctx, json!({"id": "pl"})).await.unwrap();
        assert_eq!(v["preview"]["shape"], "plan");
        assert_eq!(v["preview"]["tasks"]["total"], 4);
        assert_eq!(v["preview"]["tasks"]["done"], 1);
        let open = v["preview"]["tasks"]["open_next"].as_array().unwrap();
        assert_eq!(open[0], "Alpha task");
        assert!(v.get("body").is_none());

        // Mode 2: full body
        let v = call(&ctx, json!({"id": "pl", "full": true})).await.unwrap();
        assert!(v["body"].as_str().unwrap().contains("Alpha task"));
        assert!(v["body"].as_str().unwrap().contains("Phase 2"));
        assert!(v.get("overflow").is_none());

        // Mode 3: heading-targeted read
        let v = call(&ctx, json!({"id": "pl", "heading": "Phase 1"}))
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
            call(&ctx, json!({"id": "m"})),
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

        let v = call(&ctx, json!({"id": "a", "heading": "Alpha"}))
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

        let v = call(
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
    async fn line_slice_start_line_1_returns_first_visible_content_line() {
        // Regression for the real-world bug: a normally-created file has a blank
        // separator line between the frontmatter's closing `---` and the body
        // content, so start_line=1 must mean the first VISIBLE line ("L1"), not
        // that invisible separator.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\nL1\nL2\nL3\nL4\nL5\n",
        )
        .unwrap();

        let v = call(&ctx, json!({"id": "a", "start_line": 1, "end_line": 1}))
            .await
            .unwrap();
        assert_eq!(v["body"], "L1");
        assert_eq!(v["body_meta"]["line_count"], 1);
    }

    #[tokio::test]
    async fn heading_matches_by_short_id_prefix() {
        // Regression: SI-N style trackers write headings as
        // "## SI-23 — <long descriptive title>". Callers naturally address a
        // section by its short id; that must fuzzy-match like read_markdown/
        // edit_markdown, not require the full heading text verbatim.
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let (ctx, dir) = mk_ctx_with_root(cat);
        fs::write(
            dir.path().join("a.md"),
            "---\nkind: spec\n---\n\n## SI-23 — Even the per-cell count is isolation\n\nbody23\n\n## SI-2 — Weight semantics\n\nbody2\n",
        )
        .unwrap();

        let v = call(&ctx, json!({"id": "a", "heading": "SI-23"}))
            .await
            .unwrap();
        assert_eq!(v["body_meta"]["heading_missing"], Value::Null);
        let body = v["body"].as_str().unwrap();
        assert!(body.contains("body23"));
        assert!(!body.contains("body2\n"));
    }

    #[tokio::test]
    async fn artifact_get_includes_freshness_unknown_by_default() {
        use crate::librarian::catalog::events;
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let ctx = mk_ctx(cat);
        let res = call(&ctx, json!({"id": "a"})).await.unwrap();
        assert_eq!(res["freshness"], "unknown");
        assert!(res["latest_event"].is_null());
        let _ = events::latest_for_artifact; // keep import used
    }

    #[tokio::test]
    async fn artifact_get_freshness_after_reviewed_event() {
        use crate::librarian::catalog::events;
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
        let res = call(&ctx, json!({"id": "a"})).await.unwrap();
        assert_eq!(res["freshness"], "fresh");
        assert_eq!(res["latest_event"]["kind"], "reviewed");
    }

    #[tokio::test]
    async fn get_includes_augmentation_when_present() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
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
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"id": "aug-art"})).await.unwrap();
        let aug = &result["augmentation"];
        assert_eq!(aug["prompt"], "Keep updated");
        assert_eq!(aug["refresh_count"], 5);
        assert_eq!(aug["last_refreshed_at"], "2026-05-01T00:00:00.000Z");
        assert_eq!(aug["params"]["format"], "table");
    }

    /// Helper: context with a real current_project so provenance HEAD resolution works.
    fn mk_ctx_with_project(cat: Catalog, git_root: std::path::PathBuf) -> ToolContext {
        use crate::librarian::current_project::CurrentProject;
        TestToolContextBuilder::new(cat)
            .with_current_project(Arc::new(CurrentProject {
                abs_path: git_root.clone(),
                git_root,
                main_root: None,
                umbrella: None,
            }))
            .build()
    }

    #[tokio::test]
    async fn artifact_get_includes_provenance_keys() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        use crate::librarian::catalog::commits::{self, CommitRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        let root = std::path::PathBuf::from("/test/repo");
        let gr = crate::util::fs::RepoPath::from(&root).into_string();
        commits::upsert_many(
            &cat,
            &[
                CommitRow {
                    hash: "c_old".into(),
                    git_root: gr.clone(),
                    authored_at: Some(1),
                    subject: None,
                    topo_order: Some(0),
                },
                CommitRow {
                    hash: "c_head".into(),
                    git_root: gr.clone(),
                    authored_at: Some(2),
                    subject: None,
                    topo_order: Some(5),
                },
            ],
        )
        .unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "a".into(),
                prompt: "p".into(),
                params: "{}".into(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "0".into(),
                updated_at: "0".into(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: Some("c_old".into()),
            },
        )
        .unwrap();
        let ctx = mk_ctx_with_project(cat, root);
        let res = call(&ctx, json!({"id": "a"})).await.unwrap();
        assert_eq!(res["provenance"]["refreshed_at_commit"], "c_old");
        assert_eq!(res["provenance"]["head_commit"], "c_head");
        assert_eq!(res["provenance"]["commits_behind_head"].as_i64(), Some(5));
    }

    #[tokio::test]
    async fn artifact_get_stale_when_commits_behind_horizon() {
        use crate::librarian::catalog::commits::{self, CommitRow};
        use crate::librarian::catalog::events;
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap(); // file_mtime = 0
        let root = std::path::PathBuf::from("/test/repo");
        let gr = crate::util::fs::RepoPath::from(&root).into_string();
        commits::upsert_many(
            &cat,
            &[
                CommitRow {
                    hash: "c_old".into(),
                    git_root: gr.clone(),
                    authored_at: Some(1),
                    subject: None,
                    topo_order: Some(0),
                },
                CommitRow {
                    hash: "c_head".into(),
                    git_root: gr.clone(),
                    authored_at: Some(2),
                    subject: None,
                    topo_order: Some(100),
                },
            ],
        )
        .unwrap();
        // Reviewed event at c_old, created_at 100 > file_mtime 0, so freshness reaches
        // the commit-distance check (distance 100 > horizon 50 => stale).
        events::insert(
            &cat,
            &events::EventRow {
                id: "ev1".into(),
                artifact_id: "a".into(),
                kind: "reviewed".into(),
                payload: "{}".into(),
                anchor_commit: None,
                head_commit: Some("c_old".into()),
                author: None,
                created_at: 100,
            },
        )
        .unwrap();
        let ctx = mk_ctx_with_project(cat, root);
        let res = call(&ctx, json!({"id": "a"})).await.unwrap();
        assert_eq!(res["provenance"]["commits_behind_head"].as_i64(), Some(100));
        assert_eq!(res["freshness"], "stale");
    }

    #[tokio::test]
    async fn get_includes_entry_collection_in_augmentation() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "a".into(),
                prompt: "p".into(),
                params: "{\"rows\":[]}".into(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "0".into(),
                updated_at: "0".into(),
                render_template: None,
                params_schema: None,
                append_mode: true,
                history_cap: Some(10),
                entry_collection: Some("rows".into()),
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let res = call(&ctx, json!({"id": "a"})).await.unwrap();
        assert_eq!(res["augmentation"]["entry_collection"], "rows");
        assert_eq!(res["augmentation"]["append_mode"], true);
        assert_eq!(res["augmentation"]["history_cap"].as_i64(), Some(10));
    }

    #[tokio::test]
    async fn get_omits_augmentation_when_absent() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("plain-art")).unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"id": "plain-art"})).await.unwrap();
        assert!(result["augmentation"].is_null());
    }
    #[tokio::test]
    async fn worktree_get_of_main_id_with_shadow_returns_overlay_hint() {
        use crate::librarian::tools::worktree::test_support::{seed_main_tracker, wt_ctx};
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c)
        };
        let shadow_id = {
            let mut c = ctx.catalog.lock();
            crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id).unwrap()
        };
        let out = call(&ctx, json!({"id": main_id})).await.unwrap();
        assert_eq!(out["overlay_hint"]["shadow_id"], shadow_id);
        // `get` stays id-literal — it does NOT redirect the returned row itself.
        assert_eq!(out["id"], main_id);
    }

    #[tokio::test]
    async fn worktree_get_of_main_id_without_shadow_omits_overlay_hint() {
        use crate::librarian::tools::worktree::test_support::{seed_main_tracker, wt_ctx};
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c)
        };
        let out = call(&ctx, json!({"id": main_id})).await.unwrap();
        assert!(
            out.get("overlay_hint").is_none(),
            "no shadow forked yet: {out:?}"
        );
    }

    #[tokio::test]
    async fn non_worktree_get_omits_overlay_hint_even_when_link_exists() {
        // A worktree_of link existing in the catalog must never surface an
        // overlay_hint for a plain (non-worktree) session — gated on
        // `main_root.is_some()`, not merely on whether a matching link exists.
        use crate::librarian::catalog::links::{self, LinkRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("main-1")).unwrap();
        artifact::upsert(&cat, &mk_row("shadow-1")).unwrap();
        links::insert(
            &cat,
            &LinkRow {
                src_id: "shadow-1".into(),
                dst_id: "main-1".into(),
                rel: "worktree_of".into(),
                created_at: 0,
            },
        )
        .unwrap();

        let ctx = mk_ctx_with_project(cat, std::path::PathBuf::from("/test/repo"));
        let out = call(&ctx, json!({"id": "main-1"})).await.unwrap();
        assert!(
            out.get("overlay_hint").is_none(),
            "non-worktree session must never emit overlay_hint: {out:?}"
        );
    }

    #[tokio::test]
    async fn include_links_direction_out_hides_incoming() {
        use crate::librarian::catalog::links as lcat;
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
        let result = call(
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
        use crate::librarian::catalog::links as lcat;
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
        let result = call(
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
    async fn include_links_surfaces_entry_cite_edges() {
        use crate::librarian::catalog::entry_cite::{self, EntryCiteRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug='tracker-a' WHERE id='a'", [])
            .unwrap();
        entry_cite::insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: "tracker-a".into(),
                src_local: "W-1".into(),
                dst_ref: "some-target".into(),
                rel: "cites".into(),
                origin: "write".into(),
                created_at: 1,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"id": "a", "include_links": true}))
            .await
            .unwrap();
        assert_eq!(v["entry_links"]["outgoing"].as_array().unwrap().len(), 1);
        assert_eq!(v["entry_links"]["outgoing"][0]["dst_ref"], "some-target");
    }

    #[tokio::test]
    async fn include_links_surfaces_entry_cite_incoming_via_like() {
        use crate::librarian::catalog::entry_cite::{self, EntryCiteRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        artifact::upsert(&cat, &mk_row("b")).unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug='tracker-a' WHERE id='a'", [])
            .unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug='tracker-b' WHERE id='b'", [])
            .unwrap();
        entry_cite::insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: "tracker-b".into(),
                src_local: "W-1".into(),
                dst_ref: "tracker-a:F-3".into(),
                rel: "cites".into(),
                origin: "write".into(),
                created_at: 1,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"id": "a", "include_links": true}))
            .await
            .unwrap();
        assert_eq!(v["entry_links"]["incoming"].as_array().unwrap().len(), 1);
        assert_eq!(v["entry_links"]["incoming"][0]["src"], "tracker-b:W-1");
    }

    #[tokio::test]
    async fn include_links_surfaces_incoming_for_slugless_target() {
        use crate::librarian::catalog::entry_cite::{self, EntryCiteRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("src")).unwrap();
        artifact::upsert(&cat, &mk_row("tgt")).unwrap();
        cat.conn
            .execute("UPDATE artifact SET slug='src-tracker' WHERE id='src'", [])
            .unwrap();
        // 'tgt' is deliberately left slug-less.
        entry_cite::insert_with(
            &cat.conn,
            &EntryCiteRow {
                src_slug: "src-tracker".into(),
                src_local: "F-1".into(),
                dst_ref: "tgt".into(),
                rel: "cites".into(),
                origin: "write".into(),
                created_at: 1,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"id": "tgt", "include_links": true}))
            .await
            .unwrap();
        assert_eq!(v["entry_links"]["incoming"].as_array().unwrap().len(), 1);
        assert_eq!(v["entry_links"]["incoming"][0]["src"], "src-tracker:F-1");
    }

    #[tokio::test]
    async fn invalid_links_direction_errors() {
        use crate::librarian::catalog::Catalog;
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("x")).unwrap();
        let ctx = mk_ctx(cat);
        let err = call(
            &ctx,
            json!({"id": "x", "include_links": true, "links_direction": "sideways"}),
        )
        .await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn entry_filter_returns_matching_rows() {
        use crate::librarian::tools::augment::ArtifactAugment;
        use crate::librarian::tools::Tool;
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("roadmap")).unwrap();
        let ctx = mk_ctx(cat);
        ArtifactAugment
            .call(
                &ctx,
                json!({
                    "id": "roadmap",
                    "prompt": "maintain items",
                    "params": { "items": [
                        {"id": "R-1", "category": "hardware", "status": "open"},
                        {"id": "R-2", "category": "software", "status": "open"},
                        {"id": "R-3", "category": "hardware", "status": "done"}
                    ]},
                    "entry_collection": "items"
                }),
            )
            .await
            .unwrap();

        let out = call(
            &ctx,
            json!({
                "id": "roadmap",
                "entry_filter": {"and": [
                    {"category": {"eq": "hardware"}},
                    {"status": {"eq": "open"}}
                ]}
            }),
        )
        .await
        .unwrap();

        let entries = out["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["id"], "R-1");
        assert_eq!(out["entry_total"], 3);
    }
    #[tokio::test]
    async fn entry_filter_warns_on_unknown_field() {
        use crate::librarian::tools::augment::ArtifactAugment;
        use crate::librarian::tools::Tool;
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("roadmap2")).unwrap();
        let ctx = mk_ctx(cat);
        ArtifactAugment
            .call(
                &ctx,
                json!({
                    "id": "roadmap2",
                    "prompt": "maintain items",
                    "params": { "items": [
                        {"id": "R-1", "category": "hardware", "status": "open"},
                        {"id": "R-2", "category": "software", "status": "done"}
                    ]},
                    "entry_collection": "items"
                }),
            )
            .await
            .unwrap();

        // Typo'd field ("statuss") is present in no entry → silent empty result
        // plus a filter_warnings.unknown_fields entry (F-7).
        let out = call(
            &ctx,
            json!({ "id": "roadmap2", "entry_filter": {"statuss": {"eq": "open"}} }),
        )
        .await
        .unwrap();
        assert_eq!(out["entry_total"], 2);
        assert_eq!(out["entries"].as_array().unwrap().len(), 0);
        let unknown = out["filter_warnings"]["unknown_fields"]
            .as_array()
            .expect("filter_warnings.unknown_fields present for a typo'd field");
        assert_eq!(unknown.len(), 1);
        assert_eq!(unknown[0], "statuss");

        // A genuinely-present field produces NO warning, even on zero matches.
        let out2 = call(
            &ctx,
            json!({ "id": "roadmap2", "entry_filter": {"status": {"eq": "nonexistent"}} }),
        )
        .await
        .unwrap();
        assert_eq!(out2["entries"].as_array().unwrap().len(), 0);
        assert!(
            out2.get("filter_warnings").is_none(),
            "no warning for a known field, even with zero matches"
        );
    }

    #[tokio::test]
    async fn entry_filter_on_non_augmented_is_recoverable_error() {
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("plain")).unwrap();
        let ctx = mk_ctx(cat);
        let err = call(
            &ctx,
            json!({
                "id": "plain",
                "entry_filter": {"category": {"eq": "hardware"}}
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("not augmented")
                || err.to_string().contains("entry_collection"),
            "error message was: {}",
            err
        );
    }
    #[tokio::test]
    async fn entry_filter_missing_collection_key_is_error() {
        use crate::librarian::tools::augment::ArtifactAugment;
        use crate::librarian::tools::Tool;
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("rm2")).unwrap();
        let ctx = mk_ctx(cat);
        ArtifactAugment
            .call(
                &ctx,
                json!({
                    "id": "rm2",
                    "prompt": "p",
                    "params": { "items": [] },
                    "entry_collection": "nonexistent"
                }),
            )
            .await
            .unwrap();
        let err = call(
            &ctx,
            json!({
                "id": "rm2",
                "entry_filter": {"x": {"eq": "y"}}
            }),
        )
        .await
        .unwrap_err();
        assert!(
            err.to_string().contains("no array there") || err.to_string().contains("nonexistent"),
            "got: {err}"
        );
    }
}
