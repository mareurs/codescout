---
id: cd8b516907b6d8df
kind: plan
status: done
title: Bug-Claim Liveness (`taken` state) Implementation Plan
owners:
- marius
tags:
- bug-tracking
- status-vocabulary
- peer-sessions
- doctor
- liveness
topic: bug claim liveness
---

# Bug-Claim Liveness (`taken` state) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a seventh bug status, `taken`, that records a live session's claim on a bug via its `sessionId`, plus a `doctor` check that resolves that claim against the machine's session registries so a dead claim cannot masquerade as active.

**Architecture:** `taken` is distinguished from `investigating` by *liveness-backing*: a `taken` record carries `claimed_by` (a sessionId) which must resolve to a running session, and decays to `investigating` when it does not. A new pure module resolves a sessionId against `$HOME/.claude*/sessions/*.json` and a process probe; a new `scan_claim_liveness` in `doctor.rs` consumes it and reports three buckets. All process/filesystem access sits behind a trait so tests need no real sessions.

**Tech Stack:** Rust, `rusqlite` (catalog), `serde_json` (registry rows), `regex`, inline `#[cfg(test)]` modules.

**Spec:** [`docs/superpowers/plans/2026-09-02-bug-claim-liveness-design.md`](2026-09-02-bug-claim-liveness-design.md) — read it first; this plan argues from it and does not restate its reasoning.

## Global Constraints

- **Gate, in this exact order, chained with `;` and never `&&`:**
  `cargo fmt` ; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` ; `cargo test --workspace --no-default-features` ; `cargo test --workspace`. The lean lane runs third and the default lane last because the default lane rebuilds `target/debug/codescout` with the `librarian` feature; `&&` would skip that rebuild exactly when a failure occurred. Read the exit codes rather than short-circuiting.
- **Branch:** all work on `experiments`. `master` is protected.
- **Store the sessionId and nothing else** in frontmatter — never the session name (`nameSource: "derived"`; re-minted by compaction/resume), never the pid, never the socket path. All three are derived at read time.
- **`claimed_by` / `claimed_at` live in the artifact's `extra` map.** They are YAML frontmatter, round-trip-safe, and deliberately **not** catalog-indexed — so `find(filter={"claimed_by": …})` will not work and must not be written into any doc.
- **`claimed_at` is informational only.** It is never an input to the liveness decision. Do not add a TTL.
- **Three buckets, never two.** `claim_unresolvable_here` is a first-class outcome, not an error — on any second machine every claim made elsewhere lands there, and folding it into "dead" is a confident wrong answer at scale.
- **Report-only.** No `fix=` for this check. Releasing a claim is a judgement about whether the work stands.
- **Every new bug file needs exactly one `cluster/<slug>` tag** written through the catalog (`artifact(action="update", …, patch={tags:[…]})`), never a raw frontmatter edit — a direct edit does not reach the catalog (BL-48).

---

## File Structure

| File | Responsibility |
|---|---|
| `src/librarian/session_registry.rs` | **Create.** Pure resolution of a sessionId → `ClaimLiveness`. Owns the registry-row shape, the three-part liveness conjunction, and the `to:` address derivation. All I/O behind `ProcProbe`. |
| `src/librarian/mod.rs` | **Modify.** Declare `pub mod session_registry;`. |
| `src/librarian/tools/create.rs:81-88` | **Modify.** `BUG_STATUSES` gains `"taken"`. |
| `src/librarian/tools/doctor.rs` | **Modify.** New `scan_claim_liveness`; register it; widen `scan_non_terminal_status_with_fix_anchor`'s SQL and message. |
| `src/prompts/guides/tracker-conventions.md` | **Modify.** Status table + 3 triage-query spots. |
| `src/prompts/guides/project-activation-bootstrap.md` | **Modify.** Triage query. |
| `src/prompts/guides/librarian.md:286` | **Modify.** Name the new check on the `doctor` row. |
| `docs/issues/_TEMPLATE.md` | **Modify.** Triage query + status glossary. |
| `CLAUDE.md` | **Modify.** § *Querying active trackers* triage query. |
| `src/librarian/tools/librarian.rs:30` | **Modify.** `Librarian/description` — name the new check. Reach it with `edit_code`, not `edit_file`. |
| `docs/PROBES.md:173` | **Modify.** The `doctor` row. |

**Task order rationale:** Task 1 is pure and has no dependents' risk. Task 2 makes `taken` *representable and reachable* — the IC-3 failure mode — before anything writes one. Task 3 is the prose half of Task 2 and is split only because a reviewer could reject the wording while accepting the code. Task 4 consumes Tasks 1+2. Task 5 makes Task 4 discoverable.

---

### Task 1: Session-registry resolution

**Files:**
- Create: `src/librarian/session_registry.rs`
- Modify: `src/librarian/mod.rs` (add `pub mod session_registry;` after `pub mod preview;`)
- Test: inline `#[cfg(test)]` module in `src/librarian/session_registry.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces:
  - `pub struct SessionRegistry`
  - `pub fn SessionRegistry::load(profile_dirs: &[PathBuf]) -> SessionRegistry`
  - `pub fn SessionRegistry::resolve(&self, session_id: &str, probe: &dyn ProcProbe) -> ClaimLiveness`
  - `pub fn default_profile_dirs(home: &Path) -> Vec<PathBuf>`
  - `pub trait ProcProbe { fn starttime(&self, pid: i64) -> Option<String>; fn socket_exists(&self, path: &str) -> bool; }`
  - `pub struct RealProcProbe;`
  - `pub enum ClaimLiveness { Live { pid, name, cwd, socket, address }, Dead { pid, name, reason: DeadReason }, UnresolvableHere { profiles_searched: Vec<String> } }`
  - `pub enum DeadReason { SocketAbsent, ProcessGone, PidReused }`

