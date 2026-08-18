# Guide Ledger Phase A — Session-Scoped Storage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the guide-hint ledger out of the project tree into per-user state, give it a timestamped on-disk shape, add the `re_arm` / `expire_idle` mechanisms, and garbage-collect dead ledgers — with no change to when guides are injected.

**Architecture:** `GuideLedger` currently stores a `HashSet<String>` at `<project>/.codescout/guide_hints/<session>.json`. Phase A changes the location to `$XDG_STATE_HOME/codescout/guide_hints/`, the shape to `BTreeMap<String, DateTime<Utc>>` (reading the legacy `Vec<String>` and migrating it), and adds two removal APIs plus a 35-day GC. Every re-arm *policy* stays exactly as it is — Phase C changes behaviour, Phase A only makes it possible.

**Tech Stack:** Rust, `chrono` (already a hard dependency), `serde`/`serde_json`, `tempfile` for tests. No new dependencies. `dirs` is deliberately **not** used — it is `optional` and librarian-gated in `Cargo.toml`, so it is unavailable in a lean build.

**Spec:** `docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md` (§2 Storage, §3 API, §8 GC; the Phase A row of *Suggested phasing*)

## Global Constraints

- **Pre-commit gate, every task:** `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`. A task is not done until all three are green.
- **No new dependencies.** `chrono`, `serde`, `serde_json`, `tempfile` are already in `Cargo.toml`.
- **No env mutation in tests.** Concurrent `set_var` is UB and the suite is not serialized — see `docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md`. Environment-dependent logic is split into a pure inner function that tests call directly.
- **`GuideLedger::default()` must stay ephemeral and infallible.** ~30 internal and test `ToolContext` builders construct it with `Default::default()`; none may need changing.
- **Public API preserved this phase:** `contains`, `insert`, `clear`, `is_empty`, `notice_once`, `mid_session` keep their current signatures and semantics. `is_empty` is still consumed by `src/tools/core/types.rs:693` until Phase C.
- **Behaviour must not change.** No task in this plan alters which guides are injected or when.

---

## File Structure

| File | Responsibility | Change |
|---|---|---|
| `src/util/fs.rs` | Filesystem-location and IO helpers | **Modify** — add `per_user_state_dir()` + pure `state_dir_from()` |
| `src/tools/guide_ledger.rs` | The ledger: shape, persistence, expiry, GC | **Modify** — the bulk of this phase |
| `src/server.rs` | Construction site: chooses the ledger directory | **Modify** — ~4 lines at 263-265, plus its restart-survival test |

No new files. `guide_ledger.rs` is 216 lines and grows to roughly 400 — still one clear responsibility, so it is not split.

---

### Task 1: Per-user state directory helper

**Files:**
- Modify: `src/util/fs.rs` (add after `write_utf8`, ~line 88)
- Test: `src/util/fs.rs` (the existing `mod tests` at line 206)

**Interfaces:**
- Consumes: `crate::platform::home_dir() -> Option<PathBuf>` (`src/platform/mod.rs:20-22`)
- Produces: `pub fn per_user_state_dir() -> Option<PathBuf>` — used by Task 6

- [ ] **Step 1: Write the failing tests**

Add to the existing `mod tests` in `src/util/fs.rs`:

```rust
#[test]
fn state_dir_prefers_an_absolute_xdg_state_home() {
    let got = state_dir_from(
        Some(std::ffi::OsString::from("/xdg/state")),
        Some(PathBuf::from("/home/u")),
    );
    assert_eq!(got, Some(PathBuf::from("/xdg/state")));
}

#[test]
fn state_dir_ignores_a_relative_xdg_state_home() {
    // The XDG basedir spec requires relative paths to be treated as unset.
    let got = state_dir_from(
        Some(std::ffi::OsString::from("relative/state")),
        Some(PathBuf::from("/home/u")),
    );
    assert_eq!(got, Some(PathBuf::from("/home/u/.local/state")));
}

#[test]
fn state_dir_falls_back_to_home_local_state() {
    let got = state_dir_from(None, Some(PathBuf::from("/home/u")));
    assert_eq!(got, Some(PathBuf::from("/home/u/.local/state")));
}

#[test]
fn state_dir_is_none_when_neither_is_available() {
    // The caller degrades to an in-memory ledger rather than guessing a path.
    assert_eq!(state_dir_from(None, None), None);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib util::fs::tests::state_dir`
