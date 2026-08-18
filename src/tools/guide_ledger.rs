//! Session-scoped guide-hint ledger with disk persistence.
//!
//! Tracks which `get_guide(topic)` topics have already been surfaced to the
//! model in the current Claude Code conversation. The set is persisted to
//! `.codescout/guide_hints/<session_id>.json` so it **survives MCP server
//! restarts** — a `/mcp` reconnect re-spawns the codescout process, which would
//! otherwise reborn an empty in-memory set and re-inject every guide body the
//! conversation already holds. Fix for
//! `docs/issues/archive/2026-06-14-get-guide-reinjects-on-mcp-restart.md`.
//!
//! Keyed by `CLAUDE_CODE_SESSION_ID` (set by Claude Code in the MCP subprocess
//! env since v2.1.154) — per-process, so concurrent CC windows on one project
//! get distinct files and never collide. A `Default`-constructed ledger is
//! ephemeral (no path → no persistence); that is what the many internal/test
//! `ToolContext` builders get for free, so they compile unchanged.

use chrono::{DateTime, TimeZone, Utc};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// Prune a ledger once it has been idle this long. Measured 2026-08-18 across
/// 258 sessions: 0.0% of live sessions would be pruned at 30 days (observed
/// maxima: 28.9-day lifespan, 27.0-day idle gap), so 35 gives six extra days
/// of headroom, at a cost of ~60 bytes per file. See
/// `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` § 8.
const GC_MAX_IDLE_DAYS: i64 = 35;

/// Topics emitted this session, each stamped with when it was delivered,
/// optionally backed by a per-session JSON file. Reads go through the in-memory
/// map; mutations write through.
///
/// The stamps are what make expiry (`expire_idle`) and garbage collection
/// expressible. The pre-2026-08-18 shape was a bare `Vec<String>`, which is still
/// read and migrated — see `read_entries`.
#[derive(Debug, Default)]
pub struct GuideLedger {
    /// Per-session file (`<dir>/<session_id>.json`). `None` ⇒ ephemeral.
    path: Option<PathBuf>,
    emitted: BTreeMap<String, DateTime<Utc>>,
    /// One-shot session notices that are NOT guide topics.
    ///
    /// Deliberately a SEPARATE set from `emitted`, and not persisted:
    ///
    /// - `emitted.is_empty()` is the session-opening guide's trigger in
    ///   `Tool::call_content`. A sentinel key stashed in `emitted` would make
    ///   that false and silently suppress `SESSION_OPENING_GUIDE` — for
    ///   exactly the sessions a notice fires in, since notices fire on the
    ///   first eligible call.
    /// - keeping it out of `emitted` also keeps it out of the topic
    ///   namespace, so a notice key can never collide with a future guide
    ///   topic, and out of the persisted JSON — which now carries a
    ///   `BTreeMap<String, DateTime<Utc>>`, so a notice key would need a
    ///   meaningless delivery timestamp and would be read back as a topic
    ///   by `read_entries`.
    ///
    /// Ephemeral by design: a notice describes this process's view of the
    /// tree, not something the model has been taught.
    notices: HashSet<String>,
}

/// Accepts both on-disk shapes. `untagged` is unambiguous here because a JSON
/// array can only match `Legacy` and a JSON object can only match `Stamped`.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum LedgerFile {
    Stamped(BTreeMap<String, DateTime<Utc>>),
    Legacy(Vec<String>),
}

impl GuideLedger {
    /// Load the persisted ledger for `session_id` under `dir`. Best-effort: a
    /// missing, unreadable or malformed file yields an empty set — degrading to
    /// re-sending a guide, never to suppressing one. `dir = None` ⇒ ephemeral.
    pub fn load(session_id: &str, dir: Option<PathBuf>) -> Self {
        let path = dir
            .as_ref()
            .map(|d| d.join(format!("{}.json", sanitize(session_id))));
        if let (Some(d), Some(p)) = (dir.as_deref(), path.as_deref()) {
            gc(d, p);
        }
        let emitted = path.as_deref().map(read_entries).unwrap_or_default();
        Self {
            path,
            emitted,
            notices: HashSet::new(),
        }
    }

