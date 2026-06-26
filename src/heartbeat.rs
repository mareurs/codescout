//! Durable, discoverable OOM-forensics heartbeat.
//!
//! A codescout MCP server that leaks toward a host-OOM is **SIGKILLed**, so the
//! `tracing_appender::non_blocking` tail never flushes and the per-instance
//! diagnostic log (written under an unknown server-cwd) is unfindable
//! post-mortem. That is exactly why the 68 GB OOM left no usable trace — see
//! `docs/issues/2026-06-19-mcp-server-oom-68gb.md`.
//!
//! This module writes a **synchronous, flushed, one-line-per-tick** RSS
//! heartbeat to a **central, predictable** path —
//! `<cache>/codescout/heartbeats/<pid>.log` — so the memory ramp and the tool
//! that was in flight survive the kill and are trivial to locate afterwards
//! (`ls <cache>/codescout/heartbeats/`). It complements, and does not replace,
//! the richer `tracing` heartbeat in `server::run` (which is gated on `--debug`
//! and rides the lossy non-blocking appender).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Heartbeat tick interval. Matches the existing `--debug` heartbeat cadence.
const INTERVAL: Duration = Duration::from_secs(30);

/// How many per-instance heartbeat files to retain by count. OOM-killed
/// instances cannot clean up their own file, so we prune on startup. Files are
/// tiny (~120 B/line); a generous keep-count preserves several past post-mortems
/// even on a multi-profile machine running many concurrent servers.
const KEEP_FILES: usize = 64;

/// Age floor for pruning: never delete a heartbeat file younger than this,
/// regardless of [`KEEP_FILES`]. A SIGKILLed victim stops writing, so its mtime
/// freezes at death and it sorts as the *oldest* file — the first prune target
/// exactly when it is the most valuable. The age floor guarantees a recent
/// victim's log survives long enough to be read.
/// See `docs/issues/2026-06-26-heartbeat-prune-evicts-oom-victim.md`.
const RETAIN: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Most recently dispatched tool name + the unix-seconds it started. Lets a
/// heartbeat line name the operation in flight while RSS climbs — the single
/// datum the bug doc says was missing ("recover the offending operation").
///
/// Last-writer-wins and never cleared: under concurrent dispatch this records
/// "the most recently invoked tool", which is the useful forensic signal, and
/// the last op persists even after it returns (so a heartbeat just *after* a
/// runaway tool still names it). `Mutex::new` is const, so no lazy init needed.
static CURRENT_OP: Mutex<Option<(String, u64)>> = Mutex::new(None);

/// Record the tool about to run. Called at the single dispatch chokepoint
/// (`CodeScoutServer::call_tool_inner`). Cheap: one short lock per tool call.
pub fn note_tool(name: &str) {
    if let Ok(mut g) = CURRENT_OP.lock() {
        *g = Some((name.to_string(), now_unix()));
    }
}

/// `(tool, age_secs)` for the most recent tool, or `("idle", 0)` before any.
fn current_op() -> (String, u64) {
    match CURRENT_OP.lock().ok().and_then(|g| g.clone()) {
        Some((tool, started)) => (tool, now_unix().saturating_sub(started)),
        None => ("idle".to_string(), 0),
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Process RSS and virtual-memory sizes, in kB. All zero on platforms without
/// `/proc/self/status` (Windows, macOS) or on parse failure — the heartbeat
/// continues regardless. Moved here from `server.rs` so the heartbeat owns its
/// data source.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct SelfMemoryKb {
    pub vm_size_kb: u64,
    pub vm_rss_kb: u64,
    pub vm_data_kb: u64,
    pub vm_peak_kb: u64,
}

/// Read this process's memory footprint from `/proc/self/status` (Linux only;
/// returns zeros elsewhere or on parse failure).
pub fn read_self_memory_kb() -> SelfMemoryKb {
    let mut out = SelfMemoryKb::default();
    let Ok(text) = std::fs::read_to_string("/proc/self/status") else {
        return out;
    };
    for line in text.lines() {
        let Some((key, rest)) = line.split_once(':') else {
            continue;
        };
        let value_kb = rest
            .split_whitespace()
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0);
        match key {
            "VmSize" => out.vm_size_kb = value_kb,
            "VmRSS" => out.vm_rss_kb = value_kb,
            "VmData" => out.vm_data_kb = value_kb,
            "VmPeak" => out.vm_peak_kb = value_kb,
            _ => {}
        }
    }
    out
}

/// Central, predictable heartbeat directory: `<cache>/codescout/heartbeats`.
/// Mirrors `lsp::servers::kotlin_lsp_home_root`'s cache-root resolution so a
/// post-mortem always knows where to look, regardless of the dead instance's
/// cwd (the discoverability gap that lost the 68 GB instance's diagnostic log).
pub fn heartbeat_dir() -> PathBuf {
    dirs::cache_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(std::env::temp_dir)
        .join("codescout")
        .join("heartbeats")
}

