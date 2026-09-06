//! `librarian(action="status")` — is a reindex running, and how far along?
//!
//! The reader half of [`crate::librarian::reindex_progress`]. Publishing
//! progress that nothing can read would be the *"alarm nothing reaches"*
//! defect in `CLAUDE.md` § *Testing Discipline* — a mechanism with a passing
//! suite and no observer — so the two halves ship together.
//!
//! # This action must never take the write lock
//!
//! `LibrarianAdapter::is_write` classifies unlisted actions as writes
//! (`adapter.rs`, `_ => true`), and a write acquires the **cross-process write
//! lock** before dispatch. A running `reindex` holds that lock, so a `status`
//! left to the default would block until the run it is asking about had
//! finished, and then truthfully report that nothing is running. The new
//! instrument would be non-discriminating in exactly the way the bug it fixes
//! describes. `status` is therefore in `is_write`'s unconditional-read set, and
//! `status_is_never_classified_as_a_write` in `adapter.rs` pins it there.

use anyhow::Result;
use serde_json::{json, Value};

use crate::librarian::reindex_progress::{self, ReindexProgress};

use super::ToolContext;

fn render(p: &ReindexProgress, now: i64) -> Value {
    json!({
        "pid": p.pid,
        "scope": p.scope,
        "done": p.done,
        "total": p.total,
        "percent": if p.total == 0 { 0.0 } else {
            (f64::from(p.done) * 100.0 / f64::from(p.total) * 10.0).round() / 10.0
        },
        "started_ms": p.started_ms,
        "elapsed_ms": now - p.started_ms,
        // Reported for display only. It is NOT the liveness test: a suspended
        // laptop inflates it without the run stalling (measured 2026-09-04 over
        // ~1h of standby inside a healthy run), and a slow run is
        // indistinguishable from a dead one by age. `holder_alive` and a moving
        // `done` are the two signals that mean something.
        "heartbeat_age_ms": now - p.heartbeat_ms,
    })
}