- [ ] **Step 1: Write the failing tests**

Create `src/librarian/session_registry.rs` with the test module only (the code above it comes in Step 3):

```rust
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
                ClaimLiveness::Dead { reason: DeadReason::PidReused, .. }
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
            ClaimLiveness::Dead { reason: DeadReason::SocketAbsent, .. }
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
            ClaimLiveness::Dead { reason: DeadReason::ProcessGone, .. }
        ));
    }

    /// The third bucket. On another machine EVERY foreign claim lands here, so
    /// collapsing it into Dead is the confident wrong answer the design forbids.
    #[test]
    fn an_unknown_session_id_is_unresolvable_and_names_the_profiles_searched() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = seed_profile(tmp.path(), ".claude", &row("sid-other", 1, "1"));
        let reg = SessionRegistry::load(&[dir.clone()]);
        let probe = FakeProbe { starttimes: HashMap::new(), sockets: vec![] };
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
        assert!(matches!(reg.resolve("sid-b", &probe), ClaimLiveness::Live { .. }));
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
        assert!(matches!(reg.resolve("sid-good", &probe), ClaimLiveness::Live { .. }));
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
        assert!(dirs.iter().all(|d| !d.to_string_lossy().contains("not-claude")));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --workspace session_registry`
Expected: FAIL — the module does not compile (`cannot find type SessionRegistry`, `ProcProbe`, etc.).

- [ ] **Step 3: Write the implementation**

Insert above the test module in `src/librarian/session_registry.rs`:

```rust
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

/// The real probe. Linux-only by construction; on other platforms `starttime` returns
/// `None`, which surfaces as `ProcessGone` rather than a wrong `Live`.
pub struct RealProcProbe;

impl ProcProbe for RealProcProbe {
    fn starttime(&self, pid: i64) -> Option<String> {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // Field 2 (comm) may contain spaces and parentheses, so split after the LAST
        // ')' rather than tokenising the whole line — the standard /proc/stat parse.
        let tail = &stat[stat.rfind(')')? + 1..];
        // After comm, fields are: state(3) ppid(4) ... starttime(22).
        // tail starts at field 3, so starttime is index 19 of the remainder.
        tail.split_whitespace().nth(19).map(str::to_string)
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
        Self { rows, profiles_searched }
    }

    pub fn resolve(&self, session_id: &str, probe: &dyn ProcProbe) -> ClaimLiveness {
        let Some(row) = self.rows.iter().find(|r| r.session_id == session_id) else {
            return ClaimLiveness::UnresolvableHere {
                profiles_searched: self.profiles_searched.clone(),
            };
        };

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
```

Then add to `src/librarian/mod.rs`, immediately after the `pub mod preview;` line:

```rust
pub mod session_registry;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --workspace session_registry`
Expected: PASS, 9 tests.

- [ ] **Step 5: Check `tempfile` is available as a dev-dependency**

Run: `grep -n 'tempfile' Cargo.toml`
Expected: a line under `[dev-dependencies]`. If absent, add `tempfile = "3"` there and re-run Step 4.

- [ ] **Step 6: Prove the pid-reuse guard is load-bearing (observed RED, production path)**

This is the mutation the design calls for. Temporarily change the `match` arm in `resolve` to ignore `procStart`:

```rust
        // MUTATION — revert after observing the failure
        match &row.proc_start {
            _ => ClaimLiveness::Live {
                pid: row.pid,
                name: row.name.clone(),
                cwd: row.cwd.clone(),
                address: format!("uds:{socket}"),
                socket,
            },
        }
```

Run: `cargo test --workspace session_registry`
Expected: **FAIL** at `a_reused_pid_with_a_different_starttime_is_dead_not_live` with `a recycled pid must not report Live`.

If it PASSES, the test is not discriminating — stop and fix the test before continuing. **Then revert the mutation** and re-run to confirm green.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt` ; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` ; `cargo test --workspace --no-default-features` ; `cargo test --workspace`
Expected: all four clean. Note `session_registry` is inside `#[cfg(feature = "librarian")]`'s module tree — confirm the lean lane still compiles.

- [ ] **Step 8: Commit**

```bash
git add src/librarian/session_registry.rs src/librarian/mod.rs
git commit -m "feat(librarian): resolve a sessionId to live/dead/unresolvable

Backs the forthcoming \`taken\` bug status. Liveness is socket + process +
procStart, the third defeating pid reuse (42 registry rows vs 29 live
sockets on this machine, so reuse is not hypothetical). Three buckets, not
two: a claim made on another host is unresolvable-here, never dead."
```

---

### Task 2: `taken` is representable and reachable

**Files:**
- Modify: `src/librarian/tools/create.rs:81-88` (`BUG_STATUSES`)
- Modify: `src/librarian/tools/doctor.rs:5325` (SQL in `scan_non_terminal_status_with_fix_anchor`)
- Modify: `src/librarian/tools/doctor.rs:4453` (message text in `scan_terminal_status_with_caveat`)
- Test: inline `#[cfg(test)]` in both files

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: the string `"taken"` as a valid bug status; `scan_non_terminal_status_with_fix_anchor` treats it as non-terminal.

