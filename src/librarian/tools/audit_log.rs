//! `librarian(action="audit_log")` — query the catalog audit trail (T-1/T-2's
//! `catalog_audit` table) and dry-run/apply pruning of old rows.

use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::audit;
use crate::librarian::catalog::audit::{host, shard};
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

// Ruling 1 (Task 4 brief): `ctx.project_root()` does not exist — `project_root()`
// lives on the *agent* and is async. Duplicated from gather.rs:294 rather than
// made `pub` mid-run.
//
// Task 6: see reindex.rs's copy of this helper for why `abs_path` and the
// `ctx.workspace.roots.first()` fallback are both gone — same reasoning
// applies verbatim to the export destination this feeds.
fn project_root(ctx: &ToolContext) -> Option<std::path::PathBuf> {
    ctx.current_project
        .as_ref()
        .map(|cp| cp.main_root.clone().unwrap_or_else(|| cp.git_root.clone()))
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    // Export mode: append every unexported row to this host's committed shard
    // and advance the watermark. Not combinable with filters, nor with
    // prune_before_ms — neither can silently absorb the other's args: a
    // filter alongside export would suggest a scoped export that never
    // happened (IC-15), and prune_before_ms alongside export would run
    // export and hand the caller a different operation's result than the
    // destructive one they asked for (Task review Finding 2, 2026-09-01).
    if args.get("export").and_then(Value::as_bool).unwrap_or(false) {
        let present: Vec<&str> = PRUNE_IGNORED_FILTER_KEYS
            .iter()
            .copied()
            .chain(std::iter::once("prune_before_ms"))
            .filter(|k| args.get(*k).is_some_and(|v| !v.is_null()))
            .collect();
        if !present.is_empty() {
            return Err(RecoverableError::new(format!(
                "export does not accept filters or prune_before_ms; it exports every unexported row — remove: {}",
                present.join(", ")
            )));
        }
        let root = project_root(ctx).ok_or_else(|| {
            RecoverableError::new(
                "export requires a resolved current project with a git root — no current project is set"
                    .to_string(),
            )
        })?;
        let cat = ctx.catalog.lock();
        let r = shard::export(&cat.conn, &root)?;
        return Ok(json!({
            "exported": r.exported,
            "skipped_commits": r.skipped_commits,
            "skipped_churn": r.skipped_churn,
            "unattributed": r.unattributed,
            "files": r.files,
            "through_seq": r.through_seq,
            "dir": format!("{}/", host::AUDIT_DIR),
            "note": "a committed shard is a REPLICA of the local trail, fresh only as of through_seq — the in-transaction guarantee exists on the local database alone. Commit the files to share them.",
        }));
    }

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
    let local_filtered_total: i64 = audit::count_matching(&cat.conn, &f)?;
    let self_host = host::resolve_host_id(&cat.conn)?;

    // Merge in committed shards from other hosts (Task 4, IC-13): a count that
    // reflects only the local table is a wrong number the moment a second host
    // has ever exported. `self_host`'s own shard is skipped by `read_shards`
    // itself, since those rows already live in `rows` above.
    let shards = project_root(ctx)
        .map(|root| shard::read_shards(&root, &f, &self_host))
        .transpose()?
        .unwrap_or_default();

    let filtered_total: i64 = local_filtered_total + shards.rows.len() as i64;

    let mut merged: Vec<(i64, i64, Value)> = rows
        .iter()
        .map(|r| {
            (
                r.at_ms,
                r.seq,
                json!({
                    "host": self_host, "seq": r.seq, "at_ms": r.at_ms, "tbl": r.tbl, "op": r.op,
                    "row_id": r.row_id, "actor": r.actor, "verb": r.verb,
                    "payload": r.payload.as_deref()
                        .and_then(|p| serde_json::from_str::<Value>(p).ok()),
                }),
            )
        })
        .chain(shards.rows.iter().map(|l| {
            (
                l.at_ms,
                l.seq,
                json!({
                    "host": l.host, "seq": l.seq, "at_ms": l.at_ms, "tbl": l.tbl, "op": l.op,
                    "row_id": l.row_id, "actor": l.actor, "verb": l.verb,
                    "payload": l.payload,
                }),
            )
        }))
        .collect();
    // Cross-host ordering uses at_ms (wall-clock), never seq (a per-host
    // autoincrement not comparable across hosts).
    merged.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    // Task review Finding (2026-09-01): merge THEN truncate, never cap each
    // source independently first — capping local and shard rows separately
    // before merging can starve a bursty exporter of any slot in the window
    // even though its rows are the most recent overall.
    merged.truncate(limit);
    let entries: Vec<Value> = merged.into_iter().map(|(_, _, v)| v).collect();
    let count = entries.len();
    // truncated = filtered_total > count: the two are computed over the same
    // merged population (local + shard rows), so this holds regardless of
    // which source produced the overflow — no separate "did shards truncate"
    // conjunct is needed or correct.
    let truncated = filtered_total > count as i64;
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
        "shards": {
            "self_host": self_host,
            "files_read": shards.files_read,
            "files_skipped_by_window": shards.files_skipped_by_window,
            "unreadable_files": shards.unreadable_files,
            "malformed_lines": shards.malformed,
            "coverage": {
                "unit": "per host: [min seq, max seq] found, scoped to the shard files opened for THIS query's window — a host absent here had no shard file survive the window filter",
                "by_host": shards.hosts,
            },
            "note": "rows above whose host != self_host are a REPLICA of that host's local trail, fresh only as of that host's last export — see the export mode's own note.",
        },
        "note": "verb means 'last dispatched verb on the writing connection', not per-statement; actor 'unknown' = a writer that did not identify itself (foreign process or raw sqlite3)."
    });
    let mut shard_warnings: Vec<String> = Vec::new();
    if shards.malformed > 0 {
        shard_warnings.push(format!(
            "{} malformed shard line(s) were skipped, not counted in filtered_total",
            shards.malformed
        ));
    }
    if shards.unreadable_files > 0 {
        // Task review Finding 1 (2026-09-01): shard.rs documents and asserts
        // that a dropped file "must not vanish silently" — but this was the
        // only consumer, and it omitted the counter entirely, so a permission
        // error or a directory-shaped shard name shortened filtered_total
        // with nothing saying the number was short.
        shard_warnings.push(format!(
            "{} shard file(s) could not be read (permissions, a race, or a broken symlink) and are not counted in filtered_total",
            shards.unreadable_files
        ));
    }
    if !shard_warnings.is_empty() {
        out["shards_warning"] = json!(shard_warnings.join("; "));
    }
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
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::tools::TestToolContextBuilder;

    fn mk_ctx() -> (ToolContext, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = TestToolContextBuilder::new(Catalog::open_in_memory().unwrap())
            .with_current_project(std::sync::Arc::new(CurrentProject {
                abs_path: tmp.path().to_path_buf(),
                git_root: tmp.path().to_path_buf(),
                main_root: None,
                umbrella: None,
            }))
            .build();
        (ctx, tmp)
    }

    /// Writes `n` lines for a foreign host directly into a committed shard
    /// file, bypassing `shard::export` — this fixture is testing the READER,
    /// so it must not depend on the writer to build its input.
    fn write_foreign_shard(tmp_root: &std::path::Path, n: i64) {
        let dir = tmp_root.join(host::AUDIT_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let at_ms = 1_788_220_800_000; // 2026-09-01T00:00:00Z
        let name = host::shard_file_name("otherbox-99ffee", at_ms);
        let mut body = String::new();
        for i in 0..n {
            let line = shard::ShardLine {
                host: "otherbox-99ffee".to_string(),
                seq: i + 1,
                at_ms: at_ms + i,
                tbl: "artifact".to_string(),
                op: "insert".to_string(),
                row_id: format!("foreign-{i}"),
                actor: "unknown".to_string(),
                verb: None,
                payload: None,
            };
            body.push_str(&serde_json::to_string(&line).unwrap());
            body.push('\n');
        }
        std::fs::write(dir.join(name), body).unwrap();
    }

    #[tokio::test]
    async fn zero_results_name_their_scope() {
        let (ctx, _tmp) = mk_ctx();
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
        let (ctx, _tmp) = mk_ctx();
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
        let (ctx, _tmp) = mk_ctx();
        let err = call(&ctx, json!({"action": "audit_log", "op": "bogus"}))
            .await
            .unwrap_err();
        assert!(err.downcast_ref::<RecoverableError>().is_some());
    }

    #[tokio::test]
    async fn query_returns_rows_and_table_total() {
        let (ctx, _tmp) = mk_ctx();
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
        let (ctx, _tmp) = mk_ctx();
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
        let (ctx, _tmp) = mk_ctx();
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
        let (ctx, _tmp) = mk_ctx();
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

    #[tokio::test]
    async fn export_mode_reports_what_it_wrote() {
        let (ctx, tmp) = mk_ctx();
        {
            let cat = ctx.catalog.lock();
            // Task 6: an `artifact`/`insert` row is attributed by joining
            // `row_id` against a live `artifact.abs_path` — an artifact must
            // actually exist under this fixture's repo root (`tmp.path()`,
            // its `git_root`) or the row comes out unattributed instead of
            // exported, same as every other pre-existing single-repo test in
            // this suite.
            cat.conn
                .execute(
                    "INSERT INTO artifact \
                     (id, abs_path, kind, status, created_at, updated_at, file_mtime, file_sha256) \
                     VALUES ('x', ?1, 'spec', 'active', 0, 0, 0, '')",
                    [tmp.path().join("x.md").to_string_lossy().to_string()],
                )
                .unwrap();
            cat.conn
                    .execute(
                        "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES(1,'artifact','insert','x')",
                        [],
                    )
                    .unwrap();
        }
        let out = call(&ctx, json!({"action": "audit_log", "export": true}))
            .await
            .unwrap();
        // 2, not 1: seeding the artifact via a raw `INSERT INTO artifact`
        // fires the table's own audit trigger, producing a second
        // `artifact`/`insert` row for "x" in addition to the manually
        // inserted one above — both are now attributable and exported.
        assert_eq!(out["exported"], 2);
        assert!(out["note"].as_str().unwrap().contains("REPLICA"), "{out}");
        let dir = tmp.path().join(host::AUDIT_DIR);
        let mut wrote_a_file = false;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            wrote_a_file = entries.count() > 0;
        }
        assert!(wrote_a_file, "export must actually write a shard file");
    }

    #[tokio::test]
    async fn a_merged_query_counts_shard_rows_in_its_totals() {
        let (ctx, tmp) = mk_ctx();
        write_foreign_shard(tmp.path(), 3);
        let out = call(&ctx, json!({"action": "audit_log"})).await.unwrap();
        // IC-13: a count that reflects only the local table is a wrong number
        // once a second host has ever exported.
        assert_eq!(out["filtered_total"], 3, "{out}");
        assert_eq!(out["count"], 3, "{out}");
        assert_eq!(out["shards"]["files_read"], 1, "{out}");
    }

    #[tokio::test]
    async fn a_local_row_is_labelled_with_this_host_not_left_blank() {
        let (ctx, tmp) = mk_ctx();
        write_foreign_shard(tmp.path(), 1);
        {
            let cat = ctx.catalog.lock();
            cat.conn
                    .execute(
                        "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES(2,'artifact','insert','local-1')",
                        [],
                    )
                    .unwrap();
        }
        let out = call(&ctx, json!({"action": "audit_log"})).await.unwrap();
        let entries = out["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2, "{out}");
        for e in entries {
            assert!(
                e["host"].as_str().is_some_and(|h| !h.is_empty()),
                "every entry must name its origin host, local included: {e}"
            );
        }
        let local = entries
            .iter()
            .find(|e| e["row_id"] == "local-1")
            .expect("local row present");
        assert_ne!(
            local["host"], "otherbox-99ffee",
            "the local row's host must be THIS host, not the foreign one: {local}"
        );
        // Task review Finding 8: `!= "otherbox-99ffee"` alone survives a
        // mutation that labels local rows "local" instead of self_host —
        // pin it to the value the response itself claims is self_host.
        assert_eq!(
            local["host"], out["shards"]["self_host"],
            "the local row's host must equal shards.self_host exactly: {local}"
        );
    }

    #[tokio::test]
    async fn a_merged_query_with_local_and_shard_rows_sums_both_to_filtered_total() {
        let (ctx, tmp) = mk_ctx();
        write_foreign_shard(tmp.path(), 3);
        {
            let cat = ctx.catalog.lock();
            for i in 0..2 {
                cat.conn
                        .execute(
                            &format!(
                                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES({},'artifact','insert','local-{i}')",
                                100 + i
                            ),
                            [],
                        )
                        .unwrap();
            }
        }
        let out = call(&ctx, json!({"action": "audit_log"})).await.unwrap();
        // Task review Finding 6: no existing fixture held BOTH local and
        // shard rows — this is the one property the whole merge rests on,
        // and it is killed by neither the local-only nor the shard-only test.
        assert_eq!(out["filtered_total"], 5, "{out}");
        assert_eq!(out["count"], 5, "{out}");
    }

    #[tokio::test]
    async fn truncated_reflects_the_merged_population_not_local_alone() {
        let (ctx, tmp) = mk_ctx();
        write_foreign_shard(tmp.path(), 3);
        let out = call(&ctx, json!({"action": "audit_log", "limit": 2}))
            .await
            .unwrap();
        // Task review Finding 7: with zero local rows, a `truncated`
        // computed from the local count alone reads `false` (0 > 2) even
        // though 3 shard rows overflow a limit of 2 — only the MERGED
        // filtered_total catches this.
        assert_eq!(out["filtered_total"], 3, "{out}");
        assert_eq!(out["count"], 2, "{out}");
        assert_eq!(out["truncated"], true, "{out}");
    }

    #[tokio::test]
    async fn export_refuses_to_combine_with_prune_before_ms() {
        let (ctx, _tmp) = mk_ctx();
        let err = call(
            &ctx,
            json!({"action": "audit_log", "export": true, "prune_before_ms": 10}),
        )
        .await
        .unwrap_err();
        let re = err
            .downcast_ref::<RecoverableError>()
            .expect("must be a RecoverableError, not a silent discard of the destructive verb");
        assert!(re.to_string().contains("prune_before_ms"), "{re}");
    }

    #[tokio::test]
    async fn an_unreadable_shard_file_is_counted_not_silently_dropped() {
        let (ctx, tmp) = mk_ctx();
        let dir = tmp.path().join(host::AUDIT_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        let at_ms = 1_788_220_800_000;
        let name = host::shard_file_name("otherbox-99ffee", at_ms);
        // A directory where a shard FILE should be: the name parses as a
        // valid shard, but std::fs::read_to_string on a directory errors —
        // this exercises the unreadable_files path without relying on real
        // permission bits, which are not portable across CI runners.
        std::fs::create_dir_all(dir.join(&name)).unwrap();
        let out = call(&ctx, json!({"action": "audit_log"})).await.unwrap();
        assert_eq!(out["shards"]["unreadable_files"], 1, "{out}");
        let warning = out["shards_warning"]
            .as_str()
            .expect("an unreadable file must not vanish silently — Task review Finding 1");
        assert!(warning.contains("could not be read"), "{warning}");
    }

    #[tokio::test]
    async fn merged_query_names_shard_rows_as_a_replica_and_labels_coverage() {
        let (ctx, tmp) = mk_ctx();
        write_foreign_shard(tmp.path(), 2);
        let out = call(&ctx, json!({"action": "audit_log"})).await.unwrap();
        assert!(
            out["shards"]["note"].as_str().unwrap().contains("REPLICA"),
            "{out}"
        );
        assert!(
            out["shards"]["coverage"]["unit"]
                .as_str()
                .unwrap()
                .contains("seq"),
            "{out}"
        );
        assert_eq!(
            out["shards"]["coverage"]["by_host"]["otherbox-99ffee"],
            json!([1, 2]),
            "{out}"
        );
    }

    #[tokio::test]
    async fn export_refuses_to_combine_with_a_query_filter() {
        let (ctx, _tmp) = mk_ctx();
        let err = call(
            &ctx,
            json!({"action": "audit_log", "export": true, "tbl": "artifact"}),
        )
        .await
        .unwrap_err();
        let re = err
            .downcast_ref::<RecoverableError>()
            .expect("must be a RecoverableError, not a silent discard");
        assert!(re.to_string().contains("tbl"), "{re}");
    }
}
