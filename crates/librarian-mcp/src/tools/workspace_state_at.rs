use anyhow::{anyhow, Result};
use async_trait::async_trait;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

use super::scope::{apply_scope, Scope};
use super::state_at::{replay_state_at, resolve_cutoff_ts};
use super::{Tool, ToolContext};
use crate::catalog::find::{count_matching, find, FindOpts};
use crate::filter::FilterNode;
use crate::freshness::{compute, Freshness, FreshnessInputs};

pub struct WorkspaceStateAt;

/// Maximum artifacts returned per call (exploring-mode cap, matches codescout convention).
const MAX_ROWS: usize = 200;
const FRESHNESS_HORIZON: i64 = crate::freshness::FRESHNESS_HORIZON_DEFAULT;

#[derive(Debug, Deserialize)]
pub struct Args {
    /// Commit hash to use as the time-travel cutoff. Exactly one of `commit`
    /// or `timestamp` must be provided.
    #[serde(default)]
    pub commit: Option<String>,
    /// Unix epoch timestamp (ms) to use as the cutoff. Exactly one of
    /// `commit` or `timestamp` must be provided.
    #[serde(default)]
    pub timestamp: Option<i64>,
    /// Scope for artifact enumeration: `project` (default), `repo`,
    /// `umbrella`, or `all`.
    #[serde(default)]
    pub scope: Option<Scope>,
    /// Filter by artifact kinds (e.g. `["spec", "adr"]`). Omit for all kinds.
    #[serde(default)]
    pub kinds: Option<Vec<String>>,
    /// Include archived / superseded artifacts. Default false.
    #[serde(default)]
    pub include_archived: bool,
    /// Filter output by freshness values at-as-of (e.g. `["stale", "unknown"]`).
    /// Omit for all freshness values.
    #[serde(default)]
    pub freshness_filter: Option<Vec<String>>,
}