Expected: FAIL — `cannot find function 'state_dir_from' in this scope`

- [ ] **Step 3: Write the implementation**

Add to `src/util/fs.rs` after `write_utf8`:

```rust
/// Per-user directory for **persistent** state.
///
/// Distinct from [`crate::socket_discovery::per_user_runtime_dir`], which serves
/// sockets and lock files that are expected to die with the boot. This one holds
/// data that must outlive a reboot.
///
/// `$XDG_STATE_HOME` when set to an absolute path, else `$HOME/.local/state`.
/// `None` when neither is available — callers degrade rather than guess.
pub fn per_user_state_dir() -> Option<PathBuf> {
    state_dir_from(
        std::env::var_os("XDG_STATE_HOME"),
        crate::platform::home_dir(),
    )
}

/// Pure core of [`per_user_state_dir`], split out so tests never mutate the
/// process environment — concurrent `set_var` is UB and this suite is not
/// serialized (docs/issues/archive/2026-07-13-test-env-access-ub-nonserial-writers-race-build-tool-context.md).
fn state_dir_from(xdg: Option<std::ffi::OsString>, home: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(x) = xdg {
        let p = PathBuf::from(x);
        // The XDG basedir spec: a relative value must be treated as unset.
        if p.is_absolute() {
            return Some(p);
        }
    }
    Some(home?.join(".local").join("state"))
}
```

`PathBuf` is already imported in this module.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib util::fs::tests::state_dir`
Expected: PASS, 4 tests

- [ ] **Step 5: Run the gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/util/fs.rs
git commit -m "feat(util): add per_user_state_dir for persistent per-user state"
```

---

### Task 2: Timestamped on-disk shape, with legacy migration

**Files:**
- Modify: `src/tools/guide_ledger.rs:18-63` (imports, struct, `load`)
- Test: `src/tools/guide_ledger.rs` (`mod tests` at line 168)

**Interfaces:**
- Consumes: nothing from earlier tasks
- Produces: `GuideLedger.emitted: BTreeMap<String, DateTime<Utc>>` (private), `fn read_entries(path: &Path) -> BTreeMap<String, DateTime<Utc>>` (private) — Tasks 3, 4, 5 all build on this shape

- [ ] **Step 1: Write the failing tests**

Add to `mod tests` in `src/tools/guide_ledger.rs`:

```rust
#[test]
fn a_legacy_vec_file_is_read_and_stamped_from_its_mtime() {
    let dir = tempdir().unwrap();
    let hints = dir.path().join("guide_hints");
    std::fs::create_dir_all(&hints).unwrap();
    // The pre-2026-08-18 on-disk shape: a bare array, no timestamps.
    std::fs::write(hints.join("sess-legacy.json"), r#"["librarian","tracker-conventions"]"#)
        .unwrap();

    let l = GuideLedger::load("sess-legacy", Some(hints.clone()));
    assert!(l.contains("librarian"), "legacy topics must survive the shape change");
    assert!(l.contains("tracker-conventions"));

    // Every legacy topic is stamped with the file's mtime, so it is neither
    // instantly expired nor immortal.
    let stamps = l.stamps_for_test();
    assert_eq!(stamps.len(), 2);
    let now = chrono::Utc::now();
    for (_topic, at) in stamps {
        assert!(at <= now, "a stamp from mtime cannot be in the future");
        assert!(
            (now - at).num_seconds() < 60,
            "a file written just now must stamp as just now"
        );
    }
}

#[test]
fn the_new_shape_round_trips_through_disk() {
    let dir = tempdir().unwrap();
    let hints = dir.path().join("guide_hints");

    let mut l = GuideLedger::load("sess-new", Some(hints.clone()));
    assert!(l.insert("librarian".to_string()));
    drop(l);

    // On disk it is now an object, not an array.
    let raw = std::fs::read_to_string(hints.join("sess-new.json")).unwrap();
    assert!(raw.starts_with('{'), "expected a stamped map, got: {raw}");
    assert!(raw.contains("librarian"));

    let l2 = GuideLedger::load("sess-new", Some(hints));
    assert!(l2.contains("librarian"), "the new shape must reload");
}

#[test]
fn a_malformed_file_yields_an_empty_ledger_rather_than_a_panic() {
    let dir = tempdir().unwrap();
    let hints = dir.path().join("guide_hints");
    std::fs::create_dir_all(&hints).unwrap();
    std::fs::write(hints.join("sess-bad.json"), "{not json at all").unwrap();

    let l = GuideLedger::load("sess-bad", Some(hints));
    assert!(!l.contains("librarian"));
    assert!(l.is_empty(), "an unreadable ledger degrades to re-sending, never to suppressing");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib guide_ledger`