fn heartbeat_path(dir: &Path) -> PathBuf {
    dir.join(format!("{}.log", std::process::id()))
}

/// Append `line` synchronously and flush, so it survives a SIGKILL. Mirrors
/// `logging::sync_append`, which the panic hook uses for the same reason.
/// Best-effort: I/O errors are swallowed (instrumentation must never break the
/// server).
fn append(path: &Path, line: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
        let _ = f.flush();
    }
}

/// Pure selection half of `prune_stale`: given `(path, mtime)` pairs, return the
/// paths to delete. Retains the `keep` most-recent by mtime **and** any file
/// younger than `min_age` (relative to `now`) — a SIGKILLed crash victim's mtime
/// freezes at death and sorts oldest, so the age floor stops it being pruned out
/// from under a pending post-mortem. Pure so it is testable without fs timing.
/// See `docs/issues/2026-06-26-heartbeat-prune-evicts-oom-victim.md`.
fn stale_to_remove(
    mut entries: Vec<(PathBuf, SystemTime)>,
    keep: usize,
    now: SystemTime,
    min_age: Duration,
) -> Vec<PathBuf> {
    if entries.len() <= keep {
        return Vec::new();
    }
    // Newest first; the first `keep` are retained outright.
    entries.sort_by_key(|e| std::cmp::Reverse(e.1));
    entries
        .into_iter()
        .skip(keep)
        .filter(|(_, mtime)| {
            // Age floor: only prune genuinely old files. A file younger than
            // `min_age` — or dated in the future (clock skew makes
            // `duration_since` err) — is kept.
            now.duration_since(*mtime)
                .map(|age| age >= min_age)
                .unwrap_or(false)
        })
        .map(|(p, _)| p)
        .collect()
}

/// Keep the `keep` most-recent `*.log` heartbeat files in `dir`, plus any file
/// younger than `min_age`; delete the rest. Mirrors `logging::rotate_diagnostic_logs`.
/// Runs on startup because an OOM-killed instance can't clean up its own file —
/// and the `min_age` floor keeps that instance's own log readable afterwards.
pub fn prune_stale(dir: &Path, keep: usize, min_age: Duration) {
    let entries: Vec<(PathBuf, SystemTime)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".log"))
        .filter_map(|e| {
            let mtime = e.metadata().ok()?.modified().ok()?;
            Some((e.path(), mtime))
        })
        .collect();
    for path in stale_to_remove(entries, keep, SystemTime::now(), min_age) {
        let _ = std::fs::remove_file(path);
    }
}

/// Build one per-tick heartbeat line. Pure (time/pid passed in) so it is
/// testable. Keep it `key=value` and single-line for trivial `grep`/`awk`.
fn tick_line(
    ts: u64,
    pid: u32,
    uptime_s: u64,
    mem: &SelfMemoryKb,
    op: &str,
    op_age_s: u64,
) -> String {
    format!(
        "ts={ts} pid={pid} uptime_s={uptime_s} rss_kb={} vm_size_kb={} vm_peak_kb={} op={op} op_age_s={op_age_s}",
        mem.vm_rss_kb, mem.vm_size_kb, mem.vm_peak_kb,
    )
}