**Why this task exists separately, and why it comes before anything writes a claim:** a status the canonical triage query does not list is invisible. Shipping `taken` without this is a fresh instance of `issue-clusters:IC-3` in a repo that gates against that class.

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` in `src/librarian/tools/create.rs`:

```rust
    #[test]
    fn taken_is_an_accepted_bug_status() {
        assert!(
            BUG_STATUSES.contains(&"taken"),
            "`taken` must be creatable; BUG_STATUSES = {BUG_STATUSES:?}"
        );
    }
```

Add to the `#[cfg(test)] mod tests` in `src/librarian/tools/doctor.rs`. Note the real
scaffolding: these tests are `#[tokio::test] async fn`, build the catalog with
`Catalog::open_in_memory()`, hand it to `ctx_rooted_at(cat, &root)` **by value**, and then
reach the connection back through `ctx.catalog.lock()`. `FIXTURE_PATCH_ID` and
`git_fixture_with_commit()` already exist in that module.

```rust
    /// The IC-3 guard for this feature: a `taken` bug whose body declares a patch-id
    /// owes a status flip exactly as `open` and `investigating` do. If the SQL in
    /// `scan_non_terminal_status_with_fix_anchor` is not widened, this is silent.
    #[tokio::test]
    async fn non_terminal_status_with_fix_anchor_covers_taken() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        let body = format!("## Fix\n\nLanded. patch-id `{FIXTURE_PATCH_ID}`.");
        seed_live_bug(&cat, &root, "claimed-but-fixed", "taken", "claimed_by: sid-x\n", &body);
        let ctx = ctx_rooted_at(cat, &root);

        let v = {
            let cat = ctx.catalog.lock();
            scan_non_terminal_status_with_fix_anchor(&ctx, &cat.conn).unwrap()
        };
        assert_eq!(v.len(), 1, "a `taken` record with a fix anchor must be reported: {v:#?}");
        assert_eq!(v[0].artifact_id.as_deref(), Some("claimed-but-fixed"));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --workspace taken_is_an_accepted_bug_status non_terminal_status_with_fix_anchor_covers_taken`
Expected: both FAIL — the first on the `contains` assert, the second with `v.len() == 0`.

- [ ] **Step 3: Widen `BUG_STATUSES`**

In `src/librarian/tools/create.rs`, replace the constant:

```rust
/// The seven statuses a `kind: bug` file may carry, per
/// `get_guide("tracker-conventions")` § *Bug files*.
///
/// `taken` and `investigating` differ by **liveness-backing**: `taken` carries a
/// `claimed_by` sessionId that `doctor`'s `claim_liveness` check resolves against the
/// machine's session registries, and decays to `investigating` when that session is
/// gone. `investigating` therefore means "worked, no live owner" — the residue of an
/// unconcluded claim — rather than a second word for the same state.
const BUG_STATUSES: &[&str] = &[
    "open",
    "taken",
    "investigating",
    "fixed",
    "mitigated",
    "wontfix",
    "zombie",
];
```

- [ ] **Step 4: Widen the doctor SQL and message**

In `src/librarian/tools/doctor.rs`, in `scan_non_terminal_status_with_fix_anchor`, change the prepared statement:

```rust
        let mut stmt = conn.prepare(
            "SELECT id, abs_path, status FROM artifact \
             WHERE kind = 'bug' AND status IN ('open', 'taken', 'investigating') \
             ORDER BY abs_path",
        )?;
```

In `scan_terminal_status_with_caveat`, update the message text so the named query matches reality:

```rust
                    "status is `{status}` (terminal) but `unverified:` is set, so the canonical \
                     triage query — kind=\"bug\" with status in open/taken/investigating — cannot \
                     reach this record. The caveat says: \"{shown}\". Either discharge it and \
                     clear the field, or leave both: the record stays honest AND findable, which \
                     is the whole point of the field. See get_guide(\"tracker-conventions\") § Bug files."
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --workspace taken_is_an_accepted_bug_status non_terminal_status_with_fix_anchor_covers_taken`
Expected: both PASS.

- [ ] **Step 6: Confirm the existing `zombie` exclusion still holds**

Run: `cargo test --workspace non_terminal_status_with_fix_anchor_ignores_zombie`
Expected: PASS. Widening the SQL must not have swept `zombie` in — its fix anchor is genuine and its non-terminal status is correct.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt` ; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` ; `cargo test --workspace --no-default-features` ; `cargo test --workspace`
Expected: all clean.

- [ ] **Step 8: Commit**

```bash
git add src/librarian/tools/create.rs src/librarian/tools/doctor.rs
git commit -m "feat(librarian): accept \`taken\` as a bug status and make it reachable

Seventh status. Widened in the same commit as every query that would
otherwise hide it — a status the canonical triage query omits is invisible,
which is IC-3 in a repo that gates against it."
```

---

### Task 3: The prose surfaces for `taken`

**Files:**
- Modify: `src/prompts/guides/tracker-conventions.md:31` (status table) and its 3 triage-query spots (~742-755)
- Modify: `src/prompts/guides/project-activation-bootstrap.md:11-13`
- Modify: `docs/issues/_TEMPLATE.md:21,41`
- Modify: `CLAUDE.md` § *Querying active trackers*

**Interfaces:**
- Consumes: the vocabulary fixed in Task 2.
- Produces: nothing code-facing.

**Split from Task 2 because** a reviewer can reasonably reject wording while accepting the code. Both must land before any bug file is written with `status: taken`.

- [ ] **Step 1: Update the status table in the guide**