Expected: FAIL — `no method named 'stamps_for_test'`, and `the_new_shape_round_trips_through_disk` fails its `starts_with('{')` assertion because the current shape is an array.

- [ ] **Step 3: Write the implementation**

Replace the imports and struct at the top of `src/tools/guide_ledger.rs`:

```rust
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

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
    /// One-shot session notices that are NOT guide topics. (Unchanged — see the
    /// original field docs for why this is deliberately separate and unpersisted.)
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
```

Replace `load` and add the two readers:

```rust
    /// Load the persisted ledger for `session_id` under `dir`. Best-effort: a
    /// missing, unreadable or malformed file yields an empty set — degrading to
    /// re-sending a guide, never to suppressing one. `dir = None` ⇒ ephemeral.
    pub fn load(session_id: &str, dir: Option<PathBuf>) -> Self {
        let path = dir.map(|d| d.join(format!("{}.json", sanitize(session_id))));
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
```

Add at module level, next to `sanitize`:

```rust
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

fn file_mtime(path: &Path) -> Option<DateTime<Utc>> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(DateTime::<Utc>::from(modified))
}
```

Then fix the three existing methods the shape change breaks:

```rust
    pub fn contains(&self, topic: &str) -> bool {
        self.emitted.contains_key(topic)
    }

    pub fn insert(&mut self, topic: String) -> bool {
        let added = self.emitted.insert(topic, Utc::now()).is_none();
        if added {
            self.persist();
        }
        added
    }
```

and in `persist`, replace the serialized value:

```rust
        match serde_json::to_string(&self.emitted) {
```

(the old `let topics: Vec<&String> = self.emitted.iter().collect();` line is deleted).

`is_empty`, `clear` and `notice_once` need no change — `BTreeMap` provides `is_empty` and `clear`.

**`mid_session` does need a change**, and it will not compile without it: it writes to the field directly, and `BTreeMap::insert` takes two arguments where `HashSet::insert` took one. Route it through the public method instead, which also stamps it:

```rust
    #[cfg(test)]
    pub fn mid_session() -> Self {
        let mut ledger = Self::default();
        // Default has no path, so this insert stamps in memory and persists nothing.
        ledger.insert(crate::prompts::SESSION_OPENING_GUIDE.to_string());
        ledger
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib guide_ledger`
Expected: PASS — the 3 new tests plus the 2 pre-existing ones (`ledger_survives_reload_and_isolates_sessions`, `ephemeral_ledger_is_in_memory_only`), which must still pass unchanged.

