//! Resolve a bug file's `claimed_by` sessionId against this machine's Claude Code
//! session registries.
//!
//! **Why this exists.** `status: taken` asserts that a live session holds a bug. An
//! assertion nothing re-checks is `issue-clusters:IC-8`, so the claim is backed by a
//! sessionId that resolves — or fails to — against `$HOME/.claude*/sessions/*.json`.
//!
//! **A registry row is not liveness.** Measured 2026-09-02 on this machine: 42 registry
//! files across three profiles against 29 live sockets. Rows outlive their sessions, so
//! liveness is a three-part conjunction — socket present, process present, and
//! `procStart` equal to `/proc/<pid>/stat` field 22. The third defeats **pid reuse**;
//! without it a recycled pid reports live, which is the wrong answer in the dangerous
//! direction because it tells a reader to stay off work nobody is doing.
//!
//! **Compare `procStart` as STRINGS.** Verified 2026-09-02 against pid 2414613: field 22
//! read `79345929` and the registry read `79345929` — equal bytes, no unit conversion.
//! A numeric parse with any tolerance would reintroduce the collision this field closes.
//!
//! All process and socket access goes through [`ProcProbe`] so tests need no real
//! sessions; `load` takes its directories as a parameter for the same reason.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One row of `$CLAUDE_CONFIG_DIR/sessions/<pid>.json`.
///
/// Deliberately partial: the file carries `peerFeatures`, `version`, `status` and more,
/// none of which this module reads. `#[serde(default)]` on every optional field so a
/// future harness key, or an older row missing one, does not fail the parse.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionRow {
    pub pid: i64,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<String>,
    /// Boot-relative process start time in clock ticks — `/proc/<pid>/stat` field 22.
    #[serde(default, rename = "procStart")]
    pub proc_start: Option<String>,
    #[serde(default, rename = "messagingSocketPath")]
    pub messaging_socket_path: Option<String>,
    /// Registry-minted and re-minted by compaction/resume. Shown for humans, NEVER
    /// stored in frontmatter and never used to identify a session.
    #[serde(default)]
    pub name: Option<String>,
}

/// Why a resolved-but-not-live claim is dead. Split because the remedies read
/// differently to a human triaging the report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeadReason {
    /// The messaging socket is gone — the ordinary shape of an exited session.
    SocketAbsent,
    /// No such process.
    ProcessGone,
    /// A process with that pid exists but started at a different time: the pid was
    /// recycled onto a stale row.
    PidReused,
}

/// The outcome of resolving one `claimed_by`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimLiveness {
    Live {
        pid: i64,
        name: Option<String>,
        cwd: Option<String>,
        socket: String,
        /// Ready to paste into `SendMessage(to: …)`.
        address: String,
    },
    Dead {
        pid: i64,
        name: Option<String>,
        reason: DeadReason,
    },
    /// The sessionId is in none of the registries searched. **Not a defect** — on any
    /// other machine every foreign claim lands here.
    UnresolvableHere { profiles_searched: Vec<String> },
}

/// Filesystem and process facts, injected so tests are deterministic.
pub trait ProcProbe {
    /// `/proc/<pid>/stat` field 22 (`starttime`), verbatim, or `None` if no such process.
    fn starttime(&self, pid: i64) -> Option<String>;
    /// Whether a unix socket exists at `path`.
    fn socket_exists(&self, path: &str) -> bool;
}

/// The `/proc/<pid>/stat` field-22 parse, pulled out of `RealProcProbe::starttime` so
/// it can be tested directly against fixture lines without touching the filesystem.
/// See `RealProcProbe::starttime` for the field-counting rationale.
fn starttime_from_stat_line(line: &str) -> Option<String> {
    // Field 2 (comm) may contain spaces and parentheses, so split after the LAST
    // ')' rather than tokenising the whole line — the standard /proc/stat parse.
    let tail = &line[line.rfind(')')? + 1..];
    // After comm, fields are: state(3) ppid(4) ... starttime(22).
    // tail starts at field 3, so starttime is index 19 of the remainder.
    tail.split_whitespace().nth(19).map(str::to_string)
}

/// The real probe. Linux-only by construction; on other platforms `starttime` returns
/// `None`, which surfaces as `ProcessGone` rather than a wrong `Live`.
pub struct RealProcProbe;

impl ProcProbe for RealProcProbe {
    fn starttime(&self, pid: i64) -> Option<String> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        starttime_from_stat_line(&stat)
    }

    fn socket_exists(&self, path: &str) -> bool {
        Path::new(path).exists()
    }
}