#[async_trait]
impl Tool for WorkspaceStateAt {
    fn name(&self) -> &'static str {
        "workspace_state_at"
    }

    fn description(&self) -> &'static str {
        "Time-travel snapshot: return all artifacts in scope as they stood at the given \
         commit/timestamp, with freshness_at_as_of (replay up to cutoff) vs freshness_now \
         (current state). Useful for answering 'what was stale at release X?' questions. \
         Capped at 200 artifacts; hints.more_in_scope reports remainder."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "commit": {
                    "type": "string",
                    "description": "Commit hash as time-travel cutoff. Exactly one of commit or timestamp required."
                },
                "timestamp": {
                    "type": "integer",
                    "description": "Unix epoch timestamp (ms) as cutoff. Exactly one of commit or timestamp required."
                },
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "project = current sub-project (default); repo = whole current root; umbrella = declared umbrella members; all = workspace-wide."
                },
                "kinds": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Filter by artifact kinds (e.g. [\"spec\", \"adr\"]). Omit for all kinds."
                },
                "include_archived": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include archived/superseded artifacts."
                },
                "freshness_filter": {
                    "type": "array",
                    "items": {"type": "string", "enum": ["fresh", "stale", "unknown", "superseded"]},
                    "description": "Only return artifacts whose freshness_at_as_of matches one of these values."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;

        // Exactly one of commit / timestamp must be supplied.
        if a.commit.is_some() == a.timestamp.is_some() {
            return Err(anyhow!("supply exactly one of `commit` or `timestamp`"));
        }

        let cutoff_ts = resolve_cutoff_ts(ctx, a.commit.as_deref(), a.timestamp)?;

        // Build scope + kind/archived filter, then enumerate artifacts.
        let requested_scope = a.scope.unwrap_or_default();
        let (effective_scope, scope_fallback) =
            match (requested_scope, ctx.current_project.is_some()) {
                (Scope::Project | Scope::Repo, false) => (Scope::All, true),
                (s, _) => (s, false),
            };

        let base_filter = build_base_filter(a.kinds.as_deref(), a.include_archived);
        let current = ctx.current_project.as_deref();
        let (scoped_filter, applied) =
            apply_scope(base_filter, effective_scope, &ctx.workspace, current)?;

        // Count total matching artifacts, then fetch up to MAX_ROWS.
        let (total_in_scope, all_rows) = {
            let cat = ctx.catalog.lock();
            let total = count_matching(&cat, scoped_filter.as_ref())?;
            let rows = find(
                &cat,
                &FindOpts {
                    filter: scoped_filter,
                    limit: MAX_ROWS,
                    offset: 0,
                    semantic: None,
                },
            )?;
            (total, rows)
        };

        let more_in_scope = total_in_scope.saturating_sub(MAX_ROWS);
        let rows_to_process = &all_rows[..];

        // For each artifact: replay at-as-of state + compute freshness_now.
        let mut artifacts: Vec<Value> = Vec::with_capacity(rows_to_process.len());
        for art in rows_to_process {
            let (state, freshness_now) = {
                let cat = ctx.catalog.lock();

                // Replay up to cutoff → freshness_at_as_of
                let state = replay_state_at(&cat, art, cutoff_ts)?;

                // freshness_now: latest event (any kind) + latest reviewed event (no cutoff)
                let latest_any: Option<String> = cat
                    .conn
                    .query_row(
                        "SELECT kind FROM events WHERE artifact_id=?1 \
                         ORDER BY created_at DESC, id DESC LIMIT 1",
                        params![art.id],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?;
                let latest_reviewed_now: Option<i64> = cat
                    .conn
                    .query_row(
                        "SELECT MAX(created_at) FROM events \
                         WHERE artifact_id=?1 AND kind='reviewed'",
                        params![art.id],
                        |r| r.get::<_, Option<i64>>(0),
                    )
                    .optional()?
                    .flatten();

                let fn_now = compute(FreshnessInputs {
                    latest_event_kind: latest_any.as_deref(),
                    latest_reviewed_at: latest_reviewed_now,
                    file_updated_at: art.file_mtime,
                    topo_distance_from_head: None,
                    freshness_horizon: FRESHNESS_HORIZON,
                });

                (state, fn_now)
            };

            // Apply optional freshness filter
            if let Some(ref ff) = a.freshness_filter {
                let fa_str = freshness_to_str(state.freshness_at_as_of);
                if !ff.iter().any(|f| f == fa_str) {
                    continue;
                }
            }

            artifacts.push(json!({
                "id": art.id,
                "kind": art.kind,
                "status_at_as_of": state.status,
                "freshness_at_as_of": state.freshness_at_as_of,
                "freshness_now": freshness_now,
                "freshness_changed": state.freshness_at_as_of != freshness_now,
                "latest_event_at_as_of": state.latest_event_summary,
                "supersession_chain": state.supersession_chain,
                "rel_path": art.rel_path,
                "repo": art.repo,
            }));
        }

        let mut hints = json!({
            "scope_fallback": scope_fallback,
        });
        if more_in_scope > 0 {
            hints["more_in_scope"] = json!(more_in_scope);
            hints["hint"] = json!(
                "Result capped at 200. Narrow with `kinds`, `freshness_filter`, or a tighter scope."
            );
        }

        Ok(json!({
            "as_of": cutoff_ts,
            "scope": applied.to_json(),
            "artifacts": artifacts,
            "hints": hints,
        }))
    }
}

fn freshness_to_str(f: Freshness) -> &'static str {
    match f {
        Freshness::Fresh => "fresh",
        Freshness::Stale => "stale",
        Freshness::Unknown => "unknown",
        Freshness::Superseded => "superseded",
    }
}