- [ ] **Step 5: Run the gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/tools/guide_ledger.rs
git commit -m "feat(guide-ledger): stamp topics with delivery time, migrating the legacy array shape"
```

---

### Task 3: Atomic persistence

**Files:**
- Modify: `src/tools/guide_ledger.rs` — the `persist` method (currently 132-150)
- Test: `src/tools/guide_ledger.rs` (`mod tests`)

**Interfaces:**
- Consumes: `crate::util::fs::write_utf8(path: &Path, content: &str) -> anyhow::Result<()>` (`src/util/fs.rs:81-87`) — creates parent directories and writes via write-tmp-then-rename
- Produces: no new public API; `persist` becomes crash-safe

**This task is a refactor, not a feature — TDD's red step does not apply.** The change has no observable difference on the happy path: both the old `std::fs::write` and the new `write_utf8` create parents and land the same bytes. What changes is the failure mode (a crash mid-write can no longer leave a truncated file) and that is not reachable without fault injection. So the test below is a **characterization test**: it locks in the behaviour the refactor must preserve, and it is expected to pass both before and after. Writing it first still matters — if it fails *after* the change, the refactor broke something.

- [ ] **Step 1: Write the characterization test**

```rust
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
    assert!(target.exists(), "persist must create its parent directories");

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
```

- [ ] **Step 2: Run the test against the CURRENT implementation and record the result**

Run: `cargo test --lib guide_ledger::tests::persist_never_leaves_a_partial_file_behind`
Expected: **PASS.** That is the baseline. If it fails here, stop — the current `persist` is more broken than this plan assumes, and that is a finding worth its own bug file before continuing.

- [ ] **Step 3: Write the implementation**

Replace the body of `persist` in `src/tools/guide_ledger.rs`:

```rust
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
```

The old `if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }` block is deleted — `write_utf8` does it.

- [ ] **Step 4: Run the tests again — the characterization must still hold**

Run: `cargo test --lib guide_ledger`
Expected: PASS, all tests. A failure here means the refactor changed observable behaviour, which it must not.

- [ ] **Step 5: Run the gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/tools/guide_ledger.rs
git commit -m "refactor(guide-ledger): persist atomically via util::fs::write_utf8"
```

---

### Task 4: `re_arm` and `expire_idle`

**Files:**
- Modify: `src/tools/guide_ledger.rs` — add two methods after `clear`
- Test: `src/tools/guide_ledger.rs` (`mod tests`)

**Interfaces:**
- Consumes: the `BTreeMap` shape from Task 2
- Produces:
  - `pub fn re_arm(&mut self, topics: &[&str])` — Phase C (§4) calls this with `PROJECT_SCOPED`
  - `pub fn expire_idle(&mut self, ttl: std::time::Duration) -> usize` — Phase B (§7) calls this per request for tier-2 clients; returns how many topics were re-armed

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn re_arm_removes_only_the_named_topics_and_persists() {
    let dir = tempdir().unwrap();
    let hints = dir.path().join("guide_hints");

    let mut l = GuideLedger::load("sess-rearm", Some(hints.clone()));
    l.insert("project-activation-bootstrap".to_string());
    l.insert("librarian".to_string());
    l.insert("symbol-navigation".to_string());

    l.re_arm(&["project-activation-bootstrap"]);

    assert!(!l.contains("project-activation-bootstrap"), "named topic re-arms");
    assert!(l.contains("librarian"), "unnamed topics are untouched");
    assert!(l.contains("symbol-navigation"));
    drop(l);

    // The removal must survive a reload, or an /mcp reconnect resurrects it.
    let l2 = GuideLedger::load("sess-rearm", Some(hints));
    assert!(!l2.contains("project-activation-bootstrap"), "re_arm must persist");
    assert!(l2.contains("librarian"));
}