In `src/prompts/guides/tracker-conventions.md`, replace the `investigating` row and add `taken` above it:

```markdown
| `open` | Logged, investigation not started or paused |
| `taken` | A **live** session holds this right now. Requires `claimed_by: <sessionId>`; `doctor`'s `claim_liveness` resolves it |
| `investigating` | Worked, but **no live owner** — the residue of a claim whose session ended without concluding |
```

- [ ] **Step 2: Add the claim protocol to the same guide**

Insert immediately after the status table:

```markdown
**Claiming a bug.** Set `taken` together with your own sessionId, through the catalog —
a raw frontmatter edit does not reach it (BL-48):

```
artifact(action="update", id=…,
         patch={"status": "taken",
                "extra": {"claimed_by": "<sessionId>", "claimed_at": "YYYY-MM-DD"}})
```

You already know your sessionId without any API call: the harness makes it a path
component of your scratchpad directory, `/tmp/claude-<uid>/<project>/<SESSION-ID>/scratchpad`.
Verified 2026-09-02 to be byte-identical to the `sessionId` in the session registry.

**Store the sessionId and nothing else.** Not the session name — that is registry-minted
with `nameSource: "derived"` and re-minted by compaction, resume, or a restart under
another profile, so a name frozen into frontmatter decays silently while the sessionId
does not. Not the pid or socket path either; both are derived at read time.

**Releasing** is the same call with `status: "investigating"` and
`extra: {"claimed_by": null, "claimed_at": null}` (a null value deletes the key).
Release to `investigating`, not `open` — work probably happened and the body records it.

`claimed_at` is informational. Liveness is decided by resolving `claimed_by`, never by
the age of the claim.
```

- [ ] **Step 3: Update all three triage queries in the guide**

Replace every occurrence of

```
filter={"status": {"in": ["open", "investigating", "zombie"]}}
```

with

```
filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}}
```

and update the surrounding prose so it reads:

```markdown
`status="open"` alone hides `taken` (a live session is on it — do not duplicate the
work), `investigating` (worked, no live owner) and `zombie` (recurring-but-unconfirmed).
```

Also update the narrower two-state form later in that section:

```
{"status": {"in": ["open", "taken", "investigating"]}}
```

- [ ] **Step 4: Update the activation bootstrap guide**

In `src/prompts/guides/project-activation-bootstrap.md`, replace the query and its gloss:

```markdown
  `artifact(action="find", kind="bug", filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})` —
  `status="open"` alone hides `taken` (a live session holds it — check before starting),
  `investigating` (worked, no live owner) and
  `zombie` (recurring-but-unconfirmed — a "has this come back?" check, not a
  task to pick up) —
```

- [ ] **Step 5: Update the bug template**

In `docs/issues/_TEMPLATE.md`, update the query at line ~21 and add to the status glossary at line ~41:

```
  taken         — A live session holds this right now. Requires
                  claimed_by: <sessionId> in frontmatter. Decays to
                  `investigating` when that session exits; run
                  librarian(action="doctor") to find dead claims.
  investigating — Worked, but no live owner. The residue of an
                  unconcluded claim, not a synonym for `taken`.
```

- [ ] **Step 6: Update CLAUDE.md**

In § *Querying active trackers*, replace the bug query and its following paragraph's first sentence:

```markdown
artifact(action="find", kind="bug", filter={"status": {"in": ["open", "taken", "investigating", "zombie"]}})
```

```markdown
`status="open"` alone hides `taken` (a live session holds it), `investigating` (worked,
no live owner) and `zombie` (recurring-but-unconfirmed — a "has this come back?" check,
not a task to pick up).
```

- [ ] **Step 7: Check the prompt-surface gates and the slice cap**

Run: `cargo test --workspace prompt_surfaces_reference_only_real_tools claude_md_contains_no_deprecated_tool_names claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`
Expected: PASS.

Then read `src/prompts/README.md` and confirm whether the `project-activation-bootstrap` edit crosses the **1900-character** slice cap and whether `ONBOARDING_VERSION` needs a bump (`.codescout/project.toml` records `onboarding_version = 29`). If a cap is exceeded, trim the gloss — not the query.

- [ ] **Step 8: Run the full gate**

Run: `cargo fmt` ; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` ; `cargo test --workspace --no-default-features` ; `cargo test --workspace`
Expected: all clean.

- [ ] **Step 9: Commit**

```bash
git add src/prompts/guides/tracker-conventions.md \
        src/prompts/guides/project-activation-bootstrap.md \
        docs/issues/_TEMPLATE.md CLAUDE.md
git commit -m "docs: teach every triage surface about \`taken\`

Five prose surfaces, same commit as the code that accepts the status.
Also states what \`investigating\` now means: worked, no live owner."
```

---

### Task 4: `scan_claim_liveness`

**Files:**
- Modify: `src/librarian/tools/doctor.rs` — new `scan_claim_liveness`, registered next to its siblings around `:419`
- Test: inline `#[cfg(test)]` in `src/librarian/tools/doctor.rs`

**Interfaces:**
- Consumes: `crate::librarian::session_registry::{SessionRegistry, ClaimLiveness, DeadReason, ProcProbe, RealProcProbe, default_profile_dirs}` (Task 1); the `taken` status (Task 2).
- Produces: `Violation.check` values `claim_held_by_live_session`, `claim_held_by_dead_session`, `claim_unresolvable_here`.