/// Build a filter for kinds + archived visibility.
/// Returns `None` if no constraints (include_archived=true + no kinds) to let
/// `apply_scope` see all artifacts.
fn build_base_filter(kinds: Option<&[String]>, include_archived: bool) -> Option<FilterNode> {
    let kind_node = kinds.and_then(|ks| {
        if ks.is_empty() {
            None
        } else {
            let values: Vec<Value> = ks.iter().map(|k| Value::String(k.clone())).collect();
            Some(FilterNode::Leaf(
                [("kind".to_string(), json!({"in": values}))]
                    .into_iter()
                    .collect(),
            ))
        }
    });

    let archive_node = if include_archived {
        None
    } else {
        Some(FilterNode::Leaf(
            [(
                "status".to_string(),
                json!({"nin": ["archived", "superseded"]}),
            )]
            .into_iter()
            .collect(),
        ))
    };

    match (kind_node, archive_node) {
        (None, None) => None,
        (Some(k), None) => Some(k),
        (None, Some(a)) => Some(a),
        (Some(k), Some(a)) => Some(FilterNode::And { and: vec![k, a] }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{upsert as art_insert, ArtifactRow};
    use crate::catalog::events::{insert as ev_insert, EventRow};
    use crate::tools::observe::tests::mk_ctx;
    use tempfile::TempDir;
    use ulid::Ulid;

    fn art(id: &str, file_mtime: i64) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: "r".into(),
            rel_path: format!("{id}.md"),
            kind: "spec".into(),
            status: "active".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 0,
            file_mtime,
            file_sha256: "".into(),
            confidence: 1.0,
        }
    }

    fn ev(artifact_id: &str, kind: &str, ts: i64) -> EventRow {
        EventRow {
            id: Ulid::new().to_string(),
            artifact_id: artifact_id.into(),
            kind: kind.into(),
            payload: "{}".into(),
            anchor_commit: None,
            head_commit: None,
            author: None,
            created_at: ts,
        }
    }

    /// freshness_at_as_of vs freshness_now differ when the cutoff is before a review
    /// event, so at-as-of=unknown while now=stale (review exists but file is newer).
    ///
    /// Setup:
    ///   - 3 artifacts with file_mtime=50
    ///   - reviewed event on "a" at ts=100
    ///   - upsert "a" with file_mtime=200 (simulates file change after review)
    ///   - query at cutoff=80 (before the review)
    ///
    /// Expected:
    ///   - a.freshness_at_as_of = unknown  (review at 100 is outside cutoff window)
    ///   - a.freshness_now       = stale   (reviewed_at=100, file_mtime=200 → file newer)
    ///   - b, c: both unknown (no events)
    #[tokio::test]
    async fn freshness_diff_when_stale() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // Seed 3 artifacts with file_mtime=50
        for id in ["a", "b", "c"] {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art(id, 50)).unwrap();
        }

        // Write a reviewed event on "a" at ts=100
        {
            let cat = ctx.catalog.lock();
            ev_insert(&cat, &ev("a", "reviewed", 100)).unwrap();
        }

        // Upsert "a" with newer file_mtime=200 (simulates file change post-review)
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("a", 200)).unwrap();
        }

        // Query at cutoff=80 (before the review at 100)
        let result = WorkspaceStateAt
            .call(&ctx, json!({"timestamp": 80, "scope": "all"}))
            .await
            .unwrap();

        let arts = result["artifacts"].as_array().unwrap();

        let a_entry = arts
            .iter()
            .find(|e| e["id"] == "a")
            .expect("artifact a should be in results");

        // At cutoff=80: no reviewed events in window → unknown
        assert_eq!(
            a_entry["freshness_at_as_of"], "unknown",
            "at cutoff=80 (before review at 100), a should be unknown"
        );
        // freshness_now: reviewed_at=100, file_mtime=200 → file newer than review → stale
        assert_eq!(
            a_entry["freshness_now"], "stale",
            "now: reviewed_at=100 but file_mtime=200, so stale"
        );
        assert_eq!(
            a_entry["freshness_changed"], true,
            "freshness_changed should be true when at_as_of != now"
        );

        // b and c: no reviewed events → both unknown at-as-of and now
        let b_entry = arts.iter().find(|e| e["id"] == "b").unwrap();
        let c_entry = arts.iter().find(|e| e["id"] == "c").unwrap();
        assert_eq!(b_entry["freshness_at_as_of"], "unknown");
        assert_eq!(c_entry["freshness_now"], "unknown");
    }

    /// Seeding 250 artifacts and querying should return ≤ 200 with hints.more_in_scope ≥ 50.
    #[tokio::test]
    async fn cap_returns_hint() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        for i in 0..250_u32 {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art(&format!("art-{i:04}"), 0)).unwrap();
        }

        let result = WorkspaceStateAt
            .call(&ctx, json!({"timestamp": 999999, "scope": "all"}))
            .await
            .unwrap();

        let arts = result["artifacts"].as_array().unwrap();
        assert!(
            arts.len() <= MAX_ROWS,
            "expected ≤ {MAX_ROWS} artifacts, got {}",
            arts.len()
        );
        let more = result["hints"]["more_in_scope"]
            .as_u64()
            .expect("hints.more_in_scope should be present");
        assert!(more >= 50, "expected more_in_scope ≥ 50, got {more}");
    }

    /// Sandwich regression: a new reviewed event after the cutoff must not affect
    /// the at-as-of freshness computation. A fourth query at a wider cutoff proves
    /// the new event IS visible when the window includes it.
    #[tokio::test]
    async fn sandwich_freshness_regression() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());

        // Seed artifact A with file_mtime=10
        {
            let cat = ctx.catalog.lock();
            art_insert(&cat, &art("sa", 10)).unwrap();
        }
        // Write reviewed event at ts=20
        {
            let cat = ctx.catalog.lock();
            ev_insert(&cat, &ev("sa", "reviewed", 20)).unwrap();
        }

        // Step 1: query at ts=25 → reviewed_at=20, file_mtime=10 → fresh
        let r1 = WorkspaceStateAt
            .call(&ctx, json!({"timestamp": 25, "scope": "all"}))
            .await
            .unwrap();
        let entry1 = r1["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "sa")
            .unwrap()
            .clone();
        assert_eq!(
            entry1["freshness_at_as_of"], "fresh",
            "at ts=25 with review@20 and file_mtime=10, should be fresh"
        );
        let latest_id_r1 = entry1["latest_event_at_as_of"]["id"].clone();

        // Step 2: append another reviewed event at ts=100 (after cutoff)
        {
            let cat = ctx.catalog.lock();
            ev_insert(&cat, &ev("sa", "reviewed", 100)).unwrap();
        }

        // Step 3 (stale-assertion sandwich): query at ts=25 again → must still be fresh
        // The new event at ts=100 is OUTSIDE the as-of window (25), so it must not affect result.
        let r2 = WorkspaceStateAt
            .call(&ctx, json!({"timestamp": 25, "scope": "all"}))
            .await
            .unwrap();
        let entry2 = r2["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "sa")
            .unwrap()
            .clone();
        assert_eq!(
            entry2["freshness_at_as_of"], "fresh",
            "after adding event@100, query at ts=25 must still yield fresh (event outside window)"
        );
        // latest_event_at_as_of must still be the review@20 event, not the new review@100.
        assert_eq!(
            entry2["latest_event_at_as_of"]["id"], latest_id_r1,
            "cutoff-bounded query must not see the post-cutoff event"
        );

        // Step 4: query at ts=200 → event@100 is now inside the window → latest_event id differs.
        let r3 = WorkspaceStateAt
            .call(&ctx, json!({"timestamp": 200, "scope": "all"}))
            .await
            .unwrap();
        let entry3 = r3["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "sa")
            .unwrap()
            .clone();
        assert_eq!(
            entry3["freshness_at_as_of"], "fresh",
            "at ts=200 with review@100 and file_mtime=10, should still be fresh"
        );
        // The flip: at ts=200 the at-as-of latest event is review@100, which differs from ts=25.
        assert_ne!(
            entry3["latest_event_at_as_of"]["id"], latest_id_r1,
            "query at ts=200 must see the newer event@100 (proving the at-as-of query can flip)"
        );
    }
}
