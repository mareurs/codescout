//! Committed audit shards: the export half.
//!
//! The local WAL cannot live in git — its in-transaction guarantee exists only
//! at mutation time on a gitignored database — so what git carries is a
//! REPLICA, and every surface must say so. See the spec's § Phase 2.
//!
//! `export`/`unexported_count` have no real (non-test) caller yet — the CLI
//! or reindex integration point that calls them lands after this task — so
//! every item in this file is only reachable via this file's own
//! `#[cfg(test)] mod tests`. Same `#[cfg_attr(not(test), expect(dead_code,
//! reason = "..."))]` pattern as `host.rs`: the expectation is asserted only
//! in the non-test build, where these items are genuinely dead today, and
//! fires the moment a later change adds a real caller without deleting the
//! attribute.

use super::host::{self, AUDIT_DIR};
use crate::librarian::catalog::gc;
use anyhow::{Context, Result};
use fs4::fs_std::FileExt;
use rusqlite::Connection;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by shard::export/unexported_count, not yet wired to a live (non-test) caller until Task 4's integration point lands"
    )
)]
pub(crate) const WATERMARK_KEY: &str = "audit_exported_through_seq";

/// Changed-key sets that are pure reindex bookkeeping. An `update` whose keys
/// are a SUBSET of this is dropped from the export; one that also carries any
/// other key is real history and is kept.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by shard::is_pure_churn, live only via export, not yet wired to a live (non-test) caller"
    )
)]
const CHURN_KEYS: &[&str] = &["file_mtime", "file_sha256", "updated_at", "missing_since"];

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by shard::export, not yet wired to a live (non-test) caller"
    )
)]
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct ShardLine {
    pub host: String,
    pub seq: i64,
    pub at_ms: i64,
    pub tbl: String,
    pub op: String,
    pub row_id: String,
    pub actor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by shard::export, not yet wired to a live (non-test) caller"
    )
)]
#[derive(Debug, Default, serde::Serialize)]
pub(crate) struct ExportReport {
    pub exported: usize,
    pub skipped_commits: usize,
    pub skipped_churn: usize,
    pub files: Vec<String>,
    pub through_seq: i64,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by shard::export, not yet wired to a live (non-test) caller"
    )
)]
fn is_pure_churn(op: &str, payload: Option<&str>) -> bool {
    if op != "update" {
        return false;
    }
    let Some(p) = payload else { return false };
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(p) else {
        return false;
    };
    !map.is_empty() && map.keys().all(|k| CHURN_KEYS.contains(&k.as_str()))
}
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by shard::export/unexported_count, not yet wired to a live (non-test) caller"
    )
)]
fn watermark(conn: &Connection) -> Result<i64> {
    Ok(gc::get_meta(conn, WATERMARK_KEY)?
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(0))
}