pub async fn call(ctx: &ToolContext, _args: Value) -> Result<Value> {
    let snap = {
        let cat = ctx.catalog.lock();
        reindex_progress::read_all(&cat.conn)?
    };
    let now = reindex_progress::now_ms();

    let (live, dead): (Vec<_>, Vec<_>) = snap
        .runs
        .iter()
        .partition(|p| reindex_progress::is_alive(p));

    let running: Vec<Value> = live.iter().map(|p| render(p, now)).collect();
    let stale: Vec<Value> = dead.iter().map(|p| render(p, now)).collect();

    // Negative results name their scope. An empty `running` is the answer a
    // reader is most likely to over-trust, and there are two ways it is true
    // while a reindex really is running — neither of them visible from here.
    // docs/adrs/2026-08-27-negative-results-name-their-scope.md
    let note = if !running.is_empty() {
        format!(
            "{} reindex run(s) publishing progress. Compare `done` across two calls — \
             a moving counter is the only positive evidence of progress; elapsed time is not.",
            running.len()
        )
    } else {
        "No reindex is publishing progress in this catalog. Two things that does NOT \
         establish: a reindex running on a codescout binary built before this row \
         existed publishes nothing at all, and this call only sees the catalog it is \
         connected to. If a run is expected, ask the session that started it."
            .to_string()
    };

    let mut out = json!({
        "running": running,
        "note": note,
    });

    // Only surfaced when non-empty: a permanently-present `"stale": []` trains
    // readers to skip the key, which is where a real stale row would then hide.
    if !stale.is_empty() {
        out["stale"] = json!(stale);
        out["stale_note"] = json!(
            "Rows whose writing process is gone — a run killed mid-loop. Harmless, and \
             the next reindex prunes them; `status` does not, because pruning is a write \
             and an observer of a running reindex must not take the write lock."
        );
    }
    if !snap.unparseable.is_empty() {
        out["unparseable"] = json!(snap.unparseable);
        out["unparseable_note"] = json!(
            "Progress rows this build could not decode. Reported rather than dropped: \
             silently skipping them would make a corrupt catalog read as an idle one."
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;
    use crate::librarian::tools::TestToolContextBuilder;

    fn row(pid: u32, done: u32, total: u32) -> ReindexProgress {
        ReindexProgress {
            pid,
            started_ms: reindex_progress::now_ms() - 5_000,
            heartbeat_ms: reindex_progress::now_ms(),
            done,
            total,
            scope: "/repo".to_string(),
        }
    }

    fn ctx_with(rows: &[ReindexProgress]) -> ToolContext {
        let cat = Catalog::open_in_memory().unwrap();
        for r in rows {
            reindex_progress::publish(&cat.conn, r).unwrap();
        }
        TestToolContextBuilder::new(cat).build()
    }

    /// A pid that is definitely not running — spawned, then reaped.
    ///
    /// Not a magic constant: `0` and anything ≥ `2^31` are rejected by
    /// `platform::unix::addressable_pid` before `kill` is ever called, so a
    /// constant would leave the partition below green against a `status` that
    /// cannot recognise a real dead writer — the only case production produces.
    fn a_reaped_pid() -> u32 {
        #[cfg(windows)]
        let mut child = std::process::Command::new("cmd")
            .args(["/C", "exit"])
            .spawn()
            .unwrap();
        #[cfg(unix)]
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[tokio::test]
    async fn an_idle_catalog_reports_no_running_run_and_names_what_that_misses() {
        let v = call(&ctx_with(&[]), json!({})).await.unwrap();
        assert_eq!(v["running"].as_array().unwrap().len(), 0);
        // The note is the deliverable here, not decoration: an unqualified
        // "nothing running" is the answer that sent a previous session hunting
        // a wedge that did not exist.
        let note = v["note"].as_str().unwrap();
        assert!(note.contains("does NOT establish"), "{note}");
        assert!(note.contains("built before this row existed"), "{note}");
        // Absent, not empty — see the `stale` comment in `call`.
        assert!(v.get("stale").is_none());
        assert!(v.get("unparseable").is_none());
    }

    #[tokio::test]
    async fn a_live_run_is_reported_with_its_counter() {
        let v = call(&ctx_with(&[row(std::process::id(), 25, 100)]), json!({}))
            .await
            .unwrap();
        let runs = v["running"].as_array().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0]["done"], 25);
        assert_eq!(runs[0]["total"], 100);
        assert_eq!(runs[0]["percent"], 25.0);
        assert_eq!(runs[0]["pid"], std::process::id());
    }

    #[tokio::test]
    async fn a_dead_writers_row_is_separated_from_the_live_one() {
        // The discriminating case. Both rows carry a plausible `done`; only
        // liveness tells them apart, which is why the row holds a pid at all.
        let dead = a_reaped_pid();
        let v = call(
            &ctx_with(&[row(std::process::id(), 25, 100), row(dead, 60, 100)]),
            json!({}),
        )
        .await
        .unwrap();

        let running = v["running"].as_array().unwrap();
        assert_eq!(running.len(), 1, "{v:#}");
        assert_eq!(running[0]["pid"], std::process::id());

        let stale = v["stale"].as_array().unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0]["pid"], dead);
        assert!(v["stale_note"].as_str().unwrap().contains("write lock"));
    }

    #[tokio::test]
    async fn a_corrupt_row_is_surfaced_rather_than_read_as_idle() {
        let cat = Catalog::open_in_memory().unwrap();
        crate::librarian::catalog::gc::set_meta(
            &cat.conn,
            &reindex_progress::key_for(4242),
            "{{ not json",
        )
        .unwrap();
        let ctx = TestToolContextBuilder::new(cat).build();

        let v = call(&ctx, json!({})).await.unwrap();
        assert_eq!(v["running"].as_array().unwrap().len(), 0);
        assert_eq!(
            v["unparseable"].as_array().unwrap()[0],
            reindex_progress::key_for(4242)
        );
    }

    #[tokio::test]
    async fn a_zero_total_run_does_not_divide_by_zero() {
        let v = call(&ctx_with(&[row(std::process::id(), 0, 0)]), json!({}))
            .await
            .unwrap();
        assert_eq!(v["running"].as_array().unwrap()[0]["percent"], 0.0);
    }
}
