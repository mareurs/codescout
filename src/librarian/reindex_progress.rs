//! Durable, cross-process progress for a running `librarian(action="reindex")`.
//!
//! # Why this exists
//!
//! A re-embed emits nothing for many minutes, and **every** side effect an
//! observer can reach is non-discriminating:
//!
//! | proxy | why it is flat |
//! |---|---|
//! | vector-store point count | `upsert` is idempotent on `chunk_id` (`artifact_store.rs:137`) — it overwrites in place |
//! | `artifact_chunk` row count | no new rows for unchanged content |
//! | `artifact.embedded_sha256` | stamped only **after** the loop (`tools/reindex.rs`) |
//! | `catalog.db` mtime | same reason |
//! | codescout's own CPU | the compute is in the embedder process; this one is IO-bound on HTTP |
//!
//! The loop's iteration counter is the only monotone quantity in the system,
//! and until this module it lived in a local `u32` published solely through
//! `ctx.progress` — which is `None` whenever the client sent no
//! `_meta.progressToken`, as Claude Code does.
//!
//! Measured 2026-09-04: a 28,379-vector run was diagnosed **"wedged"** from six
//! separate proxies, every one of them consistent with a healthy run. Second
//! instance of that inference; `bug-fix-session-log:F-109` is the first, made
//! from outside the process. Six proxies for one hidden variable is one blind
//! spot counted six times, which at the point of use is indistinguishable from
//! corroboration.
//!
//! - `docs/issues/archive/2026-09-03-a-long-reindex-cannot-be-distinguished-from-a-wedged-one.md`
//! - `docs/trackers/reconnaissance-patterns.md` § `R-182`
//!
//! # The discriminator is `done`, never elapsed time
//!
//! It is tempting to age these rows out — "older than N minutes, therefore
//! stale". That is wrong in both directions and the failure is silent:
//!
//! - A **suspended laptop** inflates wall-clock elapsed without the run
//!   stalling. The 2026-09-04 run above spanned roughly an hour of standby;
//!   an age-based reader would have called a healthy run dead.
//! - A genuinely **slow** run is indistinguishable from a dead one by age, so
//!   the threshold can only ever be a guess that is wrong for some corpus.
//!
//! `done` moving is positive evidence of progress; the holder's PID being alive
//! is positive evidence of a writer. Neither is a threshold. Read those two.

use anyhow::Result;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::librarian::catalog::gc;

/// Key prefix in `catalog_meta`. **One row per running process**, never a
/// single well-known key.
///
/// This machine runs ~15 codescout processes across three config profiles
/// sharing one catalog file, and two reindexes in different repos can overlap.
/// Under a single key the second starter silently overwrites the first's row,
/// so an observer sees exactly one run and no way to tell it is the wrong one —
/// the same substitute-a-plausible-value-for-an-error shape this whole module
/// exists to remove.
pub const KEY_PREFIX: &str = "reindex_in_progress:";

/// How often the running loop refreshes its row.
///
/// A `set_meta` per item would be one extra write transaction per embed —
/// 28,379 of them on this repo's corpus — all competing with the upserts for
/// the same 5000 ms `busy_timeout` budget. That would risk *causing* the
/// contention the bug file could only speculate about. Throttled by **time**
/// rather than by item count because per-item cost varies by orders of
/// magnitude between a one-line memory and a 25 KB tracker section.
pub const HEARTBEAT_INTERVAL_MS: i64 = 2_000;

/// Progress published by a live reindex, as stored in `catalog_meta`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReindexProgress {
    /// The writing process. A reader resolves liveness from this; it is the
    /// only field that can distinguish "still working" from "died mid-run".
    pub pid: u32,
    pub started_ms: i64,
    /// Refreshed every [`HEARTBEAT_INTERVAL_MS`]. Useful for *display*; it is
    /// deliberately **not** the staleness test — see the module docs.
    pub heartbeat_ms: i64,
    /// Items **processed**, not items embedded. A run failing every embed is
    /// still making progress through the queue, and a counter that stalled on
    /// failure would report a working-but-erroring run as wedged — the exact
    /// confusion this exists to remove. Mirrors the same decision at the
    /// `ctx.progress` call site in `tools/reindex.rs`.
    pub done: u32,
    pub total: u32,
    /// Absolute root being reindexed, so an observer sharing the catalog can
    /// tell *whose* run this is.
    pub scope: String,
}

/// What [`read_all`] found.
///
/// `unparseable` is carried rather than dropped: a row this build cannot read
/// is a fact about the catalog an observer needs, and silently returning fewer
/// rows would make a corrupt row look like an idle system — precisely the
/// plausible-answer-instead-of-an-error failure this module addresses.
#[derive(Debug, Default)]
pub struct Snapshot {
    pub runs: Vec<ReindexProgress>,
    pub unparseable: Vec<String>,
}