    /// The raw stamps, for tests that assert on migration and expiry.
    #[cfg(test)]
    pub fn stamps_for_test(&self) -> Vec<(String, DateTime<Utc>)> {
        self.emitted.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }

    /// Has this topic already been surfaced this session?
    pub fn contains(&self, topic: &str) -> bool {
        self.emitted.contains_key(topic)
    }

    /// Has nothing been surfaced yet this session?
    ///
    /// True at session start and again after [`clear`](Self::clear) (workspace
    /// activate / post-compact re-arm). This is the condition that fires the
    /// session-opening guide from any tool — see
    /// `prompts::SESSION_OPENING_GUIDE`.
    pub fn is_empty(&self) -> bool {
        self.emitted.is_empty()
    }

    /// A ledger that behaves as if the session has already opened.
    ///
    /// Test-only. The session opener (`prompts::SESSION_OPENING_GUIDE`) fires on
    /// the first guide-eligible call of any session and appends a second content
    /// block, so a test asserting on *primary-block shape* must start
    /// mid-session or it measures the opener instead. Guide delivery itself is
    /// covered by `server::guide_hint_tests`.
    #[cfg(test)]
    pub fn mid_session() -> Self {
        let mut ledger = Self::default();
        // Default has no path, so this insert stamps in memory and persists nothing.
        ledger.insert(crate::prompts::SESSION_OPENING_GUIDE.to_string());
        ledger
    }

    /// Record a topic, stamping it with the current time. Returns `true` if the
    /// topic was newly added (matching `HashSet::insert`'s contract, which
    /// `src/tools/guide.rs:92` relies on for its `first_fetch` signal).
    ///
    /// A repeat insert REFRESHES the stamp rather than preserving the original:
    /// the stamp means "last delivered", because that is what `expire_idle`'s TTL
    /// and the GC's idle-age both need. Persists unconditionally — the map changed
    /// either way, and skipping the write on a repeat would let the in-memory
    /// stamps drift ahead of the on-disk ones the GC reads.
    pub fn insert(&mut self, topic: String) -> bool {
        let added = self.emitted.insert(topic, Utc::now()).is_none();
        self.persist();
        added
    }

    /// Forget all topics (workspace activate / post-compact re-arm). Persists
    /// by removing the file so a later reload re-arms every guide.
    pub fn clear(&mut self) {
        let was_nonempty = !self.emitted.is_empty();
        self.emitted.clear();
        // Notices re-arm with the guides: an `activate` is precisely the act a
        // worktree notice asks the caller to perform, and a post-compact
        // re-arm means the model no longer remembers being told.
        self.notices.clear();
        if was_nonempty {
            self.persist();
        }
    }

    /// Forget the named topics so they inject again, leaving every other topic
    /// in place. This is the surgical twin of [`clear`](Self::clear): a project
    /// switch re-teaches only the project-scoped guide, not the tool-contract
    /// guides the model already holds.
    ///
    /// Persists only when something was actually removed.
    pub fn re_arm(&mut self, topics: &[&str]) {
        let mut changed = false;
        for topic in topics {
            if self.emitted.remove(*topic).is_some() {
                changed = true;
            }
        }
        if changed {
            self.persist();
        }
    }

    /// Re-arm every topic delivered longer ago than `ttl`. Returns how many.
    ///
    /// This is the only mechanism available to clients that expose no
    /// conversation identity at all — where one MCP process serves many
    /// conversations, an unbounded ledger starves every conversation after the
    /// first. Re-sending a guide the model still holds costs tokens once;
    /// withholding one it never received is silent. Persists only on a change.
    pub fn expire_idle(&mut self, ttl: std::time::Duration) -> usize {
        let Ok(ttl) = chrono::Duration::from_std(ttl) else {
            return 0;
        };
        let cutoff = Utc::now() - ttl;
        let before = self.emitted.len();
        self.emitted.retain(|_topic, at| *at > cutoff);
        let removed = before - self.emitted.len();
        if removed > 0 {
            self.persist();
        }
        removed
    }

    /// Record a one-shot session notice. Returns `true` the FIRST time this
    /// key is seen and `false` forever after (until [`clear`](Self::clear)),
    /// so the caller can write `if ledger.notice_once(K) { emit }`.
    ///
    /// Notices live beside guide topics, never inside them — see the `notices`
    /// field for why that separation is load-bearing rather than tidy.
    pub fn notice_once(&mut self, key: &str) -> bool {
        self.notices.insert(key.to_string())
    }