**All three buckets are reported, including the live one.** That is the design as approved:
a live claim's row carries the paste-ready `to:` address, which is the half of this feature
that answers *"who is on this and can I ask them?"*. It is the one `doctor` row that is
informational rather than a defect, so **its detail text must say so in its first clause** —
a reader who cannot tell it from a violation will learn to skim the whole check.

**Test scaffolding is real, not sketched.** These tests are `#[tokio::test] async fn`; build
the catalog with `Catalog::open_in_memory()`, pass it to `ctx_rooted_at(cat, &root)` **by
value**, then reach the connection back via `ctx.catalog.lock()`. `git_fixture_with_commit()`
and `seed_live_bug` (`doctor.rs:6879`) already exist.

- [ ] **Step 1: Write the failing tests**

```rust
    /// Seeds one profile dir holding one session row; returns the dir to pass in.
    fn seed_session(
        root: &std::path::Path,
        session_id: &str,
        pid: i64,
        proc_start: &str,
    ) -> std::path::PathBuf {
        let dir = root.join(".claude").join("sessions");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{pid}.json")),
            format!(
                r#"{{"pid":{pid},"sessionId":"{session_id}","cwd":"/repo",
                     "procStart":"{proc_start}","messagingSocketPath":"/sock/{pid}.sock",
                     "name":"codescout-zz"}}"#
            ),
        )
        .unwrap();
        dir
    }

    /// `live_pid: None` means nothing is running and no socket exists.
    struct TestProbe {
        live_pid: Option<i64>,
        starttime: String,
    }
    impl crate::librarian::session_registry::ProcProbe for TestProbe {
        fn starttime(&self, pid: i64) -> Option<String> {
            (Some(pid) == self.live_pid).then(|| self.starttime.clone())
        }
        fn socket_exists(&self, _path: &str) -> bool {
            self.live_pid.is_some()
        }
    }

    #[tokio::test]
    async fn a_live_claim_is_reported_informationally_with_a_pasteable_address() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(&cat, &root, "held", "taken", "claimed_by: sid-live\n", "body\n");
        let sessions = seed_session(&root, "sid-live", 4242, "555");
        let ctx = ctx_rooted_at(cat, &root);
        let probe = TestProbe { live_pid: Some(4242), starttime: "555".into() };

        let v = {
            let cat = ctx.catalog.lock();
            scan_claim_liveness(&ctx, &cat.conn, &[sessions], &probe).unwrap()
        };
        assert_eq!(v.len(), 1, "{v:#?}");
        assert_eq!(v[0].check, "claim_held_by_live_session");
        assert!(
            v[0].detail.contains("uds:/sock/4242.sock"),
            "must carry the paste-ready address: {}",
            v[0].detail
        );
        assert!(
            v[0].detail.to_lowercase().starts_with("informational"),
            "a non-defect row must announce itself as one: {}",
            v[0].detail
        );
    }

    #[tokio::test]
    async fn a_dead_claim_names_investigating_as_the_remedy() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(&cat, &root, "stale", "taken", "claimed_by: sid-dead\n", "body\n");
        let sessions = seed_session(&root, "sid-dead", 4242, "555");
        let ctx = ctx_rooted_at(cat, &root);
        let probe = TestProbe { live_pid: None, starttime: "555".into() };

        let v = {
            let cat = ctx.catalog.lock();
            scan_claim_liveness(&ctx, &cat.conn, &[sessions], &probe).unwrap()
        };
        assert_eq!(v.len(), 1, "{v:#?}");
        assert_eq!(v[0].check, "claim_held_by_dead_session");
        assert!(
            v[0].detail.contains("investigating"),
            "the remedy must name `investigating`, not `open`: {}",
            v[0].detail
        );
    }

    /// The third bucket must not collapse into the second — they need different fixes,
    /// and on another machine EVERY foreign claim is unresolvable rather than dead.
    #[tokio::test]
    async fn an_unresolvable_claim_is_its_own_check_and_names_the_scope() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(&cat, &root, "foreign", "taken", "claimed_by: sid-elsewhere\n", "body\n");
        let sessions = seed_session(&root, "sid-somebody-else", 1, "1");
        let ctx = ctx_rooted_at(cat, &root);
        let probe = TestProbe { live_pid: None, starttime: "1".into() };

        let v = {
            let cat = ctx.catalog.lock();
            scan_claim_liveness(&ctx, &cat.conn, &[sessions], &probe).unwrap()
        };
        assert_eq!(v.len(), 1, "{v:#?}");
        assert_eq!(
            v[0].check, "claim_unresolvable_here",
            "must NOT be reported as a dead claim"
        );
        assert!(
            v[0].detail.contains(".claude"),
            "a negative result must name the scope searched: {}",
            v[0].detail
        );
    }

    #[tokio::test]
    async fn a_taken_bug_with_no_claimed_by_is_reported() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(&cat, &root, "unclaimed", "taken", "", "body\n");
        let sessions = seed_session(&root, "sid-x", 1, "1");
        let ctx = ctx_rooted_at(cat, &root);
        let probe = TestProbe { live_pid: None, starttime: "1".into() };

        let v = {
            let cat = ctx.catalog.lock();
            scan_claim_liveness(&ctx, &cat.conn, &[sessions], &probe).unwrap()
        };
        assert_eq!(v.len(), 1, "{v:#?}");
        assert_eq!(v[0].check, "claim_unresolvable_here");
    }

    #[tokio::test]
    async fn non_taken_statuses_are_not_examined() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(&cat, &root, "a", "open", "", "body\n");
        seed_live_bug(&cat, &root, "b", "investigating", "claimed_by: sid-dead\n", "body\n");
        seed_live_bug(&cat, &root, "c", "fixed", "claimed_by: sid-dead\n", "body\n");
        let sessions = seed_session(&root, "sid-other", 1, "1");
        let ctx = ctx_rooted_at(cat, &root);
        let probe = TestProbe { live_pid: None, starttime: "1".into() };

        let v = {
            let cat = ctx.catalog.lock();
            scan_claim_liveness(&ctx, &cat.conn, &[sessions], &probe).unwrap()
        };
        assert!(v.is_empty(), "only `taken` is in scope: {v:#?}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --workspace claim`