pub fn key_for(pid: u32) -> String {
    format!("{KEY_PREFIX}{pid}")
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// True when `now` has moved at least one heartbeat interval past
/// `last_published_ms`. Pure, so the cadence is testable without a clock.
pub fn should_publish(last_published_ms: i64, now: i64) -> bool {
    now - last_published_ms >= HEARTBEAT_INTERVAL_MS
}

/// Write (or overwrite) this process's progress row.
pub fn publish(conn: &Connection, p: &ReindexProgress) -> Result<()> {
    gc::set_meta(conn, &key_for(p.pid), &serde_json::to_string(p)?)
}

/// Remove one process's progress row. Idempotent — clearing an absent row is
/// success, which is what makes it safe on every exit path.
pub fn clear(conn: &Connection, pid: u32) -> Result<()> {
    conn.execute(
        "DELETE FROM catalog_meta WHERE key = ?1",
        rusqlite::params![key_for(pid)],
    )?;
    Ok(())
}

/// Every progress row currently in the catalog, live or not.
///
/// Uses the shared LIKE-escape helper rather than inlining the idiom: the
/// prefix contains `_`, a LIKE wildcard, so an unescaped pattern would also
/// match keys this module does not own.
/// (`like_escape_idiom_is_not_inlined_outside_helper` pins the rule.)
pub fn read_all(conn: &Connection) -> Result<Snapshot> {
    let like = format!(
        "{}%",
        crate::librarian::util::escape_like_pattern(KEY_PREFIX)
    );
    let mut stmt =
        conn.prepare("SELECT key, value FROM catalog_meta WHERE key LIKE ?1 ESCAPE '\\'")?;
    let rows: Vec<(String, String)> = stmt
        .query_map([like], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;

    let mut snap = Snapshot::default();
    for (key, value) in rows {
        match serde_json::from_str::<ReindexProgress>(&value) {
            Ok(p) => snap.runs.push(p),
            Err(_) => snap.unparseable.push(key),
        }
    }
    snap.runs.sort_by_key(|p| p.started_ms);
    Ok(snap)
}

/// Is the process that wrote this row still running?
///
/// `kill(pid, 0)`-based (`crate::platform::process_alive`). Two limits, stated
/// because a reader who does not know them will over-trust the answer:
/// **PID reuse** can make a dead writer's recycled number read as alive, and a
/// process owned by another user reads as dead (`EPERM`). Both are acceptable
/// here — every codescout process on a machine runs as the same user, and a
/// reused PID is far less likely than the wedge-vs-working ambiguity this
/// replaces.
pub fn is_alive(p: &ReindexProgress) -> bool {
    crate::platform::process_alive(p.pid)
}

/// Delete rows whose writing process is gone, returning how many were removed.
///
/// Called by a **starting** reindex rather than by a reader, so the repair runs
/// on the path that happens anyway and the correct behaviour leaves nothing
/// armed for the next session. A reader that pruned would be mutating during a
/// diagnostic — and would need the write lock to do it, which is the one thing
/// an observer of a running reindex must not take.
pub fn prune_dead(conn: &Connection) -> Result<usize> {
    let snap = read_all(conn)?;
    let mut removed = 0usize;
    for p in snap.runs.iter().filter(|p| !is_alive(p)) {
        clear(conn, p.pid)?;
        removed += 1;
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;

    fn sample(pid: u32, done: u32) -> ReindexProgress {
        ReindexProgress {
            pid,
            started_ms: 1_000,
            heartbeat_ms: 1_000 + i64::from(done),
            done,
            total: 100,
            scope: "/repo".to_string(),
        }
    }

    #[test]
    fn a_published_row_is_readable_by_a_separate_connection() {
        // The whole point is cross-process visibility. Two connections to one
        // file is the closest in-process stand-in; a single-connection test
        // would pass even if the value never left the writer's memory.
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("c.db");
        let writer = Catalog::open(&db).unwrap();
        publish(&writer.conn, &sample(4242, 7)).unwrap();

        let reader = Catalog::open(&db).unwrap();
        let snap = read_all(&reader.conn).unwrap();
        assert_eq!(snap.runs.len(), 1);
        assert_eq!(snap.runs[0].done, 7);
        assert_eq!(snap.runs[0].pid, 4242);
    }

    #[test]
    fn two_concurrent_runs_do_not_overwrite_each_other() {
        // The reason the key is per-PID. Under a single well-known key this
        // asserts 1, and the observer silently sees the wrong run.
        let cat = Catalog::open_in_memory().unwrap();
        publish(&cat.conn, &sample(1, 10)).unwrap();
        publish(&cat.conn, &sample(2, 20)).unwrap();
        let snap = read_all(&cat.conn).unwrap();
        assert_eq!(snap.runs.len(), 2, "one row per pid: {:?}", snap.runs);
    }

    #[test]
    fn republishing_the_same_pid_advances_it_in_place() {
        let cat = Catalog::open_in_memory().unwrap();
        publish(&cat.conn, &sample(9, 1)).unwrap();
        publish(&cat.conn, &sample(9, 500)).unwrap();
        let snap = read_all(&cat.conn).unwrap();
        assert_eq!(snap.runs.len(), 1);
        assert_eq!(snap.runs[0].done, 500);
    }

    #[test]
    fn clear_removes_only_the_named_pid() {
        let cat = Catalog::open_in_memory().unwrap();
        publish(&cat.conn, &sample(1, 10)).unwrap();
        publish(&cat.conn, &sample(2, 20)).unwrap();
        clear(&cat.conn, 1).unwrap();
        let snap = read_all(&cat.conn).unwrap();
        assert_eq!(snap.runs.len(), 1);
        assert_eq!(snap.runs[0].pid, 2);
    }

    #[test]
    fn clearing_an_absent_row_succeeds() {
        // Load-bearing: `clear` runs on every exit path including ones where
        // no row was ever written. An error here would turn a clean early
        // return into a failure.
        let cat = Catalog::open_in_memory().unwrap();
        clear(&cat.conn, 12345).unwrap();
    }

    #[test]
    fn read_all_ignores_unrelated_meta_keys() {
        let cat = Catalog::open_in_memory().unwrap();
        gc::set_meta(&cat.conn, "last_reindex_embed_error_count", "3").unwrap();
        // `reindex_in_progress` has underscores, which are LIKE wildcards. An
        // unescaped pattern matches this decoy; the escape helper does not.
        // Delete this row and the test still passes with a broken pattern.
        gc::set_meta(&cat.conn, "reindexXinXprogressX:99", "{}").unwrap();
        publish(&cat.conn, &sample(7, 1)).unwrap();

        let snap = read_all(&cat.conn).unwrap();
        assert_eq!(snap.runs.len(), 1, "{snap:?}");
        assert_eq!(snap.runs[0].pid, 7);
        assert!(snap.unparseable.is_empty(), "{:?}", snap.unparseable);
    }

    #[test]
    fn an_unreadable_row_is_reported_rather_than_dropped() {
        // Dropping it would make a corrupt catalog look like an idle one.
        let cat = Catalog::open_in_memory().unwrap();
        gc::set_meta(&cat.conn, &key_for(5), "not json").unwrap();
        let snap = read_all(&cat.conn).unwrap();
        assert!(snap.runs.is_empty());
        assert_eq!(snap.unparseable, vec![key_for(5)]);
    }

    /// A pid that is definitely not running — spawned, then reaped.
    ///
    /// Deliberately NOT a magic constant. `0` and anything ≥ `2^31` are "dead"
    /// for reasons unrelated to liveness (`platform::unix::addressable_pid`
    /// rejects them before `kill` is reached), so a constant here would leave
    /// this test green against a `prune_dead` that cannot recognise a real
    /// dead process — the only case that actually occurs in production.
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

    #[test]
    fn prune_dead_removes_a_dead_writer_and_keeps_a_live_one() {
        let cat = Catalog::open_in_memory().unwrap();
        let dead = a_reaped_pid();
        publish(&cat.conn, &sample(std::process::id(), 5)).unwrap();
        publish(&cat.conn, &sample(dead, 5)).unwrap();

        let removed = prune_dead(&cat.conn).unwrap();
        assert_eq!(removed, 1, "the reaped writer's row must be collected");
        let snap = read_all(&cat.conn).unwrap();
        assert_eq!(snap.runs.len(), 1);
        assert_eq!(
            snap.runs[0].pid,
            std::process::id(),
            "a LIVE writer's row must survive — pruning everything is the mutation \
             that would silently delete a running reindex's own progress"
        );
    }

    /// A pid read back out of `catalog_meta` is **parsed input**, not a value
    /// this process got from the kernel — a corrupt or hand-edited row can hold
    /// any `u32`. Before 2026-09-05 those values flipped `kill`'s addressing
    /// mode and reported a nonexistent writer as alive, so its row would never
    /// have been pruned. Guarded in `platform::unix::addressable_pid`; asserted
    /// here because this module is the reason that input is untrusted.
    ///
    /// The live row is the discriminator: without it, an `is_alive` hard-wired
    /// to `false` passes.
    #[test]
    fn is_alive_is_true_for_this_process_and_false_for_an_impossible_pid() {
        assert!(is_alive(&sample(std::process::id(), 0)));
        for pid in [0u32, 0x8000_0000, u32::MAX] {
            assert!(!is_alive(&sample(pid, 0)), "pid {pid} names no one process");
        }
    }

    /// The case that actually occurs: a writer that ran and exited. The test
    /// above only reaches the range guard, so on its own it would stay green
    /// against an `is_alive` that never calls `kill`.
    #[test]
    fn is_alive_tracks_a_real_process_ending() {
        assert!(!is_alive(&sample(a_reaped_pid(), 0)));
    }

    #[test]
    fn should_publish_gates_on_the_interval() {
        assert!(!should_publish(1_000, 1_000));
        assert!(!should_publish(1_000, 1_000 + HEARTBEAT_INTERVAL_MS - 1));
        assert!(should_publish(1_000, 1_000 + HEARTBEAT_INTERVAL_MS));
    }
}
