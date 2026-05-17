use anyhow::{anyhow, Result};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::ToolContext;
use crate::librarian::catalog::{artifact, events};

/// Intermediate result of replaying events for a single artifact up to a cutoff.
/// Returned by [`replay_state_at`] and consumed by both `artifact_state_at`
/// and `workspace_state_at`.
pub(crate) struct ReplayedState {
    pub(crate) status: Value,
    pub(crate) frontmatter: Map<String, Value>,
    pub(crate) freshness_at_as_of: crate::librarian::freshness::Freshness,
    pub(crate) latest_event_summary: Option<Value>,
    pub(crate) supersession_chain: Vec<String>,
}

/// Resolve a commit hash or raw timestamp to a cutoff `i64` epoch-ms value.
///
/// For commit hashes, supports prefix matching (e.g. an 8-char short SHA
/// resolves against the stored 40-char hash). Bug-tracker #8 / F-5 fix.
/// Ambiguous prefixes (matching ≥2 commits) return an explicit error
/// listing the conflicting SHAs.
pub(crate) fn resolve_cutoff_ts(
    ctx: &ToolContext,
    commit: Option<&str>,
    timestamp: Option<i64>,
) -> Result<i64> {
    if let Some(hash) = commit {
        let cat = ctx.catalog.lock();
        // Find all hashes that start with the supplied prefix. LIMIT 2 keeps
        // the ambiguity check cheap — we only need to know if there's 1 or
        // many.
        let mut stmt = cat
            .conn
            .prepare("SELECT hash, authored_at FROM commits WHERE hash LIKE ?1 || '%' LIMIT 2")?;
        let rows: Vec<(String, Option<i64>)> = stmt
            .query_map(rusqlite::params![hash], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
            })?
            .filter_map(|r| r.ok())
            .collect();
        match rows.len() {
            0 => Err(anyhow!("commit {hash} not indexed; run librarian_reindex")),
            1 => rows[0]
                .1
                .ok_or_else(|| anyhow!("commit {hash} has no authored_at timestamp")),
            _ => Err(anyhow!(
                "commit prefix {hash} is ambiguous (matches at least {} and {}); \
                 use a longer prefix or the full 40-char SHA",
                rows[0].0,
                rows[1].0
            )),
        }
    } else {
        Ok(timestamp.unwrap())
    }
}