Expected: FAIL — `cannot find function scan_claim_liveness`.

- [ ] **Step 3: Write the implementation**

Add to `src/librarian/tools/doctor.rs`, next to the other bug-record scans:

```rust
/// `claim_held_by_live_session` / `claim_held_by_dead_session` / `claim_unresolvable_here`:
/// resolves a `status: taken` bug file's `claimed_by` sessionId against this machine's
/// session registries.
///
/// **The check that makes `taken` more than an assertion.** A record claiming a live owner
/// with nothing re-checking it is `issue-clusters:IC-8`, and it fails in the costly
/// direction: it tells a reader to stay off work nobody is doing.
///
/// **Three outcomes, all reported, only two of them defects.** `claim_held_by_live_session`
/// is INFORMATIONAL and says so in its first word — it exists to carry the paste-ready
/// `to:` address, which is how a reader asks the holder rather than guessing.
/// `claim_held_by_dead_session`'s remedy is a demotion to `investigating` — *not* `open`,
/// because work probably happened and the body records it. `claim_unresolvable_here` is a
/// separate check rather than a flavour of dead: it needs a different fix, and on any
/// second machine EVERY foreign claim lands there, so reporting those as dead would be a
/// confident wrong answer at scale. Per
/// `docs/adrs/2026-08-27-negative-results-name-their-scope.md` it names the directories it
/// searched.
///
/// Reports only; there is no `fix=`. Releasing a claim is a judgement about whether the
/// work stands, which this check cannot make.
///
/// Registry dirs and the process probe are **parameters**, not ambient lookups, so tests
/// need no real sessions.
fn scan_claim_liveness(
    ctx: &ToolContext,
    conn: &rusqlite::Connection,
    profile_dirs: &[std::path::PathBuf],
    probe: &dyn crate::librarian::session_registry::ProcProbe,
) -> Result<Vec<Violation>> {
    use crate::librarian::session_registry::{ClaimLiveness, DeadReason, SessionRegistry};

    let Some(cp) = ctx.current_project.as_deref() else {
        return Ok(Vec::new());
    };
    let mut stmt = conn.prepare(
        "SELECT id, abs_path FROM artifact \
         WHERE kind = 'bug' AND status = 'taken' \
         ORDER BY abs_path",
    )?;
    let rows: Vec<(String, String)> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let registry = SessionRegistry::load(profile_dirs);
    let searched = profile_dirs
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let mut out = Vec::new();
    for (id, abs_path) in &rows {
        let path = Path::new(abs_path);
        if super::containing_root(std::slice::from_ref(&cp.git_root), path).is_none() {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        let claimed_by = match crate::librarian::frontmatter::parse(&content) {
            Ok((Some(fm), _)) => fm.extra.get("claimed_by").and_then(|v| match v {
                Value::String(s) => Some(s.trim().to_string()),
                Value::Null => None,
                other => Some(other.to_string()),
            }),
            _ => None,
        }
        .filter(|s| !s.is_empty());

        let Some(sid) = claimed_by else {
            out.push(Violation::new(
                "claim_unresolvable_here",
                Some(id.clone()),
                abs_path.clone(),
                "status is `taken` but no `claimed_by:` is set, so the claim names nobody and \
                 nothing can check it. Either add the claiming session's id, or demote to \
                 `investigating`. See get_guide(\"tracker-conventions\") § Bug files."
                    .to_string(),
            ));
            continue;
        };

        match registry.resolve(&sid, probe) {
            ClaimLiveness::Live { pid, name, cwd, address, .. } => {
                let who = name.unwrap_or_else(|| "unnamed".to_string());
                let where_ = cwd.unwrap_or_else(|| "unknown cwd".to_string());
                out.push(Violation::new(
                    "claim_held_by_live_session",
                    Some(id.clone()),
                    abs_path.clone(),
                    format!(
                        "informational, not a defect: session `{sid}` (pid {pid}, currently \
                         named `{who}`, working in {where_}) holds this bug and is running. \
                         Ask before starting — SendMessage(to: \"{address}\"). Use the id, not \
                         the name: a name is re-minted by compaction, resume, or a restart \
                         under another profile."
                    ),
                ));
            }
            ClaimLiveness::Dead { pid, name, reason } => {
                let why = match reason {
                    DeadReason::SocketAbsent => "its messaging socket is gone",
                    DeadReason::ProcessGone => "no such process is running",
                    DeadReason::PidReused => {
                        "a process with that pid exists but started at a different time, so the \
                         pid was recycled onto a stale row"
                    }
                };
                let who = name.unwrap_or_else(|| "unnamed".to_string());
                out.push(Violation::new(
                    "claim_held_by_dead_session",
                    Some(id.clone()),
                    abs_path.clone(),
                    format!(
                        "status is `taken`, claimed by session `{sid}` (pid {pid}, last known as \
                         `{who}`), but {why} — so the claim is unbacked and every triage query \
                         hands this out as work in progress. Demote to `investigating` (NOT \
                         `open`: work probably happened and the body records it) and clear \
                         `claimed_by`. This is a worklist, not a verdict — the session may have \
                         concluded without updating the record."
                    ),
                ));
            }
            ClaimLiveness::UnresolvableHere { .. } => {
                out.push(Violation::new(
                    "claim_unresolvable_here",
                    Some(id.clone()),
                    abs_path.clone(),
                    format!(
                        "status is `taken`, claimed by session `{sid}`, which appears in none of \
                         the session registries on this machine ({searched}). This is NOT \
                         evidence the claim is dead — a claim made on another host is \
                         unresolvable here by construction. Check on the claiming machine, or \
                         demote to `investigating` if the work has been abandoned."
                    ),
                ));
            }
        }
    }
    Ok(out)
}
```

