//! `librarian(action="audit_log")` — query the catalog audit trail (T-1/T-2's
//! `catalog_audit` table) and dry-run/apply pruning of old rows.

use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::audit;
use anyhow::Result;
use serde_json::{json, Value};

const DEFAULT_LIMIT: usize = 50;
const MAX_LIMIT: usize = 500;

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    // Prune mode: dry-run by default, confirm=true applies (doctor's fix convention).
    if let Some(before) = args.get("prune_before_ms").and_then(Value::as_i64) {
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
                             "hint": "pass confirm=true to apply"}));
        }
        let deleted = audit::prune_before(&cat.conn, before)?;
        return Ok(json!({"deleted": deleted, "before_ms": before}));
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
    // Negative-results ADR: the scope block says what was examined, always —
    // and a zero therefore names its window instead of implying "nothing happened".
    Ok(json!({
        "entries": entries,
        "count": entries.len(),
        "table_total": total,
        "unit": "at_ms/since/until are epoch-ms UTC",
        "scope": {
            "tbl": f.tbl, "row_id": f.row_id, "actor": f.actor, "op": f.op,
            "since": f.since, "until": f.until, "limit": limit,
        },
        "note": "verb means 'last dispatched verb on the writing connection', not per-statement; actor 'unknown' = a writer that did not identify itself (foreign process or raw sqlite3)."
    }))
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
    }
}