/// Every `<home>/.claude*/sessions` directory that exists.
///
/// Discovered rather than hardcoded: this machine runs three profiles today and the set
/// is per-machine, so a fixed list would be quietly wrong on any other host.
pub fn default_profile_dirs(home: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(home) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n == ".claude" || n.starts_with(".claude-"))
        })
        .map(|e| e.path().join("sessions"))
        .filter(|p| p.is_dir())
        .collect();
    out.sort();
    out
}

/// Every session row this machine can see, plus the directories they came from.
pub struct SessionRegistry {
    rows: Vec<SessionRow>,
    profiles_searched: Vec<String>,
}

impl SessionRegistry {
    /// Read every `*.json` under each directory. Best-effort per file: an unreadable or
    /// malformed row is skipped, never fatal — one corrupt file must not make every
    /// claim on the machine unresolvable.
    pub fn load(profile_dirs: &[PathBuf]) -> Self {
        let mut rows = Vec::new();
        let mut profiles_searched = Vec::new();
        for dir in profile_dirs {
            profiles_searched.push(dir.display().to_string());
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                if let Ok(row) = serde_json::from_str::<SessionRow>(&text) {
                    rows.push(row);
                }
            }
        }
        Self {
            rows,
            profiles_searched,
        }
    }

    pub fn resolve(&self, session_id: &str, probe: &dyn ProcProbe) -> ClaimLiveness {
        let matches: Vec<&SessionRow> = self
            .rows
            .iter()
            .filter(|r| r.session_id == session_id)
            .collect();
        if matches.is_empty() {
            return ClaimLiveness::UnresolvableHere {
                profiles_searched: self.profiles_searched.clone(),
            };
        }

        // Duplicate sessionIds across profiles are a documented reality on this
        // machine: a resumed/restarted session leaves a stale row under the old
        // profile and a live row under the new one, both keyed by the same
        // sessionId (CLAUDE.md § Observer Blindness: "`codescout-00` became
        // `codescout-cc` on a different profile and PID with its sessionId
        // unchanged"). The dangerous direction is a stale row shadowing a live
        // one, so: if ANY matching row resolves Live, report it. Otherwise
        // report the first matching row's Dead outcome — among several dead
        // rows the choice changes nothing a reader does (the remedy is "demote
        // to investigating" either way), so an arbitrary-but-documented pick
        // beats an invented recency heuristic.
        let mut first_dead = None;
        for row in &matches {
            let outcome = Self::resolve_one(row, probe);
            if matches!(outcome, ClaimLiveness::Live { .. }) {
                return outcome;
            }
            if first_dead.is_none() {
                first_dead = Some(outcome);
            }
        }
        first_dead.expect("matches is non-empty, so at least one Dead outcome was recorded")
    }
    /// Liveness for exactly one row — the socket/process/procStart conjunction.
    /// Never returns `UnresolvableHere`; that outcome belongs to `resolve`, which
    /// knows whether any row matched at all.
    fn resolve_one(row: &SessionRow, probe: &dyn ProcProbe) -> ClaimLiveness {
        let dead = |reason| ClaimLiveness::Dead {
            pid: row.pid,
            name: row.name.clone(),
            reason,
        };

        // 1. socket
        let Some(socket) = row.messaging_socket_path.clone() else {
            return dead(DeadReason::SocketAbsent);
        };
        if !probe.socket_exists(&socket) {
            return dead(DeadReason::SocketAbsent);
        }
        // 2. process
        let Some(actual_start) = probe.starttime(row.pid) else {
            return dead(DeadReason::ProcessGone);
        };
        // 3. same process — string equality, no tolerance (see module docs).
        match &row.proc_start {
            Some(recorded) if recorded == &actual_start => ClaimLiveness::Live {
                pid: row.pid,
                name: row.name.clone(),
                cwd: row.cwd.clone(),
                address: format!("uds:{socket}"),
                socket,
            },
            // A row with no recorded start cannot rule out reuse, so it is not Live.
            _ => dead(DeadReason::PidReused),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Deterministic probe: no /proc, no sockets, no real sessions.
    struct FakeProbe {
        starttimes: HashMap<i64, String>,
        sockets: Vec<String>,
    }
    impl ProcProbe for FakeProbe {
        fn starttime(&self, pid: i64) -> Option<String> {
            self.starttimes.get(&pid).cloned()
        }
        fn socket_exists(&self, path: &str) -> bool {
            self.sockets.iter().any(|s| s == path)
        }
    }

    /// Writes one profile dir holding one registry row, and returns the dir.
    fn seed_profile(root: &std::path::Path, profile: &str, json: &str) -> std::path::PathBuf {
        let dir = root.join(profile).join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("1234.json"), json).unwrap();
        dir
    }

    fn row(session_id: &str, pid: i64, proc_start: &str) -> String {
        format!(
            r#"{{"pid":{pid},"sessionId":"{session_id}","cwd":"/repo",
                 "procStart":"{proc_start}","messagingSocketPath":"/sock/{pid}.sock",
                 "name":"codescout-aa","nameSource":"derived"}}"#
        )
    }

    #[test]
    fn a_running_session_with_matching_procstart_is_live() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = seed_profile(tmp.path(), ".claude", &row("sid-live", 4242, "79345929"));
        let reg = SessionRegistry::load(&[dir]);
        let probe = FakeProbe {
            starttimes: HashMap::from([(4242, "79345929".to_string())]),
            sockets: vec!["/sock/4242.sock".to_string()],
        };
        match reg.resolve("sid-live", &probe) {
            ClaimLiveness::Live { pid, .. } => assert_eq!(pid, 4242),
            other => panic!("expected Live, got {other:?}"),
        }
    }

    /// The pid-reuse guard. This is THE test the production path must not pass
    /// without the `procStart` comparison — see Step 6.
    #[test]
    fn a_reused_pid_with_a_different_starttime_is_dead_not_live() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = seed_profile(tmp.path(), ".claude", &row("sid-reused", 4242, "79345929"));
        let reg = SessionRegistry::load(&[dir]);
        let probe = FakeProbe {
            // Process exists and socket exists, but it is a DIFFERENT process
            // that happened to get the same pid.
            starttimes: HashMap::from([(4242, "99999999".to_string())]),
            sockets: vec!["/sock/4242.sock".to_string()],
        };
        assert!(
            matches!(
                reg.resolve("sid-reused", &probe),
                ClaimLiveness::Dead {
                    reason: DeadReason::PidReused,
                    ..
                }
            ),
            "a recycled pid must not report Live"
        );
    }

    #[test]
    fn a_missing_socket_is_dead() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = seed_profile(tmp.path(), ".claude", &row("sid-nosock", 7, "5"));
        let reg = SessionRegistry::load(&[dir]);
        let probe = FakeProbe {
            starttimes: HashMap::from([(7, "5".to_string())]),
            sockets: vec![],
        };
        assert!(matches!(
            reg.resolve("sid-nosock", &probe),
            ClaimLiveness::Dead {
                reason: DeadReason::SocketAbsent,
                ..
            }
        ));
    }

    #[test]
    fn a_gone_process_is_dead() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = seed_profile(tmp.path(), ".claude", &row("sid-gone", 9, "5"));
        let reg = SessionRegistry::load(&[dir]);
        let probe = FakeProbe {
            starttimes: HashMap::new(),
            sockets: vec!["/sock/9.sock".to_string()],
        };
        assert!(matches!(
            reg.resolve("sid-gone", &probe),
            ClaimLiveness::Dead {
                reason: DeadReason::ProcessGone,
                ..
            }
        ));
    }

    /// The third bucket. On another machine EVERY foreign claim lands here, so
    /// collapsing it into Dead is the confident wrong answer the design forbids.
    #[test]
    fn an_unknown_session_id_is_unresolvable_and_names_the_profiles_searched() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = seed_profile(tmp.path(), ".claude", &row("sid-other", 1, "1"));
        let reg = SessionRegistry::load(&[dir]);
        let probe = FakeProbe {
            starttimes: HashMap::new(),
            sockets: vec![],
        };
        match reg.resolve("sid-not-here", &probe) {
            ClaimLiveness::UnresolvableHere { profiles_searched } => {
                assert_eq!(profiles_searched.len(), 1);
                assert!(profiles_searched[0].contains(".claude"));
            }
            other => panic!("expected UnresolvableHere, got {other:?}"),
        }
    }

    #[test]
    fn rows_are_collected_across_several_profile_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let a = seed_profile(tmp.path(), ".claude", &row("sid-a", 1, "1"));
        let b = seed_profile(tmp.path(), ".claude-kat", &row("sid-b", 2, "2"));
        let reg = SessionRegistry::load(&[a, b]);
        let probe = FakeProbe {
            starttimes: HashMap::from([(2, "2".to_string())]),
            sockets: vec!["/sock/2.sock".to_string()],
        };
        assert!(matches!(
            reg.resolve("sid-b", &probe),
            ClaimLiveness::Live { .. }
        ));
    }
    /// Duplicate sessionIds across profiles are a documented reality on this machine: a
    /// resumed/restarted session leaves a stale row under the old profile and a live row
    /// under the new one, both keyed by the same sessionId (CLAUDE.md § Observer
    /// Blindness: "`codescout-00` became `codescout-cc` on a different profile and PID
    /// with its sessionId unchanged"). The stale row sorts first in load order here —
    /// it must not shadow the live one.
    #[test]
    fn a_duplicate_session_id_prefers_a_live_row_over_an_earlier_stale_one() {
        let tmp = tempfile::tempdir().unwrap();
        // Stale row loads FIRST: no socket, no process — Dead if resolved alone.
        let stale = seed_profile(tmp.path(), ".claude", &row("sid-dup", 111, "1"));
        // Live row loads SECOND, same sessionId, a different (correct, running) pid.
        let live = seed_profile(tmp.path(), ".claude-kat", &row("sid-dup", 222, "2"));
        let reg = SessionRegistry::load(&[stale, live]);
        let probe = FakeProbe {
            starttimes: HashMap::from([(222, "2".to_string())]),
            sockets: vec!["/sock/222.sock".to_string()],
        };
        match reg.resolve("sid-dup", &probe) {
            ClaimLiveness::Live { pid, .. } => assert_eq!(pid, 222),
            other => panic!(
                "expected Live (the live row must not be shadowed by the earlier stale one), got {other:?}"
            ),
        }
    }

    /// A malformed row must not poison the whole registry.
    #[test]
    fn an_unparseable_row_is_skipped_not_fatal() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".claude").join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bad.json"), "{ not json").unwrap();
        std::fs::write(dir.join("good.json"), row("sid-good", 3, "3")).unwrap();
        let reg = SessionRegistry::load(&[dir]);
        let probe = FakeProbe {
            starttimes: HashMap::from([(3, "3".to_string())]),
            sockets: vec!["/sock/3.sock".to_string()],
        };
        assert!(matches!(
            reg.resolve("sid-good", &probe),
            ClaimLiveness::Live { .. }
        ));
    }

    #[test]
    fn a_live_claim_carries_a_pasteable_address() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = seed_profile(tmp.path(), ".claude", &row("sid-addr", 55, "5"));
        let reg = SessionRegistry::load(&[dir]);
        let probe = FakeProbe {
            starttimes: HashMap::from([(55, "5".to_string())]),
            sockets: vec!["/sock/55.sock".to_string()],
        };
        match reg.resolve("sid-addr", &probe) {
            ClaimLiveness::Live { address, .. } => {
                assert_eq!(address, "uds:/sock/55.sock");
            }
            other => panic!("expected Live, got {other:?}"),
        }
    }

    #[test]
    fn default_profile_dirs_globs_dot_claude_variants() {
        let tmp = tempfile::tempdir().unwrap();
        for p in [".claude", ".claude-kat", ".claude-sdd", ".not-claude"] {
            std::fs::create_dir_all(tmp.path().join(p).join("sessions")).unwrap();
        }
        let dirs = default_profile_dirs(tmp.path());
        assert_eq!(dirs.len(), 3, "got {dirs:?}");
        assert!(dirs
            .iter()
            .all(|d| !d.to_string_lossy().contains("not-claude")));
    }
    /// The plan-mandated `/proc/<pid>/stat` field-22 parse, exercised directly against a
    /// realistic single-line fixture — no probe, no filesystem.
    #[test]
    fn starttime_from_stat_line_parses_a_normal_line() {
        let line =
            "2414613 (codescout) S 1 2414613 2414613 0 -1 4194304 100 0 5 0 20 3 0 0 20 0 4 0 \
                79345929 500000000 1000 18446744073709551615 1 1 0 0 0 0 0 0 0 0 0 0 17 17 0 0 0 \
                0 0 0 0 0 0 0 0 0 0 0";
        assert_eq!(starttime_from_stat_line(line), Some("79345929".to_string()));
    }

    /// A `comm` containing both a space and a `)` — the exact hazard the parse's own
    /// comment names. The fixture is chosen so a naive `line.split_whitespace().nth(21)`
    /// (tokenising the whole line as if `comm` held no whitespace) lands on the WRONG
    /// field ("17", one of the filler values) instead of the real starttime
    /// ("999000111"). Asserting that first proves this fixture discriminates the
    /// correct `rfind(')')`-based parse from the naive one — a fixture that agrees
    /// under both implementations would not be coverage.
    #[test]
    fn starttime_from_stat_line_handles_a_comm_with_a_space_and_a_paren() {
        let line =
            "1234 (weird ) name) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 999000111 111 222";
        assert_eq!(
            line.split_whitespace().nth(21),
            Some("17"),
            "fixture does not discriminate: the naive whole-line split must disagree with the \
         correct parse for this test to prove anything"
        );
        assert_eq!(
            starttime_from_stat_line(line),
            Some("999000111".to_string())
        );
    }
}