- [ ] **Step 4: Register the check**

First confirm how `home` is already obtained in this crate:

Run: `grep -rn 'home_dir\|"HOME"' src/ --include='*.rs'`

Use whichever form already exists rather than adding a dependency. Then, immediately after
the `scan_unterminated_fence` line (~`:419`) in `call`, add:

```rust
    // The reader half of `status: taken`. Without this line the check compiles, its own
    // tests pass, and `librarian(action="doctor")` never calls it — which is exactly the
    // failure mode the status exists to make visible.
    all_violations.extend(scan_claim_liveness(
        ctx,
        &cat.conn,
        &crate::librarian::session_registry::default_profile_dirs(&home),
        &crate::librarian::session_registry::RealProcProbe,
    )?);
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --workspace claim`
Expected: 5 PASS.

- [ ] **Step 6: Prove the registration is load-bearing**

The five tests above call `scan_claim_liveness` **directly** and therefore cannot observe
Step 4 at all. Confirm that: comment out the `all_violations.extend(scan_claim_liveness(…))`
block and re-run `cargo test --workspace claim` — it still passes. That is precisely why
`declared-not-wired` is invisible, and why the next test is not optional.

**Restore the block**, then add a test that drives the real entry point:

```rust
    /// Guards the registration line, which no test above can observe. Deterministic on
    /// any machine: a sessionId this shape appears in no registry anywhere, so the
    /// unresolvable bucket is reached without depending on local sessions.
    #[tokio::test]
    async fn call_wires_scan_claim_liveness() {
        let (_tmp, root, _live) = git_fixture_with_commit();
        let cat = Catalog::open_in_memory().unwrap();
        seed_live_bug(
            &cat,
            &root,
            "e2e-claim",
            "taken",
            "claimed_by: sid-0000-no-such-session-anywhere\n",
            "body\n",
        );
        let ctx = ctx_rooted_at(cat, &root);

        let out = call(&ctx, json!({})).await.unwrap();

        assert_eq!(
            out["summary"]["by_check"]["claim_unresolvable_here"],
            json!(1),
            "call() must reach scan_claim_liveness; an unregistered check is silent: {out:#?}"
        );
    }
```

Run: `cargo test --workspace call_wires_scan_claim_liveness`
Expected: PASS. Then comment out the registration again and confirm this one **FAILS**;
restore it. An observed red here is the whole point of the test.

- [ ] **Step 7: Run the full gate**

Run: `cargo fmt` ; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` ; `cargo test --workspace --no-default-features` ; `cargo test --workspace`
Expected: all clean.

- [ ] **Step 8: Commit**

```bash
git add src/librarian/tools/doctor.rs
git commit -m "feat(doctor): resolve a \`taken\` claim against the session registries

Three buckets. Live is informational and carries the paste-ready SendMessage
address; dead names \`investigating\` as the remedy, never \`open\`;
unresolvable-here is its own check naming the profiles searched, because on
another machine every foreign claim lands there and calling those dead would
be a confident wrong answer at scale."
```
---

### Task 5: Make the new check discoverable

**Files:**
- Modify: `src/librarian/tools/librarian.rs:30` (`Librarian/description`) — via `edit_code`
- Modify: `src/prompts/guides/librarian.md:286`
- Modify: `docs/PROBES.md:173`

**Interfaces:**
- Consumes: the check names from Task 4.
- Produces: nothing code-facing.

**Why this is a task and not a footnote.** Measured 2026-09-02: `doctor` runs 23 `scan_*` checks and five surfaces describe it as a *catalog-drift* scanner over ~6. The newest check before this one, `non_terminal_status_with_fix_anchor`, appears on **none** of them. Following local precedent would ship `scan_claim_liveness` wired and undiscoverable. Filed as `docs/issues/archive/2026-09-02-doctor-doc-surfaces-describe-six-of-its-twenty-three-checks.md`; scouted in `bug-claim-liveness-session-log:F-1` and `F-2`.

**Scope boundary:** this task names the *new* check on three surfaces. Re-describing `doctor` by check family — the filed bug's option B — is **out of scope here** and stays with that bug, along with the two rustdoc module headers.

- [ ] **Step 1: Update the tool description**

Read the current string first — do not reconstruct it from this plan:

`symbols(name_path="Librarian/description", path="src/librarian/tools/librarian.rs", include_body=true)`

Then use `edit_code` (the string lives inside a symbol; `edit_file` is the wrong tool here)
to extend the `doctor:` clause so it reads:

```
doctor: catalog drift scanner (read-only by default): abs_path form, ADS colons, '..' \
segments, missing files; commits.git_root form; worktree-scoped rows; frontmatter id vs \
catalog id; and `claim_liveness` — a `status: taken` bug whose claiming session is gone \
or unresolvable on this machine.
```

- [ ] **Step 2: Update the librarian guide row**

In `src/prompts/guides/librarian.md:286`, extend the `doctor` row's *Measures* cell:

```markdown
| `doctor` | Read-only catalog drift scan (forward-slash form, NTFS ADS colons, `..` segments, missing-on-disk files, `abs_path_must_be_absolute`), plus bug-record checks including `claim_liveness` — a `status: taken` record whose `claimed_by` session is dead or unresolvable here. Manual — run after large refactors, before picking up claimed work, or when downstream LIKE queries return empty. Returns a per-check JSON report; does NOT mutate catalog state. |
```

- [ ] **Step 3: Update PROBES.md**

In `docs/PROBES.md:173`, extend the `doctor` row and add the blind spot that row currently lacks:

```markdown
| `librarian(action="doctor")` | Catalog drift: `abs_path` form, ADS colons, `..` segments, missing-on-disk files, worktree-scoped rows, frontmatter-id vs catalog-id mismatch. Also bug-record and statement-validity checks, including `claim_liveness` (a `status: taken` bug whose claiming session is gone) | Read-only unless you pass `fix=…`. Opt-in repairs are individually gated and dry-run by default. **`claim_liveness` is machine-local**: a claim made on another host reports `claim_unresolvable_here`, which is not evidence the claim is dead. **This row lists a subset** — `doctor` runs 23 checks; read the `scan_*` functions in `src/librarian/tools/doctor.rs` for the authoritative set |
```

- [ ] **Step 4: Verify the doc-ref audit is clean**

Run: `codescout audit-doc-refs --fail-on high`
Expected: exit 0. Many findings are false positives by design — read severities, not the count.

- [ ] **Step 5: Run the full gate**

Run: `cargo fmt` ; `cargo clippy --workspace --all-targets --features local-embed -- -D warnings` ; `cargo test --workspace --no-default-features` ; `cargo test --workspace`
Expected: all clean, including `prompt_surfaces_reference_only_real_tools`.

- [ ] **Step 6: Commit**

```bash
git add src/librarian/tools/librarian.rs src/prompts/guides/librarian.md docs/PROBES.md
git commit -m "docs: name claim_liveness on the three surfaces that route to doctor

The previous check shipped on none of them. PROBES.md's doctor row now also
says it lists a subset of 23 and names the machine-local blind spot."
```

---

## Out of scope, deliberately

- **IC class files.** A bug has one obvious unit of work; an `IC-N` class does not. Prove the shape on bug files first.
- **Auto-release of dead claims.** Report-only by design.
- **Re-describing `doctor` by check family.** Stays with `docs/issues/archive/2026-09-02-doctor-doc-surfaces-describe-six-of-its-twenty-three-checks.md`, including the two rustdoc headers.
- **A `claimed_by` catalog column.** `extra` is not indexed; `find(status="taken")` is the entry point, and per-claimer queries have no caller yet.
- **Same-profile bare-name addressing.** The spec's § *The messaging affordance* offers a bare `name` when the claimer shares the reader's profile and the `uds:` path otherwise. This plan emits the `uds:` form **always**, and the divergence is deliberate: `doctor` does not know which profile its *reader* is in, so the branch is not computable where the message is composed. Per `CLAUDE.md` § *Reaching a Peer Session* the socket path delivers in both cases while a bare name refuses cross-profile, so always-`uds:` is the strictly safer of the two. Revisit only if a same-profile reader finds the path form inconvenient — it is not wrong, just verbose.

## Closing the loop

After Task 5 lands and the gate is green on `experiments`:

- [x] Update `docs/superpowers/plans/2026-09-02-bug-claim-liveness-design.md` — mark the design implemented, citing the fix SHAs **and** their patch-ids (`git show <sha> | git patch-id --stable`). The SHA dies on the next rebase; the patch-id does not. — **done 2026-09-08**, `8f40eaad` / `e801f5e1` / `a9e37db2` with all three patch-ids.
- [x] Flip `bug-claim-liveness-session-log:F-1` to `fixed-verified` **in its Status line**, not only in prose — a firing recorded only in prose is invisible to every field-presence sweep. — **done 2026-09-08.**
- [ ] Leave `F-2` open: its lesson is not mechanised. The candidate mechanism — a test asserting every registered `scan_*` is named in `docs/PROBES.md` — would close it and the filed bug together, and is worth its own plan. — **left open, deliberately, 2026-09-08.** Still unmechanised.

> **Closed 2026-09-08 by a different session, six days after the code shipped, and the delay is
> the part worth keeping.** Tasks 1–5 landed 2026-09-02 (`8f40eaad`, `e801f5e1`, `a9e37db2`);
> this checklist was never reached because the session holding it exited — leaving all three
> of this stream's files, including this one, **untracked in the working tree**. So the record
> said `draft` for six days while `scan_claim_liveness` had been running in every `doctor` call
> the whole time, and no query could see the discrepancy because the files were not in git.
>
> **The reusable rule: a closing step that exists only in the worktree of the session that
> wrote it is not a step.** Commit the plan before you need the checklist, not after you finish
> it — the checklist's whole value is that it survives you, and an uncommitted one survives
> nobody. This is the same law as `OB-20`'s (a withheld commit and an unpushed one are the same
> bytes) applied to a file that was never staged at all.