    /// Best-effort write-through. Persistence is an optimization, not a
    /// correctness requirement — failures are logged at debug, never raised.
    ///
    /// Writes go through `util::fs::write_utf8`, which stages to a sibling
    /// `.tmp` and renames, so a reader can never observe a torn file.
    ///
    /// Deliberately NOT read-modify-write: merging the on-disk set back in would
    /// resurrect exactly the topics `re_arm` and `expire_idle` just removed. The
    /// in-memory map is authoritative for this process; last writer wins. Two
    /// live processes sharing one session id would need to write simultaneously
    /// for that to matter, and an MCP reconnect is kill-then-spawn, not overlap.
    fn persist(&self) {
        let Some(path) = &self.path else { return };
        if self.emitted.is_empty() {
            let _ = std::fs::remove_file(path);
            return;
        }
        match serde_json::to_string(&self.emitted) {
            Ok(json) => {
                if let Err(e) = crate::util::fs::write_utf8(path, &json) {
                    tracing::debug!("guide ledger persist failed ({}): {e}", path.display());
                }
            }
            Err(e) => tracing::debug!("guide ledger serialize failed: {e}"),
        }
    }
}

/// Delete ledgers whose newest stamp is older than [`GC_MAX_IDLE_DAYS`].
///
/// Keyed on the NEWEST stamp, deliberately: a long-running session keeps writing,
/// so idle age is what distinguishes a dead ledger from a quiet one. A file whose
/// oldest topic is ancient but whose newest is fresh belongs to a live session.
///
/// `skip` is the caller's own file, which is never pruned — loading a stale
/// ledger must return its contents, not delete them out from under the caller.
///
/// This `read_dir` is CLEANUP, not discovery: nothing locates a ledger by
/// scanning, so the directory can be relocated without auditing this call.
/// (Same note as `src/lsp/mux/mod.rs:68-72`; recon ledger R-45.)
fn gc(dir: &Path, skip: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let cutoff = Utc::now() - chrono::Duration::days(GC_MAX_IDLE_DAYS);
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path == skip || path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let newest = read_entries(&path).into_values().max();
        // An unparseable or empty ledger carries no evidence of life; fall back
        // to the file's own mtime rather than deleting it on the spot.
        let idle_since = newest.or_else(|| file_mtime(&path));
        if let Some(at) = idle_since {
            if at < cutoff {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Read one ledger file, migrating the legacy `Vec<String>` shape on the way.
/// A legacy file has no per-topic stamps, so every topic inherits the file's
/// mtime: the best available evidence of when those guides were delivered, and
/// it keeps a freshly-migrated ledger from looking instantly expired.
fn read_entries(path: &Path) -> BTreeMap<String, DateTime<Utc>> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    match serde_json::from_str::<LedgerFile>(&raw) {
        Ok(LedgerFile::Stamped(m)) => m,
        Ok(LedgerFile::Legacy(topics)) => {
            let stamp = file_mtime(path).unwrap_or_else(Utc::now);
            topics.into_iter().map(|t| (t, stamp)).collect()
        }
        Err(_) => BTreeMap::new(),
    }
}

/// Best-effort: any conversion failure (a mtime chrono can't represent, or one
/// that predates the Unix epoch) yields `None` rather than panicking, so the
/// caller's `unwrap_or_else(Utc::now)` fallback is what actually fires.
fn file_mtime(path: &Path) -> Option<DateTime<Utc>> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let since_epoch = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    let secs = i64::try_from(since_epoch.as_secs()).ok()?;
    Utc.timestamp_opt(secs, since_epoch.subsec_nanos()).single()
}

/// Session ids are uuids, but the env value / file fallback is untrusted — keep
/// the basename to a safe charset so it can't escape the directory.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn ledger_survives_reload_and_isolates_sessions() {
        let dir = tempdir().unwrap();
        let hints_dir = dir.path().join(".codescout").join("guide_hints");

        // First "process": record a topic.
        let mut l = GuideLedger::load("sess-A", Some(hints_dir.clone()));
        assert!(!l.contains("librarian"));
        assert!(l.insert("librarian".to_string()), "first insert is new");
        assert!(
            !l.insert("librarian".to_string()),
            "second insert is a no-op"
        );
        drop(l);

        // Second "process" (simulated /mcp restart): same session reloads from disk.
        let l2 = GuideLedger::load("sess-A", Some(hints_dir.clone()));
        assert!(
            l2.contains("librarian"),
            "ledger must survive reconstruction (the bug)"
        );

        // A concurrent session on the same project sees nothing of A's.
        let l3 = GuideLedger::load("sess-B", Some(hints_dir.clone()));
        assert!(!l3.contains("librarian"), "sessions must be isolated");

        // Clear persists (removes the file) → next reload re-arms (compaction).
        let mut l4 = GuideLedger::load("sess-A", Some(hints_dir.clone()));
        l4.clear();
        drop(l4);
        let l5 = GuideLedger::load("sess-A", Some(hints_dir));
        assert!(!l5.contains("librarian"), "clear must persist");
    }

    #[test]
    fn ephemeral_ledger_is_in_memory_only() {
        // The Default ledger (no path) is what the 30+ test/internal ToolContext
        // builders get — pure in-memory, no files touched.
        let mut l = GuideLedger::default();
        assert!(l.insert("x".to_string()));
        assert!(l.contains("x"));
        l.clear();
        assert!(!l.contains("x"));
    }

    #[test]
    fn a_legacy_vec_file_is_read_and_stamped_from_its_mtime() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();
        // The pre-2026-08-18 on-disk shape: a bare array, no timestamps.
        let file = hints.join("sess-legacy.json");
        std::fs::write(&file, r#"["librarian","tracker-conventions"]"#).unwrap();

        // Ground truth: the file's own mtime. A `Utc::now()` fallback mutant
        // would stamp strictly later than this, since load happens after write.
        let expected = DateTime::<Utc>::from(std::fs::metadata(&file).unwrap().modified().unwrap());

        let l = GuideLedger::load("sess-legacy", Some(hints.clone()));
        assert!(
            l.contains("librarian"),
            "legacy topics must survive the shape change"
        );
        assert!(l.contains("tracker-conventions"));

        // Every legacy topic is stamped with the file's mtime — not load time,
        // and not some other placeholder — so it is neither instantly expired
        // nor immortal.
        let stamps = l.stamps_for_test();
        assert_eq!(stamps.len(), 2);
        for (topic, at) in stamps {
            assert_eq!(
                at, expected,
                "legacy stamp for {topic} must come from mtime, not load time"
            );
        }
    }

    #[test]
    fn the_new_shape_round_trips_through_disk() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");

        let before = Utc::now();
        let mut l = GuideLedger::load("sess-new", Some(hints.clone()));
        assert!(l.insert("librarian".to_string()));
        let after = Utc::now();

        // insert() must stamp with the current time, not a placeholder — this
        // is the primary producer of the stamps expiry/GC will consume.
        let stamps = l.stamps_for_test();
        assert_eq!(stamps.len(), 1);
        let (topic, at) = &stamps[0];
        assert_eq!(topic, "librarian");
        assert!(
            *at >= before && *at <= after,
            "insert must stamp with now(): {at} not within [{before}, {after}]"
        );

        drop(l);

        // On disk it is now an object, not an array.
        let raw = std::fs::read_to_string(hints.join("sess-new.json")).unwrap();
        assert!(raw.starts_with('{'), "expected a stamped map, got: {raw}");
        assert!(raw.contains("librarian"));

        // The persisted value carries the same stamp that was held in memory,
        // not a re-derived or placeholder one.
        let persisted: BTreeMap<String, DateTime<Utc>> = serde_json::from_str(&raw).unwrap();
        assert_eq!(persisted.get("librarian"), Some(at));

        let l2 = GuideLedger::load("sess-new", Some(hints));
        assert!(l2.contains("librarian"), "the new shape must reload");
    }

    #[test]
    fn a_repeat_insert_refreshes_the_stamp_and_persists_it() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        let file = hints.join("sess-refresh.json");

        let mut l = GuideLedger::load("sess-refresh", Some(hints.clone()));
        assert!(l.insert("librarian".to_string()), "first insert is new");
        let first_stamp = l.stamps_for_test()[0].1;

        // Force a measurable gap so a refreshed stamp is provably later.
        std::thread::sleep(std::time::Duration::from_millis(10));

        assert!(
            !l.insert("librarian".to_string()),
            "second insert on an existing topic is not newly-added"
        );
        let second_stamp = l.stamps_for_test()[0].1;
        assert!(
            second_stamp > first_stamp,
            "a repeat insert must refresh the in-memory stamp (last-delivered semantics)"
        );

        // The refreshed stamp must reach disk even though the returned bool is
        // `false` — a repeat insert still changed the map, and expire_idle/GC
        // read the on-disk stamp, not just the in-memory one.
        let raw = std::fs::read_to_string(&file).unwrap();
        let persisted: BTreeMap<String, DateTime<Utc>> = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            persisted.get("librarian"),
            Some(&second_stamp),
            "the on-disk stamp must match the refreshed in-memory stamp, not the stale first one"
        );
    }

    #[test]
    fn persist_never_leaves_a_partial_file_behind() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("nested").join("guide_hints");

        let mut l = GuideLedger::load("sess-atomic", Some(hints.clone()));
        l.insert("librarian".to_string());
        l.insert("tracker-conventions".to_string());
        drop(l);

        // Parent directories are created by the writer.
        let target = hints.join("sess-atomic.json");
        assert!(
            target.exists(),
            "persist must create its parent directories"
        );

        // The atomic writer stages through a sibling `.tmp` and renames; no stray
        // temp file may survive a successful write.
        let leftovers: Vec<_> = std::fs::read_dir(&hints)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "stray temp files: {leftovers:?}");

        // And the file that landed is complete, parseable JSON.
        let raw = std::fs::read_to_string(&target).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("complete JSON");
        assert_eq!(parsed.as_object().unwrap().len(), 2);
    }

    #[test]
    fn a_malformed_file_yields_an_empty_ledger_rather_than_a_panic() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();
        std::fs::write(hints.join("sess-bad.json"), "{not json at all").unwrap();

        let l = GuideLedger::load("sess-bad", Some(hints));
        assert!(!l.contains("librarian"));
        assert!(
            l.is_empty(),
            "an unreadable ledger degrades to re-sending, never to suppressing"
        );
    }

    #[test]
    fn re_arm_removes_only_the_named_topics_and_persists() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");

        let mut l = GuideLedger::load("sess-rearm", Some(hints.clone()));
        l.insert("project-activation-bootstrap".to_string());
        l.insert("librarian".to_string());
        l.insert("symbol-navigation".to_string());

        l.re_arm(&["project-activation-bootstrap"]);

        assert!(
            !l.contains("project-activation-bootstrap"),
            "named topic re-arms"
        );
        assert!(l.contains("librarian"), "unnamed topics are untouched");
        assert!(l.contains("symbol-navigation"));
        drop(l);

        // The removal must survive a reload, or an /mcp reconnect resurrects it.
        let l2 = GuideLedger::load("sess-rearm", Some(hints));
        assert!(
            !l2.contains("project-activation-bootstrap"),
            "re_arm must persist"
        );
        assert!(l2.contains("librarian"));
    }

    #[test]
    fn re_arm_of_an_absent_topic_is_a_no_op() {
        // File-backed rather than `GuideLedger::default()`: a `default()` ledger
        // has `path: None`, so `persist()` early-returns unconditionally and this
        // test could not tell a real no-op from a mutant that calls `persist()`
        // regardless of whether anything was removed. Reading and asserting the
        // file's mtime, as `expire_idle_that_changes_nothing_does_not_touch_the_file`
        // does, closes that gap.
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");

        let mut l = GuideLedger::load("sess-rearm-absent", Some(hints.clone()));
        l.insert("librarian".to_string());
        let before = std::fs::metadata(hints.join("sess-rearm-absent.json"))
            .unwrap()
            .modified()
            .unwrap();

        l.re_arm(&["never-emitted"]);
        assert!(l.contains("librarian"));

        let after = std::fs::metadata(hints.join("sess-rearm-absent.json"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(
            before, after,
            "re_arm of an absent topic must not rewrite the file"
        );
    }

    #[test]
    fn expire_idle_rearms_only_topics_older_than_the_ttl() {
        use std::time::Duration;
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();

        // Hand-build a file with one stale topic and one fresh one, so the test
        // does not have to sleep.
        let stale = chrono::Utc::now() - chrono::Duration::hours(6);
        let fresh = chrono::Utc::now();
        let body = serde_json::json!({
            "librarian": stale.to_rfc3339(),
            "symbol-navigation": fresh.to_rfc3339(),
        });
        std::fs::write(hints.join("sess-ttl.json"), body.to_string()).unwrap();

        let mut l = GuideLedger::load("sess-ttl", Some(hints));
        let rearmed = l.expire_idle(Duration::from_secs(2 * 60 * 60)); // 2h

        assert_eq!(rearmed, 1);
        assert!(!l.contains("librarian"), "6h-old topic is past a 2h TTL");
        assert!(l.contains("symbol-navigation"), "fresh topic survives");
    }

    #[test]
    fn expire_idle_that_changes_nothing_does_not_touch_the_file() {
        use std::time::Duration;
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");

        let mut l = GuideLedger::load("sess-noop", Some(hints.clone()));
        l.insert("librarian".to_string());
        let before = std::fs::metadata(hints.join("sess-noop.json"))
            .unwrap()
            .modified()
            .unwrap();

        let rearmed = l.expire_idle(Duration::from_secs(86_400)); // 24h — nothing is that old
        assert_eq!(rearmed, 0);

        let after = std::fs::metadata(hints.join("sess-noop.json"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(before, after, "a no-op expiry must not rewrite the file");
    }

    #[test]
    fn expire_idle_of_an_unconvertible_duration_is_a_no_op() {
        // std::time::Duration::MAX is far outside chrono::Duration's representable
        // range, so `from_std` fails and the `else { return 0 }` branch fires
        // without ever touching `self.emitted`. A mutant that returns
        // `self.emitted.len()` instead of `0` here would return 1, not 0 — this
        // is the only test that exercises that branch at all.
        let mut l = GuideLedger::default();
        l.insert("librarian".to_string());
        assert_eq!(l.expire_idle(std::time::Duration::MAX), 0);
        assert!(l.contains("librarian"));
    }

    #[test]
    fn gc_drops_ledgers_idle_past_the_window_and_keeps_the_rest() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();

        let ancient = chrono::Utc::now() - chrono::Duration::days(40);
        let recent = chrono::Utc::now() - chrono::Duration::days(3);
        std::fs::write(
            hints.join("dead.json"),
            serde_json::json!({ "librarian": ancient.to_rfc3339() }).to_string(),
        )
        .unwrap();
        std::fs::write(
            hints.join("alive.json"),
            serde_json::json!({ "librarian": recent.to_rfc3339() }).to_string(),
        )
        .unwrap();

        // GC runs as a side effect of loading any ledger in that directory.
        let _ = GuideLedger::load("some-other-session", Some(hints.clone()));

        assert!(
            !hints.join("dead.json").exists(),
            "40-day-idle ledger must be pruned"
        );
        assert!(
            hints.join("alive.json").exists(),
            "3-day-idle ledger must survive"
        );
    }

    #[test]
    fn gc_keeps_a_ledger_whose_newest_stamp_is_fresh_even_if_its_oldest_is_not() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();

        // A long-running session: first guide delivered 40 days ago, still active.
        let old = chrono::Utc::now() - chrono::Duration::days(40);
        let now = chrono::Utc::now();
        std::fs::write(
            hints.join("long-runner.json"),
            serde_json::json!({
                "project-activation-bootstrap": old.to_rfc3339(),
                "librarian": now.to_rfc3339(),
            })
            .to_string(),
        )
        .unwrap();

        let _ = GuideLedger::load("some-other-session", Some(hints.clone()));

        assert!(
            hints.join("long-runner.json").exists(),
            "idle age is the NEWEST stamp; a 28.9-day session was observed in the wild"
        );
    }

    #[test]
    fn gc_never_prunes_the_ledger_being_loaded() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();
        let ancient = chrono::Utc::now() - chrono::Duration::days(40);
        std::fs::write(
            hints.join("mine.json"),
            serde_json::json!({ "librarian": ancient.to_rfc3339() }).to_string(),
        )
        .unwrap();

        // Loading my own stale ledger must return its contents, not delete them
        // mid-call and hand me an empty set.
        let l = GuideLedger::load("mine", Some(hints.clone()));
        assert!(l.contains("librarian"));
    }

    #[test]
    fn gc_ignores_non_json_files_even_when_ancient() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();

        // A stray non-ledger file with an ancient mtime must survive: gc's
        // directory walk is scoped to files it recognizes as ledgers (`.json`),
        // never to "anything old in this directory". This directory may be
        // relocated to a user-level state dir in future, which is why the
        // filter is by extension, not by age.
        let stray = hints.join("notes.txt");
        std::fs::write(&stray, "not a ledger").unwrap();
        let ancient_mtime =
            std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 24 * 60 * 60);
        filetime::set_file_mtime(&stray, filetime::FileTime::from_system_time(ancient_mtime))
            .unwrap();

        let _ = GuideLedger::load("some-other-session", Some(hints.clone()));

        assert!(
            stray.exists(),
            "gc must never delete a file without a .json extension"
        );
    }

    #[test]
    fn gc_keeps_an_unparseable_json_file_with_a_fresh_mtime() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();

        // Garbage content: read_entries returns an empty map, so `newest` is
        // None and gc must fall back to file_mtime. A fresh mtime means the
        // file is not idle yet — this pins the fallback's "keep" direction.
        let garbage = hints.join("garbage.json");
        std::fs::write(&garbage, "not valid json").unwrap();

        let _ = GuideLedger::load("some-other-session", Some(hints.clone()));

        assert!(
            garbage.exists(),
            "an unparseable .json file with a fresh mtime must survive"
        );
    }

    #[test]
    fn gc_prunes_an_unparseable_json_file_with_an_ancient_mtime() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();

        // Same garbage content, but an ancient mtime: the mtime fallback must
        // still catch a genuinely idle file. This is the branch
        // `newest.or_else(|| file_mtime(&path))` exists for, and it pins the
        // fallback's "prune" direction — without it, an unparseable ledger
        // would never be collected (a leak).
        let garbage = hints.join("garbage.json");
        std::fs::write(&garbage, "not valid json").unwrap();
        let ancient_mtime =
            std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 24 * 60 * 60);
        filetime::set_file_mtime(
            &garbage,
            filetime::FileTime::from_system_time(ancient_mtime),
        )
        .unwrap();

        let _ = GuideLedger::load("some-other-session", Some(hints.clone()));

        assert!(
            !garbage.exists(),
            "an unparseable .json file with an ancient mtime must be pruned via the mtime fallback"
        );
    }

    #[test]
    fn gc_keeps_a_json_file_whose_mtime_predates_the_unix_epoch() {
        let dir = tempdir().unwrap();
        let hints = dir.path().join("guide_hints");
        std::fs::create_dir_all(&hints).unwrap();

        // A pre-epoch mtime makes `modified.duration_since(UNIX_EPOCH)` return
        // Err, so `file_mtime` yields None too — the fail-safe path where
        // NEITHER a stamp NOR a usable mtime exists. `idle_since` must then be
        // None, and the file must survive: a mutant that substitutes any
        // "assume idle" default here (e.g. `.or(Some(cutoff))`) would delete a
        // file gc has no evidence about, which is the inversion the design
        // exists to prevent.
        //
        // Portability: FileTime::from_unix_time(-100, 0) sets a mtime just
        // before 1970. On Windows this remains a valid, in-range FILETIME
        // (which natively supports dates back to 1601), and std's SystemTime
        // on every supported platform can represent pre-epoch instants, so
        // `duration_since(UNIX_EPOCH)` returning Err is expected everywhere,
        // not a Unix-only artifact. Not #[cfg(unix)]-gated on that basis.
        let garbage = hints.join("garbage.json");
        std::fs::write(&garbage, "not valid json").unwrap();
        filetime::set_file_mtime(&garbage, filetime::FileTime::from_unix_time(-100, 0)).unwrap();

        let _ = GuideLedger::load("some-other-session", Some(hints.clone()));

        assert!(
            garbage.exists(),
            "a file with neither a parseable stamp nor a usable mtime must never be pruned"
        );
    }
}