/// Rows past the watermark that export would consider — the SAME population
/// `export` consumes, including the ones it will drop. Doctor reports this, so
/// it must not describe a different set than the verb does; a delta that
/// counts rows export will never write reads as a permanent backlog.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by doctor.rs's audit_health reporting once Task 4 wires it in; not yet wired to a live (non-test) caller"
    )
)]
pub(crate) fn unexported_count(conn: &Connection) -> Result<i64> {
    let w = watermark(conn)?;
    Ok(conn.query_row(
        "SELECT count(*) FROM catalog_audit WHERE seq > ?1",
        [w],
        |r| r.get(0),
    )?)
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by Task 4's CLI/reindex integration point; not yet wired to a live (non-test) caller"
    )
)]
pub(crate) fn export(conn: &Connection, repo_root: &Path) -> Result<ExportReport> {
    let host_id = host::resolve_host_id(conn)?;
    let from = watermark(conn)?;
    let dir = repo_root.join(AUDIT_DIR);

    let mut stmt = conn.prepare(
        "SELECT seq, at_ms, tbl, op, row_id, actor, verb, payload
             FROM catalog_audit WHERE seq > ?1 ORDER BY seq",
    )?;
    let rows = stmt
        .query_map([from], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut report = ExportReport {
        through_seq: from,
        ..Default::default()
    };
    // Keyed by month string; value is (a representative at_ms for that month,
    // used only to derive the shard filename, and the serialized lines).
    let mut by_month: BTreeMap<String, (i64, Vec<String>)> = BTreeMap::new();

    for (seq, at_ms, tbl, op, row_id, actor, verb, payload) in rows {
        // Must advance past a skip-only row too (commits, pure churn) — the
        // watermark tracks position in the SOURCE table, not export outcome.
        // Moving this below the two `continue`s below leaves it stuck behind
        // skip-only rows forever, a permanent phantom backlog in doctor's
        // `unexported_count`; see commits_rows_are_never_exported and
        // reindex_churn_updates_are_never_exported, both of which now assert
        // `unexported_count == 0` after a skip-only export (Important 2,
        // task-2 review).
        report.through_seq = report.through_seq.max(seq);
        if tbl == "commits" {
            report.skipped_commits += 1;
            continue;
        }
        if is_pure_churn(&op, payload.as_deref()) {
            report.skipped_churn += 1;
            continue;
        }
        let line = ShardLine {
            host: host_id.clone(),
            seq,
            at_ms,
            tbl,
            op,
            row_id,
            actor,
            verb,
            // A payload that fails to parse as JSON is still audit data — do
            // not let it vanish silently. `skip_serializing_if` only omits a
            // genuine `None`, so falling back to a string preserves the raw
            // bytes in the shard line instead of dropping the field with no
            // error and no counter (small fix 8, task-2 review).
            payload: payload.as_deref().map(|p| {
                serde_json::from_str(p).unwrap_or_else(|_| serde_json::Value::String(p.to_string()))
            }),
        };
        by_month
            .entry(host::month_key(at_ms))
            .or_insert_with(|| (at_ms, Vec::new()))
            .1
            .push(serde_json::to_string(&line)?);
        report.exported += 1;
    }

    if !by_month.is_empty() {
        // Only created when there is something to write — a no-op export
        // (everything skipped, or nothing past the watermark) must not touch
        // the filesystem at all (small fix 10, task-2 review). The test
        // helper `lines()` was updated to tolerate a missing directory so it
        // no longer forces this to run unconditionally.
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        for (representative_at_ms, lines) in by_month.values() {
            // Ruling 4: the filename convention has exactly one definition —
            // host::shard_file_name — so Task 1's shard_names_round_trip test
            // covers the bytes this writer actually produces. Building the
            // name inline here would let the two drift with no error anywhere.
            let name = host::shard_file_name(&host_id, *representative_at_ms);
            let path = dir.join(&name);
            // One exclusive lock per file: two sessions reindexing at once must
            // not interleave partial lines into a file that is about to be
            // committed. Same primitive as src/retrieval/index_lock.rs. Armed
            // for Task 4's production wiring — no test here exercises actual
            // contention, since `export` has no non-test caller yet and
            // cannot race itself within one test process.
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .with_context(|| format!("opening {}", path.display()))?;
            FileExt::lock_exclusive(&f)?;
            // A single `write_all` over one already-newline-terminated buffer,
            // not `writeln!`, which lowers to two separate `write_all` calls
            // (body, then "\n") — a kill between them leaves a torn line with
            // no trailing newline, and the next export's append then
            // concatenates its first JSON object onto that unterminated line
            // (small fix 3, task-2 review). One write_all is one syscall; a
            // partial write of it still can't interleave a second object onto
            // the same physical line the way two calls can.
            let r = f
                .write_all(format!("{}\n", lines.join("\n")).as_bytes())
                .and_then(|_| f.sync_all());
            let _ = FileExt::unlock(&f);
            r.with_context(|| format!("appending to {}", path.display()))?;
            report.files.push(name);
        }
        // Best-effort: fsync the directory too, so the dirent linking a
        // newly-created shard file is durable. Without this, a power loss
        // can durably advance the watermark (SQLite fsyncs its own commit)
        // while the shard file's directory entry never lands — the exact
        // silent-loss mode the append-then-watermark ordering exists to
        // prevent (small fix 4, task-2 review). Best-effort and ignored on
        // platforms/filesystems that refuse to open or sync a directory.
        if let Ok(dir_handle) = std::fs::File::open(&dir) {
            let _ = dir_handle.sync_all();
        }
    }

    // Only after every append (and, best-effort, its directory entry) is
    // durable — see the module-level ordering note on this function, and
    // a_failed_append_never_advances_the_watermark/a_crash_between_append_and_
    // watermark_duplicates_rather_than_loses below, which pin it from both
    // directions.
    gc::set_meta(conn, WATERMARK_KEY, &report.through_seq.to_string())?;
    Ok(report)
}

#[derive(Debug, Default)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by Task 4 (production wiring) via read_shards"
    )
)]
pub(crate) struct ShardRead {
    pub rows: Vec<ShardLine>,
    pub malformed: usize,
    /// host → (min seq, max seq) present. This is the coverage window, DERIVED
    /// from the rows rather than declared in a header: a header line would be
    /// duplicated by `merge=union` on every same-host branch merge, and a
    /// declared window that disagrees with the rows is worse than none.
    pub hosts: BTreeMap<String, (i64, i64)>,
    pub files_read: usize,
    pub files_skipped_by_window: usize,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by Task 4 (production wiring) via read_shards"
    )
)]
fn matches(l: &ShardLine, f: &super::AuditFilter) -> bool {
    f.tbl.as_ref().is_none_or(|v| *v == l.tbl)
        && f.row_id.as_ref().is_none_or(|v| *v == l.row_id)
        && f.actor.as_ref().is_none_or(|v| *v == l.actor)
        && f.op.as_ref().is_none_or(|v| *v == l.op)
        && f.since.is_none_or(|v| l.at_ms >= v)
        && f.until.is_none_or(|v| l.at_ms <= v)
}