/// Replay all events for `art` up to (and including) `cutoff_ts`, returning
/// the reconstructed state and freshness-at-as-of.
///
/// Caller must hold the catalog lock and pass `&Catalog` directly.
pub(crate) fn replay_state_at(
    cat: &crate::librarian::catalog::Catalog,
    art: &crate::librarian::catalog::artifact::ArtifactRow,
    cutoff_ts: i64,
) -> Result<ReplayedState> {
    // Push the cutoff into SQL via `until` so we don't pull post-cutoff
    // events just to drop them in Rust.
    let mut filtered =
        events::timeline_for_artifact(cat, &art.id, None, Some(cutoff_ts), usize::MAX)?;
    // timeline_for_artifact returns newest-first; reverse for chronological replay.
    filtered.reverse();

    // Seed frontmatter from the current artifact row (fallback for un-patched fields).
    let mut frontmatter = crate::librarian::catalog::artifact::build_frontmatter_map(art);

    let mut latest_event_row: Option<events::EventRow> = None;
    let mut latest_reviewed_at: Option<i64> = None;
    let mut latest_kind: Option<String> = None;
    let mut superseded_by: Option<String> = None;

    for ev in filtered {
        latest_kind = Some(ev.kind.clone());
        let p: Value = serde_json::from_str(&ev.payload).unwrap_or(Value::Null);
        match ev.kind.as_str() {
            "status_change" => {
                if let Some(s) = p.get("to").and_then(|v| v.as_str()) {
                    frontmatter.insert("status".into(), Value::String(s.into()));
                }
            }
            "field_patch" => {
                if let (Some(field), Some(to)) = (
                    p.get("field").and_then(|v| v.as_str()),
                    p.get("to").cloned(),
                ) {
                    frontmatter.insert(field.into(), to);
                }
            }
            "reviewed" => {
                latest_reviewed_at = Some(ev.created_at);
            }
            "superseded_by" => {
                superseded_by = p
                    .get("target_artifact_id")
                    .and_then(|v| v.as_str())
                    .map(String::from);
            }
            _ => {}
        }
        latest_event_row = Some(ev);
    }

    let freshness_at_as_of =
        crate::librarian::freshness::compute(crate::librarian::freshness::FreshnessInputs {
            latest_event_kind: latest_kind.as_deref(),
            latest_reviewed_at,
            file_updated_at: art.file_mtime,
            topo_distance_from_head: None,
            freshness_horizon: crate::librarian::freshness::FRESHNESS_HORIZON_DEFAULT,
        });

    let supersession_chain: Vec<String> = superseded_by.into_iter().collect();
    let status_now = frontmatter.get("status").cloned().unwrap_or(Value::Null);

    let latest_event_summary = latest_event_row.as_ref().map(|e| {
        json!({
            "id": e.id,
            "kind": e.kind,
            "created_at": e.created_at,
            "head_commit": e.head_commit,
        })
    });

    Ok(ReplayedState {
        status: status_now,
        frontmatter,
        freshness_at_as_of,
        latest_event_summary,
        supersession_chain,
    })
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct Args {
    pub artifact_id: String,
    #[serde(default)]
    pub commit: Option<String>,
    #[serde(default)]
    pub timestamp: Option<i64>,
}
pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let a: Args = serde_json::from_value(args)?;

    match (&a.commit, &a.timestamp) {
        (Some(_), Some(_)) | (None, None) => {
            return Err(anyhow!("supply exactly one of `commit` or `timestamp`"));
        }
        _ => {}
    }

    let cutoff_ts = resolve_cutoff_ts(ctx, a.commit.as_deref(), a.timestamp)?;

    let (state, freshness_now) = {
        use rusqlite::{params, OptionalExtension};
        let cat = ctx.catalog.lock();
        let art = artifact::get(&cat, &a.artifact_id)?
            .ok_or_else(|| anyhow!("artifact not found: {}", a.artifact_id))?;
        let state = replay_state_at(&cat, &art, cutoff_ts)?;

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

        let fn_now =
            crate::librarian::freshness::compute(crate::librarian::freshness::FreshnessInputs {
                latest_event_kind: latest_any.as_deref(),
                latest_reviewed_at: latest_reviewed_now,
                file_updated_at: art.file_mtime,
                topo_distance_from_head: None,
                freshness_horizon: crate::librarian::freshness::FRESHNESS_HORIZON_DEFAULT,
            });

        (state, fn_now)
    };

    Ok(json!({
        "as_of": cutoff_ts,
        "status_at_as_of": state.status,
        "frontmatter": Value::Object(state.frontmatter),
        "freshness_at_as_of": state.freshness_at_as_of,
        "freshness_now": freshness_now,
        "freshness_changed": state.freshness_at_as_of != freshness_now,
        "latest_event_at_as_of": state.latest_event_summary,
        "supersession_chain": state.supersession_chain,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{upsert as art_insert, ArtifactRow};
    use crate::librarian::catalog::commits::{upsert_many, CommitRow};
    use crate::librarian::catalog::events::{insert as ev_insert, EventRow};
    use crate::librarian::tools::event_create::tests::mk_ctx;
    use tempfile::TempDir;

    fn art(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            abs_path: std::path::PathBuf::from(format!("/test/r/{id}.md")),
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
            file_mtime: 0,
            file_sha256: "".into(),
            confidence: 1.0,
        }
    }

    fn seed(ctx: &ToolContext, id: &str) {
        let cat = ctx.catalog.lock();
        art_insert(&cat, &art(id)).unwrap();
    }

    fn ev(artifact_id: &str, kind: &str, payload: Value, ts: i64) -> EventRow {
        EventRow {
            id: ulid::Ulid::new().to_string(),
            artifact_id: artifact_id.into(),
            kind: kind.into(),
            payload: payload.to_string(),
            anchor_commit: None,
            head_commit: None,
            author: None,
            created_at: ts,
        }
    }

    #[tokio::test]
    async fn replay_status_change() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        seed(&ctx, "a1");

        // Write a status_change event at ts=10
        {
            let cat = ctx.catalog.lock();
            ev_insert(
                &cat,
                &ev(
                    "a1",
                    "status_change",
                    json!({"from": "active", "to": "done"}),
                    10,
                ),
            )
            .unwrap();
        }

        // Query at ts=20 — event is visible → status should be "done"
        let result = call(&ctx, json!({"artifact_id": "a1", "timestamp": 20}))
            .await
            .unwrap();
        assert_eq!(
            result["status_at_as_of"], "done",
            "at ts=20 status should be done"
        );

        // Query at ts=5 — event not yet seen → status should be "active" (seeded)
        let result = call(&ctx, json!({"artifact_id": "a1", "timestamp": 5}))
            .await
            .unwrap();
        assert_eq!(
            result["status_at_as_of"], "active",
            "at ts=5 status should be active"
        );
    }

    #[tokio::test]
    async fn superseded_by_listed_in_chain() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        seed(&ctx, "b1");

        {
            let cat = ctx.catalog.lock();
            ev_insert(
                &cat,
                &ev(
                    "b1",
                    "superseded_by",
                    json!({"target_artifact_id": "other"}),
                    5,
                ),
            )
            .unwrap();
        }

        let result = call(&ctx, json!({"artifact_id": "b1", "timestamp": 10}))
            .await
            .unwrap();
        let chain = result["supersession_chain"].as_array().unwrap();
        assert!(!chain.is_empty(), "supersession_chain should not be empty");
        assert_eq!(chain[0], "other");
    }

    #[tokio::test]
    async fn requires_exactly_one_of_commit_timestamp() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        seed(&ctx, "c1");

        // Both supplied → error
        let err = call(
            &ctx,
            json!({"artifact_id": "c1", "commit": "abc", "timestamp": 5}),
        )
        .await;
        assert!(err.is_err(), "both commit+timestamp should error");

        // Neither supplied → error
        let err = call(&ctx, json!({"artifact_id": "c1"})).await;
        assert!(err.is_err(), "neither commit nor timestamp should error");
    }

    #[tokio::test]
    async fn commit_lookup_uses_authored_at() {
        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx(tmp.path().to_path_buf());
        seed(&ctx, "d1");

        // Insert commit row: hash="abc", authored_at=15
        {
            let cat = ctx.catalog.lock();
            upsert_many(
                &cat,
                &[CommitRow {
                    hash: "abc".into(),
                    git_root: "/r".into(),
                    authored_at: Some(15),
                    subject: None,
                    topo_order: None,
                }],
            )
            .unwrap();

            // Event at ts=10 (before authored_at=15) — should be visible
            ev_insert(
                &cat,
                &ev(
                    "d1",
                    "status_change",
                    json!({"from": "active", "to": "done"}),
                    10,
                ),
            )
            .unwrap();

            // Event at ts=20 (after authored_at=15) — should NOT be visible
            ev_insert(
                &cat,
                &ev(
                    "d1",
                    "status_change",
                    json!({"from": "done", "to": "archived"}),
                    20,
                ),
            )
            .unwrap();
        }

        // Query at commit "abc" → authored_at=15 → only first event visible
        let result = call(&ctx, json!({"artifact_id": "d1", "commit": "abc"}))
            .await
            .unwrap();
        assert_eq!(
            result["status_at_as_of"], "done",
            "at commit abc (authored_at=15) status should be done, not archived"
        );
    }
}