/// Spawn the **always-on** durable heartbeat task. Writes a startup header (so a
/// dead instance's build commit is known) then appends one synchronous RSS line
/// every [`INTERVAL`] for the life of the process. Unlike the `--debug`
/// `tracing` heartbeat in `server::run`, this is not gated on debug and its
/// writes survive SIGKILL.
///
/// `instance`/`project` are recorded in the header for correlation with the
/// `tracing` logs.
pub fn spawn_durable(instance: String, project: String) {
    let dir = heartbeat_dir();
    if std::fs::create_dir_all(&dir).is_err() {
        // Can't create the dir — skip instrumentation rather than fail startup.
        return;
    }
    prune_stale(&dir, KEEP_FILES, RETAIN);
    let path = heartbeat_path(&dir);
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    // Durable header — records the build so a dead instance's commit is known
    // (bug doc Resume: "record the build commit of long-lived servers").
    append(
        &path,
        &format!(
            "# codescout-heartbeat pid={} start_ts={} version={} git_sha={} instance={instance} project={project} cwd={cwd}",
            std::process::id(),
            now_unix(),
            env!("CARGO_PKG_VERSION"),
            env!("CODESCOUT_GIT_SHA"),
        ),
    );

    let start = std::time::Instant::now();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await; // skip the immediate first tick
        loop {
            interval.tick().await;
            let mem = read_self_memory_kb();
            let (op, op_age_s) = current_op();
            let line = tick_line(
                now_unix(),
                std::process::id(),
                start.elapsed().as_secs(),
                &mem,
                &op,
                op_age_s,
            );
            append(&path, &line);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_tool_then_current_op_returns_name() {
        note_tool("semantic_search");
        let (op, _age) = current_op();
        assert_eq!(op, "semantic_search");
        // Last-writer-wins.
        note_tool("run_command");
        assert_eq!(current_op().0, "run_command");
    }

    #[test]
    fn tick_line_is_greppable_keyvalue() {
        let mem = SelfMemoryKb {
            vm_size_kb: 168_000_000,
            vm_rss_kb: 65_000_000,
            vm_data_kb: 0,
            vm_peak_kb: 168_000_000,
        };
        let line = tick_line(1_700_000_000, 4242, 90, &mem, "run_command", 12);
        assert_eq!(
            line,
            "ts=1700000000 pid=4242 uptime_s=90 rss_kb=65000000 vm_size_kb=168000000 vm_peak_kb=168000000 op=run_command op_age_s=12"
        );
        // One line, no embedded newline.
        assert!(!line.contains('\n'));
    }

    #[test]
    fn append_then_read_back_contains_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("12345.log");
        append(&path, "# header");
        append(&path, "ts=1 rss_kb=100");
        append(&path, "ts=2 rss_kb=200");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("# header"));
        assert!(body.contains("rss_kb=100"));
        assert!(body.contains("rss_kb=200"));
        // Each append is its own line.
        assert_eq!(body.lines().count(), 3);
    }

    #[test]
    fn stale_to_remove_keeps_newest_n() {
        let mk = |secs: u64| UNIX_EPOCH + Duration::from_secs(secs);
        let entries = vec![
            (PathBuf::from("a.log"), mk(10)), // oldest
            (PathBuf::from("b.log"), mk(20)),
            (PathBuf::from("c.log"), mk(30)),
            (PathBuf::from("d.log"), mk(40)), // newest
        ];
        let removed = stale_to_remove(entries, 2, mk(1000), Duration::ZERO);
        // Keep the 2 newest (c, d) → remove the 2 oldest (a, b).
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&PathBuf::from("a.log")));
        assert!(removed.contains(&PathBuf::from("b.log")));
    }

    #[test]
    fn stale_to_remove_noop_under_cap() {
        let mk = |secs: u64| UNIX_EPOCH + Duration::from_secs(secs);
        let entries = vec![
            (PathBuf::from("a.log"), mk(10)),
            (PathBuf::from("b.log"), mk(20)),
        ];
        assert!(stale_to_remove(entries, 16, mk(1000), Duration::ZERO).is_empty());
    }

    #[test]
    fn stale_to_remove_age_floor_retains_recent_victim() {
        let mk = |secs: u64| UNIX_EPOCH + Duration::from_secs(secs);
        let now = mk(1000);
        let min_age = Duration::from_secs(100);
        // victim.log is the OLDEST by mtime (frozen at SIGKILL) but RECENT in
        // absolute terms (age 50s < min_age) — it must survive. Genuinely old
        // logs (age >= min_age) past `keep` are still pruned.
        let entries = vec![
            (PathBuf::from("old1.log"), mk(100)),   // age 900 -> prune
            (PathBuf::from("old2.log"), mk(200)),   // age 800 -> prune
            (PathBuf::from("victim.log"), mk(950)), // age  50 -> RETAIN (the fix)
            (PathBuf::from("fresh.log"), mk(990)),  // newest -> within `keep`
        ];
        let removed = stale_to_remove(entries, 1, now, min_age);
        assert!(
            !removed.contains(&PathBuf::from("victim.log")),
            "recent crash victim must survive the prune despite oldest mtime"
        );
        assert!(!removed.contains(&PathBuf::from("fresh.log")));
        assert!(removed.contains(&PathBuf::from("old1.log")));
        assert!(removed.contains(&PathBuf::from("old2.log")));
        assert_eq!(removed.len(), 2);
    }

    #[test]
    fn prune_stale_removes_oldest_files() {
        let dir = tempfile::tempdir().unwrap();
        // Create 4 files with strictly increasing mtimes via filetime-free
        // approach: write, then bump mtime by re-touching is unreliable, so we
        // assert the count-based contract (≤keep ⇒ noop; >keep ⇒ shrinks).
        for i in 0..4 {
            std::fs::write(dir.path().join(format!("{i}.log")), b"x").unwrap();
        }
        prune_stale(dir.path(), 2, Duration::ZERO);
        let remaining = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(remaining, 2, "prune_stale must retain exactly `keep` files");
    }

    #[test]
    fn read_self_memory_does_not_panic() {
        // On Linux this returns real values; elsewhere zeros. Either way: no panic.
        let _ = read_self_memory_kb();
    }
}
