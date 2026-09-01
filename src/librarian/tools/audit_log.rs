//! `librarian(action="audit_log")` — query the catalog audit trail (T-1/T-2's
//! `catalog_audit` table) and dry-run/apply pruning of old rows.

use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::audit;
use anyhow::Result;
use serde_json::{json, Value};

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;

/// Filter keys `prune_before_ms` does not honor — Task review Finding A
/// (2026-09-01): the prune branch used to read only `prune_before_ms`/`confirm`
/// and silently ignore these, so `audit_log(prune_before_ms=X, actor="unknown")`
/// pruned by TIME ONLY while reading as if scoped to `actor`, deleting every
/// row older than X regardless of actor — a plausible `would_delete` number,
/// then a confirmed wrong-target delete. Presence of any of these now refuses
/// instead of silently discarding them.
const PRUNE_IGNORED_FILTER_KEYS: &[&str] = &["tbl", "row_id", "actor", "op", "since", "until"];

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    // Prune mode: dry-run by default, confirm=true applies (doctor's fix convention).
    if let Some(before) = args.get("prune_before_ms").and_then(Value::as_i64) {
        let present: Vec<&str> = PRUNE_IGNORED_FILTER_KEYS
            .iter()
            .copied()
            .filter(|k| args.get(*k).is_some_and(|v| !v.is_null()))
            .collect();
        if !present.is_empty() {
            return Err(RecoverableError::new(format!(
                "prune_before_ms does not accept filters; it prunes by time only — remove: {}",
                present.join(", ")
            )));
        }
        let confirm = args
            .get("confirm")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let cat = ctx.catalog.lock();
        if !confirm {
            let would: i64 = cat.conn.query_row(
                "SELECT count(*) FROM catalog_audit WHERE at_ms < ?1",
                [before],
                |r| r.get(0),
            )?;
            return Ok(json!({"would_delete": would, "before_ms": before,
                             "unit": "before_ms is epoch-ms UTC",
                             "hint": "pass confirm=true to apply"}));
        }
        let deleted = audit::prune_before(&cat.conn, before)?;
        return Ok(json!({"deleted": deleted, "before_ms": before,
                         "unit": "before_ms is epoch-ms UTC"}));
    }

    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .map(|v| v as usize)
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);
    let f = audit::AuditFilter {
        tbl: args.get("tbl").and_then(Value::as_str).map(String::from),
        row_id: args.get("row_id").and_then(Value::as_str).map(String::from),
        actor: args.get("actor").and_then(Value::as_str).map(String::from),
        op: args.get("op").and_then(Value::as_str).map(String::from),
        since: args.get("since").and_then(Value::as_i64),
        until: args.get("until").and_then(Value::as_i64),
    };
    if let Some(op) = &f.op {
        if !matches!(op.as_str(), "insert" | "update" | "delete") {
            return Err(RecoverableError::new(format!(
                "op '{op}' — expected one of: insert, update, delete"
            )));
        }
    }
    let cat = ctx.catalog.lock();
    let rows = audit::query(&cat.conn, &f, limit)?;
    let total: i64 = cat
        .conn
        .query_row("SELECT count(*) FROM catalog_audit", [], |r| r.get(0))?;
    // Task review Finding B (2026-09-01): `table_total` is the WHOLE table,
    // which sitting next to `count` reads as a denominator ("50 of 4127
    // matched") when it means "50 returned, capped". `filtered_total` is the
    // count under the SAME WHERE as `query()` (via `count_matching`, which
    // shares `filter_where` with it so the two can't drift), and `truncated`
    // names whether the response is a window rather than the whole match —
    // docs/PROGRESSIVE_DISCOVERABILITY.md Pattern 1 (sibling precedent:
    // `events::timeline_for_artifact` returns `truncated`).
    let filtered_total: i64 = audit::count_matching(&cat.conn, &f)?;
    let count = rows.len();
    let truncated = count as i64 == limit as i64 && filtered_total > count as i64;
    let entries: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "seq": r.seq, "at_ms": r.at_ms, "tbl": r.tbl, "op": r.op,
                "row_id": r.row_id, "actor": r.actor, "verb": r.verb,
                "payload": r.payload.as_deref()
                    .and_then(|p| serde_json::from_str::<Value>(p).ok()),
            })
        })
        .collect();
    let mut out = json!({
        "entries": entries,
        "count": count,
        "table_total": total,
        "filtered_total": filtered_total,
        "truncated": truncated,
        "unit": "at_ms/since/until are epoch-ms UTC",
        // Negative-results ADR: the scope block says what was examined, always —
        // and a zero therefore names its window instead of implying "nothing happened".
        "scope": {
            "tbl": f.tbl, "row_id": f.row_id, "actor": f.actor, "op": f.op,
            "since": f.since, "until": f.until, "limit": limit,
        },
        "note": "verb means 'last dispatched verb on the writing connection', not per-statement; actor 'unknown' = a writer that did not identify itself (foreign process or raw sqlite3)."
    });
    if truncated {
        out["hint"] = json!(format!(
            "{filtered_total} rows match but only {count} were returned — narrow with since=<epoch-ms>, tbl=..., or raise limit (max {MAX_LIMIT})"
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx() -> ToolContext {
        TestToolContextBuilder::new(Catalog::open_in_memory().unwrap()).build()
    }

    #[tokio::test]
    async fn zero_results_name_their_scope() {
        let ctx = mk_ctx();
        let out = call(&ctx, json!({"action": "audit_log", "tbl": "commits"}))
            .await
            .unwrap();
        assert_eq!(out["entries"].as_array().unwrap().len(), 0);
        let scope = &out["scope"];
        assert_eq!(
            scope["tbl"], "commits",
            "a zero says what was examined (negative-results ADR)"
        );
        assert!(
            out["unit"].as_str().unwrap().contains("ms"),
            "timestamps label their unit"
        );
    }

    #[tokio::test]
    async fn prune_is_dry_run_without_confirm() {
        let ctx = mk_ctx();
        {
            // one row to prune
            let cat = ctx.catalog.lock();
            cat.conn
                .execute(
                    "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES(1,'artifact','insert','x')",
                    [],
                )
                .unwrap();
        }
        let dry = call(&ctx, json!({"action":"audit_log","prune_before_ms": 10}))
            .await
            .unwrap();
        assert_eq!(dry["would_delete"], 1);
        assert!(dry.get("deleted").is_none());
        let wet = call(
            &ctx,
            json!({"action":"audit_log","prune_before_ms": 10, "confirm": true}),
        )
        .await
        .unwrap();
        assert_eq!(wet["deleted"], 1);
    }

    #[tokio::test]
    async fn unknown_op_is_recoverable() {
        let ctx = mk_ctx();
        let err = call(&ctx, json!({"action": "audit_log", "op": "bogus"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn query_returns_rows_and_table_total() {
        let ctx = mk_ctx();
        {
            let cat = ctx.catalog.lock();
            cat.conn
                .execute(
                    "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES(5,'artifact','insert','a1')",
                    [],
                )
                .unwrap();
        }
        let out = call(&ctx, json!({"action": "audit_log"})).await.unwrap();
        assert_eq!(out["count"], 1);
        assert_eq!(out["table_total"], 1);
        assert_eq!(out["entries"][0]["row_id"], "a1");
        // Task review Finding B: non-truncated response must say so explicitly.
        assert_eq!(out["filtered_total"], 1);
        assert_eq!(out["truncated"], false);
        assert!(out.get("hint").is_none());
    }

    // Task review Finding A (2026-09-01): prune_before_ms prunes by time
    // ONLY — a filter key alongside it must be refused, not silently
    // discarded (silent discard = a plausible would_delete count computed
    // without the filter, then a confirmed wrong-target delete).
    #[tokio::test]
    async fn prune_with_a_filter_key_is_recoverable() {
        let ctx = mk_ctx();
        let err = call(
            &ctx,
            json!({"action": "audit_log", "prune_before_ms": 10, "actor": "unknown"}),
        )
        .await
        .unwrap_err();
        let re = err
            .downcast_ref::<RecoverableError>()
            .expect("must be a RecoverableError, not a silent discard");
        assert!(
            re.to_string().contains("actor"),
            "message must name the offending key(s): {re}"
        );
    }

    #[tokio::test]
    async fn prune_responses_carry_a_unit_label() {
        let ctx = mk_ctx();
        {
            let cat = ctx.catalog.lock();
            cat.conn
                .execute(
                    "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES(1,'artifact','insert','x')",
                    [],
                )
                .unwrap();
        }
        let dry = call(&ctx, json!({"action":"audit_log","prune_before_ms": 10}))
            .await
            .unwrap();
        assert!(dry["unit"].as_str().unwrap().contains("ms"));
        let wet = call(
            &ctx,
            json!({"action":"audit_log","prune_before_ms": 10, "confirm": true}),
        )
        .await
        .unwrap();
        assert!(wet["unit"].as_str().unwrap().contains("ms"));
    }

    #[tokio::test]
    async fn query_names_truncation_and_hints_how_to_narrow() {
        let ctx = mk_ctx();
        {
            let cat = ctx.catalog.lock();
            for i in 0..3 {
                cat.conn
                    .execute(
                        &format!(
                            "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES({i},'artifact','insert','r{i}')"
                        ),
                        [],
                    )
                    .unwrap();
            }
        }
        let out = call(&ctx, json!({"action": "audit_log", "limit": 2}))
            .await
            .unwrap();
        assert_eq!(out["count"], 2);
        assert_eq!(out["filtered_total"], 3);
        assert_eq!(out["truncated"], true);
        let hint = out["hint"]
            .as_str()
            .expect("hint must be present when truncated");
        assert!(hint.contains("since") || hint.contains("limit"));
    }
}