/// Read every OTHER host's shards, filtered and deduped.
///
/// `self_host`'s own shard is skipped: those rows are already in the local
/// table, and counting them twice would produce a wrong `filtered_total` —
/// a plausible number rather than an error, which nothing downstream catches.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by Task 4 (production wiring), the merge-on-query tool surface"
    )
)]
pub(crate) fn read_shards(
    repo_root: &Path,
    f: &super::AuditFilter,
    self_host: &str,
) -> Result<ShardRead> {
    let dir = repo_root.join(AUDIT_DIR);
    let mut out = ShardRead::default();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(out); // never exported, or a fresh clone: an empty read.
    };
    let mut seen: std::collections::HashSet<(String, i64)> = Default::default();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some((file_host, month)) = host::parse_shard_file_name(&name) else {
            continue; // a README or stray file: not a shard, not malformed.
        };
        if file_host == self_host {
            continue;
        }
        if !month_in_window(&month, f) {
            out.files_skipped_by_window += 1;
            continue;
        }
        let Ok(body) = std::fs::read_to_string(e.path()) else {
            continue;
        };
        out.files_read += 1;
        for raw in body.lines() {
            if raw.trim().is_empty() {
                continue;
            }
            let Ok(line) = serde_json::from_str::<ShardLine>(raw) else {
                out.malformed += 1;
                continue;
            };
            let key = (line.host.clone(), line.seq);
            let span = out
                .hosts
                .entry(line.host.clone())
                .or_insert((line.seq, line.seq));
            span.0 = span.0.min(line.seq);
            span.1 = span.1.max(line.seq);
            if !seen.insert(key) || !matches(&line, f) {
                continue;
            }
            out.rows.push(line);
        }
    }
    out.rows.sort_by(|a, b| {
        b.at_ms
            .cmp(&a.at_ms)
            .then_with(|| b.seq.cmp(&a.seq))
            .then_with(|| a.host.cmp(&b.host))
    });
    Ok(out)
}