#[test]
fn re_arm_of_an_absent_topic_is_a_no_op() {
    let mut l = GuideLedger::default();
    l.insert("librarian".to_string());
    l.re_arm(&["never-emitted"]);
    assert!(l.contains("librarian"));
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
    let before = std::fs::metadata(hints.join("sess-noop.json")).unwrap().modified().unwrap();

    let rearmed = l.expire_idle(Duration::from_secs(86_400)); // 24h — nothing is that old
    assert_eq!(rearmed, 0);

    let after = std::fs::metadata(hints.join("sess-noop.json")).unwrap().modified().unwrap();
    assert_eq!(before, after, "a no-op expiry must not rewrite the file");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib guide_ledger`
Expected: FAIL — `no method named 're_arm'`, `no method named 'expire_idle'`

- [ ] **Step 3: Write the implementation**

Add to `impl GuideLedger`, after `clear`:

```rust
    /// Forget the named topics so they inject again, leaving every other topic
    /// in place. This is the surgical twin of [`clear`](Self::clear): a project
    /// switch re-teaches only the project-scoped guide, not the nine
    /// tool-contract guides the model already holds.
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib guide_ledger`
Expected: PASS, all tests

- [ ] **Step 5: Run the gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/tools/guide_ledger.rs
git commit -m "feat(guide-ledger): add re_arm and expire_idle"
```

---

### Task 5: Garbage-collect dead ledgers on load

**Files:**
- Modify: `src/tools/guide_ledger.rs` — add a constant, a `gc` function, and one call in `load`
- Test: `src/tools/guide_ledger.rs` (`mod tests`)

**Interfaces:**
- Consumes: `read_entries` from Task 2
- Produces: `const GC_MAX_IDLE_DAYS: i64 = 35;` and `fn gc(dir: &Path)` (private, called from `load`)

**Why 35 days.** Measured 2026-08-18 over 258 sessions in 50 `usage.db` files: the fraction of ledgers a GC would delete while their session is still alive is 3.1% at 7 days, 2.3% at 14 days, 1.2% at 21 days and **0.0% at 30 days**. Observed maxima were a 28.9-day lifespan and a 27.0-day idle gap, so 30 days is the first zero-loss value but leaves only ~1 day of headroom; 35 buys six more for ~60 bytes per file. Keyed on **idle age** (newest stamp in the file), not creation — a live long-running session keeps writing, so idle age is what separates dead from merely quiet.

- [ ] **Step 1: Write the failing tests**

```rust
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

    assert!(!hints.join("dead.json").exists(), "40-day-idle ledger must be pruned");
    assert!(hints.join("alive.json").exists(), "3-day-idle ledger must survive");
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib guide_ledger::tests::gc`
Expected: FAIL — `dead.json` still exists; nothing prunes.

- [ ] **Step 3: Write the implementation**

Add near the top of `src/tools/guide_ledger.rs`, below the imports:

```rust
/// Prune a ledger once it has been idle this long. Measured 2026-08-18 across
/// 258 sessions: 0.0% of live sessions would be pruned at 30 days (observed
/// maxima: 28.9-day lifespan, 27.0-day idle gap), so 35 gives headroom for ~60
/// bytes per file. See the spec's §8.
const GC_MAX_IDLE_DAYS: i64 = 35;
```

Add at module level:

```rust
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
```

Call it from `load`, after the path is known and before reading:

```rust
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
```

Note the signature shift: `dir` is now borrowed before being consumed, so `load` takes `dir: Option<PathBuf>` still but uses `as_ref()`/`as_deref()`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib guide_ledger`
Expected: PASS, all tests

- [ ] **Step 5: Run the gate and commit**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
git add src/tools/guide_ledger.rs
git commit -m "feat(guide-ledger): GC ledgers idle past 35 days on load"
```

---

### Task 6: Point the server at per-user state

**Files:**
- Modify: `src/server.rs:263-265` (the `guide_hints_dir` binding)
- Test: `src/server.rs` — `guide_hint_tests::guide_ledger_survives_mcp_restart` (4328-4380)

**Interfaces:**
- Consumes: `crate::util::fs::per_user_state_dir()` (Task 1), `GuideLedger::load` (Tasks 2 and 5)
- Produces: no new API. This is the wiring that makes Phase A observable.

- [ ] **Step 1: Write the failing test**

The existing `guide_ledger_survives_mcp_restart` builds servers with an injected `ServerEnv` and a tempdir project, and asserts the ledger survives reconstruction. It currently passes because the ledger lives under the project root. After this change it must *still* pass — the ledger just lives elsewhere — so add a test that pins the new location explicitly.

Add to `mod guide_hint_tests` in `src/server.rs`:

```rust
    /// The ledger must NOT live under the project root any more: a git worktree,
    /// a cross-project session, and a cwd that is not a project all resolve to
    /// different roots for the same conversation, and the ledger has to follow
    /// the conversation. See the spec's §2.
    #[tokio::test]
    async fn guide_ledger_does_not_live_under_the_project_root() {
        let (dir, server) = make_server().await;
        let ctx = shared_ctx(&server);
        let tool = tool_by_name(&server, "run_command");
        let _ = tool
            .call_content(json!({"command": "echo hi"}), &ctx)
            .await
            .unwrap();

        let in_project = dir.path().join(".codescout").join("guide_hints");
        assert!(
            !in_project.exists(),
            "guide_hints must no longer be written under the project root"
        );
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib guide_hint_tests::guide_ledger_does_not_live_under_the_project_root`
Expected: FAIL — the directory exists, because construction still points at the project root.

> **Historical record — the test shipped under a different name.** The whole-branch review
> found this test asserted only the *absence* of a project-tree ledger, which a mutation
> forcing `guide_hints_dir` to `None` would also satisfy, so a positive assertion was added
> and the test renamed to
> `guide_ledger_lives_in_the_injected_dir_not_under_the_project_root`
> (`src/server.rs:4443`). **The filter above now matches zero tests and exits 0** — a false
> green if you replay it. Use the current name. The step text is left as written because the
> plan is a record of what was planned, not of what shipped; see
> `docs/trackers/2026-08-18-guide-ledger-residuals.md` § S-08.

- [ ] **Step 3: Write the implementation**

In `src/server.rs`, replace the `guide_hints_dir` binding (currently lines 263-265):

```rust
        // Per-USER state, not per-project: the ledger follows the conversation,
        // and one conversation can span worktrees, sub-projects and repos. Keeping
        // it under a project root made it depend on the companion plugin's
        // worktree symlink and made it silently ephemeral whenever the cwd was not
        // a project. See docs/superpowers/specs/2026-08-18-guide-ledger-session-identity-design.md §2.
        let guide_hints_dir = crate::util::fs::per_user_state_dir()
            .map(|d| d.join("codescout").join("guide_hints"));
```

The now-unused `guide_project_root` binding is still used two lines above for the `cc_session_id` file fallback, so leave it. If clippy reports it unused, that means the fallback was already removed — in that case delete the binding too.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib guide_hint`
Expected: PASS — the new test, plus `guide_ledger_survives_mcp_restart` and `post_compact_rearms_guide_hints` unchanged.

- [ ] **Step 5: Run the full gate**

```bash
cargo fmt && cargo clippy -- -D warnings && cargo test
```

Expected: green. The suite is ~3900 tests.

- [ ] **Step 6: Verify against the live server**

```bash
cargo rb
```

Then `/mcp` to reconnect, make one codescout call, and confirm the new location is written and the old one is not:

```bash
ls -la ~/.local/state/codescout/guide_hints/ | tail -3
ls -la .codescout/guide_hints/ | wc -l   # unchanged from before the rebuild
```

Expected: a file named for the current `CLAUDE_CODE_SESSION_ID` under `~/.local/state/codescout/guide_hints/`, containing a JSON object with timestamps.

- [ ] **Step 7: Commit**

```bash
git add src/server.rs
git commit -m "feat(guide-ledger): store the ledger in per-user state, not the project tree"
```

---

## Migration note

Existing ledgers under `<project>/.codescout/guide_hints/` are **abandoned, not migrated**. They are keyed by session id, and any session still live at rollout re-injects its guides once — a one-off cost bounded by the number of open conversations. Migrating them would mean merging N project-local directories into one per-user directory with no way to detect collisions, which is more risk than the one-off buys back. The old directories stay gitignored (`.gitignore:46`) and can be deleted by hand.

## Out of scope for Phase A

- The session-key chain and the companion rendezvous (§1, §6) — Phase B.
- Calling `expire_idle` from a request path, and tier selection (§7 policy) — Phase B. Phase A ships the mechanism only; `expire_idle` has no production caller yet, which is fine for a `pub` method on a lib crate and will not trip `dead_code`.
- The re-arm predicate, the opener predicate, the `workspace-state.md` update, and splitting `activate_project_resets_hints` (§4, §5) — Phase C.