/// Whole-file pruning from the filename's `YYYYMM`. Inclusive at both ends and
/// deliberately coarse — a month that straddles the boundary is opened and its
/// rows filtered per-line. Being generous here is the safe direction: a file
/// wrongly skipped is a silently missing row.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "consumed by Task 4 (production wiring) via read_shards"
    )
)]
fn month_in_window(month: &str, f: &super::AuditFilter) -> bool {
    let in_bound = |ms: i64, keep_if_ge: bool| -> bool {
        let bound = host::month_key(ms);
        if keep_if_ge {
            month.as_bytes() >= bound.as_bytes()
        } else {
            month.as_bytes() <= bound.as_bytes()
        }
    };
    f.since.is_none_or(|v| in_bound(v, true)) && f.until.is_none_or(|v| in_bound(v, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::{artifact, Catalog};

    fn seed(cat: &Catalog, id: &str) {
        let row = artifact::TestArtifactRowBuilder::new(id)
            .with_status("draft")
            .build();
        artifact::upsert(cat, &row).unwrap();
    }

    fn lines(dir: &std::path::Path) -> Vec<ShardLine> {
        let mut out = Vec::new();
        // export() now only creates the audit directory when it actually has
        // something to write (small fix 10, task-2 review) — a no-op export,
        // or a test that never calls export at all, leaves it absent. That is
        // a valid "nothing exported yet" state, not an error, so tolerate it
        // here rather than forcing export to create the directory unconditionally.
        let entries = match std::fs::read_dir(dir.join(super::super::host::AUDIT_DIR)) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
            Err(e) => panic!("reading audit dir: {e}"),
        };
        for e in entries {
            let p = e.unwrap().path();
            for l in std::fs::read_to_string(&p).unwrap().lines() {
                out.push(serde_json::from_str(l).unwrap());
            }
        }
        out
    }

    #[test]
    fn export_writes_rows_past_the_watermark_and_advances_it() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let host_id = host::resolve_host_id(&cat.conn).unwrap();
        // Pin the full line shape against the actual source row, not just its
        // presence — a mutation probe found `at_ms`/`actor`/`verb`/`payload`
        // all survived being zeroed at the struct literal because nothing read
        // them back (Important 1, task-2 review).
        //
        // The insert row alone cannot cover `verb`/`payload`: per
        // audit/mod.rs's own insert_update_delete_on_artifact_each_leave_an_audit_row,
        // an insert carries no payload, and `verb` stays `None` unless something
        // stamps it first — so both would be `None == None` under a broken
        // implementation too, and the assertion could not see a real value
        // collapsing to `None` (re-review, task-2 round 2). Stamp a verb and
        // perform a column-changing UPDATE (not a pure-churn one) to get a row
        // with both fields genuinely populated, and assert against THAT row.
        cat.set_audit_verb("artifact.update").unwrap();
        cat.conn
            .execute("UPDATE artifact SET status='archived' WHERE id='a1'", [])
            .unwrap();
        let (src_at_ms, src_actor, src_verb, src_payload): (
            i64,
            String,
            Option<String>,
            Option<String>,
        ) = cat
            .conn
            .query_row(
                "SELECT at_ms, actor, verb, payload FROM catalog_audit
             WHERE tbl='artifact' AND row_id='a1' AND op='update'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert!(
            src_verb.is_some(),
            "fixture must stamp a real verb, not None"
        );
        assert!(
            src_payload.is_some(),
            "fixture's UPDATE must change a real column, not be pure churn"
        );
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert!(r.exported >= 1, "{r:?}");
        assert!(r.through_seq > 0);
        let got = lines(tmp.path());
        let line = got
            .iter()
            .find(|l| l.row_id == "a1" && l.tbl == "artifact" && l.op == "update")
            .expect("the seeded artifact's update row must be exported");
        assert_eq!(
            line.host, host_id,
            "the line's host must match the exporting host's own id, not just be non-empty"
        );
        assert_eq!(line.at_ms, src_at_ms);
        assert_eq!(line.actor, src_actor);
        assert_eq!(
            line.verb, src_verb,
            "verb must survive to the exported line"
        );
        assert_eq!(
            line.payload,
            src_payload
                .as_deref()
                .and_then(|p| serde_json::from_str(p).ok()),
            "payload must survive to the exported line"
        );
        assert!(
            got.iter().all(|l| !l.host.is_empty()),
            "every line names its host"
        );
    }

    #[test]
    fn a_second_export_with_no_new_rows_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        export(&cat.conn, tmp.path()).unwrap();
        let before = lines(tmp.path()).len();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.exported, 0, "the watermark must hold");
        assert_eq!(
            lines(tmp.path()).len(),
            before,
            "and nothing may be appended"
        );
    }

    #[test]
    fn commits_rows_are_never_exported() {
        // Git already records commits; auditing them INTO git is circular.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        cat.conn
            .execute(
                "INSERT INTO commits(hash, git_root, authored_at, subject)
                 VALUES('deadbeef','/r',1,'s')",
                [],
            )
            .unwrap();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.skipped_commits, 1, "{r:?}");
        assert!(lines(tmp.path()).iter().all(|l| l.tbl != "commits"));
        // The watermark must advance past a skip-only row too, or it sits
        // behind the row forever and doctor reports a permanent phantom
        // backlog (Important 2, task-2 review) — moving the `through_seq`
        // update below the `continue`s left every other assertion in this
        // suite green.
        assert_eq!(unexported_count(&cat.conn).unwrap(), 0);
    }

    #[test]
    fn reindex_churn_updates_are_never_exported() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        export(&cat.conn, tmp.path()).unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET file_mtime=99, file_sha256='z', updated_at=99 WHERE id='a1'",
                [],
            )
            .unwrap();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.skipped_churn, 1, "{r:?}");
        assert_eq!(r.exported, 0);
        // Same phantom-backlog guard as commits_rows_are_never_exported: a
        // churn-only row must not sit past the watermark forever.
        assert_eq!(unexported_count(&cat.conn).unwrap(), 0);
    }

    #[test]
    fn a_semantic_update_that_also_touches_mtime_is_still_exported() {
        // Pair of the above: the churn filter is a SUBSET test, not an
        // intersection test. An update carrying `status` alongside the mtime
        // trio is real history, and dropping it would lose exactly the rows the
        // trail exists for.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        export(&cat.conn, tmp.path()).unwrap();
        cat.conn
            .execute(
                "UPDATE artifact SET status='active', file_mtime=99, updated_at=99 WHERE id='a1'",
                [],
            )
            .unwrap();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(r.exported, 1, "{r:?}");
        assert_eq!(r.skipped_churn, 0);
    }

    #[test]
    fn rows_from_different_months_land_in_different_files() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id,actor)
                 VALUES(1751328000000,'artifact','delete','old','unknown'),
                       (1788220800000,'artifact','delete','new','unknown')",
                [],
            )
            .unwrap();
        export(&cat.conn, tmp.path()).unwrap();
        let dir = tmp.path().join(super::super::host::AUDIT_DIR);
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names.len(), 2, "one file per month, got {names:?}");
        assert!(names[0].ends_with("-202507.jsonl"), "{names:?}");
        assert!(names[1].ends_with("-202609.jsonl"), "{names:?}");
    }

    #[test]
    fn a_crash_between_append_and_watermark_duplicates_rather_than_loses() {
        // The ordering is load-bearing and this test is what pins it. Append
        // first, advance the watermark second: a crash in between re-exports
        // rows already on disk, and readers dedupe on (host, seq). The inverse
        // order would LOSE rows with no signal anywhere.
        //
        // Two artifacts are seeded, not one: with a single row, `n == 1` and
        // `seqs.len() == n` is `1 == 1` for ANY value of `seq` — replacing
        // `seq` with a constant at the struct literal leaves this green
        // (Important 1, task-2 review). With two distinct seqs the set only
        // has cardinality `n` if both are preserved and distinct.
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        seed(&cat, "a2");
        export(&cat.conn, tmp.path()).unwrap();
        let n = lines(tmp.path()).len();
        assert!(n >= 2, "expected at least the two seeded rows, got {n}");
        // Simulate the crash: the file is written, the watermark never advanced.
        gc::set_meta(&cat.conn, WATERMARK_KEY, "0").unwrap();
        export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(
            lines(tmp.path()).len(),
            n * 2,
            "re-export duplicates, by design"
        );
        let seqs: std::collections::HashSet<i64> =
            lines(tmp.path()).iter().map(|l| l.seq).collect();
        assert_eq!(seqs.len(), n, "and every duplicate shares its (host, seq)");
    }

    /// The test above proves the SYSTEM tolerates a stale watermark (it
    /// re-exports and duplicates rather than losing rows) but it calls
    /// `export` to completion twice — it never observes a crash occurring
    /// *between* the two steps `export` performs, so it cannot actually
    /// tell the two orders apart: inverting the order inside `export`
    /// still leaves this test green, because both calls always finish
    /// both steps. Confirmed empirically before writing this test: with
    /// the watermark-set line moved to run before the append loop, `cargo
    /// test a_crash_between_append_and_watermark_duplicates_rather_than_loses`
    /// still reported `1 passed`.
    ///
    /// This test instead forces the append itself to fail (the shard
    /// directory is made unwritable) and checks the ordering claim
    /// directly: the watermark must be exactly what it was before the
    /// call, because `export` returns its error via `?` from inside the
    /// append loop, which runs strictly before the `gc::set_meta` line.
    /// Were the order inverted, this call would advance the watermark
    /// and THEN fail to append — silently losing the row the watermark
    /// now claims was exported, which is exactly the failure mode the
    /// ordering exists to prevent.
    #[cfg(unix)]
    #[test]
    fn a_failed_append_never_advances_the_watermark() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let audit_dir = tmp.path().join(AUDIT_DIR);
        std::fs::create_dir_all(&audit_dir).unwrap();
        std::fs::set_permissions(&audit_dir, std::fs::Permissions::from_mode(0o500)).unwrap();

        // Root (and some filesystems) ignore the write bit — probe rather
        // than assume, and degrade to a no-op if the precondition never
        // triggers (same pattern as
        // rendezvous::tests::publish_degrades_to_none_on_filesystem_failure).
        // This is the SOLE guard of a load-bearing ordering invariant, so the
        // skip must be loud: silent degradation here would read identically
        // to a real pass on a CI runner that happens to run as root (small
        // fix 5, task-2 review).
        let probe_ok = std::fs::File::create(audit_dir.join("probe")).is_ok();
        if probe_ok {
            eprintln!(
                "a_failed_append_never_advances_the_watermark: SKIPPED — this \
                 environment's filesystem does not enforce 0o500 (likely running \
                 as root), so the ordering invariant was NOT exercised by this run"
            );
            let _ = std::fs::remove_file(audit_dir.join("probe"));
            let _ = std::fs::set_permissions(&audit_dir, std::fs::Permissions::from_mode(0o700));
            return;
        }

        let before = gc::get_meta(&cat.conn, WATERMARK_KEY).unwrap();
        let result = export(&cat.conn, tmp.path());
        let _ = std::fs::set_permissions(&audit_dir, std::fs::Permissions::from_mode(0o700));

        let err =
            result.expect_err("append into an unwritable directory must fail, not silently no-op");
        // Pin WHICH failure this is, not just that some error occurred — an
        // earlier failure (e.g. at create_dir_all) would also make `result`
        // an `Err` without ever reaching the append step this test targets,
        // which would pass without exercising the ordering claim at all
        // (small fix 6, task-2 review).
        let chain = format!("{err:#}");
        assert!(
            chain.contains("opening "),
            "expected the failure to originate at export's file-open step \
             (context \"opening <path>\"), got: {chain}"
        );
        let after = gc::get_meta(&cat.conn, WATERMARK_KEY).unwrap();
        assert_eq!(
            before, after,
            "a failed append must never advance the watermark — advancing it \
                 here would claim row a1 was exported when it never touched disk"
        );
    }

    #[test]
    fn unexported_count_matches_what_the_next_export_would_write() {
        let tmp = tempfile::tempdir().unwrap();
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let pending = unexported_count(&cat.conn).unwrap();
        let r = export(&cat.conn, tmp.path()).unwrap();
        assert_eq!(
            pending as usize,
            r.exported + r.skipped_commits + r.skipped_churn,
            "doctor's delta must describe the same population export consumes"
        );
        assert_eq!(unexported_count(&cat.conn).unwrap(), 0);
    }

    fn write_shard(root: &std::path::Path, name: &str, lines: &[&str]) {
        let dir = root.join(super::super::host::AUDIT_DIR);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), lines.join("\n")).unwrap();
    }

    fn foreign_line(seq: i64, at_ms: i64, row_id: &str) -> String {
        serde_json::json!({
            "host": "otherbox-99ffee", "seq": seq, "at_ms": at_ms,
            "tbl": "artifact", "op": "delete", "row_id": row_id,
            "actor": "codescout:sess-b", "payload": {"id": row_id}
        })
        .to_string()
    }

    #[test]
    fn a_foreign_hosts_rows_are_read_back() {
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[&foreign_line(1, 1_788_220_800_000, "gone-1")],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].row_id, "gone-1");
        assert_eq!(r.hosts.len(), 1, "coverage names the host");
    }

    #[test]
    fn our_own_hosts_shard_is_not_read_back() {
        // Our rows are already in the local table. Reading our own shard too
        // would double-count them in `filtered_total` — a wrong NUMBER, which
        // nothing downstream can catch.
        let tmp = tempfile::tempdir().unwrap();
        let mut mine: serde_json::Value =
            serde_json::from_str(&foreign_line(1, 1_788_220_800_000, "x")).unwrap();
        mine["host"] = serde_json::json!("me-000000");
        write_shard(tmp.path(), "me-000000-202609.jsonl", &[&mine.to_string()]);
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert!(r.rows.is_empty(), "got {:?}", r.rows);
    }

    #[test]
    fn a_malformed_line_is_counted_not_silently_dropped() {
        // A shard is a git-merged file; a bad line is expected eventually. A
        // silent skip makes a partial answer look complete (IC-13) — the exact
        // class this feature exists to avoid.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line(1, 1_788_220_800_000, "ok-1"),
                "{not json",
                "",
                &foreign_line(2, 1_788_220_800_001, "ok-2"),
            ],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 2, "the good lines still arrive");
        assert_eq!(r.malformed, 1, "and the bad one is REPORTED");
    }

    #[test]
    fn duplicate_host_seq_pairs_collapse_to_one_row() {
        // merge=union and crash-re-export both produce duplicates by design.
        // Three copies (not two) so a mutation that dedups on line POSITION
        // rather than (host, seq) — e.g. keeping only the first two lines
        // regardless of content — cannot pass by accident: it must actually
        // collapse on the key to reach the single-row assertion.
        let tmp = tempfile::tempdir().unwrap();
        let l = foreign_line(7, 1_788_220_800_000, "dup");
        write_shard(tmp.path(), "otherbox-99ffee-202609.jsonl", &[&l, &l, &l]);
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1, "deduped on (host, seq)");
        assert_eq!(r.rows[0].row_id, "dup");
    }

    #[test]
    fn filters_apply_to_shard_rows_the_same_way_they_apply_locally() {
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line(1, 1_788_220_800_000, "wanted"),
                &foreign_line(2, 1_788_220_800_001, "other"),
            ],
        );
        let f = crate::librarian::catalog::audit::AuditFilter {
            row_id: Some("wanted".into()),
            ..Default::default()
        };
        let r = read_shards(tmp.path(), &f, "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].row_id, "wanted");
    }

    #[test]
    fn a_since_window_skips_whole_files_by_name() {
        // The filename encodes the month, so an out-of-window file is never
        // opened. This is the property that keeps merge-on-query affordable.
        // Assert `files_read`/`files_skipped_by_window`, not just row count —
        // a reader that opens every file and filters per-line would satisfy a
        // row-count-only assertion while losing the property that makes this
        // affordable.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202507.jsonl",
            &[&foreign_line(1, 1_751_328_000_000, "old")],
        );
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[&foreign_line(2, 1_788_220_800_000, "new")],
        );
        let f = crate::librarian::catalog::audit::AuditFilter {
            since: Some(1_788_220_000_000),
            ..Default::default()
        };
        let r = read_shards(tmp.path(), &f, "me-000000").unwrap();
        assert_eq!(r.files_read, 1, "only the in-window file is opened");
        assert_eq!(r.files_skipped_by_window, 1);
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].row_id, "new");
    }

    #[test]
    fn a_straddling_month_is_opened_and_filtered_per_line() {
        // Whole-file pruning is by filename month only, and must be generous
        // at the boundary: a file whose month straddles `since` is opened even
        // though it also carries rows that predate the cutoff, and only the
        // per-line filter (not the file-level window) excludes those rows.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line(1, 1_788_220_000_000, "before-cutoff"),
                &foreign_line(2, 1_788_220_900_000, "after-cutoff"),
            ],
        );
        let f = crate::librarian::catalog::audit::AuditFilter {
            since: Some(1_788_220_800_000),
            ..Default::default()
        };
        let r = read_shards(tmp.path(), &f, "me-000000").unwrap();
        assert_eq!(
            r.files_read, 1,
            "the straddling file is opened, not skipped"
        );
        assert_eq!(r.files_skipped_by_window, 0);
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.rows[0].row_id, "after-cutoff");
    }

    #[test]
    fn a_missing_audit_directory_is_an_empty_read_not_an_error() {
        // Every clone that has never exported is in this state.
        let tmp = tempfile::tempdir().unwrap();
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert!(r.rows.is_empty());
        assert_eq!(r.files_read, 0);
    }

    #[test]
    fn a_stray_non_shard_file_is_ignored_and_not_counted_malformed() {
        // The audit directory is committed and will accumulate READMEs and
        // stray files. Those must return None from the parser and must NOT
        // inflate `malformed` — reporting them as malformed would train
        // readers to ignore the malformed count that matters.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(tmp.path(), "README.md", &["not a shard at all"]);
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[&foreign_line(1, 1_788_220_800_000, "ok")],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.rows.len(), 1);
        assert_eq!(r.malformed, 0, "a stray file is not a malformed shard line");
        assert_eq!(r.files_read, 1, "only the real shard was opened");
    }

    #[test]
    fn coverage_reports_the_min_and_max_seq_actually_present() {
        // Coverage must be DERIVED from the rows present, never declared in a
        // header (a header would be duplicated by merge=union). Seed 3
        // out-of-order seqs so a mutation that reports (first, last) by file
        // position rather than true min/max cannot pass by accident.
        let tmp = tempfile::tempdir().unwrap();
        write_shard(
            tmp.path(),
            "otherbox-99ffee-202609.jsonl",
            &[
                &foreign_line(5, 1_788_220_800_000, "mid"),
                &foreign_line(1, 1_788_220_800_001, "lo"),
                &foreign_line(9, 1_788_220_800_002, "hi"),
            ],
        );
        let r = read_shards(tmp.path(), &Default::default(), "me-000000").unwrap();
        assert_eq!(r.hosts.get("otherbox-99ffee"), Some(&(1, 9)));
    }
}
